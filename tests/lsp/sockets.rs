use std::{sync::LazyLock, thread::sleep, time::Duration};

use crate::helpers::{test_config, TEST_TRACING};
use espx_lsp_server::{
    scratch::{Client, Server, TcpPacket},
    sockets::rpc::ServerRelayRequest,
};
use seraphic::JSONRPC_FIELD;

#[tokio::test]
async fn client_server_comms() {
    LazyLock::force(&TEST_TRACING);

    std::panic::set_hook(Box::new(|v| {
        let payload = v.payload();
        let str = payload.downcast_ref::<String>().cloned().unwrap_or(
            payload
                .downcast_ref::<&'static str>()
                .map(|str| str.to_string())
                .unwrap_or("Any".to_string()),
        );

        tracing::error!("thread panicked at: {:#?}\npayload: {str:#?}", v.location(),)
    }));
    tracing::warn!("spinning up headless Language Server");

    let config = test_config(false);
    tracing::warn!("initializing with config: {config:#?}");
    let addr = "127.0.0.1:1598";
    let mut server = Server::new(config, addr).await;
    tokio::spawn(async move { server.main_loop().await });
    sleep(Duration::from_millis(500));

    let mut client = Client::connect(addr)
        .await
        .expect("should not fail to start relay client");

    let req = ServerRelayRequest {
        payload: lsp_server::Message::Notification(lsp_server::Notification {
            method: "test".to_string(),
            params: serde_json::json!({}),
        }),
    };

    let expected = seraphic::socket::Response {
        jsonrpc: JSONRPC_FIELD.to_string(),
        id: "1".to_string(),
        error: None,
        result: Some(serde_json::to_value(&req.payload).unwrap()),
    };

    client.send(req, "1").await.unwrap();

    let res: seraphic::socket::Response = TcpPacket::read_from_stream(&mut client.stream)
        .await
        .unwrap()
        .try_into()
        .unwrap();

    assert_eq!(res, expected);
    tracing::warn!("got expected response from server!");

    let req = ServerRelayRequest {
        payload: lsp_server::Message::Notification(lsp_server::Notification {
            method: "test".to_string(),
            params: serde_json::json!({}),
        }),
    };
    client.send(req, "1").await.unwrap();

    let res: seraphic::socket::Response = TcpPacket::read_from_stream(&mut client.stream)
        .await
        .unwrap()
        .try_into()
        .unwrap();

    assert_eq!(res, expected);

    tracing::warn!("got expected response from server!");
}
