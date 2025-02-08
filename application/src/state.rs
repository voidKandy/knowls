use crate::{database::Record, rpc::ConnectionInfo};
use knowls::{
    other_err,
    rpc::{HealthResponse, LspMessage, LspRequest, Request, Response, RpcMessage},
    MainResult,
};
use seraphic::ResponseWrapper;
use std::{collections::HashMap, sync::Arc};
use surrealdb::RecordId;
use tokio::{net::TcpListener, sync::RwLock};
use tracing::field::AsField;

use crate::database::{models::Knowledge, Database};

pub struct State {
    pub database: Database,
    pub knowledge: HashMap<RecordId, Knowledge>,
    pub connections: Arc<RwLock<HashMap<String, ConnectionInfo>>>,
}

impl State {
    pub fn new(database: Database) -> Self {
        Self {
            database,
            knowledge: HashMap::new(),
            connections: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Does 2 things:
    /// 1. Drops any connections who's threads have finished
    /// 2. Goes through any connection with pending RPC requests and handles them
    pub async fn manage_connections(&mut self) -> MainResult<()> {
        self.connections.write().await.retain(|c, info| {
            if info.handle.is_finished() {
                tracing::warn!("dropping connection to: {c:#?}");
                false
            } else {
                true
            }
        });

        let conns_to_handle = self.all_connection_keys_with_incoming()?;

        if !conns_to_handle.is_empty() {
            tracing::warn!(
                "the following connection have unhandled messages: {conns_to_handle:#?}"
            );

            for addr in conns_to_handle {
                tracing::warn!("handling messages for {addr:#?}");
                let mut info = self
                    .connections
                    .write()
                    .await
                    .remove(&addr)
                    .expect("somehow got an invalid connection key?");

                loop {
                    let mut info_w = info.incoming.write().await;
                    let message = match info_w.pop_front() {
                        Some(msg) => msg,
                        None => break,
                    };
                    drop(info_w);
                    if let Some(response) = self
                        .handle_rpc_message(message)
                        .await
                        .expect("failed to handle rpc message")
                    {
                        info.push_outbound(response).await;
                    }
                }
                info.incoming_pending
                    .store(false, std::sync::atomic::Ordering::Relaxed);

                self.connections.write().await.insert(addr, info);
            }
        }
        Ok(())
    }

    async fn handle_rpc_message(&mut self, msg: RpcMessage) -> MainResult<Option<RpcMessage>> {
        match msg {
            seraphic::Message::Req { id, req } => match req {
                Request::Health(_) => {
                    return Ok(Some(Response::from(HealthResponse {}).into_message(id)));
                }
                Request::Lsp(req) => {
                    if let Some(res_msg) = self.handle_lsp_req_message(req).await? {
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

    fn all_connection_keys_with_incoming(&self) -> MainResult<Vec<String>> {
        Ok(self
            .connections
            .try_read()?
            .iter()
            .fold(vec![], |mut acc, (addr, info)| {
                if info
                    .incoming_pending
                    .load(std::sync::atomic::Ordering::Relaxed)
                {
                    acc.push(addr.to_string())
                }
                acc
            }))
    }

    async fn handle_lsp_req_message(&mut self, req: LspRequest) -> MainResult<Option<LspMessage>> {
        match Into::<lsp_server::Message>::into(req.msg) {
            lsp_server::Message::Request(req) => {
                return self.handle_lsp_request(req);
            }
            lsp_server::Message::Response(res) => {
                return self.handle_lsp_response(res);
            }
            lsp_server::Message::Notification(noti) => {
                return self.handle_lsp_notification(noti);
            }
        }
    }

    /// I don't know why this end of the connection would ever receive responses
    fn handle_lsp_response(&mut self, res: lsp_server::Response) -> MainResult<Option<LspMessage>> {
        Ok(None)
    }

    fn handle_lsp_request(&mut self, req: lsp_server::Request) -> MainResult<Option<LspMessage>> {
        match req.method.as_str() {
            "textDocument/definition" => {}
            "textDocument/hover" => {}
            "textDocument/diagnostic" => {}
            m => {
                tracing::warn!("unhandled request method: {m:#?}");
            }
        }
        Ok(None)
    }

    fn handle_lsp_notification(
        &mut self,
        noti: lsp_server::Notification,
    ) -> MainResult<Option<LspMessage>> {
        match noti.method.as_str() {
            "textDocument/didChange" => {}
            "textDocument/didSave" => {}
            "textDocument/didOpen" => {}
            m => {
                tracing::warn!("unhandled notification: {m:#?}");
            }
        }
        Ok(None)
    }
}
