use std::collections::HashMap;

use crate::state::State;
use crate::tui::app::Mode;
use crate::tui::config::{key_event_to_string, parse_key_event, Config};

use super::{super::action::Action, ComponentId};
use super::{Component, PageComponent};
use color_eyre::owo_colors::OwoColorize;
use color_eyre::Result;
use crossterm::event::KeyEvent;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Style, Stylize},
    text::{Span, Text},
    widgets::{Block, Borders, Clear, Paragraph, Widget, Wrap},
    Frame,
};
use surrealdb::sql::statements::CreateStatement;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug, Clone)]
pub struct HelpComponent {
    command_tx: Option<UnboundedSender<Action>>,
    config: Config,
}

impl Default for HelpComponent {
    fn default() -> Self {
        Self {
            command_tx: None,
            config: Config::default(),
        }
    }
}

#[derive(Debug, Default)]
struct HelpPopup<'a> {
    content: Text<'a>,
}

impl Widget for HelpPopup<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // ensure that all cells under the popup are cleared to avoid leaking content
        Clear.render(area, buf);
        let block = Block::new()
            .title("Keybinds")
            .title_style(Style::new().yellow())
            .borders(Borders::ALL)
            .border_style(Style::new().red());
        Paragraph::new(self.content)
            .wrap(Wrap { trim: true })
            .style(Style::new().yellow())
            .block(block)
            .render(area, buf);
    }
}

impl HelpComponent {
    fn popup(&self) -> HelpPopup {
        tracing::warn!("keybindings for help popup: {:#?}", self.config.keybindings);
        let text = self
            .config
            .keybindings
            .iter()
            .fold(String::new(), |acc, (mode, map)| {
                format!(
                    "{acc}{}",
                    map.iter()
                        .fold(format!("==={mode:#?}===\n"), |inner_acc, (keys, action)| {
                            format!(
                                "{inner_acc}{}\n",
                                keys.iter()
                                    .fold(format!("---{action:?}---"), |in_inner_acc, key| {
                                        format!("{in_inner_acc}\n{}\n", key_event_to_string(key))
                                    })
                                    .to_string()
                            )
                        })
                )
            });
        let content = Text::raw(text);
        HelpPopup { content }
    }
}

impl PageComponent for HelpComponent {
    fn id(&self) -> ComponentId {
        "help".into()
    }
    fn selection_keys(&self) -> Vec<KeyEvent> {
        vec![parse_key_event("?").unwrap()]
    }
    fn bindings(&self) -> std::collections::HashMap<Vec<KeyEvent>, Action> {
        let map = HashMap::new();
        map
    }
    // fn position(&self) -> super::ComponentPosition {
    //     let id = "help".into();
    //     if self.open {
    //         return super::ComponentPosition::Popup(id);
    //     }
    //     super::ComponentPosition::Header(id)
    // }
}
impl Component for HelpComponent {
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
            // Action::HelpDialogue(open) => {
            //     return Ok(Some(Action::ChangeMode(crate::tui::app::Mode::Help)));
            // }

            // Action::Tick => self.app_tick()?,
            // Action::Render => self.render_tick()?,
            _ => {}
        };
        Ok(None)
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        // let [top, _] = Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(area);
        self.popup().render(area, frame.buffer_mut());
        Ok(())
    }
}
