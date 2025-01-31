use seraphic::Message;
use seraphic::{
    derive::{RequestWrapper, ResponseWrapper, RpcNamespace, RpcRequest},
    ResponseWrapper, RpcNamespace, RpcRequest, RpcResponse,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, RequestWrapper)]
pub enum Request {
    Health(HealthRequest),
}

#[derive(Debug, PartialEq, ResponseWrapper)]
pub enum Response {
    Health(HealthResponse),
}

pub type RpcMessage = Message<Request, Response>;

#[derive(RpcNamespace, PartialEq, Copy, Clone)]
pub enum Namespace {
    Health,
}

#[derive(RpcRequest, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[rpc_request(namespace = "Namespace:health")]
pub struct HealthRequest {}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HealthResponse {}
