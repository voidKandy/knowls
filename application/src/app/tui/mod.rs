pub mod props;
use color_eyre::Result;
use crossterm::event::KeyEvent;
use props::{database::DatabaseProps, global::GlobalProps, knowledge::KnowledgeProps, TuiProp};
use ratatui::{
    crossterm::{
        self,
        event::{Event, KeyCode},
    },
    layout::{Constraint, Flex, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Text},
    widgets::{Block, Clear, Paragraph},
    DefaultTerminal, Frame,
};
use std::{sync::Arc, time::Duration};
use tokio::sync::RwLock;
use tracing::warn;

use super::State;

#[derive(Debug, PartialEq, Eq, Default)]
pub enum CurrentPane {
    #[default]
    Normal,
    Global,
    Knowledge,
    DB,
}

impl ToString for CurrentPane {
    fn to_string(&self) -> String {
        match self {
            Self::DB => "Database",
            Self::Global => "Global",
            Self::Knowledge => "Knowledge",
            Self::Normal => "Home",
        }
        .to_string()
    }
}

struct TuiProps {
    knowledge_props: KnowledgeProps,
    database_props: DatabaseProps,
    global_props: GlobalProps,
}

pub struct Tui {
    state: Arc<RwLock<State>>,
    props: TuiProps,
    show_help: bool,
    current_pane: CurrentPane,
    should_quit: bool,
}

impl Tui {
    pub async fn new(state: Arc<RwLock<State>>) -> Self {
        let r = state.read().await;
        let props = {
            TuiProps {
                global_props: GlobalProps::from_state_read(&r).await,
                knowledge_props: KnowledgeProps::from_state_read(&r).await,
                database_props: DatabaseProps::from_state_read(&r).await,
            }
        };
        drop(r);

        Self {
            state,
            current_pane: CurrentPane::default(),
            show_help: false,
            props,
            should_quit: false,
        }
    }

    fn handle_key_event(&mut self, event: KeyEvent) {
        warn!("handling key event: {event:#?}");
        if self.show_help && event.code == KeyCode::Char('q') {
            self.show_help = false;
            return;
        }
        if event.code == KeyCode::Char('?') {
            self.show_help = true;
        }
        match self.current_pane {
            _ if self.current_pane == DatabaseProps::select_me().1 => {
                DatabaseProps::handle_keyevent(self, event).expect("failed to handle key event");
            }
            _ if self.current_pane == KnowledgeProps::select_me().1 => {
                KnowledgeProps::handle_keyevent(self, event).expect("failed to handle key event");
            }
            _ if self.current_pane == GlobalProps::select_me().1 => {
                GlobalProps::handle_keyevent(self, event).expect("failed to handle key event");
            }
            CurrentPane::Normal => match event.code {
                _ if event.code == DatabaseProps::select_me().0 => {
                    self.current_pane = DatabaseProps::select_me().1;
                }
                _ if event.code == KnowledgeProps::select_me().0 => {
                    self.current_pane = KnowledgeProps::select_me().1;
                }
                _ if event.code == GlobalProps::select_me().0 => {
                    self.current_pane = GlobalProps::select_me().1;
                }
                KeyCode::Char('q') => {
                    self.should_quit = true;
                }
                _ => {}
            },
            _ => {}
        }
    }

    pub fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        warn!("running terminal");
        loop {
            self.props.database_props.run(&mut self.state);
            self.props.knowledge_props.run(&mut self.state);
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
        let horizontal =
            Layout::horizontal([Constraint::Percentage(80), Constraint::Percentage(20)]);
        let global_and_db_vertical =
            Layout::vertical([Constraint::Percentage(10), Constraint::Percentage(90)]);

        let [top_bar_area, content_area] = first_vertical.areas(frame.area());
        let [knowledge_area, global_and_db_area] = horizontal.areas(content_area);
        let [global_area, db_area] = global_and_db_vertical.areas(global_and_db_area);

        let [back_or_exit_button, back_or_exit_message] =
            if self.current_pane == CurrentPane::Normal {
                ["q", " to quit"]
            } else {
                ["Esc", " to go back"]
            };

        let current_pane_msg = match self.current_pane {
            CurrentPane::DB => "Database",
            CurrentPane::Knowledge => "Knowledge",
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

        self.props.knowledge_props.draw(
            self.current_pane == KnowledgeProps::select_me().1,
            frame,
            knowledge_area,
        );

        self.props.database_props.draw(
            self.current_pane == DatabaseProps::select_me().1,
            frame,
            db_area,
        );

        self.props.global_props.draw(
            self.current_pane == GlobalProps::select_me().1,
            frame,
            global_area,
        );

        if self.show_help {
            let block = Block::bordered().fg(Color::Yellow).title("Help");
            let area = help_popup(frame.area(), 60, 20);
            frame.render_widget(Clear, area); //this clears out the background
            let lines: Vec<Line> = vec![
                Line::from(vec![
                    format!(" {} ", DatabaseProps::select_me().0).bold().into(),
                    format!("to select {}", DatabaseProps::select_me().1.to_string()).into(),
                ]),
                Line::from(vec![
                    format!(" {} ", KnowledgeProps::select_me().0).bold().into(),
                    format!("to select {}", KnowledgeProps::select_me().1.to_string()).into(),
                ]),
                Line::from(vec![
                    format!(" {} ", GlobalProps::select_me().0).bold().into(),
                    format!("to select {}", GlobalProps::select_me().1.to_string()).into(),
                ]),
            ];

            let paragraph = Paragraph::new(lines).block(block);
            frame.render_widget(paragraph, area);
        }
    }
}

fn help_popup(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let vertical = Layout::vertical([Constraint::Percentage(percent_y)]).flex(Flex::Center);
    let horizontal = Layout::horizontal([Constraint::Percentage(percent_x)]).flex(Flex::Center);

    let [area] = vertical.areas(area);
    let [area] = horizontal.areas(area);
    area
}
