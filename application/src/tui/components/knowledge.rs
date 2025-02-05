use std::{path::PathBuf, process::Command, str::FromStr};

use super::user_input::UserInputPopupConfig;
use super::{super::action::Action, user_input::UserInputPopup};
use super::{Component, PageComponent};
use crate::{
    database::models::Knowledge,
    state::State,
    tui::config::{key_event_to_string, Config},
};
use color_eyre::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use knowls::MainResult;
use ratatui::widgets::WidgetRef;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Flex, Layout, Rect},
    prelude::CrosstermBackend,
    style::{Color, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, HighlightSpacing, List, ListItem, Paragraph, Widget, Wrap},
    Frame,
};
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug)]
pub struct KnowledgeComponent {
    command_tx: Option<UnboundedSender<Action>>,
    config: Config,
    // String is ID of the Knowledge
    knowledge: Vec<(String, Knowledge)>,
    current_knowledge: Option<usize>,
    add_knowledge_popup: Option<AddKnowledgePopup>,
}

type AddKnowledgePopup = UserInputPopup<AddKnowledge>;
#[derive(Debug)]
struct AddKnowledge;
impl UserInputPopupConfig for AddKnowledge {
    fn popup_area(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
        let vertical = Layout::vertical([Constraint::Percentage(percent_y)]).flex(Flex::Center);
        let horizontal = Layout::horizontal([Constraint::Percentage(percent_x)]).flex(Flex::Center);
        let [area] = vertical.areas(area);
        let [area] = horizontal.areas(area);
        area
    }
    fn trigger_action_from_input(input: String) -> color_eyre::Result<Action> {
        let path = PathBuf::from_str(&input)?;
        let knowledge = Knowledge {
            id: input,
            content: std::fs::read_to_string(&path)?,
        };
        Ok(Action::InsertKnowledge(knowledge))
    }
}

impl From<&State> for KnowledgeComponent {
    fn from(value: &State) -> Self {
        Self {
            command_tx: None,
            config: Config::default(),
            current_knowledge: None,
            add_knowledge_popup: None,
            knowledge: value
                .knowledge
                .iter()
                .map(|(id, knowledge)| (id.to_string(), knowledge.clone()))
                .collect(),
        }
    }
}

impl KnowledgeComponent {
    fn cycle_knowledge(&mut self, asc: bool) {
        if self.knowledge.is_empty() {
            return;
        }
        if asc {
            match self.current_knowledge {
                Some(ref mut n) => {
                    if *n == 0 {
                        *n = self.knowledge.len() - 1
                    } else {
                        *n -= 1
                    }
                }
                None => {
                    self.current_knowledge = Some(self.knowledge.len() - 1);
                }
            }
        } else {
            match self.current_knowledge {
                Some(ref mut n) => {
                    if *n >= self.knowledge.len() {
                        *n = 0
                    } else {
                        *n += 1
                    }
                }
                None => {
                    self.current_knowledge = Some(0);
                }
            }
        }
    }
}

impl PageComponent for KnowledgeComponent {
    fn position(&self) -> super::ComponentPosition {
        super::ComponentPosition::Body {
            id: "knowledge".into(),
            selection_keys: vec!['k'],
        }
    }
}

impl Component for KnowledgeComponent {
    fn handle_key_event(&mut self, key: crossterm::event::KeyEvent) -> Result<Option<Action>> {
        match key.code {
            code if self.add_knowledge_popup.is_some() => {
                let popup = self.add_knowledge_popup.as_mut().unwrap();
                match code {
                    // close popup
                    KeyCode::Esc => {
                        self.add_knowledge_popup = None;
                        return Ok(None);
                    }
                    _ if key.kind == KeyEventKind::Press => return popup.handle_keyevent(key),
                    _ => {}
                }
            }
            KeyCode::Char('k') => {
                self.cycle_knowledge(true);
            }
            KeyCode::Char('j') => {
                self.cycle_knowledge(false);
            }

            KeyCode::Char('o') => {
                if let Some(i) = self.current_knowledge {
                    let content = self.knowledge.get(i).as_ref().unwrap().1.content.to_owned();
                    return Ok(Some(Action::OpenEditor(content)));
                }
            }

            KeyCode::Char('a') => {
                // open add knowledge popup
                self.add_knowledge_popup = Some(AddKnowledgePopup::new_with_title(
                    "Type a Path to a Knowledge Source",
                ));
            }

            _ => {}
        }
        Ok(None)
    }

    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
        self.command_tx = Some(tx);
        Ok(())
    }
    fn register_config_handler(&mut self, config: Config) -> Result<()> {
        self.config = config;
        Ok(())
    }

    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            // Action::Tick => self.app_tick()?,
            // Action::Render => self.render_tick()?,
            _ => {}
        };
        if let Some(popup) = self.add_knowledge_popup.as_mut() {
            popup.update(action)?;
        }
        Ok(None)
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        let [header, body] =
            Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(area);
        let paragraph = Paragraph::new("=== Knowledge ===").left_aligned();
        frame.render_widget(paragraph, header);

        let block = Block::new()
            .title(Line::raw("Entries").centered())
            .borders(Borders::ALL);

        let items: Vec<ListItem> = self
            .knowledge
            .iter()
            .enumerate()
            .map(|(i, (id, _knowledge))| {
                let mut color = Color::Cyan;

                if self
                    .current_knowledge
                    .as_ref()
                    .is_some_and(|current| &i == current)
                {
                    color = Color::LightMagenta;
                }

                ListItem::from(id.to_owned()).bg(color)
            })
            .collect();

        // Create a List from all list items and highlight the currently selected one
        let list = List::new(items)
            .block(block)
            // .highlight_style(SELECTED_STYLE)
            .highlight_symbol(">")
            .highlight_spacing(HighlightSpacing::Always);
        frame.render_widget(list, body);

        if let Some(popup) = self.add_knowledge_popup.as_mut() {
            let area = AddKnowledge::popup_area(area, 60, 20);
            popup.draw(frame, area)?;
        }
        Ok(())
    }
}
