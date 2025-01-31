use super::{CurrentPane, TuiProp};
use crate::app::tui::Tui;
use crossterm::event::KeyCode;
use knowls::{util::oneof::OneOf, MainResult};
use ratatui::{
    style::{Color, Style, Stylize},
    text::{Line, Text},
    widgets::{Block, Borders, Paragraph},
};
use std::sync::Arc;
use throbber_widgets_tui::{Throbber, ThrobberState};
use tokio::sync::RwLock;

pub struct GlobalProps {
    pub connected: OneOf<Vec<String>, throbber_widgets_tui::ThrobberState>,
}

impl TuiProp for GlobalProps {
    fn color(&self, selected: bool) -> ratatui::prelude::Color {
        if selected {
            Color::Cyan
        } else {
            Color::LightCyan
        }
    }
    fn select_me() -> (crossterm::event::KeyCode, super::CurrentPane) {
        (KeyCode::Char('g'), CurrentPane::Global)
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
        GlobalProps {
            connected: OneOf::Right(ThrobberState::default()),
        }
    }
    fn run(&mut self, state_ref: &mut Arc<RwLock<crate::app::State>>) {
        if let OneOf::Right(ref mut throbber_state) = self.connected {
            throbber_state.calc_next();
        }
        let r = state_ref.try_read().unwrap();
        if !r.connections.is_empty() {
            r.connections.keys().for_each(|k| match self.connected {
                OneOf::Left(ref mut vec) => {
                    vec.push(k.to_owned());
                }
                OneOf::Right(_) => self.connected = OneOf::Left(vec![k.to_owned()]),
            });
        }
    }
    fn draw(&mut self, selected: bool, frame: &mut ratatui::Frame, area: ratatui::prelude::Rect) {
        let block = Block::new()
            .title(Line::raw("Global").centered())
            .borders(Borders::ALL)
            .fg(self.color(selected));

        match &mut self.connected {
            OneOf::Left(addr) => {
                let text = Text::from(Line::from(format!("Connected at address: {addr:#?}")))
                    .patch_style(Style::new().fg(Color::Green));
                let p = Paragraph::new(text).block(block);
                frame.render_widget(p, area);
            }
            OneOf::Right(ref mut throbber_state) => {
                let throbber = Throbber::default()
                    .label("Awaiting LSP connection...")
                    .throbber_set(throbber_widgets_tui::WHITE_SQUARE)
                    .use_type(throbber_widgets_tui::WhichUse::Spin);

                frame.render_stateful_widget(throbber, area, throbber_state);
            }
        }
    }
}
