use serde::{Deserialize, Serialize};
use strum::Display;

use crate::database::models::Knowledge;

use super::{app::Mode, components::ComponentId};

#[derive(Debug, Clone, PartialEq, Eq, Display, Serialize, Deserialize)]
pub enum Action {
    Tick,
    Render,
    Resize(u16, u16),
    /// open/close flag; true = open
    HelpDialogue(bool),
    ChangeMode(Mode),
    /// opens editor with given buffer
    OpenEditor(String),
    ChangeBody(ComponentId),
    Suspend,
    Resume,
    Quit,
    ClearScreen,
    Error(String),
    Help,
}
