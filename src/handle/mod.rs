use std::sync::Arc;

use crate::{
    handle::{buffer_operations::BufferOpChannelHandler, error::HandleResult},
    state::LspState,
};
use lsp_server::Message as LSPMessage;
use tokio::sync::{Mutex, RwLockWriteGuard};
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

pub type HandlerSharedState<'outer, 'inner, 'state> =
    Arc<Mutex<&'outer mut RwLockWriteGuard<'inner, LspState<'state>>>>;

pub fn new_handler_shared_state<'outer, 'inner, 'state>(
    w: &'outer mut RwLockWriteGuard<'inner, LspState<'state>>,
) -> HandlerSharedState<'outer, 'inner, 'state> {
    Arc::new(Mutex::new(w))
}
