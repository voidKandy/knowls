use lsp_types::{MessageType, ShowMessageParams};

use super::buffer_operations::{
    BufferOpChannelError, BufferOpChannelSender, BufferOpError, BufferOperation,
};
use crate::{
    error::{error_chain_fmt, StateError},
    interact::InteractError,
};
use std::fmt::{Debug, Display, Formatter, Result as FmtResult};

pub type HandleResult<T> = Result<T, HandleError>;
#[derive(thiserror::Error)]
pub enum HandleError {
    #[error(transparent)]
    Undefined(#[from] anyhow::Error),
    Json(#[from] serde_json::error::Error),
    BufferOp(#[from] BufferOpError),
    EspxAgent(#[from] espionox::agents::error::AgentError),
    Stream(#[from] espionox::language_models::completions::streaming::StreamError),
    Interact(#[from] InteractError),
    State(#[from] StateError),
}

impl Debug for HandleError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        error_chain_fmt(self, f)
    }
}

impl Display for HandleError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        let display = match self {
            Self::Undefined(err) => err.to_string(),
            Self::BufferOp(err) => err.to_string(),
            Self::EspxAgent(err) => err.to_string(),
            Self::Stream(err) => err.to_string(),
            Self::Json(err) => err.to_string(),
            Self::State(err) => err.to_string(),
            Self::Interact(err) => err.to_string(),
        };
        write!(f, "{}", display)
    }
}

impl From<BufferOpChannelError> for HandleError {
    fn from(value: BufferOpChannelError) -> Self {
        Self::BufferOp(Into::<BufferOpError>::into(value))
    }
}

impl HandleError {
    pub async fn notification_err(
        self,
        task_sender: &mut BufferOpChannelSender,
    ) -> HandleResult<()> {
        task_sender
            .send_operation(BufferOperation::ShowMessage(ShowMessageParams {
                typ: MessageType::ERROR,
                message: format!("An error occured in notification handler: {self:?}"),
            }))
            .await
            .map_err(|err| err.into())
    }
    pub async fn request_err(self, task_sender: &mut BufferOpChannelSender) -> HandleResult<()> {
        task_sender
            .send_operation(BufferOperation::ShowMessage(ShowMessageParams {
                typ: MessageType::ERROR,
                message: format!("An error occured in request handler: {self:?}"),
            }))
            .await
            .map_err(|err| err.into())
    }
}
