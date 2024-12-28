use super::{CurrentPane, TuiProp};
use crate::{agents::AgentID, MainResult};
use crossterm::event::KeyCode;
use ratatui::{
    style::{Color, Stylize},
    text::Line,
    widgets::{Block, Borders, HighlightSpacing, List, ListItem},
};

pub struct AgentProps {
    pub current_agent: usize,
    pub all_agents: Vec<AgentID>,
}

impl AgentProps {
    fn prev_agent(&mut self) {
        if self.current_agent == 0 {
            self.current_agent = self.all_agents.len() - 1;
        } else {
            self.current_agent -= 1;
        }
    }
    fn next_agent(&mut self) {
        if self.current_agent == self.all_agents.len() - 1 {
            self.current_agent = 0;
        } else {
            self.current_agent += 1;
        }
    }
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
    fn handle_keyevent(
        tui: &mut crate::ui::tui::Tui,
        event: crossterm::event::KeyEvent,
    ) -> MainResult<()> {
        match event.code {
            KeyCode::Esc => {
                tui.current_pane = CurrentPane::Normal;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                tui.props.agents_props.next_agent();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                tui.props.agents_props.prev_agent();
            }
            _ => {}
        }
        Ok(())
    }
    async fn from_state_read(
        r: &tokio::sync::RwLockReadGuard<'_, crate::state::LspState<'static>>,
    ) -> Self {
        AgentProps {
            current_agent: 0,
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
            .enumerate()
            .map(|(i, agent_id)| {
                let mut color = Color::Cyan;

                if self.current_agent == i {
                    color = Color::LightMagenta;
                }

                ListItem::from(agent_id.to_string()).fg(color)
            })
            .collect();

        let list = List::new(items)
            .block(block)
            // .highlight_style(SELECTED_STYLE)
            .highlight_symbol(">")
            .highlight_spacing(HighlightSpacing::Always);
        frame.render_widget(list, area);
    }
}
