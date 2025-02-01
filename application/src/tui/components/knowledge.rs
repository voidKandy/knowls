use super::super::action::Action;
use super::Component;
use crate::tui::config::{key_event_to_string, Config};
use color_eyre::Result;
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
pub struct KnowledgeComponent {
    open: bool,
    command_tx: Option<UnboundedSender<Action>>,
    config: Config,
}

impl Default for KnowledgeComponent {
    fn default() -> Self {
        Self {
            open: false,
            command_tx: None,
            config: Config::default(),
        }
    }
}

#[derive(Debug, Default)]
struct KnowledgePopup<'a> {
    content: Text<'a>,
}

impl Widget for KnowledgePopup<'_> {
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

impl KnowledgeComponent {
    fn popup(&self) -> KnowledgePopup {
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
        KnowledgePopup { content }
    }
}

impl Component for KnowledgeComponent {
    fn position(&self) -> super::ComponentPosition {
        if self.open {
            return super::ComponentPosition::BodyLeft;
        }
        super::ComponentPosition::Header
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
        let [top, _] = Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(area);
        if self.open {
            self.popup().render(area, frame.buffer_mut());
        } else {
            let span = Span::styled("Press ? for help", Style::new().dim());
            let paragraph = Paragraph::new(span).left_aligned();
            frame.render_widget(paragraph, top);
        }
        Ok(())
    }
}
