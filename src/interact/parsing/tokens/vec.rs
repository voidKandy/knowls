use crate::util::Diff;

use super::{super::comments::ParsedComment, Token};
use lsp_types::Position;
use std::{cmp::Ordering, fmt::Debug};
use tracing::warn;

#[derive(Debug, Clone)]
pub struct TokenVec<'i> {
    pub(super) vec: Vec<Token<'i>>,
    comment_indices: Vec<usize>,
}

impl<'i> AsRef<Vec<Token<'i>>> for TokenVec<'i> {
    fn as_ref(&self) -> &Vec<Token<'i>> {
        &self.vec
    }
}

/// Converts TokenVec to string, but only including Block tokens
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

/// Helper iterator through all ParsedComment vectors
impl<'o, 'i> IntoIterator for &'o TokenVec<'i> {
    type Item = (usize, &'o ParsedComment<'i>);
    type IntoIter = std::vec::IntoIter<Self::Item>;
    fn into_iter(self) -> Self::IntoIter {
        let mut comments = vec![];
        for idx in self.comment_indices.iter() {
            if let Some(Token::Comment(c)) = self.vec.iter().nth(*idx) {
                comments.push((*idx, c))
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
                    if super::super::cmp_pos_range(&c.range, pos) == Ordering::Equal {
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
