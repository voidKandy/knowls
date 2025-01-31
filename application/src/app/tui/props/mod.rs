pub mod database;
pub mod global;
pub mod knowledge;
use std::{fmt::Debug, sync::Arc};

use crate::app::State;

use super::{CurrentPane, Tui};
use crossterm::event::KeyEvent;
use knowls::MainResult;
use ratatui::{crossterm::event::KeyCode, layout::Rect, style::Color, Frame};
use tokio::{
    sync::{oneshot::Receiver, RwLock, RwLockReadGuard},
    task::JoinHandle,
};

#[allow(async_fn_in_trait, private_interfaces)]
pub trait TuiProp {
    fn run(&mut self, state_ref: &mut Arc<RwLock<State>>) {}
    fn handle_keyevent(tui: &mut Tui, event: KeyEvent) -> MainResult<()>;
    async fn from_state_read(r: &RwLockReadGuard<'_, State>) -> Self;
    fn draw(&mut self, selected: bool, frame: &mut Frame, area: Rect);
    fn select_me() -> (KeyCode, CurrentPane);
    fn color(&self, selected: bool) -> Color;
}

pub trait TaskingProp<M: Send + Debug + Sync + 'static> {
    fn task(&mut self) -> &mut Option<JoinHandle<()>>;
    fn set_task(&mut self, handle: JoinHandle<()>);
    fn set_recv(&mut self, recv: Receiver<M>);
    fn start_task<ARGS, F, T>(&mut self, args: ARGS, f: F)
    where
        ARGS: Sync + Send + 'static,
        F: FnOnce(ARGS) -> T + Sync + Send + 'static,
        T: std::future::Future<Output = MainResult<M>> + Send + 'static,
    {
        if self.task().is_some() {
            panic!("tried to schedule a task before one finished");
        }
        let (tx, rcv) = tokio::sync::oneshot::channel::<M>();
        self.set_recv(rcv);
        let handle = tokio::spawn(async move {
            let message = f(args).await.unwrap();
            tx.send(message).unwrap();
        });
        self.set_task(handle);
    }
}
