use std::{cmp::Ordering, sync::LazyLock};

use espx_lsp_server::{
    interact::{
        parsing::{
            cmp_pos_range,
            comments::ParsedComment,
            language_ext_from_uri,
            lexer::Lexer,
            tokens::{vec::TokenVec, Token},
        },
        Interact, InteractArg, InteractVar,
    },
    util::Diff,
};
use lsp_types::{Position, Range};
use tracing::warn;

use crate::{
    helpers::TEST_TRACING,
    test_docs::{test_doc_1, test_doc_1diff, test_doc_2},
};

#[test]
fn pos_in_range_works() {
    let pos = Position {
        line: 12,
        character: 4,
    };

    let range = Range {
        start: Position {
            line: 1,
            character: 0,
        },
        end: Position {
            line: 11,
            character: 0,
        },
    };

    assert_eq!(Ordering::Greater, cmp_pos_range(&range, &pos));

    let pos = Position {
        line: 1,
        character: 4,
    };

    let range = Range {
        start: Position {
            line: 3,
            character: 0,
        },
        end: Position {
            line: 11,
            character: 0,
        },
    };

    assert_eq!(Ordering::Less, cmp_pos_range(&range, &pos));

    let pos = Position {
        line: 1,
        character: 4,
    };

    let range = Range {
        start: Position {
            line: 1,
            character: 4,
        },
        end: Position {
            line: 11,
            character: 0,
        },
    };

    assert_eq!(Ordering::Equal, cmp_pos_range(&range, &pos));

    let pos = Position {
        line: 11,
        character: 4,
    };

    let range = Range {
        start: Position {
            line: 1,
            character: 4,
        },
        end: Position {
            line: 11,
            character: 4,
        },
    };

    assert_eq!(Ordering::Equal, cmp_pos_range(&range, &pos));
}

#[test]
fn lexing_rust_comments_works() {
    let input = r#"
pub mod lexer;
use std::sync::LazyLock;

use lsp_types::Range;

use super::{InteractError, InteractResult};

// @^ Comment
pub struct ParsedComment {
    content: String,
    range: Range,
}

/*
Multiline
comment
*/
pub struct MoreCode;

// +_
pub struct EvenMoreCode {
    i: u32,
    s: &str,
}
        "#
    .to_owned();
    let ext = "rs";

    let mut lexer = Lexer::new(&input, ext);
    warn!("created lexer: {lexer:?}");
    let tokens = lexer.lex_input();

    let expected = vec![
        Token::Block(String::from("pub mod lexer;\nuse std::sync::LazyLock;")),
        Token::Block(String::from("use lsp_types::Range;")),
        Token::Block(String::from("use super::{InteractError, InteractResult};")),
        Token::CommentStr,
        Token::Comment(ParsedComment::new(
            Some(Interact::new(
                InteractVar::AGENT_PROMPT,
                vec![
                    InteractArg::Char('^'),
                    InteractArg::String("Comment".to_string()),
                ],
            )),
            " @^ Comment",
            Range {
                start: lsp_types::Position {
                    line: 8,
                    character: 3,
                },
                end: lsp_types::Position {
                    line: 8,
                    character: 14,
                },
            },
        )),
        Token::Block(String::from(
            r#"pub struct ParsedComment {
    content: String,
    range: Range,
}"#,
        )),
        Token::CommentStr,
        Token::Comment(ParsedComment::new(
            None,
            "\nMultiline\ncomment\n",
            lsp_types::Range {
                start: lsp_types::Position {
                    line: 14,
                    character: 3,
                },
                end: lsp_types::Position {
                    line: 17,
                    character: 1,
                },
            },
        )),
        Token::CommentStr,
        Token::Block(String::from("pub struct MoreCode;")),
        Token::CommentStr,
        Token::Comment(ParsedComment::new(
            Some(Interact::new(
                InteractVar::AGENT_PUSH,
                vec![InteractArg::Char('_')],
            )),
            " +_",
            Range {
                start: lsp_types::Position {
                    line: 20,
                    character: 3,
                },
                end: lsp_types::Position {
                    line: 20,
                    character: 6,
                },
            },
        )),
        Token::Block(String::from(
            "pub struct EvenMoreCode {\n    i: u32,\n    s: &str,\n}",
        )),
        Token::End,
    ];

    let all = tokens.as_ref().clone().into_iter().zip(expected);

    for (token, exp) in all {
        assert_eq!(exp, token)
    }

    let first_parsed_comment: &ParsedComment = tokens.into_iter().next().unwrap().1;

    let expected_range = lsp_types::Range {
        start: lsp_types::Position {
            line: 8,
            character: 5,
        },
        end: lsp_types::Position {
            line: 8,
            character: 14,
        },
    };

    let expected_content = " Comment".to_string();

    // let (range, content) = first_parsed_comment.text_for_interact().unwrap();

    assert_eq!(
        expected_range,
        first_parsed_comment.range_without_comment(ext).unwrap()
    );
    assert_eq!(
        expected_content,
        first_parsed_comment.content_without_comment(ext).unwrap()
    );
}

#[test]
fn lcs_works() {
    LazyLock::force(&TEST_TRACING);
    let doc1 = test_doc_1();
    let doc1diff = test_doc_1diff();

    let ext = language_ext_from_uri(&doc1.0);
    let mut l = Lexer::new(&doc1.1, ext);
    let tokens1 = l.lex_input();

    let mut l = Lexer::new(&doc1diff.1, ext);
    let tokensdiff = l.lex_input();

    let lcs = Diff::get_lcs(&tokens1, &tokensdiff);
    let expected = vec![
        Token::Block(String::from("use std::io::{self, Read};")),
        Token::CommentStr,
        Token::End,
    ];

    assert_eq!(lcs, expected);
}

#[test]
fn diffing_works() {
    LazyLock::force(&TEST_TRACING);
    let doc1 = test_doc_1();
    let doc1diff = test_doc_1diff();

    let ext = language_ext_from_uri(&doc1.0);
    let mut l = Lexer::new(&doc1.1, ext);
    let tokens1 = l.lex_input();

    let mut l = Lexer::new(&doc1diff.1, ext);
    let tokensdiff = l.lex_input();

    let expected = vec![
        Diff::Change(
            2,
            Token::Comment(ParsedComment::new(
                Some(Interact::try_from_str("@_").unwrap()),
                " @_",
                Range {
                    start: Position {
                        line: 2,
                        character: 3,
                    },
                    end: Position {
                        line: 2,
                        character: 6,
                    },
                },
            )),
        ),
        Diff::Change(
            3,
            Token::Block(
                r#"fn main() {
    let mut raw = String::from("string");
    io::stdin()
        .read_to_string(&mut raw)
        .expect("failed to read io");
}"#
                .to_string(),
            ),
        ),
        Diff::Change(4, Token::Block("struct BePushed;".to_string())),
        Diff::Delete(5),
        Diff::Delete(6),
        Diff::Delete(7),
        Diff::Delete(8),
        Diff::Delete(9),
    ];

    let d = Diff::get_diffs(tokens1, tokensdiff);

    assert_eq!(expected, d);
}
