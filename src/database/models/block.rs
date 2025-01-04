use crate::interact::parsing::tokens::{vec::TokenVec, Token};
use lsp_types::Uri;
use serde::{Deserialize, Serialize};
use surrealdb::sql::{Id, Thing};

use super::{DBItem, EmbeddedDBItem};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DBBlock {
    id: Thing,
    pub uri: Uri,
    pub idx: usize,
    pub content: String,
    pub embedding: Option<Vec<f32>>,
}

impl DBItem for DBBlock {
    const DB_ID: &str = "block";
    fn thing(&self) -> &Thing {
        &self.id
    }
}

impl EmbeddedDBItem for DBBlock {
    fn embedding(&self) -> Option<&[crate::embeddings::EmbeddingFloat]> {
        self.embedding.as_ref().map(|v| &**v)
    }
    fn update_with_embedding(&mut self, embedding: Vec<crate::embeddings::EmbeddingFloat>) {
        self.embedding = Some(embedding);
    }
    fn content_to_embed(&self) -> &str {
        &self.content
    }
}

impl DBBlock {
    pub const LINES_PER_BLOCK: usize = 25;

    pub fn new(uri: Uri, idx: usize, content: String) -> Self {
        let id = Id::String(format!("{}{}", uri.path().to_string(), idx));
        let id = Thing::from((Self::DB_ID, id));
        Self {
            id,
            idx,
            uri,
            content,
            embedding: None,
        }
    }

    pub fn from_tokens(tokens: &TokenVec, uri: Uri) -> Vec<Self> {
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
            let start = chunks_taken * Self::LINES_PER_BLOCK;
            let content: String = lines
                .iter()
                .skip(start)
                .take(Self::LINES_PER_BLOCK)
                .map(|slice| *slice)
                .collect::<Vec<&str>>()
                .join("\n");

            if content.trim().is_empty() {
                break;
            }

            let block_params = Self::new(uri.clone(), chunks_taken, content);

            all.push(block_params);
            chunks_taken += 1;
        }
        all
    }
}

mod tests {
    use crate::{database::models::DBItem, interact::parsing::lexer::Lexer};
    use lsp_types::Uri;
    use std::str::FromStr;
    use surrealdb::sql::Thing;

    use super::DBBlock;

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
        let tokens = lexer.lex_input();

        let first_chunk_content = String::from(
            "fn main() {\n    let mut raw = String::new();\n    io::stdin()\n        .read_to_string(&mut raw)\n        .expect(\"failed to read io\");\n}struct ToBePushed;fn again() {\n    let mut raw = String::new();\n    io::stdin()\n        .read_to_string(&mut raw)\n        .expect(\"failed to read io\");\n}fn again_again() {\n    let mut raw = String::new();\n    io::stdin()\n        .read_to_string(&mut raw)\n        .expect(\"failed to read io\");\n}fn again_again_again() {\n    let mut raw = String::new();\n    io::stdin()\n        .read_to_string(&mut raw)\n        .expect(\"failed to read io\");\n}",
        );
        let second_chunk_content = String::from(
            r#"fn again_again_again() {
    let mut raw = String::new();
    io::stdin()
        .read_to_string(&mut raw)
        .expect("failed to read io");
}"#,
        );

        // let id1 = DBBlock::id(&test_doc_1_uri, 0);
        // let id2 = DBBlock::id(&test_doc_1_uri, 1);

        let doc_block1 = DBBlock::new(test_doc_1_uri.clone(), 0, first_chunk_content);
        let doc_block2 = DBBlock::new(test_doc_1_uri.clone(), 1, second_chunk_content);

        let expected = vec![doc_block1, doc_block2];

        let out = DBBlock::from_tokens(&tokens, test_doc_1_uri);

        for (i, val) in out.iter().enumerate() {
            if i == 0 {
                assert!(val.content.clone().lines().count() <= DBBlock::LINES_PER_BLOCK - 1);
            }
            if !expected.contains(&val) {
                panic!(
                    "expected does not contain values:\nEXPECTED: {expected:#?}\nVALUE: {val:#?}"
                )
            }
        }
    }
}
