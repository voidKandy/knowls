use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use strum::Display;

use crate::database::models::Knowledge;

use super::{app::Mode, components::ComponentId};

#[derive(Debug, Clone, PartialEq, Eq, Display, Serialize, Deserialize)]
pub enum Action {
    // This is specific to knowledge
    // No other module has a specific action therefore this is marked as smelly
    InsertKnowledge(Knowledge),

    Tick,
    Render,
    Resize(u16, u16),
    ChangeMode(Mode),
    /// opens editor with given buffer
    OpenEditor(String),
    Suspend,
    Resume,
    Quit,
    ClearScreen,
    Error(String),
    Help,
}
