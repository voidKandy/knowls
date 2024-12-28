use crate::{error::error_chain_fmt, MainErr};
use std::fmt::{Debug, Display, Formatter, Result as FmtResult};

use super::AgentID;

pub type AgentsResult<T> = Result<T, AgentsError>;

#[derive(thiserror::Error)]
pub enum AgentsError {
    #[error(transparent)]
    Undefined(#[from] MainErr),
    IncorrectAgentIDVariant(AgentID),
    AgentNotPresent(AgentID),
    // DocAgentNotPresent(Uri),
    // CustomAgentNotPresent(char),
}

impl Debug for AgentsError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        error_chain_fmt(self, f)
    }
}

impl Display for AgentsError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        let display = match self {
            Self::Undefined(err) => err.to_string(),
            Self::IncorrectAgentIDVariant(id) => {
                format!("Incorrect Agent ID variant, got: {id:#?}")
            }
            Self::AgentNotPresent(id) => {
                format!("No agent present for document: {id:#?}")
            }
        };
        write!(f, "{}", display)
    }
}
