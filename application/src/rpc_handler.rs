use crate::state::SharedState;
use knowls::{
    other_err,
    rpc::{HealthResponse, LspMessage, LspRequest, Request, Response, RpcMessage},
    MainResult,
};
use lsp_server::ResponseError;
use lsp_types::{
    request::DocumentDiagnosticRequest, DiagnosticSeverity, Hover, HoverParams, Position,
    PublishDiagnosticsParams, Range,
};
use seraphic::{
    packet::{PacketRead, TcpPacket},
    ResponseWrapper,
};
use std::{
    collections::{HashMap, VecDeque},
    net::SocketAddr,
    sync::{atomic::AtomicBool, Arc},
    task::{Context, Waker},
    time::{Duration, Instant},
};
use tokio::net::TcpListener;
use tokio::{
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpStream,
    },
    sync::RwLock,
    task::JoinHandle,
};

#[derive(Debug)]
pub struct RpcConnectionHandler {
    pub state: SharedState,
    pub listener: TcpListener,
    // pub connections: HashMap<String, ConnectionInfo>,
}

type SharedMessageRecord = Arc<RwLock<Vec<RpcMessage>>>;
#[derive(Debug)]
/// Information of connection stored on connection handler thread
pub struct ConnectionThreadState {
    state: SharedState,
    read: OwnedReadHalf,
    write: OwnedWriteHalf,
    messages: SharedMessageRecord,
    // incoming: SharedMessageQueue,
    // incoming_pending: Arc<AtomicBool>,
    // outbound: SharedMessageQueue,
    // outbound_pending: Arc<AtomicBool>,
}

#[derive(Debug)]
/// Information of connection stored on application main thread
pub(super) struct ConnectionInfo {
    pub handle: JoinHandle<()>,
    pub established: Instant,
    pub messages: SharedMessageRecord,
    // pub incoming: SharedMessageQueue,
    // pub incoming_pending: Arc<AtomicBool>,
    // pub outbound: SharedMessageQueue,
    // pub outbound_pending: Arc<AtomicBool>,
    // pub typ: ConnectionType,
}

#[derive(Debug, PartialEq, Eq)]
enum ConnectionType {
    Undefined,
    Lsp { received_shutdown: bool },
}

impl ConnectionType {
    /// Once a connection has an LSP message pass through it, it will be marked as an LSP connection
    fn make_lsp(&mut self) {
        if *self == ConnectionType::Undefined {
            *self = ConnectionType::Lsp {
                received_shutdown: false,
            }
        }
    }
}

impl RpcConnectionHandler {
    pub fn new(listener: TcpListener, state: SharedState) -> Self {
        Self {
            listener,
            state,
            // connections: HashMap::new(),
        }
    }

    pub async fn main_loop(&mut self, should_run: Arc<AtomicBool>) -> MainResult<()> {
        loop {
            let should_break = !should_run.load(std::sync::atomic::Ordering::Relaxed);
            if should_break {
                tracing::warn!("breaking rpc handler main loop");
                break;
            }

            if let Ok(Ok((stream, addr))) =
                tokio::time::timeout(Duration::from_millis(200), self.listener.accept()).await
            {
                let conn_info =
                    ConnectionThreadState::spawn_handle(Arc::clone(&self.state), stream);
                self.state
                    .write()
                    .await
                    .connections
                    .insert(addr.to_string(), conn_info);
            } else {
                self.state.write().await.connections.retain(|c, info| {
                    if info.handle.is_finished() {
                        tracing::warn!("dropping connection to: {c:#?}");
                        false
                    } else {
                        true
                    }
                });
            }
        }
        Ok(())
    }
}

impl ConnectionThreadState {
    fn new(state: SharedState, stream: TcpStream) -> Self {
        let (read, write) = stream.into_split();
        Self {
            state,
            read,
            write,
            messages: Arc::new(RwLock::new(vec![])),
        }
    }

    /// Spins up handle and returns connection info
    pub fn spawn_handle(state: SharedState, stream: TcpStream) -> ConnectionInfo {
        let established = Instant::now();

        let mut thread_state = ConnectionThreadState::new(state, stream);
        let messages = thread_state.messages.clone();
        let handle = tokio::spawn(async move {
            tracing::warn!("spawned connection handle");
            let mut received_shutdown = false;
            loop {
                if let Ok(_) = thread_state.read.readable().await {
                    match TcpPacket::async_read(&mut thread_state.read).await {
                        Err(err) => {
                            panic!("connection thread encountered error when reading: {err:#?}");
                        }
                        Ok(PacketRead::Empty) => {}
                        Ok(PacketRead::Disconnected) => {
                            tracing::warn!("connection with client closed");
                            break;
                        }
                        Ok(PacketRead::Message(msg)) => {
                            match handle_rpc_message(
                                &thread_state.state,
                                msg,
                                &mut received_shutdown,
                            )
                            .await
                            {
                                Ok(res) => {
                                    if let Some(msg) = res {
                                        tracing::warn!("returning msg: {msg:#?}");
                                        thread_state.write.writable().await;
                                        TcpPacket::async_write(&mut thread_state.write, &msg)
                                            .await
                                            .expect("failed to write msg");
                                    }
                                }
                                Err(e) => tracing::error!("error handling rpc message: {e:#?}"),
                            }
                        }
                    }
                }
            }
        });

        ConnectionInfo {
            established,
            handle,
            messages,
        }
    }
}

async fn handle_rpc_message(
    state: &SharedState,
    msg: RpcMessage,
    received_shutdown: &mut bool,
) -> MainResult<Option<RpcMessage>> {
    tracing::warn!("handle rpc message: {msg:#?}");
    match msg {
        seraphic::Message::Req { id, req } => match req {
            Request::Health(_) => {
                return Ok(Some(Response::from(HealthResponse {}).into_message(id)));
            }
            Request::Lsp(req) => {
                if let Some(res_msg) = handle_lsp_req_message(state, req, received_shutdown)? {
                    tracing::warn!("got response: {res_msg:#?}");
                    return Ok(Some(
                        Response::Lsp(knowls::rpc::LspResponse { msg: res_msg }).into_message(id),
                    ));
                }
                return Ok(None);
            }
        },
        _ => Err(other_err!("did not expect non req RpcMessage: {msg:#?}")),
    }
}

fn handle_lsp_req_message(
    state: &SharedState,
    req: LspRequest,
    received_shutdown: &mut bool,
) -> MainResult<Option<LspMessage>> {
    match Into::<lsp_server::Message>::into(req.msg) {
        lsp_server::Message::Request(req) => {
            return handle_lsp_request(state, req, received_shutdown);
        }
        lsp_server::Message::Response(res) => {
            return handle_lsp_response(state, res);
        }
        lsp_server::Message::Notification(noti) => {
            return handle_lsp_notification(state, noti);
        }
    }
}
/// I don't know why this end of the connection would ever receive responses
fn handle_lsp_response(
    _state: &SharedState,
    _res: lsp_server::Response,
) -> MainResult<Option<LspMessage>> {
    Ok(None)
}

fn handle_lsp_request(
    state: &SharedState,
    req: lsp_server::Request,
    received_shutdown: &mut bool,
) -> MainResult<Option<LspMessage>> {
    tracing::warn!("handle lsp req: {req:#?}");
    if *received_shutdown {
        let response = lsp_server::Response {
            result: None,
            error: Some(ResponseError {
                // invalid request error code
                code: -32600,
                message: format!("Shutdown request has been received, {req:#?} is invalid"),
                data: None,
            }),
            id: req.id,
        };
        return Ok(Some(lsp_server::Message::Response(response).into()));
    }

    match req.method.as_str() {
        "textDocument/definition" => {}
        "textDocument/hover" => {
            let r = state.try_read().expect("failed to read lock state");
            let params = serde_json::from_value::<HoverParams>(req.params)?;
            let pos = params.text_document_position_params.position;

            if let Some(content) = r
                .lsp_documents
                .get(&params.text_document_position_params.text_document.uri)
            {
                let line = content
                    .lines()
                    .nth(pos.line as usize)
                    .expect("should have gotten line");
                let (char_before, char_after) = (
                    pos.character
                        .checked_sub(1)
                        .and_then(|i| line.chars().nth(i as usize)),
                    line.chars().nth(pos.character as usize),
                );

                match (char_before, char_after) {
                    (Some(bc), Some(ac)) => {
                        if bc == '@' {
                            if let Some(kn) = r.knowledge.values().find(|k| k.lsp_char == ac) {
                                let hover = Hover {
                                    contents: lsp_types::HoverContents::Markup(
                                        lsp_types::MarkupContent {
                                            kind: lsp_types::MarkupKind::Markdown,
                                            value: kn.content.to_owned(),
                                        },
                                    ),
                                    range: None,
                                };

                                let json =
                                    serde_json::to_value(hover).expect("could not serialize hover");
                                let msg = lsp_server::Message::Response(lsp_server::Response {
                                    id: req.id,
                                    result: Some(json),
                                    error: None,
                                });
                                return Ok(Some(msg.into()));
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        "textDocument/diagnostic" => {}
        "shutdown" => {
            let response = lsp_server::Response {
                id: req.id,
                result: None,
                error: None,
            };
            *received_shutdown = true;
            return Ok(Some(lsp_server::Message::Response(response).into()));
        }
        m => {
            tracing::warn!("unhandled request method: {m:#?}");
        }
    }
    Ok(None)
}

fn handle_lsp_notification(
    state: &SharedState,
    noti: lsp_server::Notification,
) -> MainResult<Option<LspMessage>> {
    tracing::warn!("handle lsp noti: {noti:#?}");
    match noti.method.as_str() {
        "textDocument/didChange" => {
            let mut w = state.try_write().expect("failed to get state write lock");
            let params =
                serde_json::from_value::<lsp_types::DidOpenTextDocumentParams>(noti.params)?;
            w.lsp_documents
                .insert(params.text_document.uri, params.text_document.text);
        }
        "textDocument/didSave" => {
            let mut w = state.try_write().expect("failed to get state write lock");
            let params =
                serde_json::from_value::<lsp_types::DidSaveTextDocumentParams>(noti.params)?;
            w.lsp_documents.insert(
                params.text_document.uri.clone(),
                params.text.clone().unwrap(),
            );
            let diagnostic = diagnose_document(params.text_document.uri, params.text.unwrap());
            let params = serde_json::to_value(diagnostic).unwrap();
            let msg = lsp_server::Message::Notification(lsp_server::Notification {
                method: "textDocument/publishDiagnostics".to_string(),
                params,
            });
            tracing::warn!("returning: {msg:#?}");
            return Ok(Some(msg.into()));
        }
        "textDocument/didOpen" => {
            let mut w = state.try_write().expect("failed to get state write lock");
            let params =
                serde_json::from_value::<lsp_types::DidOpenTextDocumentParams>(noti.params)?;
            w.lsp_documents
                .insert(params.text_document.uri, params.text_document.text);
        }
        m => {
            tracing::warn!("unhandled notification: {m:#?}");
        }
    }
    Ok(None)
}

/// Currently just puts diagnostic on any 'word' that starts with a @
fn diagnose_document(uri: lsp_types::Uri, str: String) -> lsp_types::PublishDiagnosticsParams {
    let mut diagnostics = vec![];

    let mut current_range = Option::<Range>::None;
    let mut current_word = Option::<String>::None;
    for (i, line) in str.lines().enumerate() {
        for (k, ch) in line.char_indices() {
            match (current_range, ch) {
                (None, '@') => {
                    let range = Range {
                        start: Position {
                            line: i as u32,
                            character: k as u32,
                        },
                        end: Position {
                            line: 0,
                            character: 0,
                        },
                    };
                    current_range = Some(range);
                    match current_word {
                        Some(ref mut w) => w.push(ch),
                        None => current_word = Some(ch.to_string()),
                    }
                }
                (Some(ref mut r), ch) => {
                    if ch.is_whitespace() {
                        r.end = Position {
                            line: i as u32,
                            character: k as u32,
                        }
                    } else {
                        match current_word {
                            Some(ref mut w) => w.push(ch),
                            None => current_word = Some(ch.to_string()),
                        }
                    }
                }
                _ => {}
            }
        }

        if let Some(range) = current_range.take() {
            let diagnostic = lsp_types::Diagnostic {
                severity: Some(DiagnosticSeverity::INFORMATION),
                range,
                message: current_word.take().unwrap_or("no word?".to_string()),
                ..Default::default()
            };
            diagnostics.push(diagnostic);
        }
    }
    lsp_types::PublishDiagnosticsParams {
        diagnostics,
        uri,
        version: None,
    }
}
