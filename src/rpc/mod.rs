pub mod lsp;
pub mod messages;
pub use messages::*;
use std::{fmt::Debug, path::Path, time::Duration};
use tokio::io::AsyncReadExt;
use tracing::warn;

pub struct TcpPacket;
impl TcpPacket {
    const HEADER_SIZE: usize = std::mem::size_of::<u32>() / std::mem::size_of::<u8>();

    pub fn serialize(mes: impl Into<seraphic::socket::Message>) -> Vec<u8> {
        let payload = Into::<seraphic::socket::Message>::into(mes);
        let vec = serde_json::to_vec(&payload).unwrap();
        let size: u32 = vec.len() as u32;

        tracing::warn!("serialized payload of size: {size}");

        let mut buff = Vec::with_capacity(Self::HEADER_SIZE + vec.len());
        buff.extend_from_slice(&size.to_le_bytes());
        buff.extend_from_slice(&vec);
        buff
    }

    pub async fn read_from_stream<R>(reader: &mut R) -> tokio::io::Result<seraphic::socket::Message>
    where
        R: AsyncReadExt + Unpin,
    {
        let mut header_buf = [0u8; Self::HEADER_SIZE];

        tokio::time::timeout(
            Duration::from_millis(250),
            reader.read_exact(&mut header_buf),
        )
        .await
        .expect("timeout when reading")
        .expect("failed to read header");

        let payload_size = u32::from_le_bytes(header_buf) as usize;
        tracing::warn!("expecting payload of size: {payload_size}");

        let mut payload_buf = vec![0; payload_size];

        tokio::time::timeout(
            Duration::from_millis(250),
            reader.read_exact(&mut payload_buf),
        )
        .await
        .expect("timeout when reading")
        .expect("failed to read payload");

        let message: seraphic::socket::Message = serde_json::from_slice(&payload_buf)
            .map_err(|e| tokio::io::Error::new(tokio::io::ErrorKind::InvalidData, e))?;
        tracing::warn!("got message: {message:#?}");

        Ok(message)
    }
}
