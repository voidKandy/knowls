use super::{CurrentPane, TuiProp};
use crate::agents::AgentID;
use crossterm::event::KeyCode;
use ratatui::{
    style::{Color, Stylize},
    text::Line,
    widgets::{Block, Borders, HighlightSpacing, List, ListItem},
};

pub struct AgentProps {
    pub current_agent: Option<AgentID>,
    pub all_agents: Vec<AgentID>,
}

impl TuiProp for AgentProps {
    fn color(&self, selected: bool) -> ratatui::prelude::Color {
        if selected {
            Color::Green
        } else {
            Color::LightGreen
        }
    }
    fn select_me() -> (crossterm::event::KeyCode, super::CurrentPane) {
        (KeyCode::Char('a'), CurrentPane::Agents)
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
        AgentProps {
            current_agent: None,
            all_agents: r.agents.iter_agents().map(|(id, _)| id.clone()).collect(),
        }
    }
    fn draw(&mut self, selected: bool, frame: &mut ratatui::Frame, area: ratatui::prelude::Rect) {
        let block = Block::new()
            .title(Line::raw("Agents").centered())
            .borders(Borders::ALL)
            .fg(self.color(selected));

        let items: Vec<ListItem> = self
            .all_agents
            .iter()
            .map(|agent_id| {
                let mut color = Color::Cyan;

                if self
                    .current_agent
                    .as_ref()
                    .is_some_and(|current| agent_id == current)
                {
                    color = Color::LightMagenta;
                }

                ListItem::from(agent_id.to_string()).bg(color)
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
