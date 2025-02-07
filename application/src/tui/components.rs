use std::{collections::HashMap, sync::LazyLock};

use color_eyre::Result;
use crossterm::event::{KeyEvent, MouseEvent};
use ratatui::{
    layout::{Constraint, Layout, Rect, Size},
    Frame,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tokio::sync::mpsc::UnboundedSender;

use crate::state::State;

use super::{
    action::Action,
    config::{Config, KeyBindings},
    tui::Event,
};

pub mod connections;
pub mod database;
pub mod fps;
pub mod help;
pub mod home;
pub mod knowledge;
pub mod user_input;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct ComponentId(String);

impl AsRef<str> for ComponentId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<&str> for ComponentId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

#[derive(Debug, Clone)]
pub enum ComponentPosition {
    Header(ComponentId),
    Popup(ComponentId),
    Body {
        id: ComponentId,
        selection_keys: Vec<char>,
    },
    SideBar(ComponentId),
}

impl ComponentPosition {
    pub fn id(&self) -> &ComponentId {
        match self {
            Self::Header(id) => id,
            Self::Popup(id) => id,
            Self::Body { id, .. } => id,
            Self::SideBar(id) => id,
        }
    }
}
/// Defines the header and body areas
pub const OUTER_VERTICAL_LAYOUT: LazyLock<Layout> =
    LazyLock::new(|| Layout::vertical([Constraint::Percentage(5), Constraint::Percentage(95)]));
/// Splits up the body area
pub const BODY_LAYOUT: LazyLock<Layout> =
    LazyLock::new(|| Layout::horizontal([Constraint::Percentage(75), Constraint::Percentage(25)]));

/// Page component actions are actions specific to page components
/// it's easy to just give page components a function to do something on a key event,
/// this is how we make that `observable` by the `Help` component
#[derive(Clone, Debug, serde::Deserialize)]
pub struct PageComponentAction {
    pub id: u32,
    pub name: String,
    pub description: String,
}

#[macro_export]
macro_rules! impl_into_u32 {
    ($enum:ident) => {
        impl Into<u32> for $enum {
            fn into(self) -> u32 {
                self as u32
            }
        }
    };
}

impl PageComponentAction {
    /// The best way to utilize this function is to implement `strum::Display`
    /// and use the `impl_into_u32` macro on the `Action` enum you've defined for your `PageComponent`
    pub fn new(action_variant: impl Into<u32> + ToString, description: &str) -> Self {
        Self {
            name: action_variant.to_string(),
            id: action_variant.into(),
            description: description.to_owned(),
        }
    }
}

pub type PageComponentBindings = HashMap<Vec<KeyEvent>, PageComponentAction>;
/// Page components are rendered in the `body` of the application based on the current mode
pub trait PageComponent: Component {
    fn id(&self) -> ComponentId;
    /// Keys that can be pressed while the application is in Normal mode to switch to this page
    fn selection_keys(&self) -> Vec<KeyEvent>;
    /// Bindings that are used when this component is the current page
    /// Because this returns a reference, any struct implementing `PageComponent`
    /// needs to have a *field* that holds these bindings
    fn bindings(&self) -> &PageComponentBindings;
    /// This should replace the `handle_key_event` function provided by `Component`
    /// In order to do this, get the associate `PageComponentAction` and call this function
    fn handle_action(&mut self, action: &PageComponentAction) -> Result<Option<Action>>;
}

/// `Component` is a trait that represents a visual and interactive element of the user interface.
///
/// Implementors of this trait can be registered with the main application loop and will be able to
/// receive events, update state, and be rendered on the screen.
pub trait Component {
    /// Register an action handler that can send actions for processing if necessary.
    ///
    /// # Arguments
    ///
    /// * `tx` - An unbounded sender that can send actions.
    ///
    /// # Returns
    ///
    /// * `Result<()>` - An Ok result or an error.
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
        let _ = tx; // to appease clippy
        Ok(())
    }

    /// Register a configuration handler that provides configuration settings if necessary.
    ///
    /// # Arguments
    ///
    /// * `config` - Configuration settings.
    ///
    /// # Returns
    ///
    /// * `Result<()>` - An Ok result or an error.
    fn register_config_handler(&mut self, config: Config) -> Result<()> {
        let _ = config; // to appease clippy
        Ok(())
    }
    /// Initialize the component with a specified area if necessary.
    ///
    /// # Arguments
    ///
    /// * `area` - Rectangular area to initialize the component within.
    ///
    /// # Returns
    ///
    /// * `Result<()>` - An Ok result or an error.
    fn init(&mut self, area: Size) -> Result<()> {
        let _ = area; // to appease clippy
        Ok(())
    }
    /// Handle incoming events and produce actions if necessary.
    ///
    /// # Arguments
    ///
    /// * `event` - An optional event to be processed.
    ///
    /// # Returns
    ///
    /// * `Result<Option<Action>>` - An action to be processed or none.
    fn handle_events(&mut self, event: Option<Event>) -> Result<Option<Action>> {
        let action = match event {
            Some(Event::Key(key_event)) => self.handle_key_event(key_event)?,
            Some(Event::Mouse(mouse_event)) => self.handle_mouse_event(mouse_event)?,
            _ => None,
        };
        Ok(action)
    }
    /// Handle key events and produce actions if necessary.
    ///
    /// # Arguments
    ///
    /// * `key` - A key event to be processed.
    ///
    /// # Returns
    ///
    /// * `Result<Option<Action>>` - An action to be processed or none.
    fn handle_key_event(&mut self, key: KeyEvent) -> Result<Option<Action>> {
        let _ = key; // to appease clippy
        Ok(None)
    }
    /// Handle mouse events and produce actions if necessary.
    ///
    /// # Arguments
    ///
    /// * `mouse` - A mouse event to be processed.
    ///
    /// # Returns
    ///
    /// * `Result<Option<Action>>` - An action to be processed or none.
    fn handle_mouse_event(&mut self, mouse: MouseEvent) -> Result<Option<Action>> {
        let _ = mouse; // to appease clippy
        Ok(None)
    }
    /// Update the state of the component based on a received action. (REQUIRED)
    ///
    /// # Arguments
    ///
    /// * `action` - An action that may modify the state of the component.
    ///
    /// # Returns
    ///
    /// * `Result<Option<Action>>` - An action to be processed or none.
    fn update(&mut self, state: &State, action: Action) -> Result<Option<Action>> {
        let _ = action; // to appease clippy
        Ok(None)
    }
    /// Render the component on the screen. (REQUIRED)
    ///
    /// # Arguments
    ///
    /// * `f` - A frame used for rendering.
    /// * `area` - The area in which the component should be drawn.
    ///
    /// # Returns
    ///
    /// * `Result<()>` - An Ok result or an error.
    fn draw(&mut self, frame: &mut Frame, area: Rect) -> Result<()>;
}
