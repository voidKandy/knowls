use crate::handle::{buffer_operations::BufferOpChannelHandler, error::HandleResult};
use lsp_server::Message as LSPMessage;
use tracing::warn;
pub mod buffer_operations;
pub mod diagnostics;
pub mod error;
pub mod notifications;
pub mod requests;

pub fn handle_other(msg: LSPMessage) -> HandleResult<BufferOpChannelHandler> {
    warn!("unhandled message {:?}", msg);
    Ok(BufferOpChannelHandler::new())
}
pub type BufferOpChannelJoinHandle = tokio::task::JoinHandle<error::HandleResult<()>>;
