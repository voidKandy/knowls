pub mod parsing;
use parsing::tokens::vec::TokenVec;

// i think knowledge could be generalized more to add different kinds of knowledge bases
// also, i think it's time to use treesitter
#[derive(Debug)]
pub enum Knowledge<'k> {
    Document(TokenVec<'k>),
}
