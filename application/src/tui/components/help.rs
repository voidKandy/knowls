use std::collections::HashMap;

use super::Component;
use super::{super::action::Action, ComponentId};
use crate::state::State;
use crate::tui::app::Mode;
use crate::tui::config::{key_event_to_string, Config};
use color_eyre::Result;
use crossterm::event::KeyEvent;
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
        let mode = match self.component_mode.as_ref() {
            Some(id) => Mode::Component(id.clone()),
            None => Mode::Normal,
        };
        let text = self
            .config
            .keybindings
            .get(&mode)
            .unwrap_or(&HashMap::<Vec<KeyEvent>, Action>::new())
            .iter()
            .fold(format!("==={mode:?}===\n"), |inner_acc, (keys, action)| {
                format!(
                    "{inner_acc}{}\n",
                    keys.iter()
                        .fold(format!("---{action:?}---"), |in_inner_acc, key| {
                            format!("{in_inner_acc}\n{}\n", key_event_to_string(key))
                        })
                        .to_string()
                )
            });
        let content = Text::raw(text);
        HelpPopup { content }
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
