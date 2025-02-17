use crate::{
    lsp::{completions::CompletionConfig, LspHandler},
    state::{SharedState, State, StateReadGuard},
};
use knowls::{
    other_err,
    rpc::{HealthResponse, LspMessage, LspRequest, Request, Response, RpcMessage},
    MainResult,
};
use lsp_server::{RequestId, ResponseError};
use lsp_types::{
    request::DocumentDiagnosticRequest, CompletionContext, CompletionItem,
    CompletionItemLabelDetails, CompletionList, CompletionParams, CompletionResponse,
    CompletionTriggerKind, DiagnosticSeverity, Hover, HoverParams, Position,
    PublishDiagnosticsParams, Range, TextDocumentPositionParams,
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
    pub completion_config: CompletionConfig,
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
    // As soon as an LSP message is received a handler is built from default
    lsp_handler: LspHandler,
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
    pub fn new(
        completion_config: CompletionConfig,
        listener: TcpListener,
        state: SharedState,
    ) -> Self {
        Self {
            completion_config,
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
                let conn_info = ConnectionThreadState::spawn_handle(
                    self.completion_config.clone(),
                    Arc::clone(&self.state),
                    stream,
                );
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
    fn new(completion_config: CompletionConfig, state: SharedState, stream: TcpStream) -> Self {
        let (read, write) = stream.into_split();
        let r = state.try_read().unwrap();
        let lsp_handler = LspHandler::new(completion_config);
        drop(r);
        Self {
            lsp_handler,
            state,
            read,
            write,
            messages: Arc::new(RwLock::new(vec![])),
        }
    }

    /// Spins up handle and returns connection info
    pub fn spawn_handle(
        config: CompletionConfig,
        state: SharedState,
        stream: TcpStream,
    ) -> ConnectionInfo {
        let established = Instant::now();

        let mut thread_state = ConnectionThreadState::new(config, state, stream);
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
                            match match msg {
                                seraphic::Message::Req {
                                    id,
                                    req: Request::Lsp(req),
                                } => handle_lsp_req_message(
                                    &thread_state.state,
                                    id,
                                    req,
                                    &mut thread_state.lsp_handler,
                                ),
                                _ => handle_rpc_message(&thread_state.state, msg).await,
                            } {
                                Ok(res) => {
                                    if let Some(msg) = res {
                                        tracing::warn!("returning msg: {msg:#?}");
                                        thread_state
                                            .write
                                            .writable()
                                            .await
                                            .expect("could not get writable state");
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
) -> MainResult<Option<RpcMessage>> {
    tracing::warn!("handle rpc message: {msg:#?}");
    match msg {
        seraphic::Message::Req { id, req } => match req {
            Request::Health(_) => {
                return Ok(Some(Response::from(HealthResponse {}).into_message(id)));
            }
            _ => {
                tracing::warn!("unhandled RPC req: {req:#?}");
                Ok(None)
            }
        },
        _ => Err(other_err!("did not expect non req RpcMessage: {msg:#?}")),
    }
}

fn handle_lsp_req_message(
    state: &SharedState,
    id: String,
    req: LspRequest,
    handler: &mut LspHandler,
) -> MainResult<Option<RpcMessage>> {
    let result = match Into::<lsp_server::Message>::into(req.msg) {
        lsp_server::Message::Request(req) => handler.handle_lsp_request(state, req),
        lsp_server::Message::Response(res) => handler.handle_lsp_response(state, res),
        lsp_server::Message::Notification(noti) => handler.handle_lsp_notification(state, noti),
    };
    result.map(|opt| {
        opt.and_then(|msg| Some(Response::Lsp(knowls::rpc::LspResponse { msg }).into_message(id)))
    })
}
