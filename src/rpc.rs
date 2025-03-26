use seraphic::Message;
use seraphic::{
    derive::{RequestWrapper, ResponseWrapper, RpcNamespace, RpcRequest},
    ResponseWrapper, RpcNamespace, RpcRequest, RpcResponse,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, RequestWrapper)]
pub enum Request {
    Health(HealthRequest),
    Lsp(LspRequest),
}

#[derive(Debug, PartialEq, ResponseWrapper)]
pub enum Response {
    Health(HealthResponse),
    Lsp(LspResponse),
}

pub type RpcMessage = Message<Request, Response>;

#[derive(RpcNamespace, PartialEq, Copy, Clone)]
pub enum Namespace {
    Health,
    Lsp,
}

#[derive(RpcRequest, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[rpc_request(namespace = "Namespace:health")]
pub struct HealthRequest {}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HealthResponse {}

/// We need to wrap the message so we can have a type that implements `PartialEq`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspMessage {
    pub msg: lsp_server::Message,
}

impl AsRef<lsp_server::Message> for LspMessage {
    fn as_ref(&self) -> &lsp_server::Message {
        &self.msg
    }
}
impl From<lsp_server::Message> for LspMessage {
    fn from(msg: lsp_server::Message) -> Self {
        Self { msg }
    }
}

impl Into<lsp_server::Message> for LspMessage {
    fn into(self) -> lsp_server::Message {
        self.msg
    }
}

impl PartialEq for LspMessage {
    fn eq(&self, other: &Self) -> bool {
        match &self.msg {
            lsp_server::Message::Request(p) => {
                if let lsp_server::Message::Request(op) = &other.msg {
                    return p.method == op.method && p.params == op.params;
                }
            }
            lsp_server::Message::Response(p) => {
                if let lsp_server::Message::Response(op) = &other.msg {
                    let errors_match = match &p.error {
                        Some(e) => op.error.as_ref().is_some_and(|oe| {
                            oe.code == e.code && oe.message == e.message && oe.data == e.data
                        }),
                        None => op.error.is_none(),
                    };
                    return p.id == op.id && p.result == op.result && errors_match;
                }
            }
            lsp_server::Message::Notification(p) => {
                if let lsp_server::Message::Notification(op) = &other.msg {
                    return p.method == op.method && p.params == op.params;
                }
            }
        }
        false
    }
}

#[derive(RpcRequest, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[rpc_request(namespace = "Namespace:lsp")]
pub struct LspRequest {
    pub msg: LspMessage,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LspResponse {
    pub msg: LspMessage,
}
