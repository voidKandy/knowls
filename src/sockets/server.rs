use crate::{
    config::Config,
    other_err,
    server::{self, buffer_operations::BufferOpChannelStatus, requests::handle_request},
    sockets::{SocketGuard, CLIENTSIDE_RELAY_ADDR, SERVERSIDE_RELAY_ADDR},
    state::{handle_lsp_message, LspState, SharedState},
    MainResult,
};
use lsp_types::{
    CodeActionProviderCapability, DiagnosticServerCapabilities, InitializeParams,
    ServerCapabilities, TextDocumentSyncCapability, TextDocumentSyncKind, TextDocumentSyncOptions,
    TextDocumentSyncSaveOptions, WorkDoneProgressOptions,
};
use seraphic::{thread::RpcListeningThread, RpcHandler, RpcRequestWrapper, RpcResponse};
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Interest},
    net::{lookup_host, unix::SocketAddr, ToSocketAddrs, UnixListener, UnixStream},
    sync::{mpsc::error::TryRecvError, RwLock},
    task::JoinHandle,
};
use tracing::warn;

use super::rpc::{ServerRPCWrapper, ServerRelayResponse};

struct ConnectionContext {
    rpc_listener: RpcListeningThread,
    state: SharedState<'static>,
}

impl ConnectionContext {
    fn init_thread(mut self) -> JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                match self.rpc_listener.recv.try_recv() {
                    Ok(val) => {
                        let id = val.id.clone();
                        let wrapper = ServerRPCWrapper::try_from_rpc_req(val)
                            .expect("failed to get rpc wrapper");

                        let response = handle_rpc_request(wrapper, id, self.state.clone())
                            .await
                            .expect("failed to handle rpc request");

                        self.rpc_listener
                            .sender
                            .send(response)
                            .await
                            .expect("failed to send rpc response");
                    }
                    Err(TryRecvError::Empty) => {}
                    Err(TryRecvError::Disconnected) => return,
                }
            }
        })
    }
}

async fn handle_rpc_request(
    request: ServerRPCWrapper,
    id: String,
    state: SharedState<'static>,
) -> MainResult<seraphic::socket::Response> {
    match request {
        ServerRPCWrapper::Relay(message) => {
            let payload = handle_lsp_message(state, message.payload).await?;
            let response = serde_json::to_value(ServerRelayResponse { payload }).unwrap();
            return Ok(seraphic::socket::Response::from((Ok(response), id)));
        }
    }
}

/// This is the outermost struct for the LSP application
/// It can handle connections to thin RPC Clients as well as full LSP Clients
pub struct LspApplicationServer {
    config: Config,
    /// Keys are tokio::net::unix::SocketAddr as a string
    connections: HashMap<String, JoinHandle<()>>,
}

impl LspApplicationServer {
    #[tracing::instrument(name = "new relay server")]
    pub async fn new(config: Config) -> Self {
        Self {
            config,
            connections: HashMap::new(),
        }
    }

    pub async fn try_connect(&mut self, addr: impl ToSocketAddrs) -> MainResult<()> {
        let key = lookup_host(&addr).await?.next().unwrap().to_string();
        let rpc_listener = RpcListeningThread::new(addr).await?;
        let state = LspState::new(&self.config).await?.as_shared();
        let context = ConnectionContext {
            rpc_listener,
            state,
        };
        let thread = context.init_thread();
        self.connections.insert(key, thread);
        Ok(())
    }

    #[tracing::instrument(name = "relay server main loop", skip_all)]
    pub async fn main_loop(mut self) {
        let mut buf = [0; 1024];
        //     let mut send_back_queue = VecDeque::new();
        //     loop {
        //         let ready = self
        //             .socket_stream
        //             .ready(Interest::READABLE | Interest::WRITABLE)
        //             .await
        //             .expect("could not get ready state");
        //
        //         if ready.is_readable() {
        //             match self.socket_stream.try_read(&mut buf) {
        //                 Ok(0) => {
        //                     warn!("connection with client closed");
        //                     break;
        //                 }
        //                 Ok(n) => {
        //                     warn!("client read {n} bytes");
        //                     let msg = serde_json::from_slice::<lsp_server::Message>(&buf[..n])
        //                         .expect("failed to deserialize buffer");
        //                     warn!("Received on server side: {msg:#?}");
        //                     if let Some(msg) = self
        //                         .handle_lsp_message(msg)
        //                         .await
        //                         .expect("failed to handle lsp message")
        //                     {
        //                         send_back_queue.push_back(msg);
        //                     }
        //                 }
        //                 Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
        //                     warn!("reading from server stream would block");
        //                 }
        //                 Err(e) => {
        //                     panic!("error in server receiving: {e:#?}")
        //                 }
        //             }
        //         }
        //         if let Some(msg) = send_back_queue.pop_front() {
        //             if ready.is_writable() {
        //                 let v = serde_json::to_vec(&msg).expect("failed to serialize message");
        //                 match self.socket_stream.try_write(&v) {
        //                     Ok(_) => {
        //                         warn!("successfully sent message to relay client");
        //                     }
        //                     Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
        //                         warn!(
        //                             "writing from server stream would block, pushing msg back to queue"
        //                         );
        //                         send_back_queue.push_front(msg);
        //                     }
        //                     Err(e) => {
        //                         panic!("error in server sending: {e:#?}")
        //                     }
        //                 }
        //             }
        //         }
        //     }
    }
}
