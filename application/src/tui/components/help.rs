use std::collections::HashMap;

use super::Component;
use super::{super::action::Action, ComponentId};
use crate::state::State;
use crate::tui::app::Mode;
use crate::tui::components::PageComponentAction;
use crate::tui::config::{key_event_to_string, Config};
use color_eyre::Result;
use crossterm::event::KeyEvent;
use ratatui::layout::{Constraint, Layout};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Style, Stylize},
    text::Text,
    widgets::{Block, Borders, Clear, Paragraph, Widget, Wrap},
    Frame,
};
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug, Clone)]
pub struct HelpComponent {
    command_tx: Option<UnboundedSender<Action>>,
    config: Config,
    pub component_mode: Option<ComponentId>,
}

impl Default for HelpComponent {
    fn default() -> Self {
        Self {
            command_tx: None,
            config: Config::default(),
            component_mode: None,
        }
    }
}

#[derive(Debug, Default)]
struct HelpPopup<'a> {
    action_keybinds: Paragraph<'a>,
    component_action_keybinds: Option<Paragraph<'a>>,
}

impl Widget for HelpPopup<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // ensure that all cells under the popup are cleared to avoid leaking content
        Clear.render(area, buf);
        let constraints = match self.component_action_keybinds {
            Some(_) => vec![Constraint::Fill(1), Constraint::Fill(1)],
            None => vec![Constraint::Fill(1), Constraint::Fill(0)],
        };

        let chunks = Layout::vertical(constraints)
            .flex(ratatui::layout::Flex::SpaceBetween)
            .split(area);
        // let block = Block::new()
        //     .title("Keybinds")
        //     .title_style(Style::new().yellow())
        //     .borders(Borders::ALL)
        //     .border_style(Style::new().red());

        self.action_keybinds.render(chunks[0], buf);
        if let Some(paragraph) = self.component_action_keybinds {
            paragraph.render(chunks[1], buf);
        }
    }
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
    fn update(
        &mut self,
        state: &crate::state::StateReadGuard<'_>,
        action: Action,
    ) -> Result<Option<Action>> {
        Ok(None)
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        // tracing::warn!("keybindings for help popup: {:#?}", self.config.keybindings);
        let mode = match self.component_mode.as_ref() {
            Some(id) => Mode::Component(id.clone()),
            None => Mode::Normal,
        };
        let block = Block::bordered().title(format!("=== {mode:?} Keybinds ==="));
        let binds = self
            .config
            .keybindings
            .get(&mode)
            .expect("help did not get any binds for given mode");

        let text = binds
            .iter()
            .fold(String::new(), |inner_acc, (keys, action)| {
                format!(
                    "{inner_acc}{}\n",
                    keys.iter()
                        .fold(format!("---{action:?}---"), |in_inner_acc, key| {
                            format!("{in_inner_acc}\n{}\n", key_event_to_string(key))
                        })
                        .to_string()
                )
            });
        let action_keybinds = Paragraph::new(text).block(block);
        let mut component_action_keybinds = None;

        if let Mode::Component(id) = mode {
            let render_component_action =
                |action: &PageComponentAction, keys: &Vec<KeyEvent>| -> String {
                    let mut all_keys_str = keys.iter().fold(String::from("["), |acc, k| {
                        format!("{acc}{}, ", key_event_to_string(k))
                    });
                    // pop off trailing ', '
                    all_keys_str.pop();
                    all_keys_str.pop();
                    all_keys_str.push(']');

                    format!("{all_keys_str} - {}", action.description)
                };
            let block = Block::bordered().title("Component Action Keybinds");
            let binds = self
                .config
                .page_component_keybindings
                .get(&id)
                .expect("help did not get keybinds for component");
            let text = binds
                .iter()
                .fold(String::new(), |inner_acc, (keys, action)| {
                    format!("{inner_acc}{}\n", render_component_action(action, keys))
                });
            component_action_keybinds = Some(Paragraph::new(text).block(block));
        }
        HelpPopup {
            action_keybinds,
            component_action_keybinds,
        }
        .render(area, frame.buffer_mut());

        Ok(())
    }
}
