use serde::{Deserialize, Serialize};
use strum::Display;

use super::app::Mode;

#[derive(Debug, Clone, PartialEq, Eq, Display, Serialize, Deserialize)]
pub enum Action {
    Tick,
    Render,
    Resize(u16, u16),
    /// open/close flag; true = open
    HelpDialogue(bool),
    ChangeMode(Mode),
    Suspend,
    Resume,
    Quit,
    ClearScreen,
    Error(String),
    Help,
}
