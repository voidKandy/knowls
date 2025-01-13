use crate::{
    other_err,
    rpc::messages::{Request, RpcMessage, RpcPacket},
    MainResult,
};
use tokio::{
    io::AsyncWriteExt,
    net::{TcpStream, ToSocketAddrs},
};

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

    pub async fn send(&mut self, req: Request, id: &str) -> MainResult<()> {
        let msg = RpcMessage::Req {
            id: id.to_string(),
            req,
        };

        RpcPacket::async_write(&mut self.stream, &msg)
            .await
            .map_err(|e| other_err!("problem writing to stream on clientside: {e:#?}"))?;

        self.stream
            .flush()
            .await
            .map_err(|e| other_err!("failed to flush stream: {e:#?}"))
    }
}
