pub mod agent;
mod error;
pub mod execution;
pub(super) mod logic;
pub mod parsing;
pub mod state;

pub use error::{InteractError, InteractResult};
pub use logic::{
    Interact, InteractArg, InteractLspMessage, InteractLspNotification, InteractLspRequest,
    InteractVar,
};
