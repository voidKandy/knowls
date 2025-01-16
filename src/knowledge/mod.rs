pub mod parsing;
use std::collections::HashMap;

use lsp_types::{Range, Uri};
use parsing::tokens::vec::TokenVec;
use surrealdb::sql::Id;

use crate::interact::{Interact, InteractWrapper};

pub fn uri_to_surreal_id(uri: &Uri) -> Id {
    Id::String(uri.to_string())
}

// i think knowledge could be generalized more to add different kinds of knowledge bases
// also, i think it's time to use treesitter
#[derive(Debug, Clone)]
pub enum Knowledge {
    Document {
        tokens: TokenVec,
        // down the line, it would be better if this was a map that would return an interact in 0(1) given a position
        interacts: HashMap<Range, InteractWrapper>,
    },
}
