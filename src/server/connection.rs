use super::ServerState;
use crate::{
    agents::Agents,
    knowledge::Knowledge,
    other_err,
    rpc::{
        self,
        lsp::buffer_operations::BufferOpChannelStatus,
        messages::{LspRelayResponse, Request, Response, RpcMessage, RpcPacket},
    },
    MainResult,
};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    io::AsyncWriteExt,
    net::{
        tcp::{ReadHalf, WriteHalf},
        TcpStream,
    },
    sync::RwLock,
    task::JoinHandle,
};

#[derive(Debug)]
pub(super) struct ConnectionThreadState<'c> {
    read: ReadHalf<'c>,
    write: WriteHalf<'c>,
    agents: Agents,
    knowledge: HashMap<surrealdb::sql::Id, Knowledge<'c>>,
    server_state: Arc<RwLock<ServerState<'static>>>,
}

impl<'c> ConnectionThreadState<'c> {
    const THREAD_IDLE_TIMEOUT: Duration = Duration::from_secs(10);

    fn new(stream: &'c mut TcpStream, server_state: Arc<RwLock<ServerState<'static>>>) -> Self {
        let (read, write) = stream.split();
        Self {
            read,
            write,
            agents: Agents::new(),
            knowledge: HashMap::new(),
            server_state,
        }
    }

    pub(super) fn spawn_handle(
        mut stream: TcpStream,
        server_state: Arc<RwLock<ServerState<'static>>>,
    ) -> JoinHandle<()> {
        tokio::spawn(async move {
            tracing::warn!("spawned connection handle");
            let mut thread_state = ConnectionThreadState::new(&mut stream, server_state);

            let mut last_idle = Option::<Instant>::None;
            let mut send_queue: Vec<RpcMessage> = vec![];

            loop {
                tokio::select! {
                    Ok(_) = thread_state.read.readable() => {
                        tracing::warn!("readable");
                        last_idle = None;

                        match RpcPacket::async_read(&mut thread_state.read).await {
                            Ok(Some(msg)) => {
                                tracing::warn!("server received: {msg:#?}");
                                match msg {
                                    RpcMessage::Req{ id, req } =>{
                                        if let Some(res) = thread_state.handle_rpc_request(req).await.expect("failure in handling rpc request") {
                                           send_queue.push(RpcMessage::Res {id, res});
                                        }
                                    },
                                    _ => {},
                                }
                            }
                            Ok(None) => {},
                            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                tracing::warn!("would block");
                                tokio::time::sleep(Duration::from_millis(10)).await;
                                continue;
                            }
                            Err(e) => {
                                panic!("failed to read from rpc client on server side: {e:#?}");
                            }
                        }
                    },

                    Ok(_) = thread_state.write.writable() => {
                        while let Some(res_msg) = send_queue.pop() {
                            tracing::warn!("server responding: {res_msg:#?}");

                            RpcPacket::async_write(&mut thread_state.write, &res_msg).await
                                .expect("failed to write rpc response  on serverside");


                            thread_state
                                .write
                                .flush()
                                .await
                                .expect("failed to flush serverside stream");
                        }
                    },

                    else => {
                        match last_idle {
                            Some(instant) => {
                                if instant.elapsed() >= ConnectionThreadState::THREAD_IDLE_TIMEOUT {
                                    tracing::warn!("thread timeout!");
                                    return;
                                }
                            }
                            None => {
                                let now = Instant::now();
                                tracing::warn!("setting idle now: {now:#?}");
                                last_idle = Some(now);
                            }
                        }
                    }
                }
            }
        })
    }

    async fn handle_rpc_request(&mut self, req: Request) -> MainResult<Option<Response>> {
        match req {
            Request::Lsp(lsp_req) => {
                let message = serde_json::from_value::<lsp_server::Message>(lsp_req.payload)
                    .expect("failed to get lsp_server::Message from relay rq payload");
                if let Some(msg) = self.handle_lsp_message(message).await? {
                    let response = LspRelayResponse::from(msg);
                    return Ok(Some(response.into()));
                }
            }
        }
        Ok(Option::<Response>::None)
    }

    pub async fn handle_lsp_message(
        &mut self,
        message: lsp_server::Message,
    ) -> MainResult<Option<lsp_server::Message>> {
        match match message {
            lsp_server::Message::Notification(not) => {
                rpc::lsp::notifications::handle_notification(not, self.server_state.clone()).await
            }
            lsp_server::Message::Request(req) => {
                if req.method.as_str() == "shutdown" {
                    tracing::warn!("shutting down server");
                    return Ok(None);
                }
                rpc::lsp::requests::handle_request(req, self.server_state.clone()).await
            }
            _ => Err(std::io::Error::other("No handler for responses").into()),
        } {
            Ok(mut buffer_op_channel_handler) => {
                while let Some(status) = buffer_op_channel_handler.receiver.recv().await {
                    match status.unwrap() {
                        BufferOpChannelStatus::Finished => break,
                        BufferOpChannelStatus::Working(buffer_op) => {
                            return Ok(buffer_op.do_operation().await.unwrap())
                        }
                    }
                }
            }
            Err(err) => return Err(other_err!("error in handler: {}", err)),
        }
        Ok(None)
    }
}
