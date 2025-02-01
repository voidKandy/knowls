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
    command_tx: Option<UnboundedSender<Action>>,
}

impl Default for KnowledgeComponent {
    fn default() -> Self {
        Self { command_tx: None }
    }
}

impl Component for KnowledgeComponent {
    fn position(&self) -> super::ComponentPosition {
        super::ComponentPosition::Body {
            id: "knowledge".into(),
            selection_keys: vec!['k'],
        }
    }
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
        self.command_tx = Some(tx);
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
        let span = Span::styled("KNOWLEDGE 'n stuff", Style::new().dim());
        let paragraph = Paragraph::new(span).left_aligned();
        frame.render_widget(paragraph, top);
        Ok(())
    }
}
