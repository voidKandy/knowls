use crate::app::tui::Tui;

use super::{CurrentPane, TuiProp};
use crossterm::event::KeyCode;
use knowls::MainResult;
use ratatui::{
    style::{Color, Stylize},
    text::Line,
    widgets::{Block, Borders, HighlightSpacing, List, ListItem},
};
use surrealdb::RecordId;

pub struct KnowledgeProps {
    pub current_knowledge: Option<RecordId>,
    pub all_knowledges: Vec<RecordId>,
}

impl TuiProp for KnowledgeProps {
    fn color(&self, selected: bool) -> ratatui::prelude::Color {
        if selected {
            Color::Magenta
        } else {
            Color::LightMagenta
        }
    }
    fn select_me() -> (crossterm::event::KeyCode, super::CurrentPane) {
        (KeyCode::Char('d'), CurrentPane::Knowledge)
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

    fn run(&mut self, state_ref: &mut std::sync::Arc<tokio::sync::RwLock<crate::app::State>>) {
        let r = state_ref.try_read().unwrap();
        self.all_knowledges = r.knowledge.keys().map(|k| k.to_owned()).collect();
    }

    async fn from_state_read(r: &tokio::sync::RwLockReadGuard<'_, crate::app::State>) -> Self {
        KnowledgeProps {
            current_knowledge: None,
            all_knowledges: r.knowledge.keys().map(|k| k.to_owned()).collect(),
        }
    }

    fn draw(&mut self, selected: bool, frame: &mut ratatui::Frame, area: ratatui::prelude::Rect) {
        let block = Block::new()
            .title(Line::raw("Knowledge").centered())
            .borders(Borders::ALL)
            .fg(self.color(selected));

        let items: Vec<ListItem> = self
            .all_knowledges
            .iter()
            .map(|knowledge_uri| {
                let mut color = Color::Cyan;

                if self
                    .current_knowledge
                    .as_ref()
                    .is_some_and(|current| knowledge_uri == current)
                {
                    color = Color::LightMagenta;
                }

                ListItem::from(knowledge_uri.to_string()).bg(color)
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
