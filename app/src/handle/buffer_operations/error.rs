use crate::error::error_chain_fmt;
use anyhow::anyhow;
use crossbeam_channel::SendError;
use lsp_server::Message;
use std::fmt::{Debug, Display, Formatter, Result as FmtResult};

pub type BufferOpResult<T> = Result<T, BufferOpError>;
#[derive(thiserror::Error)]
pub enum BufferOpError {
    #[error(transparent)]
    Undefined(#[from] anyhow::Error),
    Io(#[from] std::io::Error),
    Channel(#[from] BufferOpChannelError),
    Json(#[from] serde_json::Error),
}

impl Debug for BufferOpError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        error_chain_fmt(self, f)
    }
}

impl Display for BufferOpError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        let display = match self {
            Self::Undefined(err) => err.to_string(),
            Self::Json(err) => err.to_string(),
            Self::Channel(err) => err.to_string(),
            Self::Io(err) => err.to_string(),
        };
        write!(f, "{}", display)
    }
}

pub type BufferOpChannelResult<T> = Result<T, BufferOpChannelError>;
#[derive(thiserror::Error)]
pub enum BufferOpChannelError {
    #[error(transparent)]
    Undefined(#[from] anyhow::Error),
    TokioSend(anyhow::Error),
    Timeout,
    CrossBeamSend(#[from] SendError<Message>),
    Json(#[from] serde_json::Error),
}

impl<E> From<tokio::sync::mpsc::error::SendError<E>> for BufferOpChannelError {
    fn from(value: tokio::sync::mpsc::error::SendError<E>) -> Self {
        Self::TokioSend(anyhow!("Send Error: {:?}", value))
    }
}

impl Debug for BufferOpChannelError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        error_chain_fmt(self, f)
    }
}

impl Display for BufferOpChannelError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        let display = match self {
            Self::Undefined(err) => err.to_string(),
            Self::TokioSend(err) => err.to_string(),
            Self::Timeout => "Timeout".to_string(),
            Self::Json(err) => err.to_string(),
            Self::CrossBeamSend(err) => err.to_string(),
        };
        write!(f, "{}", display)
    }
}
