use std::collections::HashMap;

use color_eyre::Result;
use ratatui::{prelude::*, widgets::*};
use tokio::sync::mpsc::UnboundedSender;

use crate::state::State;
use crate::tui::config::parse_key_event;

use super::super::{action::Action, config::Config};
use super::{Component, PageComponent};

#[derive(Default)]
pub struct Home {
    command_tx: Option<UnboundedSender<Action>>,
    config: Config,
}

impl Home {
    pub fn new() -> Self {
        Self::default()
    }
}

impl PageComponent for Home {
    fn id(&self) -> super::ComponentId {
        "home".into()
    }
    fn selection_keys(&self) -> Vec<crossterm::event::KeyEvent> {
        vec![parse_key_event("h").unwrap()]
    }
    fn bindings(&self) -> std::collections::HashMap<Vec<crossterm::event::KeyEvent>, Action> {
        let map = HashMap::new();
        map
    }
    // fn position(&self) -> super::ComponentPosition {
    //     super::ComponentPosition::Body {
    //         id: "home".into(),
    //         selection_keys: vec!['h'],
    //     }
    // }
}
impl Component for Home {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
        self.command_tx = Some(tx);
        Ok(())
    }

    fn register_config_handler(&mut self, config: Config) -> Result<()> {
        self.config = config;
        Ok(())
    }

    fn update(&mut self, _state: &State, action: Action) -> Result<Option<Action>> {
        match action {
            Action::Tick => {
                // add any logic here that should run on every tick
            }
            Action::Render => {
                // add any logic here that should run on every render
            }
            _ => {}
        }
        Ok(None)
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        frame.render_widget(Paragraph::new("Welcome to KnowLS"), area);
        Ok(())
    }
}
