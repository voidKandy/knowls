use super::ServerState;
use crate::{
    other_err,
    rpc::{
        self,
        lsp::buffer_operations::BufferOpChannelStatus,
        messages::{HealthResponse, LspRelayResponse, Request, Response, RpcMessage, RpcPacket},
    },
    MainResult,
};
use seraphic::packet::PacketRead;
use std::{
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
    server_state: Arc<RwLock<ServerState>>,
}

impl<'c> ConnectionThreadState<'c> {
    const THREAD_IDLE_TIMEOUT: Duration = Duration::from_secs(30);

    fn new(stream: &'c mut TcpStream, server_state: Arc<RwLock<ServerState>>) -> Self {
        let (read, write) = stream.split();
        Self {
            read,
            write,
            server_state,
        }
    }

    pub(super) fn spawn_handle(
        mut stream: TcpStream,
        server_state: Arc<RwLock<ServerState>>,
    ) -> JoinHandle<()> {
        tokio::spawn(async move {
            tracing::warn!("spawned connection handle");
            let mut thread_state = ConnectionThreadState::new(&mut stream, server_state);

            let mut last_idle = Option::<Instant>::None;
            let mut send_queue: Vec<RpcMessage> = vec![];

            loop {
                if let Some(Ok(_)) = if !send_queue.is_empty() {
                    Some(thread_state.write.writable().await)
                } else {
                    None
                } {
                    while let Some(res_msg) = send_queue.pop() {
                        tracing::warn!("server responding: {res_msg:#?}");

                        RpcPacket::async_write(&mut thread_state.write, &res_msg)
                            .await
                            .expect("failed to write rpc response  on serverside");

                        thread_state
                            .write
                            .flush()
                            .await
                            .expect("failed to flush serverside stream");
                    }
                } else if let Ok(read) = tokio::time::timeout(
                    Duration::from_millis(500),
                    RpcPacket::async_read(&mut thread_state.read),
                )
                .await
                {
                    tracing::warn!("readable");
                    last_idle = None;
                    match read {
                        Ok(PacketRead::Message(msg)) => {
                            tracing::warn!("server received: {msg:#?}");
                            match msg {
                                RpcMessage::Req { id, req } => {
                                    if let Some(res) = thread_state
                                        .handle_rpc_request(req)
                                        .await
                                        .expect("failure in handling rpc request")
                                    {
                                        send_queue.push(RpcMessage::Res { id, res });
                                    }
                                }
                                _ => {
                                    tracing::warn!("no logic implemented for handling {msg:#?} on the serverside");
                                }
                            }
                        }

                        Ok(PacketRead::Disconnected) => {
                            tracing::warn!("disconnected");
                            break;
                        }

                        Ok(PacketRead::Empty) => {
                            tracing::warn!("empty");
                            tokio::time::sleep(Duration::from_millis(10)).await;
                            continue;
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            tracing::warn!("would block");
                            tokio::time::sleep(Duration::from_millis(10)).await;
                            continue;
                        }

                        Err(e) => {
                            panic!("failed to read from rpc client on server side: {e:#?}");
                        }
                    }
                } else {
                    match last_idle {
                        Some(instant) => {
                            if instant.elapsed() >= ConnectionThreadState::THREAD_IDLE_TIMEOUT {
                                tracing::warn!("thread timeout!");
                                break;
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
        })
    }

    async fn handle_rpc_request(&mut self, req: Request) -> MainResult<Option<Response>> {
        match req {
            Request::Lsp(lsp_req) => {
                let message = serde_json::from_value::<lsp_server::Message>(lsp_req.payload)
                    .expect("failed to get lsp_server::Message from relay rq payload");
                tracing::warn!("succesfully deserialized lsp relay message: {message:#?}");
                if let Some(msg) = self.handle_lsp_message(message).await? {
                    let response = LspRelayResponse::from(msg);
                    return Ok(Some(response.into()));
                }
            }
            Request::Health(_) => {
                let response = Response::from(HealthResponse {});
                return Ok(Some(response));
            }
        }
        Ok(Option::<Response>::None)
    }

    // perhaps later should pass more than just innermost state to these functions, but for now
    // this is fine
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
