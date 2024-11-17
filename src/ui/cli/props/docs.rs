use crossterm::event::KeyCode;
use lsp_types::Uri;
use ratatui::{
    style::{Color, Style, Stylize},
    text::{Line, Text},
    widgets::{Block, Borders, HighlightSpacing, List, ListItem, Paragraph},
};
use throbber_widgets_tui::ThrobberState;

use crate::util::OneOf;

use super::{CurrentPane, TuiProp};

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
    async fn handle_keyevent(
        tui: &mut crate::ui::cli::new_tui::Tui,
        event: crossterm::event::KeyEvent,
    ) -> anyhow::Result<()> {
        Ok(())
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
