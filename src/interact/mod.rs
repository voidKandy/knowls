pub mod agent;
pub mod database;
mod error;
pub mod execution;
pub(super) mod logic;
pub mod parsing;

pub use error::{InteractError, InteractResult};
pub use logic::{
    Interact, InteractArg, InteractLspMessage, InteractLspNotification, InteractLspRequest,
    InteractVar,
};
