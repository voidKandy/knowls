use std::io::Read;

use super::AppSectionState;
use crate::state::SharedState;
use egui::{RichText, Ui};
use egui_extras::{Size, StripBuilder};
use tokio::{sync::mpsc::error::TryRecvError, task::JoinHandle};
use tracing_log::log::warn;

#[derive(Debug)]
pub struct DBSectionState {
    thread_handle: Option<JoinHandle<()>>,
    db_thread_alive: bool,
    recv: tokio::sync::mpsc::Receiver<DBUiMessageFromThread>,
    sender: tokio::sync::mpsc::Sender<DBUiMessageToThread>,
    thread_recv: Option<tokio::sync::mpsc::Receiver<DBUiMessageToThread>>,
    thread_sender: Option<tokio::sync::mpsc::Sender<DBUiMessageFromThread>>,
    health_status: Option<bool>,
    db_stdout: Option<String>,
}

#[derive(Debug)]
enum DBUiMessageFromThread {
    Healthy(bool),
    StdOut(String),
}
#[derive(Debug)]
enum DBUiMessageToThread {
    SpinUp,
    FlushStdout,
    CheckHealth,
    Kill,
}

impl Default for DBSectionState {
    fn default() -> Self {
        let (sender, thread_recv) = tokio::sync::mpsc::channel::<DBUiMessageToThread>(5);
        let (thread_sender, recv) = tokio::sync::mpsc::channel::<DBUiMessageFromThread>(5);
        Self {
            thread_handle: None,
            db_thread_alive: false,
            recv,
            sender,
            thread_recv: Some(thread_recv),
            thread_sender: Some(thread_sender),
            health_status: None,
            db_stdout: None,
        }
    }
}

impl DBSectionState {
    fn reset_channels(&mut self) {
        warn!("resetting channels");
        let (sender, thread_recv) = tokio::sync::mpsc::channel::<DBUiMessageToThread>(5);
        let (thread_sender, recv) = tokio::sync::mpsc::channel::<DBUiMessageFromThread>(5);
        self.thread_recv = Some(thread_recv);
        self.thread_sender = Some(thread_sender);
        self.sender = sender;
        self.recv = recv;
    }

    fn init_thread(&mut self, mut thread_state_clone: SharedState) {
        if self.thread_handle.is_none() {
            warn!("initializing thread");
            let sender = self.thread_sender.take().expect("No sender?");
            let mut recv = self.thread_recv.take().expect("No sender?");

            let handle = tokio::task::spawn(async move {
                loop {
                    if let Some(msg) = recv.recv().await {
                        warn!("Thread received msg: {msg:#?}");
                        match msg {
                            DBUiMessageToThread::SpinUp => {
                                let mut w = thread_state_clone
                                    .get_write()
                                    .expect("failed to get state write access");
                                if let Some(db) = w.database.as_mut() {
                                    db.init_thread().await.unwrap();
                                }
                                warn!("initialized db thread, dropping guard");
                                drop(w);
                            }
                            DBUiMessageToThread::CheckHealth => {
                                let r = thread_state_clone.get_read().unwrap();
                                if let Some(db) = r.database.as_ref() {
                                    if let Some(thread) = db.thread.as_ref() {
                                        let is_healthy = thread.client.health().await.is_ok();
                                        sender
                                            .send(DBUiMessageFromThread::Healthy(is_healthy))
                                            .await
                                            .expect("failed to send");
                                    }
                                }
                                drop(r);
                            }
                            DBUiMessageToThread::FlushStdout => {
                                let mut w = thread_state_clone.get_write().unwrap();
                                if let Some(db) = w.database.as_mut() {
                                    if let Some(thread) = db.thread.as_mut() {
                                        let mut buf = String::new();
                                        thread
                                            .stdout
                                            .read_to_string(&mut buf)
                                            .expect("failed to read from stdout");
                                        sender
                                            .send(DBUiMessageFromThread::StdOut(buf))
                                            .await
                                            .expect("failed to send");
                                    }
                                }
                            }
                            DBUiMessageToThread::Kill => {
                                break;
                            }
                        }
                    }
                }
            });
            warn!("set thread handle to some");
            self.thread_handle = Some(handle);
        }
    }
}

impl AppSectionState for DBSectionState {
    fn render(&mut self, ui: &mut Ui, state: SharedState) {
        let thread_state_clone = state.clone();
        self.init_thread(thread_state_clone);
        let width = ui.available_width() / 4.;

        StripBuilder::new(ui)
            .size(Size::exact(width)) // top cell
            .vertical(|mut strip| {
                strip.strip(|builder| {
                    builder.sizes(Size::remainder(), 2).horizontal(|mut strip| {
                        strip.cell(|ui| {
                            if let Some(db) = state
                                .get_read()
                                .ok()
                                .as_ref()
                                .and_then(|r| r.database.as_ref())
                            {
                                self.db_thread_alive = db.thread.is_some();
                                let namespace =
                                    RichText::new(format!("Namespace: {} ", db.config.namespace))
                                        .size(20.);
                                let database =
                                    RichText::new(format!("Database: {} ", db.config.database))
                                        .size(20.);
                                ui.label(namespace);
                                ui.label(database);

                                if let Some(stdout) = self.db_stdout.as_mut() {
                                    ui.text_edit_multiline(stdout);
                                }
                            }
                        });

                        strip.cell(|ui| {
                            match self.db_thread_alive {
                                true => {
                                    let kill_button = ui.button("Kill Database Instance");
                                    if kill_button.clicked() {
                                        self.sender.try_send(DBUiMessageToThread::Kill).unwrap();
                                    }
                                }
                                false => {
                                    let spinup_button = ui.button("Spin Up Instance");
                                    if spinup_button.clicked() {
                                        self.sender.try_send(DBUiMessageToThread::SpinUp).unwrap();
                                    }
                                }
                            }

                            match self.health_status {
                                Some(healthy) => {
                                    let message = if healthy { "is" } else { "is not" };
                                    ui.label(format!("DB {message} healthy"));
                                }
                                None => {
                                    let health_button = ui.button("Health Check");
                                    if health_button.clicked() {
                                        self.sender
                                            .try_send(DBUiMessageToThread::CheckHealth)
                                            .unwrap();
                                    }
                                }
                            }

                            let flush_button = ui.button("Flush Stdout");
                            if flush_button.clicked() {
                                self.sender
                                    .try_send(DBUiMessageToThread::FlushStdout)
                                    .unwrap();
                            }
                        });
                    });
                });
            });

        if self.thread_handle.is_some() {
            match self.recv.try_recv() {
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    self.thread_handle = None;
                    self.reset_channels();
                }
                Ok(mes) => match mes {
                    DBUiMessageFromThread::Healthy(healthy) => {
                        self.health_status = Some(healthy);
                    }
                    DBUiMessageFromThread::StdOut(str) => self.db_stdout = Some(str),
                },
            }
        }
    }
}
