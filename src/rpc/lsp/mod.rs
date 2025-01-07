use crate::server::ServerStateWriteGuard;
use crate::{MainErr, MainResult};
use buffer_operations::BufferOpChannelHandler;
use buffer_operations::BufferOpChannelSender;
use lsp_server::Message as LSPMessage;
use lsp_types::{MessageType, ShowMessageParams};
use std::sync::Arc;
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
    Arc<Mutex<&'outer mut ServerStateWriteGuard<'inner>>>;

pub fn new_handler_shared_state<'outer, 'inner, 'state>(
    w: &'outer mut ServerStateWriteGuard<'inner>,
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
