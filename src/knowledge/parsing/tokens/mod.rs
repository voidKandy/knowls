pub mod vec;

use super::comments::ParsedComment;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Token {
    CommentStr,
    Comment(ParsedComment),
    Block(String),
    End,
}

impl Token {
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
