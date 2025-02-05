use std::{
    marker::PhantomData,
    path::PathBuf,
    str::FromStr,
    time::{Duration, Instant},
};

use color_eyre::owo_colors::OwoColorize;
use crossterm::event::{KeyCode, KeyEvent};
use knowls::{MainErr, MainResult};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Flex, Layout, Rect},
    style::{Color, Style, Stylize},
    text::Line,
    widgets::{Block, Borders, Clear, Paragraph, Widget, WidgetRef, Wrap},
};
use tokio::sync::mpsc::UnboundedSender;

use crate::{database::models::Knowledge, tui::action::Action};

use super::Component;

#[derive(Debug, Clone)]
enum InputPopupStatus {
    Idle,
    Error { timestamp: Instant, err_msg: String },
    Success(Instant),
}

const STATUS_RESET_DURATION: Duration = Duration::from_secs(2);

#[derive(Debug, Clone)]
pub struct UserInputPopup<A> {
    title: String,
    /// Current value of the input box
    input: String,
    status: InputPopupStatus,
    /// Position of cursor in the editor area.
    character_index: usize,
    /// Action to trigger when submitting the text
    _phantom: PhantomData<A>,
    // action_trigger_fn: Box<dyn FnMut(String) -> MainResult<Action>>,
}

pub trait UserInputPopupConfig {
    fn trigger_action_from_input(input: String) -> color_eyre::Result<Action>;
    fn popup_area(area: Rect, percent_x: u16, percent_y: u16) -> Rect;
}

impl<A: UserInputPopupConfig> Default for UserInputPopup<A> {
    fn default() -> Self {
        Self {
            title: String::from("User Input"),
            input: String::new(),
            status: InputPopupStatus::Idle,
            character_index: 0,
            _phantom: PhantomData,
        }
    }
}

impl<A: UserInputPopupConfig> UserInputPopup<A> {
    pub fn new_with_title(title: &str) -> Self {
        UserInputPopup {
            title: String::from(title),
            ..Default::default()
        }
    }
    /// helper function to create a centered rect using up certain percentage of the available rect `r`

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

    fn submit(&mut self) -> color_eyre::Result<Option<Action>> {
        match A::trigger_action_from_input(self.input.clone()) {
            Ok(action) => {
                self.status = InputPopupStatus::Success(Instant::now());
                Ok(Some(action))
            }
            Err(err) => {
                self.status = InputPopupStatus::Error {
                    timestamp: Instant::now(),
                    err_msg: err.to_string(),
                };
                Ok(None)
            }
        }
    }

    pub fn handle_keyevent(&mut self, key: KeyEvent) -> color_eyre::Result<Option<Action>> {
        match key.code {
            KeyCode::Enter => {
                return self.submit();
            }
            KeyCode::Char(to_insert) => self.enter_char(to_insert),
            KeyCode::Backspace => self.delete_char(),
            KeyCode::Left => self.move_cursor_left(),
            KeyCode::Right => self.move_cursor_right(),
            // KeyCode::Esc => self.input_mode = InputMode::Normal,
            _ => {}
        }
        Ok(None)
    }
}

impl<A: UserInputPopupConfig> Component for UserInputPopup<A> {
    fn update(&mut self, action: Action) -> color_eyre::eyre::Result<Option<Action>> {
        match action {
            Action::Tick => {
                if let Some(instant) = match self.status {
                    InputPopupStatus::Error { timestamp, .. } => Some(timestamp),
                    InputPopupStatus::Success(inst) => Some(inst),
                    _ => None,
                } {
                    if instant.elapsed() >= STATUS_RESET_DURATION {
                        self.status = InputPopupStatus::Idle;
                    }
                }
            }
            _ => {}
        }
        Ok(None)
    }

    fn draw(&mut self, frame: &mut ratatui::Frame, area: Rect) -> color_eyre::eyre::Result<()> {
        // ensure that all cells under the popup are cleared to avoid leaking content
        // We add a little padding
        let clear_area = {
            let mut a = area.clone();
            a.x += a.x / 10;
            a.y += a.y / 10;
            a
        };
        let buf = frame.buffer_mut();
        Clear.render(clear_area, buf);
        let (title_bottom, title_color) = match &self.status {
            InputPopupStatus::Idle => (" Exit <Esc> - Subit <Enter> ".to_string(), Color::Gray),
            InputPopupStatus::Error { err_msg, .. } => {
                tracing::error!("input popup submit failed: {err_msg:#?}");
                (err_msg.to_owned(), Color::Red)
            }
            InputPopupStatus::Success(_) => (String::from("Added knowledge path"), Color::Green),
        };
        let block = Block::new()
            .title(self.title.clone())
            .title_style(title_color)
            .title_bottom(title_bottom)
            .borders(Borders::ALL);
        Paragraph::new(self.input.to_owned())
            .wrap(Wrap { trim: true })
            .block(block)
            .render(area, buf);
        Ok(())
    }
}
