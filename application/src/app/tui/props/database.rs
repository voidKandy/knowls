use super::{CurrentPane, TuiProp};
use crate::{app::tui::Tui, database::config::DatabaseConfig};
use crossterm::event::KeyCode;
use knowls::{util::oneof::OneOf, MainResult};
use ratatui::{
    style::{Color, Style, Stylize},
    text::{Line, Text},
    widgets::{Block, Borders, Paragraph},
};
use throbber_widgets_tui::{Throbber, ThrobberState};

pub struct DatabaseProps {
    pub config: DatabaseConfig,
}

impl TuiProp for DatabaseProps {
    fn color(&self, selected: bool) -> ratatui::prelude::Color {
        if selected {
            Color::Cyan
        } else {
            Color::LightCyan
        }
    }
    fn select_me() -> (crossterm::event::KeyCode, super::CurrentPane) {
        (KeyCode::Char('g'), CurrentPane::DB)
    }
    fn handle_keyevent(tui: &mut Tui, event: crossterm::event::KeyEvent) -> MainResult<()> {
        match event.code {
            KeyCode::Esc => {
                tui.current_pane = CurrentPane::Normal;
            }
            _ => {}
        }
        Ok(())
    }
    async fn from_state_read(r: &tokio::sync::RwLockReadGuard<'_, crate::app::State>) -> Self {
        DatabaseProps {
            config: r.database.config().clone(),
        }
    }
    fn run(&mut self, state_ref: &mut std::sync::Arc<tokio::sync::RwLock<crate::app::State>>) {}
    fn draw(&mut self, selected: bool, frame: &mut ratatui::Frame, area: ratatui::prelude::Rect) {
        let block = Block::new()
            .title(Line::raw("Database").centered())
            .borders(Borders::ALL)
            .fg(self.color(selected));
        let text = Text::from(self.config.tui_display()).patch_style(Style::new().fg(Color::Green));
        let p = Paragraph::new(text).block(block);
        frame.render_widget(p, area);
    }
}
