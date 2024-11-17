pub mod agents;
pub mod database;
pub mod docs;
pub mod global;
use super::new_tui::{CurrentPane, Tui};
use crate::state::{LspState, SharedState};
use crossterm::event::KeyEvent;
use ratatui::{crossterm::event::KeyCode, layout::Rect, style::Color, Frame};
use tokio::sync::RwLockReadGuard;

#[allow(async_fn_in_trait, private_interfaces)]
pub trait TuiProp {
    fn run(&mut self, state_ref: &mut SharedState<'static>) {}
    async fn handle_keyevent(tui: &mut Tui, event: KeyEvent) -> anyhow::Result<()>;
    async fn from_state_read(r: &RwLockReadGuard<'_, LspState<'static>>) -> Self;
    fn draw(&mut self, selected: bool, frame: &mut Frame, area: Rect);
    fn select_me() -> (KeyCode, CurrentPane);
    fn color(&self, selected: bool) -> Color;
}
