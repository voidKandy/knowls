use crate::MainResult;

use super::{CurrentPane, TuiProp};
use crossterm::event::KeyCode;
use lsp_types::Uri;
use ratatui::{
    style::{Color, Stylize},
    text::Line,
    widgets::{Block, Borders, HighlightSpacing, List, ListItem},
};

pub struct DocsProps {
    pub current_doc: Option<Uri>,
    pub all_docs: Vec<Uri>,
}

impl TuiProp for DocsProps {
    fn color(&self, selected: bool) -> ratatui::prelude::Color {
        if selected {
            Color::Magenta
        } else {
            Color::LightMagenta
        }
    }
    fn select_me() -> (crossterm::event::KeyCode, super::CurrentPane) {
        (KeyCode::Char('d'), CurrentPane::Docs)
    }
    fn handle_keyevent(
        tui: &mut crate::ui::tui::Tui,
        event: crossterm::event::KeyEvent,
    ) -> MainResult<()> {
        match event.code {
            KeyCode::Esc => {
                tui.current_pane = CurrentPane::Normal;
            }
            _ => {}
        }
        Ok(())
    }

    fn run(&mut self, state_ref: &mut crate::state::SharedState<'static>) {
        let r = state_ref.0.try_read().unwrap();
        self.all_docs = r.documents.keys().map(|k| k.to_owned()).collect();
    }

    async fn from_state_read(
        r: &tokio::sync::RwLockReadGuard<'_, crate::state::LspState<'static>>,
    ) -> Self {
        DocsProps {
            current_doc: None,
            all_docs: r.documents.keys().map(|k| k.to_owned()).collect(),
        }
    }

    fn draw(&mut self, selected: bool, frame: &mut ratatui::Frame, area: ratatui::prelude::Rect) {
        let block = Block::new()
            .title(Line::raw("Documents").centered())
            .borders(Borders::ALL)
            .fg(self.color(selected));

        let items: Vec<ListItem> = self
            .all_docs
            .iter()
            .map(|doc_uri| {
                let mut color = Color::Cyan;

                if self
                    .current_doc
                    .as_ref()
                    .is_some_and(|current| doc_uri == current)
                {
                    color = Color::LightMagenta;
                }

                ListItem::from(doc_uri.to_string()).bg(color)
            })
            .collect();

        // Create a List from all list items and highlight the currently selected one
        let list = List::new(items)
            .block(block)
            // .highlight_style(SELECTED_STYLE)
            .highlight_symbol(">")
            .highlight_spacing(HighlightSpacing::Always);
        frame.render_widget(list, area);
    }
}
