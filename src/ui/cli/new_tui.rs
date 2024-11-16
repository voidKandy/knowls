use std::sync::LazyLock;

use crate::{agents::AgentID, state::SharedState};
use color_eyre::Result;
use crossterm::event::KeyEvent;
use espionox::{
    language_models::completions::streaming::CompletionStreamStatus,
    prelude::{Message, MessageRole, MessageStack},
};
use ratatui::{
    crossterm::{
        self,
        event::{Event, KeyCode, KeyEventKind},
    },
    layout::{Constraint, Flex, Layout, Position},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Text},
    widgets::{Block, Borders, Padding, Paragraph, Wrap},
    DefaultTerminal, Frame,
};
use tokio::sync::mpsc::error::TryRecvError;
use tracing::warn;

enum InputMode {
    Normal,
    Conversing,
    SideBar,
}

struct ChatData {
    agent_id: AgentID,
    input: String,
    character_index: usize,
    input_mode: InputMode,
    messages: MessageStack,
    current_msg_from_agent: Option<String>,
}

pub struct Tui {
    chat_data: ChatData,

    tx: tokio::sync::mpsc::UnboundedSender<TuiEventReq>,
    rx: tokio::sync::mpsc::UnboundedReceiver<TuiEventRes>,
    task: Option<tokio::task::JoinHandle<()>>,
    should_quit: bool,
}

struct TuiEventReqHandler<'state> {
    tx: tokio::sync::mpsc::UnboundedSender<TuiEventRes>,
    rx: tokio::sync::mpsc::UnboundedReceiver<TuiEventReq>,
    state: SharedState<'state>,
}

impl<'state> TuiEventReqHandler<'state> {
    async fn run(mut self) {
        loop {
            match self.rx.try_recv() {
                Ok(event) => {
                    warn!("task received event");
                    self.handle_event(event).await;
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    warn!("channel disconnected, killing task");
                    return;
                }
            }
        }
    }

    async fn handle_event(&mut self, event: TuiEventReq) {
        match event {
            TuiEventReq::PromptModel { id, prompt } => {
                let mut w = self.state.0.try_write().expect("could not get write lock");
                if let Some(agents) = w.agents.as_mut() {
                    if let Some(agent) = agents.get_agent_mut(id) {
                        agent.cache.push(Message::new_user(&prompt));
                        let mut stream_handler = agent.stream_completion().await.unwrap();
                        while let Ok(Some(CompletionStreamStatus::Working(tok))) =
                            stream_handler.receive(Some(agent)).await
                        {
                            self.tx.send(TuiEventRes::AgentStream(Some(tok))).unwrap();
                        }
                        self.tx.send(TuiEventRes::AgentStream(None)).unwrap();
                    }
                }
            }
        }
    }
}

enum TuiEventRes {
    /// If none, the stream has closed, otherwise is a token
    AgentStream(Option<String>),
}

enum TuiEventReq {
    PromptModel { id: AgentID, prompt: String },
}

impl Tui {
    pub fn new(state: SharedState<'static>, agent_id: AgentID) -> Self {
        let (rq_tx, rq_rx) = tokio::sync::mpsc::unbounded_channel::<TuiEventReq>();
        let (rs_tx, rs_rx) = tokio::sync::mpsc::unbounded_channel::<TuiEventRes>();

        let mut messages = MessageStack::init();

        {
            let r = state.0.try_read().unwrap();
            if let Some(agents) = &r.agents {
                if let Some(agent) = agents.get_agent_ref(&agent_id) {
                    messages = agent.cache.clone();
                }
            }
        }

        let handler = TuiEventReqHandler {
            tx: rs_tx,
            rx: rq_rx,
            state,
        };
        let task = tokio::spawn(async move { handler.run().await });

        Self {
            chat_data: ChatData {
                agent_id,
                current_msg_from_agent: None,
                input: String::new(),
                input_mode: InputMode::Normal,
                messages,
                character_index: 0,
            },
            task: Some(task),
            tx: rq_tx,
            rx: rs_rx,
            should_quit: false,
        }
    }

    fn move_cursor_left(&mut self) {
        let cursor_moved_left = self.chat_data.character_index.saturating_sub(1);
        self.chat_data.character_index = self.clamp_cursor(cursor_moved_left);
    }

    fn move_cursor_right(&mut self) {
        let cursor_moved_right = self.chat_data.character_index.saturating_add(1);
        self.chat_data.character_index = self.clamp_cursor(cursor_moved_right);
    }

    fn enter_char(&mut self, new_char: char) {
        let index = self.byte_index();
        self.chat_data.input.insert(index, new_char);
        self.move_cursor_right();
    }

    /// Returns the byte index based on the character position.
    ///
    /// Since each character in a string can be contain multiple bytes, it's necessary to calculate
    /// the byte index based on the index of the character.
    fn byte_index(&self) -> usize {
        self.chat_data
            .input
            .char_indices()
            .map(|(i, _)| i)
            .nth(self.chat_data.character_index)
            .unwrap_or(self.chat_data.input.len())
    }

    fn delete_char(&mut self) {
        let is_not_cursor_leftmost = self.chat_data.character_index != 0;
        if is_not_cursor_leftmost {
            // Method "remove" is not used on the saved text for deleting the selected char.
            // Reason: Using remove on String works on bytes instead of the chars.
            // Using remove would require special care because of char boundaries.

            let current_index = self.chat_data.character_index;
            let from_left_to_current_index = current_index - 1;

            // Getting all characters before the selected character.
            let before_char_to_delete = self
                .chat_data
                .input
                .chars()
                .take(from_left_to_current_index);
            // Getting all characters after selected character.
            let after_char_to_delete = self.chat_data.input.chars().skip(current_index);

            // Put all characters together except the selected one.
            // By leaving the selected one out, it is forgotten and therefore deleted.
            self.chat_data.input = before_char_to_delete.chain(after_char_to_delete).collect();
            self.move_cursor_left();
        }
    }

    fn clamp_cursor(&self, new_cursor_pos: usize) -> usize {
        new_cursor_pos.clamp(0, self.chat_data.input.chars().count())
    }

    fn reset_cursor(&mut self) {
        self.chat_data.character_index = 0;
    }

    fn submit_message(&mut self) {
        let message = Message::new_user(&self.chat_data.input);
        self.chat_data.messages.push(message.clone());
        let message = TuiEventReq::PromptModel {
            id: self.chat_data.agent_id.clone(),
            prompt: self.chat_data.input.drain(..).collect(),
        };

        self.tx.send(message).unwrap();
        // self.get_comp_stream_handler(message).await.unwrap();

        self.chat_data.input.clear();
        self.reset_cursor();
    }

    fn handle_key_event(&mut self, event: KeyEvent) {
        warn!("handling key event: {event:#?}");
        match self.chat_data.input_mode {
            InputMode::Normal => match event.code {
                KeyCode::Char('e') => {
                    self.chat_data.input_mode = InputMode::SideBar;
                }
                KeyCode::Char('c') => {
                    self.chat_data.input_mode = InputMode::Conversing;
                }
                KeyCode::Char('q') => {
                    self.should_quit = true;
                }
                _ => {}
            },
            InputMode::Conversing if event.kind == KeyEventKind::Press => match event.code {
                KeyCode::Enter => {
                    self.submit_message();
                }
                KeyCode::Char(to_insert) => self.enter_char(to_insert),
                KeyCode::Backspace => self.delete_char(),
                KeyCode::Left => self.move_cursor_left(),
                KeyCode::Right => self.move_cursor_right(),
                KeyCode::Esc => self.chat_data.input_mode = InputMode::Normal,
                _ => {}
            },
            InputMode::SideBar if event.kind == KeyEventKind::Press => match event.code {
                // KeyCode::Enter => {
                //     self.submit_message();
                // }
                // KeyCode::Char(to_insert) => self.enter_char(to_insert),
                // KeyCode::Backspace => self.delete_char(),
                // KeyCode::Left => self.move_cursor_left(),
                // KeyCode::Right => self.move_cursor_right(),
                KeyCode::Esc => self.chat_data.input_mode = InputMode::Normal,
                _ => {}
            },
            _ => {}
        }
    }

    pub fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        warn!("running terminal");
        loop {
            terminal.draw(|frame| self.draw(frame))?;
            if self.should_quit {
                return Ok(());
            }
            // warn!("drew frame");
            let event = crossterm::event::read()?;

            if let Event::Key(event) = event {
                self.handle_key_event(event);
            }

            if let Some(task) = self.task.as_ref() {
                if task.is_finished() {
                    self.task = None;
                }
            }

            match self.rx.try_recv() {
                Ok(mes) => match mes {
                    TuiEventRes::AgentStream(tok_opt) => match tok_opt {
                        Some(tok) => match self.chat_data.current_msg_from_agent.as_mut() {
                            Some(ref mut msg) => msg.push_str(&tok),
                            None => self.chat_data.current_msg_from_agent = Some(tok),
                        },
                        None => {
                            self.chat_data.messages.push(Message::new_assistant(
                                &self
                                    .chat_data
                                    .current_msg_from_agent
                                    .take()
                                    .unwrap()
                                    .drain(..)
                                    .collect::<String>(),
                            ));
                        }
                    },
                },
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    warn!("channel disconnected");
                    return Ok(());
                }
            }
        }
    }

    fn draw(&self, frame: &mut Frame) {
        let horizontal =
            Layout::horizontal([Constraint::Percentage(80), Constraint::Percentage(20)]);

        let vertical = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(3),
        ]);

        let [chat_area, sidebar_area] = horizontal.areas(frame.area());
        let [help_area, messages_area, input_area] = vertical.areas(chat_area);

        let block = Block::new()
            .borders(Borders::ALL)
            .fg(match self.chat_data.input_mode {
                InputMode::SideBar => Color::Magenta,
                _ => Color::Blue,
            });

        // for _ in (0..3).into_iter() {
        //
        // }

        // let area = Layout::vertical([Constraint::Percentage(30), Constraint::Percentage(70)])
        //     .flex(Flex::Start)
        //     .split(block);

        frame.render_widget(block, sidebar_area);

        let (msg, style) = match self.chat_data.input_mode {
            InputMode::Normal => (
                vec![
                    "Press ".into(),
                    "q".bold(),
                    " to exit, ".into(),
                    "e".bold(),
                    " →  ".bold(),
                    "c".bold(),
                    " ↓  ".bold(),
                ],
                Style::default()
                    .add_modifier(Modifier::RAPID_BLINK)
                    .fg(Color::LightRed),
            ),
            InputMode::Conversing => (
                vec![
                    "Press ".into(),
                    "Esc".bold(),
                    " to stop conversing, ".into(),
                    "Enter".bold(),
                    " to send message".into(),
                ],
                Style::default().fg(Color::LightRed),
            ),
            InputMode::SideBar => (
                vec![
                    "Press ".into(),
                    "Esc".bold(),
                    " to stop get out, ".into(),
                    // "Enter".bold(),
                    // " to record the message".into(),
                ],
                Style::default().fg(Color::LightRed),
            ),
        };
        let text = Text::from(Line::from(msg)).patch_style(style);
        let help_message = Paragraph::new(text);
        frame.render_widget(help_message, help_area);

        let input = Paragraph::new(self.chat_data.input.as_str())
            .style(match self.chat_data.input_mode {
                InputMode::Conversing => Style::default().fg(Color::Yellow),
                _ => Style::default(),
            })
            .fg(Color::LightCyan)
            .block(Block::bordered().title("Input"));
        frame.render_widget(input, input_area);
        match self.chat_data.input_mode {
            // Hide the cursor. `Frame` does this by default, so we don't need to do anything here

            // Make the cursor visible and ask ratatui to put it at the specified coordinates after
            // rendering
            #[allow(clippy::cast_possible_truncation)]
            InputMode::Conversing => frame.set_cursor_position(Position::new(
                // Draw the cursor at the current position in the input field.
                // This position is can be controlled via the left and right arrow key
                input_area.x + self.chat_data.character_index as u16 + 1,
                // Move one line down, from the border to the input line
                input_area.y + 1,
            )),
            _ => {}
        }

        let mut messages: Vec<(Paragraph, u16)> = self
            .chat_data
            .messages
            .as_ref()
            .iter()
            .map(|m| {
                let block = {
                    let (role, color) = match &m.role {
                        MessageRole::User => ("user", Color::Cyan),
                        MessageRole::System => ("system", Color::Yellow),
                        MessageRole::Assistant => ("assistant", Color::Magenta),
                        MessageRole::Other { alias, .. } => (alias.as_str(), Color::Blue),
                    };
                    Block::new()
                        .title_top(role)
                        .fg(color)
                        .borders(Borders::ALL)
                        .padding(Padding::uniform(1))
                };

                warn!("creatin paragraph for message: {m:#?}");

                (
                    Paragraph::new(format!("{}\n", m.content)).block(block),
                    get_message_height(&m.content) + 2,
                )
            })
            .collect();

        if let Some(current_compl) = &self.chat_data.current_msg_from_agent {
            let p = Paragraph::new(format!("{current_compl}\n"))
                .block(Block::new().title_top("assistant").fg(Color::Magenta));
            messages.push((p, get_message_height(current_compl) + 2));
        }

        // let messages = List::new(messages)
        //     .block(Block::bordered().title("Messages"))
        //     .fg(Color::LightGreen);

        let heights = messages.iter().map(|(_, h)| *h).collect::<Vec<u16>>();
        let areas = Layout::vertical(heights)
            .flex(Flex::Start)
            .split(messages_area);

        for (area, paragraph) in areas
            .into_iter()
            .zip(
                messages
                    .into_iter()
                    .map(|(m, _)| m)
                    .collect::<Vec<Paragraph<'_>>>(),
            )
            .into_iter()
        {
            warn!("rendering paragraph: {paragraph:#?}");
            frame.render_widget(paragraph, *area);
        }
        // for (area, (description, constraints)) in areas.iter().zip(EXAMPLE_DATA.iter()) {
        //     Example::new(constraints, description, flex, spacing).render(*area, buf);
        // }

        // frame.render_widget(messages, messages_area);
    }
}

const SINGLE_CHAR_SIZE: LazyLock<usize> = LazyLock::new(|| Line::from("a").width());
fn get_message_height(
    s: &str,
    // max_width: u16
) -> u16 {
    // let mut count = 0;
    // let line_buf = Line::from(s);
    // let newlines = line_buf.width() / max_width as usize;

    if s.is_empty() {
        0
    } else {
        s.split('\n').count() as u16
    }
}
