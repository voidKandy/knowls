use super::{CurrentPane, TuiProp};
use crate::util::OneOf;
use crossterm::event::KeyCode;
use ratatui::{
    style::{Color, Style, Stylize},
    text::{Line, Text},
    widgets::{Block, Borders, Paragraph},
};
use throbber_widgets_tui::{Throbber, ThrobberState};

pub struct GlobalProps {
    pub connected: OneOf<String, throbber_widgets_tui::ThrobberState>,
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
    fn handle_keyevent(
        tui: &mut crate::ui::tui::Tui,
        event: crossterm::event::KeyEvent,
    ) -> anyhow::Result<()> {
        match event.code {
            KeyCode::Esc => {
                tui.current_pane = CurrentPane::Normal;
            }
            _ => {}
        }
        Ok(())
    }
    async fn from_state_read(
        r: &tokio::sync::RwLockReadGuard<'_, crate::state::LspState<'static>>,
    ) -> Self {
        GlobalProps {
            connected: OneOf::Right(ThrobberState::default()),
        }
    }
    fn run(&mut self, state_ref: &mut crate::state::SharedState<'static>) {
        if let OneOf::Right(ref mut throbber_state) = self.connected {
            throbber_state.calc_next();
        }
        let r = state_ref.0.try_read().expect("failed read lock");
        if let Some(addr) = r.attached.as_ref() {
            // let path = addr.as_pathname().unwrap();
            self.connected = OneOf::Left(format!("{addr:#?}"));
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
