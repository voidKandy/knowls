use seraphic::{RpcNamespace, RpcRequest, RpcRequestWrapper};
use serde::{Deserialize, Serialize};

#[derive(RpcNamespace, PartialEq, Copy, Clone)]
pub enum Namespace {
    Relay,
    Agents,
    Documents,
}

#[derive(Debug, RpcRequestWrapper)]
pub enum ServerRPCWrapper {
    Relay(ServerRelayRequest),
}

#[derive(RpcRequest, Debug, Clone, Serialize, Deserialize)]
#[rpc_request(namespace = "Namespace:relay")]
pub struct ServerRelayRequest {
    pub payload: lsp_server::Message,
}

impl From<lsp_server::Message> for ServerRelayRequest {
    fn from(value: lsp_server::Message) -> Self {
        Self { payload: value }
    }
}

impl From<lsp_server::Message> for ServerRelayResponse {
    fn from(value: lsp_server::Message) -> Self {
        Self {
            payload: Some(value),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerRelayResponse {
    pub payload: Option<lsp_server::Message>,
}
