use crate::{
    agents::Agents,
    knowledge::Knowledge,
    sockets::{
        rpc::{ServerRPCWrapper, ServerRelayResponse},
        TcpPacket,
    },
    MainResult,
};
use seraphic::{RpcRequestWrapper, RpcResponse};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{
        tcp::{ReadHalf, WriteHalf},
        TcpStream,
    },
    sync::RwLock,
    task::JoinHandle,
};

use super::ServerState;

#[derive(Debug)]
pub(super) struct ConnectionThreadState<'c> {
    read: ReadHalf<'c>,
    write: WriteHalf<'c>,
    agents: Agents,
    knowledge: HashMap<surrealdb::sql::Id, Knowledge<'c>>,
    server_state: Arc<RwLock<ServerState<'static>>>,
}

impl<'c> ConnectionThreadState<'c> {
    const THREAD_IDLE_TIMEOUT: Duration = Duration::from_secs(30);

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
            let mut send_queue: Vec<seraphic::socket::Response> = vec![];

            loop {
                tokio::select! {
                    Ok(_) = thread_state.read.readable() => {
                        tracing::warn!("readable");
                        last_idle = None;

                        match TcpPacket::read_from_stream(&mut thread_state.read).await {
                            Ok(msg) => {
                                let req: seraphic::socket::Request = msg.try_into().expect("received response in server");
                                let id = req.id.clone();
                                let msg = ServerRPCWrapper::try_from_rpc_req(req)
                                    .expect("failed to get server rpc request");
                                if let Some(response) = thread_state.handle_rpc_request(msg).await.expect("failure in handling rpc request") {
                                        send_queue.push(response.into_response(id));
                                }
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
                    },

                    Ok(_) = thread_state.write.writable() => {
                        tracing::warn!("writable");
                        if !send_queue.is_empty() {
                            last_idle = None;
                            while let Some(res) = send_queue.pop() {
                                tracing::warn!("server responding: {res:#?}");
                                let packet = TcpPacket::serialize(res);
                                // let res =
                                //     serde_json::to_vec(&msg).expect("failed to serialize relay response");

                                thread_state
                                    .write
                                    .write(&packet)
                                    .await
                                    .expect("failed to write rpc response  on serverside");

                                thread_state
                                    .write
                                    .flush()
                                    .await
                                    .expect("failed to flush serverside stream");
                            }
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

    async fn handle_rpc_request(
        &mut self,
        req_wrapper: ServerRPCWrapper,
    ) -> MainResult<Option<impl RpcResponse>> {
        match req_wrapper {
            ServerRPCWrapper::Relay(relay) => {
                let lsp_message = relay.payload;
                // let json = serde_json::to_value(&lsp_message.payload).unwrap();
                // let response = ServerRelayResponse {
                //     payload: Some(lsp_message.payload),
                // };
                // let response = seraphic::socket::Response::from((Ok(json), id));
            }
        }
        Ok(Option::<ServerRelayResponse>::None)
    }
}
