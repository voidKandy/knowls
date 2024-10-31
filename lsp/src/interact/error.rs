use crate::error::error_chain_fmt;
use std::fmt::{Debug, Display, Formatter, Result as FmtResult};

pub type InteractResult<T> = Result<T, InteractError>;

#[derive(thiserror::Error)]
pub enum InteractError {
    #[error(transparent)]
    Undefined(#[from] anyhow::Error),
    RegistryFull,
    UnhandledLanguageExtension(String),
    NoCommentToken,
    NoInteractInComment,
    InvalidInteractId(u8),
    // InvalidScopeId(u8),
    // InvaliCommandId(u8),
    AllWhitespace,
    NoScopeCharacter,
}

impl Debug for InteractError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        error_chain_fmt(self, f)
    }
}

impl Display for InteractError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        let display = match self {
            Self::Undefined(err) => err.to_string(),
            Self::RegistryFull => "Registry Full".to_owned(),
            Self::UnhandledLanguageExtension(ext) => format!("Unhandled Languge Extension: {ext}"),
            Self::NoInteractInComment => "No Interact In Comment".to_string(),
            Self::InvalidInteractId(id) => format!("{id} is not a valid interact id"),
            // Self::InvaliCommandId(id) => format!("No Command with id: {id}"),
            // Self::InvalidScopeId(id) => format!("No Scope with id: {id}"),
            Self::AllWhitespace => "All Whitespace".to_owned(),
            Self::NoCommentToken => "No Comment Token".to_owned(),
            Self::NoScopeCharacter => "No Scope Character".to_owned(),
        };
        write!(f, "{}", display)
    }
}
