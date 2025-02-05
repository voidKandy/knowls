use std::{marker::PhantomData, path::PathBuf, str::FromStr};

use crossterm::event::{KeyCode, KeyEvent};
use knowls::{MainErr, MainResult};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Flex, Layout, Rect},
    style::{Color, Style, Stylize},
    text::Line,
    widgets::{Block, Borders, Clear, Paragraph, Widget, WidgetRef, Wrap},
};

use crate::{database::models::Knowledge, tui::action::Action};

#[derive(Debug, Clone)]
pub struct UserInputPopup<A> {
    /// Current value of the input box
    input: String,
    status: Option<(String, Color)>,
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
            input: String::new(),
            status: None,
            character_index: 0,
            _phantom: PhantomData,
        }
    }
}

impl<A: UserInputPopupConfig> UserInputPopup<A> {
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

    fn submit(&mut self) {
        match PathBuf::from_str(&self.input) {
            Ok(_path) => {
                self.status = Some((String::from("Added knowledge path"), Color::Green));
            }
            Err(e) => self.status = Some((format!("Did not pass valid path: {e:#?}"), Color::Red)),
        }
    }

    pub fn handle_keyevent(&mut self, key: KeyEvent) -> color_eyre::Result<Option<Action>> {
        match key.code {
            KeyCode::Enter => {
                self.submit();
                let action = A::trigger_action_from_input(self.input.clone())?;
                return Ok(Some(action));
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

impl<A: UserInputPopupConfig> WidgetRef for UserInputPopup<A> {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        // ensure that all cells under the popup are cleared to avoid leaking content
        // We add a little padding
        let clear_area = {
            let mut a = area.clone();
            a.x += a.x / 10;
            a.y += a.y / 10;
            a
        };
        Clear.render(clear_area, buf);
        let block = Block::new()
            .title("Add Knowlege")
            .title_style(Style::new().yellow())
            .title_bottom(" Exit <Esc> - Subit <Enter> ")
            .borders(Borders::ALL);
        Paragraph::new(self.input.to_owned())
            .wrap(Wrap { trim: true })
            .block(block)
            .render(area, buf);
        if let Some((status, color)) = self.status.as_ref() {
            Line::styled(status, color.to_owned());
        }
    }
}
