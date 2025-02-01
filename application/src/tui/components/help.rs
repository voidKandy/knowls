use crate::tui::config::{key_event_to_string, Config};

use super::Component;
use super::{super::action::Action, ComponentId};
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
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug, Clone)]
pub struct HelpComponent {
    open: bool,
    command_tx: Option<UnboundedSender<Action>>,
    config: Config,
}

impl Default for HelpComponent {
    fn default() -> Self {
        Self {
            open: false,
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
    /// For displaying all keys that will change the body of the application
    fn body_change_message(&self) -> String {
        let normal_map = self
            .config
            .keybindings
            .get(&crate::tui::app::Mode::Normal)
            .expect("should have normal map");

        normal_map
            .iter()
            .filter_map(|(k, v)| {
                if let Action::ChangeBody(id) = v {
                    Some((k, id))
                } else {
                    None
                }
            })
            .collect::<Vec<(&Vec<KeyEvent>, &ComponentId)>>()
            .into_iter()
            .fold(String::new(), |acc, (keys, id)| {
                let mut all_keys_str = keys.iter().fold(String::from("["), |acc, k| {
                    format!("{acc}{}, ", key_event_to_string(k))
                });
                // pop off trailing ', '
                all_keys_str.pop();
                all_keys_str.pop();
                all_keys_str.push(']');

                format!("{acc} {all_keys_str} for {}", id.as_ref())
            })
    }
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

impl Component for HelpComponent {
    fn position(&self) -> super::ComponentPosition {
        let id = "help".into();
        if self.open {
            return super::ComponentPosition::Popup(id);
        }
        super::ComponentPosition::Header(id)
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
            Action::HelpDialogue(open) => {
                self.open = open;
                if open {
                    return Ok(Some(Action::ChangeMode(crate::tui::app::Mode::Help)));
                } else {
                    return Ok(Some(Action::ChangeMode(crate::tui::app::Mode::Normal)));
                }
            }

            // Action::Tick => self.app_tick()?,
            // Action::Render => self.render_tick()?,
            _ => {}
        };
        Ok(None)
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        let [top, _] = Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(area);
        if self.open {
            self.popup().render(area, frame.buffer_mut());
        } else {
            let span = Span::styled(
                format!("Press ? for help | {}", self.body_change_message()),
                Style::new().dim(),
            );
            let paragraph = Paragraph::new(span).left_aligned();
            frame.render_widget(paragraph, top);
        }
        Ok(())
    }
}
