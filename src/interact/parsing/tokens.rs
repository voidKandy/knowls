use super::comments::ParsedComment;
use lsp_types::Position;
use serde::{Deserialize, Serialize};
use std::{
    cmp::Ordering,
    fmt::{Debug, Display},
};
use tracing::warn;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Token<'i> {
    CommentStr,
    Comment(ParsedComment<'i>),
    Block(String),
    End,
}

#[derive(Debug, Clone)]
pub struct TokenVec<'i> {
    vec: Vec<Token<'i>>,
    comment_indices: Vec<usize>,
}
impl<'i> AsRef<Vec<Token<'i>>> for TokenVec<'i> {
    fn as_ref(&self) -> &Vec<Token<'i>> {
        &self.vec
    }
}

impl<'i> Display for Token<'i> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            Self::End => "End",
            Self::CommentStr => "CommentStr",
            Self::Block(_) => "Block(String)",
            Self::Comment(_) => "Comment(ParsedComment)",
        };
        write!(f, "{str}")
    }
}

impl<'i> ToString for TokenVec<'i> {
    fn to_string(&self) -> String {
        let mut buffer = String::new();
        for tok in self.vec.iter() {
            if let Token::Block(str) = tok {
                buffer.push_str(&str);
            }
        }
        buffer
    }
}

impl<'i> IntoIterator for TokenVec<'i> {
    type Item = ParsedComment<'i>;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        let mut comments = vec![];
        for idx in self.comment_indices {
            if let Some(Token::Comment(c)) = self.vec.iter().nth(idx) {
                comments.push(c.clone())
            }
        }

        comments.into_iter()
    }
}

impl<'i> TokenVec<'i> {
    pub fn new(vec: Vec<Token<'i>>, comment_indices: Vec<usize>) -> Self {
        for idx in comment_indices.iter() {
            match vec.iter().nth(*idx) {
                Some(Token::Comment(_)) => {}
                o => panic!("encountered {o:?} where Comment should be"),
            }
        }

        Self {
            vec,
            comment_indices,
        }
    }

    pub fn comment_indices(&self) -> &Vec<usize> {
        &self.comment_indices
    }

    #[tracing::instrument(name = "getting comment in position")]
    pub fn comment_in_position(&self, pos: &Position) -> Option<(&ParsedComment, usize)> {
        warn!("this document has {} comments", self.comment_indices.len());
        for idx in self.comment_indices.iter() {
            let mut iter = self.vec.iter();
            if let Some(token) = iter.nth(*idx) {
                warn!("got token: {token:#?} at idx: {idx}");
                if let Token::Comment(c) = token {
                    warn!("got comment: {c:#?}");
                    if super::cmp_pos_range(&c.range, pos) == Ordering::Equal {
                        return Some((&c, *idx));
                    }
                    warn!("Position: {pos:#?} not in comment range");
                }
            }
        }
        None
    }

    pub fn get(&self, idx: usize) -> Option<&Token> {
        self.vec.iter().nth(idx)
    }
}
