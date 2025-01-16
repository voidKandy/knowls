use std::{sync::LazyLock, thread::sleep, time::Duration};

use crate::helpers::{test_config, TEST_TRACING};
use espx_lsp_server::{
    client::Client,
    rpc::messages::{
        HealthRequest, HealthResponse, LspRelayRequest, LspRelayResponse, Response, RpcMessage,
        RpcPacket,
    },
    server::Server,
};
use seraphic::{packet::PacketRead, JSONRPC_FIELD};

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

    let req = HealthRequest {};
    let expected = HealthResponse {};

    client.send(req.into(), "1").await.unwrap();

    tracing::warn!("client listenening for response");
    loop {
        match RpcPacket::async_read(&mut client.stream).await.unwrap() {
            PacketRead::Message(msg) => {
                if let RpcMessage::Res { res, .. } = msg {
                    assert_eq!(res, Response::from(expected));
                    tracing::warn!("got expected response from server!");
                    return;
                }
            }
            PacketRead::Empty => {}
            PacketRead::Disconnected => {
                break;
            }
        }
    }

    panic!("client disconnected before receiving response");
}
