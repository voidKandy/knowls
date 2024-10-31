use super::{DatabaseStruct, IntoOneOf};
use crate::{
    database::error::DatabaseError,
    interact::lexer::{Token, TokenVec},
    util::OneOf,
};
use anyhow::anyhow;
use base64::{prelude::BASE64_URL_SAFE_NO_PAD, Engine};
use lsp_types::Uri;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use surrealdb::sql::Thing;
use tracing::warn;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DBBlock {
    pub id: Thing,
    pub uri: Uri,
    pub content: String,
    pub content_embedding: Option<Vec<f32>>,
}

const LINES_PER_BLOCK: usize = 25;

#[derive(Debug, Clone, PartialEq, Serialize)]
struct DBBlockID(String);

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DBBlockParams {
    id: DBBlockID,
    pub uri: Option<Uri>,
    pub content: Option<String>,
    content_embedding: Option<Vec<f32>>,
}

impl TryFrom<Thing> for DBBlockID {
    type Error = anyhow::Error;
    fn try_from(value: Thing) -> Result<Self, Self::Error> {
        match value.id {
            surrealdb::sql::Id::String(string) => Ok(Self(string)),
            _ => Err(anyhow!("{:#?} cannot be turned into a DBBlockID", value.id)),
        }
    }
}

impl From<(&Uri, usize)> for DBBlockID {
    fn from((uri, doc_idx): (&Uri, usize)) -> Self {
        let encoded = BASE64_URL_SAFE_NO_PAD.encode(uri.as_str());
        let id = format!("{doc_idx}{encoded}");
        warn!("created id: {id}");
        DBBlockID(id)
    }
}

impl TryInto<(Uri, usize)> for DBBlockID {
    type Error = anyhow::Error;
    fn try_into(self) -> Result<(Uri, usize), Self::Error> {
        let doc_idx: usize = self.0[..=0].parse()?;
        let rest = &self.0[1..];
        let uri_str = BASE64_URL_SAFE_NO_PAD
            .decode(rest)?
            .into_iter()
            .map(|int| int as char)
            .collect::<String>();
        let uri = Uri::from_str(&uri_str)?;
        Ok((uri, doc_idx))
    }
}

impl DBBlockParams {
    pub fn new(uri: Uri, idx_in_doc: usize, content: Option<String>) -> Self {
        let id = DBBlockID::from((&uri, idx_in_doc));
        Self {
            id,
            uri: Some(uri),
            content,
            content_embedding: None,
        }
    }
}

pub fn block_params_from(tokens: &TokenVec, uri: Uri) -> Vec<DBBlockParams> {
    let mut all = vec![];
    let mut whole_doc_buffer = String::new();
    for token in tokens.as_ref() {
        if let Token::Block(block) = token {
            whole_doc_buffer.push_str(&block);
        }
    }

    let lines = whole_doc_buffer.lines().collect::<Vec<&str>>();
    let mut chunks_taken = 0;

    loop {
        let start = chunks_taken * LINES_PER_BLOCK;
        let content: String = lines
            .iter()
            .skip(start)
            .take(LINES_PER_BLOCK)
            .map(|slice| *slice)
            .collect::<Vec<&str>>()
            .join("\n");

        if content.trim().is_empty() {
            break;
        }

        let block_params = DBBlockParams::new(uri.clone(), chunks_taken, Some(content));

        all.push(block_params);
        chunks_taken += 1;
    }
    all
}

impl<'l> IntoOneOf<'l, DBBlock, DBBlockParams> for DBBlock {
    fn one_of(me: &'l Self) -> OneOf<&'l DBBlock, &'l DBBlockParams> {
        OneOf::Left(me)
    }
}

impl<'l> IntoOneOf<'l, DBBlock, DBBlockParams> for DBBlockParams {
    fn one_of(me: &'l Self) -> OneOf<&'l DBBlock, &'l DBBlockParams> {
        OneOf::Right(me)
    }
}

impl<'l> DatabaseStruct<'l, DBBlockParams> for DBBlock {
    fn db_id() -> &'static str {
        "blocks"
    }

    fn thing(&self) -> &Thing {
        &self.id
    }
    fn content(
        oneof: &'l impl IntoOneOf<'l, Self, DBBlockParams>,
    ) -> crate::database::error::DatabaseResult<String> {
        match IntoOneOf::<Self, DBBlockParams>::one_of(oneof) {
            OneOf::Left(me) => {
                let content_embedding_string = {
                    if let Some(emb) = &me.content_embedding {
                        format!("content_embedding: {},", serde_json::to_value(emb)?)
                    } else {
                        String::new()
                    }
                };

                Ok(format!(
                    r#"CONTENT {{
                uri: {},
                content: {},
                {}
                }}"#,
                    serde_json::to_value(&me.uri)?,
                    serde_json::to_value(&me.content)?,
                    content_embedding_string
                ))
            }
            OneOf::Right(params) => {
                let uri_string = {
                    if let Some(uri) = &params.uri {
                        format!(",uri: {}", serde_json::to_value(uri.as_str())?)
                    } else {
                        String::new()
                    }
                };

                let content_string = {
                    if let Some(content) = &params.content {
                        format!(",content: {}", serde_json::to_value(content)?)
                    } else {
                        String::new()
                    }
                };

                let content_embedding_string = {
                    if let Some(emb) = &params.content_embedding {
                        format!(",content_embedding: {}", serde_json::to_value(emb)?)
                    } else {
                        String::new()
                    }
                };

                Ok(format!(
                    r#"CONTENT {{ {} }}"#,
                    [uri_string, content_string, content_embedding_string]
                        .join(" ")
                        .trim()
                        .trim_start_matches(','),
                ))
            }
        }
    }

    fn upsert(params: &DBBlockParams) -> crate::database::error::DatabaseResult<String> {
        if params.uri.is_none() || params.content.is_none() {
            return Err(DatabaseError::DbStruct(format!(
                "All fields need to be Some for a create statement, got: {:?} {:?} ",
                params.uri, params.content,
            )));
        }

        Ok(format!(
            "UPSERT {}:{} {};",
            Self::db_id(),
            params.id.0,
            Self::content(params)?
        ))
    }
}

mod tests {
    use super::{block_params_from, DBBlockID, DBBlockParams};
    use crate::{
        database::models::block::LINES_PER_BLOCK,
        interact::{lexer::Lexer, registry::InteractRegistry},
    };
    use lsp_types::Uri;
    use std::str::FromStr;

    #[test]
    fn block_id_encoding_decoding() {
        let test_doc_1_uri = Uri::from_str("test_doc_1.rs").unwrap();
        let id = DBBlockID::from((&test_doc_1_uri, 0));
        let (uri, idx): (Uri, usize) = id.try_into().unwrap();

        assert_eq!(uri, test_doc_1_uri);
        assert_eq!(0, idx);
    }

    #[test]
    fn correctly_parses_block() {
        let test_doc_1_uri = Uri::from_str("test_doc_1.rs").unwrap();
        let input = r#" 
// Comment without any command

// @_hey
fn main() {
    let mut raw = String::new();
    io::stdin()
        .read_to_string(&mut raw)
        .expect("failed to read io");
}

// +_
struct ToBePushed;

fn again() {
    let mut raw = String::new();
    io::stdin()
        .read_to_string(&mut raw)
        .expect("failed to read io");
}

fn again_again() {
    let mut raw = String::new();
    io::stdin()
        .read_to_string(&mut raw)
        .expect("failed to read io");
}

fn again_again_again() {
    let mut raw = String::new();
    io::stdin()
        .read_to_string(&mut raw)
        .expect("failed to read io");
}"#
        .to_string();

        let mut lexer = Lexer::new(&input, "rs");
        let tokens = lexer.lex_input(&InteractRegistry::default());

        let first_chunk_content = Some(String::from(
            r#" 

fn main() {
    let mut raw = String::new();
    io::stdin()
        .read_to_string(&mut raw)
        .expect("failed to read io");
}

struct ToBePushed;

fn again() {
    let mut raw = String::new();
    io::stdin()
        .read_to_string(&mut raw)
        .expect("failed to read io");
}

fn again_again() {
    let mut raw = String::new();
    io::stdin()
        .read_to_string(&mut raw)
        .expect("failed to read io");
}
"#,
        ));
        let second_chunk_content = Some(String::from(
            r#"fn again_again_again() {
    let mut raw = String::new();
    io::stdin()
        .read_to_string(&mut raw)
        .expect("failed to read io");
}"#,
        ));
        let expected = vec![
            DBBlockParams::new(test_doc_1_uri.clone(), 0, first_chunk_content),
            DBBlockParams::new(test_doc_1_uri.clone(), 1, second_chunk_content),
        ];

        let out = block_params_from(&tokens, test_doc_1_uri);

        for (i, val) in out.iter().enumerate() {
            if i == 0 {
                assert_eq!(
                    val.content.clone().unwrap().lines().count(),
                    LINES_PER_BLOCK - 1
                );
            }
            if !expected.contains(&val) {
                panic!(
                    "expected does not contain values:\nEXPECTED: {expected:#?}\nVALUE: {val:#?}"
                )
            }
        }
    }
}
