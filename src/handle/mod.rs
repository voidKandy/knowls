use std::sync::Arc;

use crate::{
    handle::buffer_operations::BufferOpChannelHandler, state::LspState, MainErr, MainResult,
};
use buffer_operations::BufferOpChannelSender;
use lsp_server::Message as LSPMessage;
use lsp_types::{MessageType, ShowMessageParams};
use tokio::sync::{Mutex, RwLockWriteGuard};
use tracing::warn;
pub mod buffer_operations;
pub mod diagnostics;
pub mod notifications;
pub mod requests;

pub fn handle_other(msg: LSPMessage) -> MainResult<BufferOpChannelHandler> {
    warn!("unhandled message {:?}", msg);
    Ok(BufferOpChannelHandler::new())
}
pub type BufferOpChannelJoinHandle = tokio::task::JoinHandle<MainResult<()>>;

pub type HandlerSharedState<'outer, 'inner, 'state> =
    Arc<Mutex<&'outer mut RwLockWriteGuard<'inner, LspState<'state>>>>;

pub fn new_handler_shared_state<'outer, 'inner, 'state>(
    w: &'outer mut RwLockWriteGuard<'inner, LspState<'state>>,
) -> HandlerSharedState<'outer, 'inner, 'state> {
    Arc::new(Mutex::new(w))
}

pub async fn show_notification_err(
    err: &MainErr,
    task_sender: &mut BufferOpChannelSender,
) -> MainResult<()> {
    task_sender
        .send_operation(buffer_operations::BufferOperation::ShowMessage(
            ShowMessageParams {
                typ: MessageType::ERROR,
                message: format!("An error occured in notification handler: {err:?}"),
            },
        ))
        .await
        .map_err(|err| err.into())
}
pub async fn show_request_err(
    err: &MainErr,
    task_sender: &mut BufferOpChannelSender,
) -> MainResult<()> {
    task_sender
        .send_operation(buffer_operations::BufferOperation::ShowMessage(
            ShowMessageParams {
                typ: MessageType::ERROR,
                message: format!("An error occured in request handler: {err:?}"),
            },
        ))
        .await
        .map_err(|err| err.into())
}
