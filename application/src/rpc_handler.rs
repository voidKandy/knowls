use crate::state::SharedState;
use knowls::{
    other_err,
    rpc::{HealthResponse, LspMessage, LspRequest, Request, Response, RpcMessage},
    MainResult,
};
use lsp_server::ResponseError;
use lsp_types::{
    request::DocumentDiagnosticRequest, Hover, HoverParams, Position, PublishDiagnosticsParams,
    Range,
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
}

type SharedMessageQueue = Arc<RwLock<VecDeque<RpcMessage>>>;
#[derive(Debug)]
/// Information of connection stored on connection handler thread
pub struct ConnectionThreadState {
    read: OwnedReadHalf,
    write: OwnedWriteHalf,
    incoming: SharedMessageQueue,
    incoming_pending: Arc<AtomicBool>,
    outbound: SharedMessageQueue,
    outbound_pending: Arc<AtomicBool>,
}

#[derive(Debug)]
/// Information of connection stored on application main thread
pub(super) struct ConnectionInfo {
    pub handle: JoinHandle<()>,
    pub established: Instant,
    pub incoming: SharedMessageQueue,
    pub incoming_pending: Arc<AtomicBool>,
    pub outbound: SharedMessageQueue,
    pub outbound_pending: Arc<AtomicBool>,
    pub typ: ConnectionType,
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
        Self { listener, state }
    }

    pub async fn main_loop(&mut self, should_run: Arc<AtomicBool>) -> MainResult<()> {
        loop {
            let should_break = !should_run.load(std::sync::atomic::Ordering::Relaxed);
            if should_break {
                tracing::warn!("breaking rpc handler main loop");
                break;
            }

            tokio::select! {
                Ok(Ok((stream, addr))) = tokio::time::timeout(Duration::from_millis(200), self.listener.accept()) => {
                        let mut w = self.state.write().await;
                        let conn =  ConnectionThreadState::spawn_handle(stream);
                        w.connections.insert(addr.to_string(), conn);
                    },
                else => {
                    let conns_to_handle =
                        self.state.read().await.connections.iter().fold(vec![], |mut acc, (addr, info)| {
                            if info
                                .incoming_pending
                                .load(std::sync::atomic::Ordering::Relaxed)
                            {
                                acc.push(addr.to_string())
                            }
                            acc
                        });
                    self.manage_connections(conns_to_handle).await?;
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
        }
        Ok(())
    }

    /// Goes through given connection ids and handles pending RPC messages
    async fn manage_connections(&mut self, conns_to_handle: Vec<String>) -> MainResult<()> {
        if !conns_to_handle.is_empty() {
            tracing::warn!(
                "the following connection have unhandled messages: {conns_to_handle:#?}"
            );

            for addr in conns_to_handle {
                tracing::warn!("handling messages for {addr:#?}");

                let mut info = self
                    .state
                    .write()
                    .await
                    .connections
                    .remove(&addr)
                    .expect("somehow got an invalid connection key?");

                loop {
                    let mut incoming_queue = info.incoming.write().await;
                    let message = match incoming_queue.pop_front() {
                        Some(msg) => msg,
                        None => break,
                    };
                    drop(incoming_queue);

                    if let Some(response) = self
                        .handle_rpc_message(message, &mut info)
                        .await
                        .expect("failed to handle rpc message")
                    {
                        info.push_outbound(response).await;
                        assert!(
                            info.outbound_pending
                                .load(std::sync::atomic::Ordering::Relaxed),
                            "outbound should be pending"
                        );
                    }
                }
                info.incoming_pending
                    .store(false, std::sync::atomic::Ordering::Relaxed);

                self.state.write().await.connections.insert(addr, info);
            }
        }
        Ok(())
    }

    async fn handle_rpc_message(
        &mut self,
        msg: RpcMessage,
        conn: &mut ConnectionInfo,
    ) -> MainResult<Option<RpcMessage>> {
        tracing::warn!("handle rpc message: {msg:#?}");
        match msg {
            seraphic::Message::Req { id, req } => match req {
                Request::Health(_) => {
                    return Ok(Some(Response::from(HealthResponse {}).into_message(id)));
                }
                Request::Lsp(req) => {
                    if let ConnectionType::Undefined = conn.typ {
                        conn.typ.make_lsp();
                    }
                    if let Some(res_msg) = self.handle_lsp_req_message(req, conn)? {
                        tracing::warn!("got response: {res_msg:#?}");
                        return Ok(Some(
                            Response::Lsp(knowls::rpc::LspResponse { msg: res_msg })
                                .into_message(id),
                        ));
                    }
                    return Ok(None);
                }
            },
            _ => Err(other_err!("did not expect non req RpcMessage: {msg:#?}")),
        }
    }

    fn handle_lsp_req_message(
        &mut self,
        req: LspRequest,
        conn: &mut ConnectionInfo,
    ) -> MainResult<Option<LspMessage>> {
        match Into::<lsp_server::Message>::into(req.msg) {
            lsp_server::Message::Request(req) => {
                return handle_lsp_request(&self.state, req, conn);
            }
            lsp_server::Message::Response(res) => {
                return handle_lsp_response(&self.state, res);
            }
            lsp_server::Message::Notification(noti) => {
                return handle_lsp_notification(&self.state, noti);
            }
        }
    }
}

impl ConnectionInfo {
    /// pushes message to queue and marks outbound as having pending messages
    pub async fn push_outbound(&mut self, message: RpcMessage) {
        tracing::warn!("pushing outbound: {message:#?}");
        self.outbound.write().await.push_back(message);
        if !self
            .outbound_pending
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            self.outbound_pending
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }
}

impl ConnectionThreadState {
    fn new(
        stream: TcpStream,
        incoming: SharedMessageQueue,
        outbound: SharedMessageQueue,
        incoming_pending: Arc<AtomicBool>,
        outbound_pending: Arc<AtomicBool>,
    ) -> Self {
        let (read, write) = stream.into_split();
        Self {
            read,
            write,
            incoming,
            incoming_pending,
            outbound,
            outbound_pending,
        }
    }

    async fn push_incoming(&mut self, message: RpcMessage) {
        self.incoming.write().await.push_back(message);
        if !self
            .incoming_pending
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            self.incoming_pending
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }

    /// Spins up handle and returns connection info
    pub fn spawn_handle(stream: TcpStream) -> ConnectionInfo {
        let established = Instant::now();
        let incoming = Arc::new(RwLock::new(VecDeque::new()));
        let outbound = Arc::new(RwLock::new(VecDeque::new()));
        let incoming_pending = Arc::new(AtomicBool::new(false));
        let outbound_pending = Arc::new(AtomicBool::new(false));

        let thread_incoming = Arc::clone(&incoming);
        let thread_outbound = Arc::clone(&outbound);
        let thread_incoming_pending = Arc::clone(&incoming_pending);
        let thread_outbound_pending = Arc::clone(&outbound_pending);

        let handle = tokio::spawn(async move {
            tracing::warn!("spawned connection handle");
            let mut thread_state = ConnectionThreadState::new(
                stream,
                thread_incoming,
                thread_outbound,
                thread_incoming_pending,
                thread_outbound_pending,
            );
            loop {
                tokio::select! {
                    Ok(_) = thread_state.read.readable() => {
                        match TcpPacket::async_read(&mut thread_state.read).await {
                            Err(err) => {
                                panic!("connection thread encountered error when reading: {err:#?}");
                            },
                            Ok(PacketRead::Empty) =>{},
                            Ok(PacketRead::Disconnected) =>{
                                tracing::warn!("connection with client closed");
                                break;
                            },
                            Ok(PacketRead::Message(msg)) =>{
                                thread_state.push_incoming(msg).await;
                            },
                        }
                    },
                    Ok(_) =  thread_state.write.writable() => {
                        let pending = thread_state.outbound_pending.load(std::sync::atomic::Ordering::Relaxed);
                        if pending {
                            let mut w = thread_state.outbound.try_write().expect("failed to get write lock");
                            tracing::warn!("flushing outbound queue: {w:#?}");
                            while let Some(msg) = w.pop_front() {
                                TcpPacket::async_write(&mut thread_state.write, &msg).await.expect("failed to write msg");
                            }
                            thread_state.outbound_pending.store(false, std::sync::atomic::Ordering::Relaxed);
                        }
                    },
                    else => {
                        tracing::error!("not read or write ready within the given timeout");
                    }

                }
            }
        });

        ConnectionInfo {
            typ: ConnectionType::Undefined,
            incoming,
            established,
            outbound,
            handle,
            outbound_pending,
            incoming_pending,
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
    conn: &mut ConnectionInfo,
) -> MainResult<Option<LspMessage>> {
    tracing::warn!("handle lsp req: {req:#?}");
    if let ConnectionType::Lsp {
        received_shutdown: true,
    } = conn.typ
    {
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
                    line.chars().nth(pos.character as usize - 1),
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
            if let ConnectionType::Lsp {
                ref mut received_shutdown,
            } = conn.typ
            {
                *received_shutdown = true;
            } else {
                panic!("non lsp connection handling lsp message");
            }
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

        if let Some(range) = current_range {
            let diagnostic = lsp_types::Diagnostic {
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
