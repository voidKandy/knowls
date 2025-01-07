use seraphic::RpcRequest;
use tokio::{
    io::AsyncWriteExt,
    net::{TcpStream, ToSocketAddrs},
};

use crate::{other_err, rpc::TcpPacket, MainResult};

#[derive(Debug)]
pub struct Client {
    pub stream: TcpStream,
}

impl Client {
    #[tracing::instrument(name = "client connecting", skip_all)]
    pub async fn connect(addr: impl ToSocketAddrs) -> MainResult<Self> {
        let stream = TcpStream::connect(addr).await?;
        Ok(Self { stream })
    }

    pub async fn send(&mut self, r: impl RpcRequest, id: &str) -> MainResult<()> {
        let req: seraphic::socket::Request = r.into_rpc_request(id)?;
        tracing::warn!("client sending: {req:#?}");
        let packet = TcpPacket::serialize(req);

        self.stream
            .write(&packet)
            .await
            .map_err(|e| other_err!("problem writing to stream on clientside: {e:#?}"))?;

        self.stream
            .flush()
            .await
            .map_err(|e| other_err!("failed to flush stream: {e:#?}"))
    }
}
