use crate::{database::Record, rpc::ConnectionInfo};
use knowls::{
    other_err,
    rpc::{HealthResponse, LspMessage, LspRequest, Request, Response, RpcMessage},
    MainResult,
};
use lsp_server::ResponseError;
use lsp_types::{DidOpenTextDocumentParams, DidSaveTextDocumentParams, Hover, HoverParams};
use seraphic::ResponseWrapper;
use std::{collections::HashMap, sync::Arc};
use surrealdb::RecordId;
use tokio::{net::TcpListener, sync::RwLock};
use tracing::field::AsField;

use crate::database::{models::Knowledge, Database};

pub struct ConnectionInfoWrapper {
    pub info: ConnectionInfo,
    typ: ConnectionType,
}

impl From<ConnectionInfo> for ConnectionInfoWrapper {
    fn from(info: ConnectionInfo) -> Self {
        Self {
            info,
            typ: ConnectionType::Undefined,
        }
    }
}

#[derive(PartialEq, Eq)]
enum ConnectionType {
    Undefined,
    Lsp { received_shutdown: bool },
}

impl ConnectionInfoWrapper {
    /// Once a connection has an LSP message pass through it, it will be marked as an LSP connection
    fn make_lsp(&mut self) {
        if self.typ == ConnectionType::Undefined {
            self.typ = ConnectionType::Lsp {
                received_shutdown: false,
            }
        }
    }
}

pub struct State {
    pub database: Database,
    pub knowledge: HashMap<RecordId, Knowledge>,
    pub connections: Arc<RwLock<HashMap<String, ConnectionInfoWrapper>>>,
    pub lsp_documents: HashMap<lsp_types::Uri, String>,
}

impl State {
    pub fn new(database: Database) -> Self {
        Self {
            database,
            knowledge: HashMap::new(),
            connections: Arc::new(RwLock::new(HashMap::new())),
            lsp_documents: HashMap::new(),
        }
    }

    /// Does 2 things:
    /// 1. Drops any connections who's threads have finished
    /// 2. Goes through any connection with pending RPC requests and handles them
    pub async fn manage_connections(&mut self) -> MainResult<()> {
        self.connections.write().await.retain(|c, info| {
            if info.info.handle.is_finished() {
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
                    let mut incoming_queue = info.info.incoming.write().await;
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
                        tracing::warn!("pushing response to outbound queue: {response:#?}");
                        info.info.push_outbound(response).await;
                    }
                }
                info.info
                    .incoming_pending
                    .store(false, std::sync::atomic::Ordering::Relaxed);

                self.connections.write().await.insert(addr, info);
            }
        }
        Ok(())
    }

    async fn handle_rpc_message(
        &mut self,
        msg: RpcMessage,
        conn: &mut ConnectionInfoWrapper,
    ) -> MainResult<Option<RpcMessage>> {
        tracing::warn!("handle rpc message: {msg:#?}");
        match msg {
            seraphic::Message::Req { id, req } => match req {
                Request::Health(_) => {
                    return Ok(Some(Response::from(HealthResponse {}).into_message(id)));
                }
                Request::Lsp(req) => {
                    if let ConnectionType::Undefined = conn.typ {
                        conn.make_lsp();
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

    fn all_connection_keys_with_incoming(&self) -> MainResult<Vec<String>> {
        Ok(self
            .connections
            .try_read()?
            .iter()
            .fold(vec![], |mut acc, (addr, info)| {
                if info
                    .info
                    .incoming_pending
                    .load(std::sync::atomic::Ordering::Relaxed)
                {
                    acc.push(addr.to_string())
                }
                acc
            }))
    }

    fn handle_lsp_req_message(
        &mut self,
        req: LspRequest,
        conn: &mut ConnectionInfoWrapper,
    ) -> MainResult<Option<LspMessage>> {
        tracing::warn!("handle lsp message: {req:#?}");
        match Into::<lsp_server::Message>::into(req.msg) {
            lsp_server::Message::Request(req) => {
                return self.handle_lsp_request(req, conn);
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
    fn handle_lsp_response(
        &mut self,
        _res: lsp_server::Response,
    ) -> MainResult<Option<LspMessage>> {
        Ok(None)
    }

    fn handle_lsp_request(
        &mut self,
        req: lsp_server::Request,
        conn: &mut ConnectionInfoWrapper,
    ) -> MainResult<Option<LspMessage>> {
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
                let params = serde_json::from_value::<HoverParams>(req.params)?;
                let pos = params.text_document_position_params.position;

                if let Some(content) = self
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
                                if let Some(kn) = self.knowledge.values().find(|k| k.lsp_char == ac)
                                {
                                    let hover = Hover {
                                        contents: lsp_types::HoverContents::Markup(
                                            lsp_types::MarkupContent {
                                                kind: lsp_types::MarkupKind::Markdown,
                                                value: kn.content.to_owned(),
                                            },
                                        ),
                                        range: None,
                                    };

                                    let json = serde_json::to_value(hover)
                                        .expect("could not serialize hover");
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
        &mut self,
        noti: lsp_server::Notification,
    ) -> MainResult<Option<LspMessage>> {
        match noti.method.as_str() {
            "textDocument/didChange" => {
                let params = serde_json::from_value::<DidOpenTextDocumentParams>(noti.params)?;
                self.lsp_documents
                    .insert(params.text_document.uri, params.text_document.text);
            }
            "textDocument/didSave" => {
                let params = serde_json::from_value::<DidSaveTextDocumentParams>(noti.params)?;
                self.lsp_documents
                    .insert(params.text_document.uri, params.text.unwrap());
            }
            "textDocument/didOpen" => {}
            m => {
                tracing::warn!("unhandled notification: {m:#?}");
            }
        }
        Ok(None)
    }
}
