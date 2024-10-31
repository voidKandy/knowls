use super::{registry::InteractRegistry, InteractError, InteractResult};
use crate::interact::comment_str_map::{get_comment_string_info, CommentStrInfo};
use lsp_types::{Position, Range};
use serde::{Deserialize, Serialize};
use std::{
    cmp::Ordering,
    fmt::{Debug, Display},
};
use tracing::warn;

#[derive(Debug)]
pub struct Lexer<'i> {
    input: &'i str,
    comment_str_info: CommentStrInfo<'i>,
    buffer: String,
    position: usize,      // current position in input (points to current char)
    read_position: usize, // current reading position in input (after current char)
    ch: Option<char>,     // NONE if at end of input
    current_line: usize,
    current_char: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedComment {
    interact: Option<u8>,
    pub content: String,
    pub range: Range,
}

#[derive(Debug, Clone)]
pub struct TokenVec {
    vec: Vec<Token>,
    comment_indices: Vec<usize>,
}

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
                    if cmp_pos_range(&c.range, pos) == Ordering::Equal {
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

impl AsRef<Vec<Token>> for TokenVec {
    fn as_ref(&self) -> &Vec<Token> {
        &self.vec
    }
}

impl IntoIterator for TokenVec {
    type Item = ParsedComment;
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

/// Returns Ordering::Equal if the position is within the range, otherwise denotes which direction
/// it is out of range
pub fn cmp_pos_range(range: &Range, pos: &Position) -> Ordering {
    if pos.line < range.start.line
        || pos.character < range.start.character && pos.line == range.start.line
    {
        return Ordering::Less;
    }

    if pos.line > range.end.line
        || pos.character > range.end.character && pos.line == range.end.line
    {
        return Ordering::Greater;
    }

    Ordering::Equal
}

impl ParsedComment {
    pub fn new(interact: Option<u8>, content: &str, range: Range) -> Self {
        Self {
            interact,
            content: content.to_string(),
            range,
        }
    }
    /// returns range and text of comment without interract
    /// returns none if there is no interact code
    pub fn text_for_interact(&self) -> Option<(Range, String)> {
        self.interact.and_then(|_| {
            // for now all interact codes have only 2 chars, no more no less.
            // This will likely change in the future
            let chars_amt = 2;

            let whitespace_amt = self
                .content
                .chars()
                .position(|c| !c.is_whitespace())
                .unwrap();

            let mut range = self.range.clone();

            let skip_amt = chars_amt + whitespace_amt - 1;
            range.start.character += skip_amt as u32;
            let ret_str = self.content.chars().skip(skip_amt + 1).collect();

            Some((range, ret_str))
        })
    }

    pub fn try_get_interact_integer(&self) -> InteractResult<u8> {
        self.interact.ok_or(InteractError::NoInteractInComment)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Token {
    CommentStr,
    Comment(ParsedComment),
    Block(String),
    End,
}

impl Display for Token {
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

impl<'i> Lexer<'i> {
    pub fn new(input: &'i String, ext: &'i str) -> Self {
        let comment_str_info = get_comment_string_info(ext).expect("no comment string");

        Self {
            input,
            buffer: String::new(),
            position: 0,
            read_position: 1,
            ch: input.to_lowercase().chars().nth(0),
            comment_str_info,
            current_line: 0,
            current_char: 0,
        }
    }

    /// Checks that the current char is indeed the beginning of a slice that will result in the
    /// given &str
    fn at_beginning_of_slice(&self, slice: &str) -> bool {
        let mut chars = slice.chars();
        let mut i = 0;
        let first = chars
            .next()
            .expect("should not pass an empty string to this function");

        if self.ch != Some(first) {
            return false;
        }

        if slice.len() == 1 {
            return true;
        }

        while let Some(next_expected_char) = chars.next() {
            if let Some(next_peek) = self.peek_offset(i) {
                if next_peek != next_expected_char {
                    return false;
                }
            }
            i += 1;
        }
        if let Some(n) = self.peek_offset(i) {
            return n.is_whitespace();
        }
        true
    }

    fn comment_str(&self, multiline_start: Option<bool>) -> Option<&str> {
        match multiline_start {
            None => Some(self.comment_str_info.singleline()),
            Some(is_start) => {
                if is_start {
                    self.comment_str_info.multiline_start()
                } else {
                    self.comment_str_info.multiline_end()
                }
            }
        }
    }

    fn current_position(&self) -> Position {
        Position {
            line: self.current_line as u32,
            character: self.current_char as u32,
        }
    }

    fn progress_char(&mut self) {
        if self.ch == Some('\n') {
            self.current_line += 1;
            self.current_char = 0;
        }
        self.ch = self.input.chars().nth(self.read_position);
        self.position = self.read_position;
        self.current_char += 1;
        self.read_position += 1
    }

    fn peek(&self) -> Option<char> {
        self.input.chars().nth(self.read_position)
    }

    fn peek_offset(&self, offset: usize) -> Option<char> {
        self.input.chars().nth(self.read_position + offset)
    }

    #[tracing::instrument(name = "lex input into TokenVec")]
    pub fn lex_input(&mut self, registry: &InteractRegistry) -> TokenVec {
        let mut vec = vec![];
        let mut comment_indices = vec![];
        let mut start_opt = Option::<Position>::None;
        let mut end_opt = Option::<Position>::None;
        // let mut buffer = String::new();

        let singleline_comment_str = self.comment_str(None).unwrap().to_owned();
        let singleline_comment_first_char =
            singleline_comment_str.chars().nth(0).unwrap().to_owned();

        let multiline_comment_start_str = self
            .comment_str(Some(true))
            .and_then(|s| Some(s.to_owned()));
        let multiline_comment_start_first_char = multiline_comment_start_str
            .as_ref()
            .and_then(|str| str.chars().nth(0).and_then(|i| Some(i.to_owned())));

        while let Some(c) = self.ch {
            match c {
                _ if c == singleline_comment_first_char
                    || Some(c) == multiline_comment_start_first_char =>
                {
                    let singleline_start = self
                        .at_beginning_of_slice(&singleline_comment_str)
                        .to_owned();

                    let multiline_start = multiline_comment_start_str
                        .as_ref()
                        .and_then(|slice| Some(self.at_beginning_of_slice(&slice)))
                        .unwrap_or(false);

                    if !singleline_start && !multiline_start {
                        self.buffer.push(c);
                        self.progress_char();
                        continue;
                    }

                    let (start_slice, end_slice) = {
                        if singleline_start {
                            (&singleline_comment_str, "\n".to_owned())
                        } else {
                            (
                                multiline_comment_start_str
                                    .as_ref()
                                    .expect("should be some in this branch"),
                                self.comment_str(Some(false))
                                    .expect("this should be some in this branch")
                                    .to_owned(),
                            )
                        }
                    };

                    let end_slice_first_char = end_slice
                        .chars()
                        .nth(0)
                        .expect("end slice should not be an empty string");

                    if !self.buffer.is_empty() {
                        vec.push(Token::Block(self.buffer.drain(..).collect::<String>()));
                    }

                    for _ in 0..start_slice.len() {
                        self.progress_char();
                    }

                    start_opt = Some(self.current_position());
                    vec.push(Token::CommentStr);

                    while let Some(peek) = self.peek() {
                        self.buffer.push(self.ch.unwrap());
                        self.progress_char();
                        if peek == end_slice_first_char {
                            if self.at_beginning_of_slice(&end_slice) {
                                end_opt = Some(self.current_position());
                                for _ in 0..end_slice.len() - 1 {
                                    self.progress_char();
                                }
                                break;
                            }
                        }
                    }

                    let range = Range {
                        start: start_opt.expect("start opt should be some at this point"),
                        end: end_opt.expect("end opt should be some at this point"),
                    };

                    let content = self.buffer.drain(..).collect::<String>();

                    let interact = registry.try_get_interact(&content);

                    comment_indices.push(vec.len());
                    vec.push(Token::Comment(ParsedComment {
                        interact,
                        content,
                        range,
                    }));

                    if multiline_start {
                        vec.push(Token::CommentStr);
                    };
                }

                '\n' => {
                    self.buffer.push(c);
                    if self.peek() == Some('\n') {
                        while self.peek() == Some('\n') {
                            self.progress_char();
                            self.buffer.push(self.ch.expect("this should be some"));
                        }
                        vec.push(Token::Block(self.buffer.drain(..).collect::<String>()))
                    }
                }

                _ => {
                    self.buffer.push(c);
                }
            }
            self.progress_char();
        }

        if !self.buffer.is_empty() {
            vec.push(Token::Block(self.buffer.drain(..).collect()));
        }

        vec.push(Token::End);

        let token_vec = TokenVec::new(vec, comment_indices);
        warn!("returning token vec: {token_vec:#?}");
        token_vec
    }
}
mod tests {
    use crate::interact::lexer::Lexer;

    #[test]
    fn at_begginning_of_slice_works() {
        let input = "pub mod lexer;".to_owned();
        let mut lexer = Lexer::new(&input, "rs");

        lexer.progress_char();
        lexer.progress_char();
        lexer.progress_char();
        lexer.progress_char();

        assert!(lexer.at_beginning_of_slice("mod"));

        let input = "\n".to_owned();
        let lexer = Lexer::new(&input, "rs");
        assert!(lexer.at_beginning_of_slice("\n"));
    }
}
