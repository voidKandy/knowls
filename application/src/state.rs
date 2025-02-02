use crate::rpc::ConnectionInfo;
use knowls::{
    other_err,
    rpc::{HealthResponse, Request, Response, RpcMessage},
    MainResult,
};
use seraphic::ResponseWrapper;
use std::{collections::HashMap, sync::Arc};
use tokio::{net::TcpListener, sync::RwLock};

use crate::database::{models::Knowledge, Database};

pub struct Application {
    listener: TcpListener,
    state: Arc<RwLock<State>>,
}

pub struct State {
    pub database: Database,
    pub knowledge: HashMap<surrealdb::RecordId, Knowledge>,
    pub connections: HashMap<String, ConnectionInfo>,
}

impl State {
    pub fn new(database: Database) -> Self {
        Self {
            database,
            knowledge: HashMap::new(),
            connections: HashMap::new(),
        }
    }

    /// Does 2 things:
    /// 1. Drops any connections who's threads have finished
    /// 2. Goes through any connection with pending RPC requests and handles them
    pub async fn manage_connections(&mut self) -> MainResult<()> {
        self.connections.retain(|c, info| {
            if info.handle.is_finished() {
                tracing::warn!("dropping connection to: {c:#?}");
                false
            } else {
                true
            }
        });

        let conns_to_handle = self.all_connection_keys_with_incoming();

        if !conns_to_handle.is_empty() {
            tracing::warn!(
                "the following connection have unhandled messages: {conns_to_handle:#?}"
            );

            for addr in conns_to_handle {
                tracing::warn!("handling messages for {addr:#?}");
                let mut info = self
                    .connections
                    .remove(&addr)
                    .expect("somehow got an invalid connection key?");

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

                self.connections.insert(addr, info);
            }
        }
        Ok(())
    }

    async fn handle_rpc_message(&mut self, msg: RpcMessage) -> MainResult<RpcMessage> {
        match msg {
            seraphic::Message::Req { id, req } => match req {
                Request::Health(_) => Ok(Response::from(HealthResponse {}).into_message(id)),
            },
            _ => Err(other_err!("did not expect non req RpcMessage: {msg:#?}")),
        }
    }

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
