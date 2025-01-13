pub mod parsing;
use lsp_types::Uri;
use parsing::tokens::vec::TokenVec;
use surrealdb::sql::Id;

pub fn uri_to_surreal_id(uri: &Uri) -> Id {
    Id::String(uri.to_string())
}

// i think knowledge could be generalized more to add different kinds of knowledge bases
// also, i think it's time to use treesitter
#[derive(Debug, Clone)]
pub enum Knowledge {
    Document(TokenVec),
}
