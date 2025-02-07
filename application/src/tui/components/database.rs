use std::time::Instant;

use color_eyre::Result;
use knowls::util::oneof::OneOf;
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};
use throbber_widgets_tui::ThrobberState;

use crate::{database::config::DatabaseConfig, state::State};

use super::{Component, PageComponent};

use super::super::action::Action;

#[derive(Debug, Clone)]
pub struct DatabaseComponent {
    healthy: OneOf<ThrobberState, bool>,
    config: DatabaseConfig,
}

impl From<&State> for DatabaseComponent {
    fn from(value: &State) -> Self {
        Self {
            healthy: OneOf::Left(ThrobberState::default()),
            config: value.database.config().clone(),
        }
    }
}

impl Component for DatabaseComponent {
    fn update(&mut self, _state: &State, action: Action) -> Result<Option<Action>> {
        match action {
            Action::Tick => {
                if let OneOf::Left(ref mut throbber) = self.healthy {
                    throbber.calc_next();
                }
            }
            _ => {}
        };
        Ok(None)
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        let [throbber, rest] =
            Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(area);
        let block = Block::new()
            .borders(Borders::all())
            .title("Database")
            .title_style(Style::new().green());
        let address = Line::raw(format!(
            "{}://{}:{}",
            self.config.protocol, self.config.host, self.config.port,
        ))
        .centered()
        .bold()
        .style(Style::new().green());

        let lines = vec![
            address,
            Line::raw(format!("{}@{}", self.config.user, self.config.pass))
                .style(Style::new().red()),
            Line::raw(format!("Database: {}", self.config.database)),
            Line::raw(format!("Namespace: {}", self.config.namespace)),
        ];

        if let OneOf::Left(ref mut throbber_state) = self.healthy {
            let full = throbber_widgets_tui::Throbber::default()
                .label("Running...")
                .style(ratatui::style::Style::default().fg(ratatui::style::Color::Cyan))
                .throbber_style(
                    ratatui::style::Style::default()
                        .fg(ratatui::style::Color::Red)
                        .add_modifier(ratatui::style::Modifier::BOLD),
                )
                .throbber_set(throbber_widgets_tui::CLOCK)
                .use_type(throbber_widgets_tui::WhichUse::Spin);
            frame.render_stateful_widget(full, throbber, throbber_state);
        }

        let paragraph = Paragraph::new(lines).left_aligned().block(block);
        frame.render_widget(paragraph, rest);

        Ok(())
    }
}
