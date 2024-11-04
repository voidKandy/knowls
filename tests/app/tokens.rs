use std::cmp::Ordering;

use espx_lsp_server::interact::{
    parsing::{cmp_pos_range, comments::ParsedComment, lexer::Lexer, tokens::Token},
    Interact, InteractArg, InteractVar,
};
use lsp_types::{Position, Range};
use tracing::warn;

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

// @_ Comment
pub struct ParsedComment {
    content: String,
    range: Range,
}

/*
Multiline
comment
*/
pub struct MoreCode;
        "#
    .to_owned();
    let ext = "rs";

    let mut lexer = Lexer::new(&input, ext);
    warn!("created lexer: {lexer:?}");
    let tokens = lexer.lex_input();

    let expected = vec![
        Token::Block(String::from(
            "\npub mod lexer;\nuse std::sync::LazyLock;\n\n",
        )),
        Token::Block(String::from("use lsp_types::Range;\n\n")),
        Token::Block(String::from(
            "use super::{InteractError, InteractResult};\n\n",
        )),
        Token::CommentStr,
        Token::Comment(ParsedComment::new(
            Some(Interact::new(
                InteractVar::AGENT_PROMPT,
                vec![
                    InteractArg::Char('_'),
                    InteractArg::String("Comment".to_string()),
                ],
            )),
            " @_ Comment",
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
}

"#,
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
        Token::Block(String::from(
            r#"
pub struct MoreCode;
        "#,
        )),
        Token::End,
    ];

    let all = tokens.as_ref().clone().into_iter().zip(expected);

    for (token, exp) in all {
        assert_eq!(token, exp)
    }

    let first_parsed_comment: ParsedComment = tokens.into_iter().next().unwrap();

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
