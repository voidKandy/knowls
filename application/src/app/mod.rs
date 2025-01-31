mod connection;
pub mod tui;
use connection::ConnectionInfo;
use knowls::{
    other_err,
    rpc::{HealthResponse, Request, Response, RpcMessage},
    MainResult,
};
use seraphic::ResponseWrapper;
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};
use tokio::{
    net::{TcpListener, ToSocketAddrs},
    sync::RwLock,
};

use crate::database::{models::Knowledge, Database};

pub struct Application {
    listener: TcpListener,
    state: Arc<RwLock<State>>,
}

type SharedMessageQueue = Arc<RwLock<VecDeque<RpcMessage>>>;

pub struct State {
    database: Database,
    knowledge: HashMap<surrealdb::RecordId, Knowledge>,
    connections: HashMap<String, ConnectionInfo>,
}

impl State {
    fn all_connection_keys_with_incoming(&self) -> Vec<String> {
        self.connections
            .iter()
            .fold(vec![], |mut acc, (addr, info)| {
                if info
                    .incoming_pending
                    .load(std::sync::atomic::Ordering::Relaxed)
                {
                    acc.push(addr.to_string())
                }
                acc
            })
    }
}

impl Application {
    pub async fn new(database: Database, addr: impl ToSocketAddrs) -> MainResult<Self> {
        let listener = TcpListener::bind(addr).await?;
        let state = Arc::new(RwLock::new(State {
            database,
            // Populate this with the database once you have your shit together
            knowledge: HashMap::new(),
            connections: HashMap::new(),
        }));
        Ok(Self { listener, state })
    }

    pub fn clone_state(&self) -> Arc<RwLock<State>> {
        Arc::clone(&self.state)
    }

    #[tracing::instrument(name = "application main loop", skip_all)]
    pub async fn main_loop(&mut self) {
        loop {
            match self.listener.accept().await {
                Ok((stream, addr)) => {
                    tracing::warn!("connected: {addr:#?}");
                    let connection_info = connection::ConnectionThreadState::spawn_handle(stream);
                    let mut w = self.state.write().await;
                    w.connections.insert(addr.to_string(), connection_info);
                }
                Err(e) => tracing::warn!("couldn't accept connection: {e:?}"),
            }
            let mut w = self.state.write().await;
            w.connections.retain(|c, info| {
                if info.handle.is_finished() {
                    tracing::warn!("dropping connection to: {c:#?}");
                    false
                } else {
                    true
                }
            });
            let conns_to_handle = w.all_connection_keys_with_incoming();
            drop(w);

            if !conns_to_handle.is_empty() {
                tracing::warn!(
                    "the following connection have unhandled messages: {conns_to_handle:#?}"
                );

                for addr in conns_to_handle {
                    tracing::warn!("handling messages for {addr:#?}");
                    w = self.state.write().await;
                    let mut info = w
                        .connections
                        .remove(&addr)
                        .expect("somehow got an invalid connection key?");
                    drop(w);

                    loop {
                        let mut info_w = info.incoming.write().await;
                        let message = match info_w.pop_front() {
                            Some(msg) => msg,
                            None => break,
                        };
                        drop(info_w);
                        let response = self
                            .handle_rpc_message(message)
                            .await
                            .expect("failed to handle rpc message");
                        info.push_outbound(response).await;
                    }
                    info.incoming_pending
                        .store(false, std::sync::atomic::Ordering::Relaxed);

                    w = self.state.write().await;
                    w.connections.insert(addr, info);
                }
            }
        }
    }

    async fn handle_rpc_message(&mut self, msg: RpcMessage) -> MainResult<RpcMessage> {
        match msg {
            seraphic::Message::Req { id, req } => match req {
                Request::Health(_) => Ok(Response::from(HealthResponse {}).into_message(id)),
            },
            _ => Err(other_err!("did not expect non req RpcMessage: {msg:#?}")),
        }
    }
}
