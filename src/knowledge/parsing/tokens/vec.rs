use super::{super::comments::ParsedComment, Token};
use lsp_types::Position;
use std::{cmp::Ordering, fmt::Debug};
use tracing::warn;

#[derive(Debug, Clone)]
pub struct TokenVec {
    pub(super) vec: Vec<Token>,
    comment_indices: Vec<usize>,
}

impl AsRef<Vec<Token>> for TokenVec {
    fn as_ref(&self) -> &Vec<Token> {
        &self.vec
    }
}

/// Converts TokenVec to string, but only including Block tokens
impl ToString for TokenVec {
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

/// Helper iterator through all ParsedComment tokens
impl<'o> IntoIterator for &'o TokenVec {
    type Item = (usize, &'o Token);
    type IntoIter = std::vec::IntoIter<Self::Item>;
    fn into_iter(self) -> Self::IntoIter {
        let mut comments = vec![];
        for idx in self.comment_indices.iter() {
            if let Some(tok) = self.vec.iter().nth(*idx) {
                if let Token::Comment(_) = tok {
                    comments.push((*idx, tok))
                }
            }
        }

        comments.into_iter()
    }
}

impl TokenVec {
    pub fn new(vec: Vec<Token>, comment_indices: Vec<usize>) -> Self {
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

    // pub fn update_from_diff(&mut self, diff: Vec<Diff<Token<'i>>>) {
    //     for d in diff {
    //         match d {
    //             Diff::Delete(idx) => {
    //                 self.vec.remove(idx);
    //             }
    //             Diff::Change(idx, tok) => {
    //                 self.vec[idx] = tok;
    //             }
    //             Diff::Insert(idx, tok) => {
    //                 let before = &self.vec[..idx];
    //                 let after = &self.vec[idx..];
    //                 let vec = [before, &vec![tok], after].concat();
    //                 let comment_indices = vec.iter().enumerate().fold(vec![], |mut acc, (i, t)| {
    //                     if let Token::Comment(_) = t {
    //                         acc.push(i);
    //                     }
    //                     acc
    //                 });
    //                 *self = Self::new(vec, comment_indices);
    //             }
    //         }
    //     }
    // }
}
