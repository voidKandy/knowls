use super::{CurrentPane, TaskingProp, TuiProp};
use crate::util::OneOf;
use crossterm::event::KeyCode;
use ratatui::{
    style::{Color, Style, Stylize},
    text::{Line, Text},
    widgets::{Block, Borders, Paragraph},
};
use throbber_widgets_tui::{Throbber, ThrobberState};
use tokio::sync::oneshot::{error::TryRecvError, Receiver};

#[derive(Debug)]
enum DBPropMessage {
    DatabaseHealth(bool),
}
unsafe impl Send for DBPropMessage {}
unsafe impl Sync for DBPropMessage {}

pub struct DBProps {
    pub healthy: OneOf<bool, throbber_widgets_tui::ThrobberState>,
    task_recv: Option<Receiver<DBPropMessage>>,
    task: Option<tokio::task::JoinHandle<()>>,
}

impl TaskingProp<DBPropMessage> for DBProps {
    fn task(&mut self) -> &mut Option<tokio::task::JoinHandle<()>> {
        &mut self.task
    }
    fn set_task(&mut self, handle: tokio::task::JoinHandle<()>) {
        self.task = Some(handle);
    }
    fn set_recv(&mut self, recv: Receiver<DBPropMessage>) {
        self.task_recv = Some(recv);
    }
}

impl TuiProp for DBProps {
    fn color(&self, selected: bool) -> ratatui::prelude::Color {
        if selected {
            Color::Blue
        } else {
            Color::LightBlue
        }
    }
    fn select_me() -> (crossterm::event::KeyCode, super::CurrentPane) {
        (KeyCode::Char('b'), CurrentPane::DB)
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
        let healthy = {
            if let Some(db) = &r.database {
                if let Some(thread) = &db.thread {
                    OneOf::Left::<bool, ThrobberState>(thread.client.health().await.is_ok());
                }
            }

            OneOf::Right(ThrobberState::default())
        };

        DBProps {
            healthy,
            task: None,
            task_recv: None,
        }
    }

    fn run(&mut self, state_ref: &mut crate::state::SharedState<'static>) {
        let r = state_ref.0.try_read().expect("failed read lock");
        if let Some(db) = &r.database {
            if let Some(thread) = &db.thread {
                if self.task.is_none() {
                    let thread_clone = thread.client.clone();
                    self.start_task(thread_clone, |client| async move {
                        let healthy = client.health().await.is_ok();
                        Ok(DBPropMessage::DatabaseHealth(healthy))
                    });
                }
            }
        }

        if let Some(task) = self.task.as_ref() {
            if task.is_finished() && self.task_recv.is_some() {
                match self.task_recv.as_mut().unwrap().try_recv() {
                    Ok(mes) => match mes {
                        DBPropMessage::DatabaseHealth(health) => self.healthy = OneOf::Left(health),
                    },
                    Err(TryRecvError::Empty) => {}
                    Err(TryRecvError::Closed) => {
                        self.task_recv = None;
                    }
                }
            }
        }
        if let OneOf::Right(ref mut throbber_state) = self.healthy {
            throbber_state.calc_next();
        }
    }

    fn draw(&mut self, selected: bool, frame: &mut ratatui::Frame, area: ratatui::prelude::Rect) {
        let block = Block::new()
            .title(Line::raw("Database").centered())
            .borders(Borders::ALL)
            .fg(self.color(selected));

        match &mut self.healthy {
            OneOf::Left(status) => {
                let health = if *status {
                    "is healthy"
                } else {
                    "is not healthy!"
                };
                let text = Text::from(Line::from(format!("Database {health}")))
                    .patch_style(Style::new().fg(Color::Green));
                let p = Paragraph::new(text).block(block);
                frame.render_widget(p, area);
            }
            OneOf::Right(ref mut throbber_state) => {
                let throbber = Throbber::default()
                    .label("Awaiting database connection...")
                    .throbber_set(throbber_widgets_tui::WHITE_SQUARE)
                    .use_type(throbber_widgets_tui::WhichUse::Spin);

                frame.render_stateful_widget(throbber, area, throbber_state);
            }
        }
    }
}
