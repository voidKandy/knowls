use super::user_input::UserInputPopupConfig;
use super::{super::action::Action, user_input::UserInputPopup};
use super::{Component, PageComponent, PageComponentAction, PageComponentBindings};
use crate::impl_into_u32;
use crate::tui::config::parse_key_event;
use crate::{database::models::Knowledge, state::State, tui::config::Config};
use color_eyre::Result;
use crossterm::event::KeyCode;
use ratatui::style::Style;
use ratatui::symbols::scrollbar;
use ratatui::widgets::{Clear, Scrollbar, ScrollbarOrientation, ScrollbarState, Widget, Wrap};
use ratatui::{
    layout::{Constraint, Flex, Layout, Rect},
    style::{Color, Stylize},
    text::Line,
    widgets::{Block, Borders, HighlightSpacing, List, ListItem, Paragraph},
    Frame,
};
use std::collections::HashMap;
use std::{path::PathBuf, str::FromStr};
use surrealdb::RecordId;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug)]
pub struct KnowledgeComponent {
    command_tx: Option<UnboundedSender<Action>>,
    config: Config,
    bindings: PageComponentBindings,
    knowledge: Vec<(RecordId, Knowledge)>,
    current_knowledge: Option<usize>,
    popup: Option<Popup>,
}

#[derive(Debug)]
enum Popup {
    AddKnowledge(AddKnowledgePopup),
    ViewKnowledge(ViewKnowledgePopup),
}

impl From<AddKnowledgePopup> for Popup {
    fn from(value: AddKnowledgePopup) -> Self {
        Self::AddKnowledge(value)
    }
}
impl From<ViewKnowledgePopup> for Popup {
    fn from(value: ViewKnowledgePopup) -> Self {
        Self::ViewKnowledge(value)
    }
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
        let path = PathBuf::from_str(&input)?.canonicalize()?;
        tracing::warn!("got knowledge path: {path:?}");
        let content = std::fs::read_to_string(&path)?;
        let knowledge = Knowledge::new(path, content);
        Ok(Action::InsertKnowledge(knowledge))
    }
}

#[derive(Debug)]
struct ViewKnowledgePopup {
    name: String,
    content: String,
    vertical_scroll: usize,
    vertical_scroll_state: ScrollbarState,
}

impl Component for ViewKnowledgePopup {
    fn handle_key_event(&mut self, key: crossterm::event::KeyEvent) -> Result<Option<Action>> {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                self.vertical_scroll = self.vertical_scroll.saturating_add(1);
                self.vertical_scroll_state =
                    self.vertical_scroll_state.position(self.vertical_scroll);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.vertical_scroll = self.vertical_scroll.saturating_sub(1);
                self.vertical_scroll_state =
                    self.vertical_scroll_state.position(self.vertical_scroll);
            }

            _ => {}
        }
        Ok(None)
    }
    fn draw(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        // ensure that all cells under the popup are cleared to avoid leaking content
        // We add a little padding
        let buf = frame.buffer_mut();
        Clear.render(area, buf);
        let block = Block::new()
            .title(self.name.clone())
            .title_style(Style::new().yellow())
            .style(Style::new())
            .borders(Borders::ALL);
        Paragraph::new(self.content.to_owned())
            .wrap(Wrap { trim: true })
            .scroll((self.vertical_scroll as u16, 0))
            .block(block)
            .render(area, buf);
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .symbols(scrollbar::VERTICAL)
                .begin_symbol(None)
                .track_symbol(None)
                .end_symbol(None),
            area,
            &mut self.vertical_scroll_state,
        );
        Ok(())
    }
}

impl From<&State> for KnowledgeComponent {
    fn from(value: &State) -> Self {
        Self {
            command_tx: None,
            config: Config::default(),
            current_knowledge: None,
            bindings: Self::action_bindings(),
            popup: None,
            knowledge: value
                .knowledge
                .iter()
                .map(|(id, knowledge)| (id.to_owned(), knowledge.clone()))
                .collect(),
        }
    }
}

impl KnowledgeComponent {
    fn action_bindings() -> PageComponentBindings {
        let mut map = HashMap::new();
        let move_up =
            PageComponentAction::new(KnowledgeAction::MoveUp, "move to the next knowledge item");
        let move_down = PageComponentAction::new(
            KnowledgeAction::MoveDown,
            "move to the previous knowledge item",
        );
        let close_popup = PageComponentAction::new(
            KnowledgeAction::ClosePopup,
            "Close the currently opened popup",
        );
        let view_knowledge = PageComponentAction::new(
            KnowledgeAction::ViewKnowledge,
            "View the currently highlighted knowledge",
        );
        let add_knowledge = PageComponentAction::new(
            KnowledgeAction::AddKnowledge,
            "Open the add knowledge popup",
        );
        let open_in_editor = PageComponentAction::new(
            KnowledgeAction::OpenInEditor,
            "Open currently highlighted knowledge in editor",
        );

        map.insert(vec![parse_key_event("j").unwrap()], move_down);
        map.insert(vec![parse_key_event("k").unwrap()], move_up);
        map.insert(vec![parse_key_event("esc").unwrap()], close_popup);
        map.insert(vec![parse_key_event("v").unwrap()], view_knowledge);
        map.insert(vec![parse_key_event("a").unwrap()], add_knowledge);
        map.insert(vec![parse_key_event("o").unwrap()], open_in_editor);

        map
    }
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

#[derive(Debug, strum::Display, Clone, Copy)]
enum KnowledgeAction {
    MoveUp,
    MoveDown,
    ClosePopup,
    ViewKnowledge,
    AddKnowledge,
    OpenInEditor,
}
impl_into_u32!(KnowledgeAction);

impl PageComponent for KnowledgeComponent {
    fn id(&self) -> super::ComponentId {
        "knowledge".into()
    }
    fn selection_keys(&self) -> Vec<crossterm::event::KeyEvent> {
        vec![parse_key_event("k").unwrap()]
    }
    fn bindings(&self) -> &PageComponentBindings {
        &self.bindings
    }
    fn handle_action(
        &mut self,
        action: &PageComponentAction,
    ) -> Result<Option<crate::tui::action::Action>> {
        tracing::warn!("knowledge component handling action: {action:#?}");
        match action.id {
            _ if action.id == KnowledgeAction::MoveUp as u32 => {
                self.cycle_knowledge(true);
            }
            _ if action.id == KnowledgeAction::MoveDown as u32 => {
                self.cycle_knowledge(true);
            }
            _ if action.id == KnowledgeAction::ClosePopup as u32 && self.popup.is_some() => {
                self.popup = None;
            }
            _ if action.id == KnowledgeAction::ViewKnowledge as u32 && self.popup.is_none() => {
                tracing::warn!("opening view popup");
                if let Some(i) = self.current_knowledge {
                    let current_knowledge = &self.knowledge[i];

                    self.popup = Some(
                        ViewKnowledgePopup {
                            vertical_scroll_state: ScrollbarState::new(
                                current_knowledge.1.content.lines().count(),
                            ),
                            name: current_knowledge.1.kid.to_string(),
                            content: current_knowledge.1.content.to_owned(),
                            vertical_scroll: 0,
                        }
                        .into(),
                    );
                }
            }
            _ if action.id == KnowledgeAction::AddKnowledge as u32 && self.popup.is_none() => {
                tracing::warn!("opening add popup");
                // open add knowledge popup
                self.popup = Some(
                    AddKnowledgePopup::new_with_title("Type a Path to a Knowledge Source").into(),
                );
            }

            _ if action.id == KnowledgeAction::OpenInEditor as u32 && self.popup.is_some() => {
                if let Some(i) = self.current_knowledge {
                    let content = self.knowledge.get(i).as_ref().unwrap().1.content.to_owned();
                    return Ok(Some(Action::OpenEditor(content)));
                }
            }

            _ => {}
        }
        Ok(None)
    }
}

impl Component for KnowledgeComponent {
    fn handle_key_event(&mut self, key: crossterm::event::KeyEvent) -> Result<Option<Action>> {
        tracing::warn!("handling key event: {:#?}", key.code);
        let action_opt = self.bindings.remove(&vec![key]);
        tracing::warn!("got action: {action_opt:#?}");
        let mut return_action = None;
        match self.popup.as_mut() {
            Some(popup)
                if action_opt
                    .as_ref()
                    .is_none_or(|k| k.id != KnowledgeAction::ClosePopup as u32) =>
            {
                match popup {
                    Popup::AddKnowledge(ref mut p) => return p.handle_key_event(key),
                    Popup::ViewKnowledge(ref mut p) => return p.handle_key_event(key),
                }
            }
            None | Some(_) if action_opt.is_some() => {
                let action = action_opt.as_ref().unwrap();
                return_action = self.handle_action(action)?;
            }
            _ => {
                tracing::warn!("key event {key:#?} triggered nothing");
            }
        }

        if let Some(action) = action_opt {
            self.bindings.insert(vec![key], action);
        }

        Ok(return_action)
    }

    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
        self.command_tx = Some(tx);
        Ok(())
    }
    fn register_config_handler(&mut self, config: Config) -> Result<()> {
        self.config = config;
        Ok(())
    }

    fn update(&mut self, state: &State, action: Action) -> Result<Option<Action>> {
        match action {
            // Action::Tick => self.app_tick()?,
            // Action::Render => self.render_tick()?,
            _ => {}
        };

        let knowledge = state
            .knowledge
            .iter()
            .map(|(id, knowledge)| (id.to_owned(), knowledge.clone()))
            .collect();
        self.knowledge = knowledge;

        if let Some(popup) = self.popup.as_mut() {
            if let Popup::AddKnowledge(p) = popup {
                p.update(state, action)?;
            }
        }
        Ok(None)
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        let [header, body] =
            Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(area);
        let paragraph = Paragraph::new("=== Knowledge ===")
            .style(Style::new().green().bold())
            .centered();
        frame.render_widget(paragraph, header);

        let block = Block::new()
            .title(Line::raw("Entries"))
            .borders(Borders::ALL);

        let items: Vec<ListItem> = self
            .knowledge
            .iter()
            .enumerate()
            .map(|(i, (_id, knowledge))| {
                let mut color = Color::Cyan;

                if self
                    .current_knowledge
                    .as_ref()
                    .is_some_and(|current| &i == current)
                {
                    color = Color::LightMagenta;
                }

                ListItem::from(knowledge.kid.to_string()).bg(color)
            })
            .collect();

        // Create a List from all list items and highlight the currently selected one
        let list = List::new(items)
            .block(block)
            // .highlight_style(SELECTED_STYLE)
            .highlight_symbol(">")
            .highlight_spacing(HighlightSpacing::Always);
        frame.render_widget(list, body);

        if let Some(popup) = self.popup.as_mut() {
            match popup {
                Popup::AddKnowledge(p) => {
                    let area = AddKnowledge::popup_area(area, 60, 20);
                    p.draw(frame, area)?;
                }
                Popup::ViewKnowledge(p) => {
                    p.draw(frame, area)?;
                }
            }
        }
        Ok(())
    }
}
