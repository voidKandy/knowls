use std::marker::PhantomData;

use anyhow::anyhow;
use color_eyre::{owo_colors::colors::css::LightCyan, Result};
use crossterm::event::KeyEvent;
use espionox::{
    language_models::completions::streaming::{CompletionStreamStatus, ProviderStreamHandler},
    prelude::{Message, MessageStack},
};
use futures::{FutureExt, StreamExt};
use ratatui::{
    crossterm::{
        self,
        event::{self, Event, KeyCode, KeyEventKind},
    },
    layout::{Constraint, Layout, Position},
    prelude::Backend,
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, List, ListItem, Paragraph},
    DefaultTerminal, Frame, Terminal,
};
use tokio::sync::RwLockWriteGuard;
use tracing::warn;

use crate::{
    agents::AgentID,
    state::{LspState, SharedState},
};

// pub async fn run_terminal(agent_id: AgentID) -> Result<()> {
//     let terminal = ratatui::init();
//     let app = Tui::new(agent_id);
//     let res = app.run(terminal).await;
//     ratatui::restore();
//     res
// }

/// Tui holds the state of the application
pub struct Tui<'a> {
    /// Current value of the input box
    input: String,
    /// Position of cursor in the editor area.
    character_index: usize,
    /// Current input mode
    input_mode: InputMode,
    /// History of recorded messages
    messages: MessageStack,

    agent_id: AgentID,
    completion_handler: UiCompletionHandler,
    state_lock_opt: Option<RwLockWriteGuard<'a, LspState<'static>>>,
}

enum AppEvent {
    Key(KeyEvent),
    Error,
    Tick,
}

struct EventHandler {
    _tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    rx: tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
    task: Option<tokio::task::JoinHandle<()>>,
}

impl EventHandler {
    pub fn new() -> Self {
        let tick_rate = std::time::Duration::from_millis(250);

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<AppEvent>();
        let _tx = tx.clone();

        let task = tokio::spawn(async move {
            let mut reader = crossterm::event::EventStream::new();
            let mut interval = tokio::time::interval(tick_rate);
            loop {
                let delay = interval.tick();
                let crossterm_event = reader.next().fuse();
                tokio::select! {
                  maybe_event = crossterm_event => {
                    match maybe_event {
                      Some(Ok(evt)) => {
                        match evt {
                          crossterm::event::Event::Key(key) => {
                            if key.kind == crossterm::event::KeyEventKind::Press {
                              tx.send(AppEvent::Key(key)).unwrap();
                            }
                          },
                          _ => {},
                        }
                      }
                      Some(Err(_)) => {
                        tx.send(AppEvent::Error).unwrap();
                      }
                      None => {},
                    }
                  },
                  _ = delay => {
                      tx.send(AppEvent::Tick).unwrap();
                  },
                }
            }
        });

        Self {
            _tx,
            rx,
            task: Some(task),
        }
    }
    async fn next(&mut self) -> color_eyre::Result<AppEvent> {
        self.rx
            .recv()
            .await
            .ok_or(color_eyre::eyre::eyre!("unable to get next event"))
    }
}

#[derive(Debug, Default)]
struct UiCompletionHandler {
    stream_handler: Option<ProviderStreamHandler>,
    current_completion: Option<String>,
}

enum InputMode {
    Normal,
    Editing,
}

impl<'a, 'l> Tui<'a> {
    pub fn new(agent_id: AgentID) -> Self {
        Self {
            input: String::new(),
            input_mode: InputMode::Normal,
            messages: Vec::new().into(),
            character_index: 0,
            agent_id,
            completion_handler: UiCompletionHandler::default(),
            state_lock_opt: None,
        }
    }

    pub fn state_lock(&mut self, w: RwLockWriteGuard<'a, LspState<'static>>) {
        self.state_lock_opt = Some(w);
    }

    fn move_cursor_left(&mut self) {
        let cursor_moved_left = self.character_index.saturating_sub(1);
        self.character_index = self.clamp_cursor(cursor_moved_left);
    }

    fn move_cursor_right(&mut self) {
        let cursor_moved_right = self.character_index.saturating_add(1);
        self.character_index = self.clamp_cursor(cursor_moved_right);
    }

    fn enter_char(&mut self, new_char: char) {
        let index = self.byte_index();
        self.input.insert(index, new_char);
        self.move_cursor_right();
    }

    /// Returns the byte index based on the character position.
    ///
    /// Since each character in a string can be contain multiple bytes, it's necessary to calculate
    /// the byte index based on the index of the character.
    fn byte_index(&self) -> usize {
        self.input
            .char_indices()
            .map(|(i, _)| i)
            .nth(self.character_index)
            .unwrap_or(self.input.len())
    }

    fn delete_char(&mut self) {
        let is_not_cursor_leftmost = self.character_index != 0;
        if is_not_cursor_leftmost {
            // Method "remove" is not used on the saved text for deleting the selected char.
            // Reason: Using remove on String works on bytes instead of the chars.
            // Using remove would require special care because of char boundaries.

            let current_index = self.character_index;
            let from_left_to_current_index = current_index - 1;

            // Getting all characters before the selected character.
            let before_char_to_delete = self.input.chars().take(from_left_to_current_index);
            // Getting all characters after selected character.
            let after_char_to_delete = self.input.chars().skip(current_index);

            // Put all characters together except the selected one.
            // By leaving the selected one out, it is forgotten and therefore deleted.
            self.input = before_char_to_delete.chain(after_char_to_delete).collect();
            self.move_cursor_left();
        }
    }

    fn clamp_cursor(&self, new_cursor_pos: usize) -> usize {
        new_cursor_pos.clamp(0, self.input.chars().count())
    }

    fn reset_cursor(&mut self) {
        self.character_index = 0;
    }

    async fn submit_message(&mut self) {
        let message = Message::new_user(&self.input);
        self.messages.push(message.clone());
        self.get_comp_stream_handler(message).await.unwrap();

        self.input.clear();
        self.reset_cursor();
    }

    async fn get_comp_stream_handler(&mut self, message: Message) -> anyhow::Result<()> {
        if let Some(agents) = self
            .state_lock_opt
            .as_mut()
            .ok_or(anyhow::anyhow!("No state lock"))?
            .agents
            .as_mut()
        {
            if let Some(agent) = agents.get_agent_mut(&self.agent_id) {
                agent.cache.push(message);
                let handler = agent
                    .stream_completion()
                    .await
                    .expect("failed to get stream handler");
                self.completion_handler.stream_handler = Some(handler);
                return Ok(());
            } else {
                return Err(anyhow!("got no agent"));
            }
        }
        Err(anyhow::anyhow!("No agents in state"))
    }

    async fn progress_comp_stream(&mut self) -> anyhow::Result<()> {
        if self.completion_handler.stream_handler.is_none() {
            warn!("tried to progress comp stream that is none");
            return Ok(());
        }
        if let Some(agents) = self
            .state_lock_opt
            .as_mut()
            .ok_or(anyhow::anyhow!("No state lock"))?
            .agents
            .as_mut()
        {
            if let Some(agent) = agents.get_agent_mut(&self.agent_id) {
                if let Ok(Some(status)) = self
                    .completion_handler
                    .stream_handler
                    .as_mut()
                    .unwrap()
                    .receive(Some(agent))
                    .await
                {
                    match status {
                        CompletionStreamStatus::Finished => {
                            let message = Message::new_assistant(
                                &self.completion_handler.current_completion.take().unwrap(),
                            );
                            self.messages.push(message);
                        }
                        CompletionStreamStatus::Working(tok) => {
                            match self.completion_handler.current_completion.as_mut() {
                                None => self.completion_handler.current_completion = Some(tok),
                                Some(content) => content.push_str(&tok),
                            }
                        }
                    }
                }
            } else {
                return Err(anyhow!("got no agent"));
            }
        }
        Err(anyhow::anyhow!("No agents in state"))
    }

    pub async fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        warn!("running terminal");
        loop {
            terminal.draw(|frame| self.draw(frame))?;
            warn!("drew frame");

            if let Event::Key(key) = event::read()? {
                warn!("handling key event: {key:#?}");
                match self.input_mode {
                    InputMode::Normal => match key.code {
                        KeyCode::Char('e') => {
                            self.input_mode = InputMode::Editing;
                        }
                        KeyCode::Char('q') => {
                            return Ok(());
                        }
                        _ => {}
                    },
                    InputMode::Editing if key.kind == KeyEventKind::Press => match key.code {
                        KeyCode::Enter => self.submit_message().await,
                        KeyCode::Char(to_insert) => self.enter_char(to_insert),
                        KeyCode::Backspace => self.delete_char(),
                        KeyCode::Left => self.move_cursor_left(),
                        KeyCode::Right => self.move_cursor_right(),
                        KeyCode::Esc => self.input_mode = InputMode::Normal,
                        _ => {}
                    },
                    InputMode::Editing => {}
                }
            }
            // self.progress_comp_stream().await.unwrap();
        }
    }

    fn draw(&self, frame: &mut Frame) {
        let vertical = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(3),
        ]);
        let [help_area, messages_area, input_area] = vertical.areas(frame.area());

        let (msg, style) = match self.input_mode {
            InputMode::Normal => (
                vec![
                    "Press ".into(),
                    "q".bold(),
                    " to exit, ".into(),
                    "e".bold(),
                    " to start editing.".bold(),
                ],
                Style::default()
                    .add_modifier(Modifier::RAPID_BLINK)
                    .fg(Color::LightRed),
            ),
            InputMode::Editing => (
                vec![
                    "Press ".into(),
                    "Esc".bold(),
                    " to stop editing, ".into(),
                    "Enter".bold(),
                    " to record the message".into(),
                ],
                Style::default().fg(Color::LightRed),
            ),
        };
        let text = Text::from(Line::from(msg)).patch_style(style);
        let help_message = Paragraph::new(text);
        frame.render_widget(help_message, help_area);

        let input = Paragraph::new(self.input.as_str())
            .style(match self.input_mode {
                InputMode::Normal => Style::default(),
                InputMode::Editing => Style::default().fg(Color::Yellow),
            })
            .fg(Color::LightCyan)
            .block(Block::bordered().title("Input"));
        frame.render_widget(input, input_area);
        match self.input_mode {
            // Hide the cursor. `Frame` does this by default, so we don't need to do anything here
            InputMode::Normal => {}

            // Make the cursor visible and ask ratatui to put it at the specified coordinates after
            // rendering
            #[allow(clippy::cast_possible_truncation)]
            InputMode::Editing => frame.set_cursor_position(Position::new(
                // Draw the cursor at the current position in the input field.
                // This position is can be controlled via the left and right arrow key
                input_area.x + self.character_index as u16 + 1,
                // Move one line down, from the border to the input line
                input_area.y + 1,
            )),
        }

        let mut messages: Vec<ListItem> = self
            .messages
            .as_ref()
            .iter()
            .map(|m| {
                let content =
                    Line::from(Span::raw(format!("{}: {m}", m.role.actual().to_string())));
                ListItem::new(content)
            })
            .collect();

        // if let Some(current_compl) = &self.completion_handler.current_completion {
        //     messages.push(ListItem::new(Line::from(Span::raw(format!(
        //         "....: {current_compl}"
        //     )))));
        // }

        let messages = List::new(messages)
            .block(Block::bordered().title("Messages"))
            .fg(Color::LightGreen);

        frame.render_widget(messages, messages_area);
    }
}
