use crate::{
    agents::AgentID,
    state::{LspState, SharedState},
    util::OneOf,
};
use color_eyre::{owo_colors::OwoColorize, Result};
use crossterm::event::KeyEvent;
use lsp_types::Uri;
use ratatui::{
    crossterm::{
        self,
        event::{Event, KeyCode},
    },
    layout::{Constraint, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Text},
    widgets::{Block, Borders, HighlightSpacing, List, ListItem, Paragraph},
    DefaultTerminal, Frame,
};
use std::{collections::HashMap, time::Duration};
use throbber_widgets_tui::{Throbber, ThrobberState};
use tokio::sync::RwLockReadGuard;
use tracing::warn;

use super::props::{
    agents::AgentProps, database::DBProps, docs::DocsProps, global::GlobalProps, TuiProp,
};

#[derive(Debug, PartialEq, Eq, Default)]
pub enum CurrentPane {
    #[default]
    Normal,
    Agents,
    Global,
    Docs,
    DB,
}

struct TuiProps {
    agents_props: AgentProps,
    docs_props: DocsProps,
    database_props: DBProps,
    global_props: GlobalProps,
}

pub struct Tui {
    props: TuiProps,
    current_pane: CurrentPane,
    state: SharedState<'static>,
    should_quit: bool,
}

impl Tui {
    pub async fn new(state: SharedState<'static>) -> Self {
        let props = {
            let r = state.0.try_read().unwrap();
            TuiProps {
                global_props: GlobalProps::from_state_read(&r).await,
                agents_props: AgentProps::from_state_read(&r).await,
                docs_props: DocsProps::from_state_read(&r).await,
                database_props: DBProps::from_state_read(&r).await,
            }
        };

        Self {
            current_pane: CurrentPane::default(),
            props,
            state,
            should_quit: false,
        }
    }

    fn handle_key_event(&mut self, event: KeyEvent) {
        warn!("handling key event: {event:#?}");
        match self.current_pane {
            CurrentPane::Normal => match event.code {
                _ if event.code == AgentProps::select_me().0 => {
                    self.current_pane = AgentProps::select_me().1;
                }
                _ if event.code == DBProps::select_me().0 => {
                    self.current_pane = DBProps::select_me().1;
                }
                _ if event.code == DocsProps::select_me().0 => {
                    self.current_pane = DocsProps::select_me().1;
                }
                _ if event.code == GlobalProps::select_me().0 => {
                    self.current_pane = GlobalProps::select_me().1;
                }
                KeyCode::Char('q') => {
                    self.should_quit = true;
                }
                _ => {}
            },
            _ => match event.code {
                KeyCode::Esc => {
                    self.current_pane = CurrentPane::Normal;
                }
                _ => {}
            },
        }
    }

    pub fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        warn!("running terminal");
        loop {
            self.props.database_props.run(&mut self.state);
            self.props.agents_props.run(&mut self.state);
            self.props.docs_props.run(&mut self.state);
            self.props.global_props.run(&mut self.state);
            terminal.draw(|frame| self.draw(frame))?;
            if self.should_quit {
                return Ok(());
            }
            if crossterm::event::poll(Duration::from_millis(300))? {
                let event = crossterm::event::read().unwrap();
                if let Event::Key(event) = event {
                    self.handle_key_event(event);
                }
            }
        }
    }

    fn draw(&mut self, frame: &mut Frame) {
        let first_vertical =
            Layout::vertical([Constraint::Percentage(5), Constraint::Percentage(95)]);
        let horizontal = Layout::horizontal([
            Constraint::Percentage(40),
            Constraint::Percentage(40),
            Constraint::Percentage(20),
        ]);
        let global_and_db_vertical =
            Layout::vertical([Constraint::Percentage(10), Constraint::Percentage(90)]);

        let [top_bar_area, content_area] = first_vertical.areas(frame.area());
        let [agents_area, docs_area, global_and_db_area] = horizontal.areas(content_area);
        let [global_area, db_area] = global_and_db_vertical.areas(global_and_db_area);

        let [back_or_exit_button, back_or_exit_message] =
            if self.current_pane == CurrentPane::Normal {
                ["q", " to quit"]
            } else {
                ["Esc", " to go back"]
            };

        let current_pane_msg = match self.current_pane {
            CurrentPane::DB => "Database",
            CurrentPane::Docs => "Docs",
            CurrentPane::Agents => "Agents",
            CurrentPane::Global => "Global",
            CurrentPane::Normal => "Home",
        };
        let top_bar_msg = vec![
            current_pane_msg.bold(),
            "    Press ".into(),
            back_or_exit_button.bold(),
            back_or_exit_message.into(),
            "    Press ".into(),
            "?".bold(),
            " For help".into(),
        ];

        let text = Text::from(Line::from(top_bar_msg)).patch_style(Style::new().fg(Color::Magenta));
        let help_message = Paragraph::new(text);
        frame.render_widget(help_message, top_bar_area);

        self.props.agents_props.draw(
            self.current_pane == AgentProps::select_me().1,
            frame,
            agents_area,
        );

        self.props.docs_props.draw(
            self.current_pane == DocsProps::select_me().1,
            frame,
            docs_area,
        );

        self.props
            .database_props
            .draw(self.current_pane == DBProps::select_me().1, frame, db_area);

        self.props.global_props.draw(
            self.current_pane == GlobalProps::select_me().1,
            frame,
            global_area,
        );
    }
}
