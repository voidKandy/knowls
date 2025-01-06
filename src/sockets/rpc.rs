use seraphic::{RpcNamespace, RpcRequest, RpcRequestWrapper};
use serde::{Deserialize, Serialize};

#[derive(RpcNamespace, PartialEq, Copy, Clone)]
enum Namespace {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerRelayResponse {
    pub payload: Option<lsp_server::Message>,
}
