pub mod vec;
use super::comments::ParsedComment;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Token<'i> {
    CommentStr,
    Comment(ParsedComment<'i>),
    Block(String),
    End,
}

impl<'i> Token<'i> {
    /// Displays only enough info to know which type of token the token is
    pub fn variant_display(&self) -> &str {
        match self {
            Self::End => "End",
            Self::CommentStr => "CommentStr",
            Self::Block(_) => "Block(String)",
            Self::Comment(_) => "Comment(ParsedComment)",
        }
    }
}
