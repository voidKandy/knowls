use std::process::Command;

use super::super::action::Action;
use super::Component;
use crate::{
    database::models::Knowledge,
    state::State,
    tui::config::{key_event_to_string, Config},
};
use color_eyre::Result;
use crossterm::event::KeyCode;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    prelude::CrosstermBackend,
    style::{Color, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, HighlightSpacing, List, ListItem, Paragraph, Widget, Wrap},
    Frame,
};
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug, Clone)]
pub struct KnowledgeComponent {
    command_tx: Option<UnboundedSender<Action>>,
    config: Config,
    // String is ID of the Knowledge
    knowledge: Vec<(String, Knowledge)>,
    current_knowledge: Option<usize>,
}

impl From<&State> for KnowledgeComponent {
    fn from(value: &State) -> Self {
        Self {
            command_tx: None,
            config: Config::default(),
            current_knowledge: None,
            knowledge: value
                .knowledge
                .iter()
                .map(|(id, knowledge)| (id.to_string(), knowledge.clone()))
                .collect(),
        }
    }
}

impl KnowledgeComponent {
    fn cycle_knowledge(&mut self, asc: bool) {
        if self.knowledge.is_empty() {
            return;
        }
        if asc {
            match self.current_knowledge {
                Some(ref mut n) => {
                    if *n == 0 {
                        *n = self.knowledge.len() - 1
                    } else {
                        *n -= 1
                    }
                }
                None => {
                    self.current_knowledge = Some(self.knowledge.len() - 1);
                }
            }
        } else {
            match self.current_knowledge {
                Some(ref mut n) => {
                    if *n >= self.knowledge.len() {
                        *n = 0
                    } else {
                        *n += 1
                    }
                }
                None => {
                    self.current_knowledge = Some(0);
                }
            }
        }
    }
}

impl Component for KnowledgeComponent {
    fn position(&self) -> super::ComponentPosition {
        super::ComponentPosition::Body {
            id: "knowledge".into(),
            selection_keys: vec!['k'],
        }
    }

    fn handle_key_event(&mut self, key: crossterm::event::KeyEvent) -> Result<Option<Action>> {
        match key.code {
            KeyCode::Char('k') => {
                self.cycle_knowledge(true);
            }
            KeyCode::Char('j') => {
                self.cycle_knowledge(false);
            }

            KeyCode::Char('o') => {
                return Ok(Some(Action::ChangeMode(crate::tui::app::Mode::Editor)));
            }
            _ => {}
        }
        Ok(None)
    }

    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
        self.command_tx = Some(tx);
        Ok(())
    }
    fn register_config_handler(&mut self, config: Config) -> Result<()> {
        self.config = config;
        Ok(())
    }

    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            // Action::Tick => self.app_tick()?,
            // Action::Render => self.render_tick()?,
            _ => {}
        };
        Ok(None)
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        let [header, body] =
            Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(area);
        let paragraph = Paragraph::new("=== Knowledge ===").left_aligned();
        frame.render_widget(paragraph, header);

        let block = Block::new()
            .title(Line::raw("Entries").centered())
            .borders(Borders::ALL);

        let items: Vec<ListItem> = self
            .knowledge
            .iter()
            .enumerate()
            .map(|(i, (id, _knowledge))| {
                let mut color = Color::Cyan;

                if self
                    .current_knowledge
                    .as_ref()
                    .is_some_and(|current| &i == current)
                {
                    color = Color::LightMagenta;
                }

                ListItem::from(id.to_owned()).bg(color)
            })
            .collect();

        // Create a List from all list items and highlight the currently selected one
        let list = List::new(items)
            .block(block)
            // .highlight_style(SELECTED_STYLE)
            .highlight_symbol(">")
            .highlight_spacing(HighlightSpacing::Always);
        frame.render_widget(list, body);
        Ok(())
    }
}
