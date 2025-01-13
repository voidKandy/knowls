use seraphic::{
    derive::{RequestWrapper, ResponseWrapper, RpcNamespace, RpcRequest},
    packet::TcpPacket,
    ResponseWrapper, RpcNamespace, RpcRequest, RpcResponse,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub type RpcPacket = TcpPacket<RpcMessage>;
pub type RpcMessage = seraphic::Message<Request, Response>;
seraphic::derive::wrapper!(RequestWrapper, Request, [LspRelayRequest]);
seraphic::derive::wrapper!(ResponseWrapper, Response, [LspRelayResponse]);

#[derive(RpcNamespace, PartialEq, Copy, Clone)]
pub enum Namespace {
    Lsp,
    Agents,
    Documents,
}

#[derive(RpcRequest, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[rpc_request(namespace = "Namespace:lsp")]
pub struct LspRelayRequest {
    /// lsp_server::Message as JSON
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LspRelayResponse {
    /// lsp_server::Message as JSON
    pub payload: Option<Value>,
}

impl From<lsp_server::Message> for LspRelayRequest {
    fn from(value: lsp_server::Message) -> Self {
        Self {
            payload: serde_json::to_value(&value).unwrap(),
        }
    }
}

impl From<lsp_server::Message> for LspRelayResponse {
    fn from(value: lsp_server::Message) -> Self {
        Self {
            payload: Some(serde_json::to_value(&value).unwrap()),
        }
    }
}
