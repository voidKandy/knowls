use crate::{agents::error::AgentsError, database::error::DatabaseError};
use std::fmt::{Debug, Display, Formatter, Result as FmtResult};

#[allow(unused_must_use)]
pub fn error_chain_fmt(
    e: &impl std::error::Error,
    f: &mut std::fmt::Formatter<'_>,
) -> std::fmt::Result {
    writeln!(f, "{}\n", e);
    let mut current = e.source();
    while let Some(cause) = current {
        writeln!(f, "Caused by:\n\t{}", cause)?;
        current = cause.source();
    }
    Ok(())
}

pub type StateResult<T> = Result<T, StateError>;
#[derive(thiserror::Error)]
pub enum StateError {
    #[error(transparent)]
    Undefined(#[from] anyhow::Error),
    DatabaseNotPresent,
    RegistryNotPresent,
    AgentsNotPresent,
    Database(#[from] DatabaseError),
    Agents(#[from] AgentsError),
}

impl Debug for StateError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        error_chain_fmt(self, f)
    }
}

impl Display for StateError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        let display = match self {
            Self::Undefined(err) => err.to_string(),
            Self::DatabaseNotPresent => String::from("Database Not Present"),
            Self::RegistryNotPresent => String::from("Registry Not Present"),
            Self::AgentsNotPresent => String::from("Agents Not Present"),
            Self::Agents(err) => err.to_string(),
            Self::Database(err) => err.to_string(),
        };
        write!(f, "{}", display)
    }
}
