use std::{
    any::Any,
    fs::OpenOptions,
    io::Write,
    path::PathBuf,
    process::Command,
    str::FromStr,
    sync::{Arc, LazyLock},
};

use color_eyre::{eyre::Context, Result};
use crossterm::{
    event::{self, Event as TermEvent, KeyCode, KeyEvent, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use knowls::MainResult;
use ratatui::{
    prelude::{CrosstermBackend, Rect},
    widgets::{Clear, Widget},
    Frame,
};
use serde::{Deserialize, Serialize};
use tokio::{
    net::{TcpListener, ToSocketAddrs},
    sync::mpsc,
};
use tracing::{debug, info};

use crate::{
    database::{models::Knowledge, Database, Record},
    state::State,
};

use super::{
    action::Action,
    components::{
        fps::FpsCounter, help::HelpComponent, home::Home, knowledge::KnowledgeComponent, Component,
        ComponentId, ComponentPosition, PageComponent, BODY_LAYOUT, OUTER_VERTICAL_LAYOUT,
    },
    config::{parse_key_event, Config},
    tui::{Event, Tui},
};

pub struct App {
    config: Config,
    tick_rate: f64,
    frame_rate: f64,
    editor_open: bool,
    components: Vec<Box<dyn PageComponent>>,
    current_body_component: ComponentId,
    should_quit: bool,
    should_suspend: bool,
    mode: Mode,
    last_tick_key_events: Vec<KeyEvent>,
    action_tx: mpsc::UnboundedSender<Action>,
    action_rx: mpsc::UnboundedReceiver<Action>,

    rpc_listener: TcpListener,
    state: State,
}

#[derive(Default, Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Mode {
    #[default]
    Normal,
    Help,
}

use std::collections::HashMap;
use surrealdb::RecordId;

type Terminal = ratatui::Terminal<CrosstermBackend<std::io::Stdout>>;
async fn mock_state(database: Database) -> State {
    let mut knowledge = HashMap::new();

    let mock_entries = vec![
        (
            PathBuf::from_str("testknowledge/1").unwrap(),
            "Zig is a general-purpose programming language.",
        ),
        (
            PathBuf::from_str("testknowledge/2").unwrap(),
            "Rust provides memory safety without garbage collection.",
        ),
        (
            PathBuf::from_str("testknowledge/3").unwrap(),
            "SurrealDB is a multi-model database for web applications.",
        ),
    ];

    for (path, content) in mock_entries {
        let k = Knowledge::new(path, content);
        let r: Option<Record<Knowledge>> = database
            .client
            .create("knowledge")
            .content(k)
            .await
            .unwrap();
        let r = r.unwrap();
        knowledge.insert(r.id, r.obj);
    }

    State {
        database,
        knowledge,
        connections: HashMap::new(),
    }
}

impl App {
    pub async fn new(
        tick_rate: f64,
        frame_rate: f64,
        rpc_listen_addr: impl ToSocketAddrs,
        database: Database,
    ) -> MainResult<Self> {
        let (action_tx, action_rx) = mpsc::unbounded_channel();
        let rpc_listener = TcpListener::bind(rpc_listen_addr).await?;
        let state = mock_state(database).await;
        // let state = State::new(database);
        Ok(Self {
            editor_open: false,
            rpc_listener,
            tick_rate,
            frame_rate,
            components: vec![
                Box::new(Home::new()),
                Box::new(FpsCounter::default()),
                Box::new(HelpComponent::default()),
                Box::new(KnowledgeComponent::from(&state)),
            ],
            current_body_component: Home::default().position().id().clone(),
            state,
            should_quit: false,
            should_suspend: false,
            config: Config::new()?,
            mode: Mode::Normal,
            last_tick_key_events: Vec::new(),
            action_tx,
            action_rx,
        })
    }

    fn run_editor(&mut self, terminal: &mut Terminal, buffer: &str) -> Result<()> {
        // this should gracefully error as to not break the tui
        std::io::stdout().execute(LeaveAlternateScreen)?;
        disable_raw_mode()?;
        let path = "/tmp/knowledge.md";
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)
            .expect("failed to create file");

        writeln!(file, "{buffer}")?;
        tracing::warn!("wrote to created file");

        Command::new("hx")
            .arg(path)
            .status()
            .expect("failed to run open command");
        tracing::warn!("editor should open");
        std::io::stdout().execute(EnterAlternateScreen)?;
        enable_raw_mode()?;
        terminal.clear()?;
        Ok(())
    }

    pub async fn run(&mut self) -> Result<()> {
        let mut tui = Tui::new()?
            // .mouse(true) // uncomment this line to enable mouse support
            .tick_rate(self.tick_rate)
            .frame_rate(self.frame_rate);
        tui.enter()?;

        for component in self.components.iter_mut() {
            if let ComponentPosition::Body { selection_keys, id } = component.position() {
                let normal_map = self
                    .config
                    .keybindings
                    .get_mut(&Mode::Normal)
                    .expect("normal map should exist");
                for key in selection_keys {
                    tracing::warn!("adding {key:#?}");
                    let ret = normal_map.insert(
                        vec![parse_key_event(&key.to_string()).unwrap()],
                        Action::ChangeBody(id.clone()),
                    );
                    // it might be fine that this panics but im not sure
                    assert!(ret.is_none(), "overwrote change body key!!");
                }
            }
        }
        for component in self.components.iter_mut() {
            component.register_action_handler(self.action_tx.clone())?;
            component.register_config_handler(self.config.clone())?;
            component.init(tui.size()?)?;
        }

        let action_tx = self.action_tx.clone();
        loop {
            self.handle_events(&mut tui).await?;
            if !self.editor_open {
                self.handle_actions(&mut tui).await?;
            }

            self.state
                .manage_connections()
                .await
                .expect("failed to manage connections");
            if self.should_suspend {
                tui.suspend()?;
                action_tx.send(Action::Resume)?;
                action_tx.send(Action::ClearScreen)?;
                // tui.mouse(true);
                tui.enter()?;
            } else if self.should_quit {
                tui.stop()?;
                break;
            }
        }
        tui.exit()?;
        Ok(())
    }

    async fn handle_events(&mut self, tui: &mut Tui) -> Result<()> {
        let Some(event) = tui.next_event().await else {
            return Ok(());
        };
        let action_tx = self.action_tx.clone();
        match event {
            Event::Quit => action_tx.send(Action::Quit)?,
            Event::Tick => action_tx.send(Action::Tick)?,
            Event::Render => action_tx.send(Action::Render)?,
            Event::Resize(x, y) => action_tx.send(Action::Resize(x, y))?,
            Event::Key(key) => {
                self.handle_key_event(key)?;
            }
            _ => {}
        }
        for component in self.components.iter_mut() {
            if let Some(action) = component.handle_events(Some(event.clone()))? {
                action_tx.send(action)?;
            }
        }
        Ok(())
    }

    fn handle_key_event(&mut self, key: KeyEvent) -> Result<()> {
        let action_tx = self.action_tx.clone();
        let Some(keymap) = self.config.keybindings.get(&self.mode) else {
            return Ok(());
        };
        match keymap.get(&vec![key]) {
            Some(action) => {
                info!("Got action: {action:?}");
                action_tx.send(action.clone())?;
            }
            _ => {
                // If the key was not handled as a single key action,
                // then consider it for multi-key combinations.
                self.last_tick_key_events.push(key);

                // Check for multi-key combinations
                if let Some(action) = keymap.get(&self.last_tick_key_events) {
                    info!("Got action: {action:?}");
                    action_tx.send(action.clone())?;
                }
            }
        }
        Ok(())
    }

    async fn handle_actions(&mut self, tui: &mut Tui) -> Result<()> {
        while let Ok(action) = self.action_rx.try_recv() {
            if action != Action::Tick && action != Action::Render {
                debug!("{action:?}");
            }
            match action {
                Action::Tick => {
                    self.last_tick_key_events.drain(..);
                }
                Action::Quit => self.should_quit = true,
                Action::Suspend => self.should_suspend = true,
                Action::Resume => self.should_suspend = false,
                Action::ClearScreen => tui.terminal.clear()?,
                Action::Resize(w, h) => self.handle_resize(tui, w, h)?,
                Action::Render => self.render(tui)?,
                Action::OpenEditor(ref buffer) => {
                    self.editor_open = true;
                    tracing::warn!("going into editor mode");
                    self.run_editor(&mut tui.terminal, &buffer)
                        .expect("failed to run editor");
                    self.editor_open = false;
                    tracing::warn!("out of editor mode");
                }
                Action::InsertKnowledge(knowledge) => {
                    let r: Option<Record<Knowledge>> = self
                        .state
                        .database
                        .client
                        .create("knowledge")
                        .content(knowledge)
                        .await?;
                    let r = r.unwrap();
                    self.state.knowledge.insert(r.id, r.obj);
                    // this will not get passed to component action handler
                    return Ok(());
                }
                Action::ChangeMode(mode) => {
                    self.mode = mode;
                }
                Action::ChangeBody(ref id) => self.current_body_component = id.clone(),
                _ => {}
            }
            for component in self.components.iter_mut() {
                if let Some(action) = component.update(action.clone())? {
                    self.action_tx.send(action)?
                };
            }
        }
        Ok(())
    }

    fn handle_resize(&mut self, tui: &mut Tui, w: u16, h: u16) -> Result<()> {
        tui.resize(Rect::new(0, 0, w, h))?;
        self.render(tui)?;
        Ok(())
    }

    fn render(&mut self, tui: &mut Tui) -> Result<()> {
        let [header, body] = LazyLock::force(&OUTER_VERTICAL_LAYOUT).areas(tui.get_frame().area());
        let [body, sidebar] = LazyLock::force(&BODY_LAYOUT).areas(body);

        tui.draw(|frame| {
            for component in self.components.iter_mut() {
                let (should_render, area) = match component.position() {
                    ComponentPosition::Header(_) => (true, header),
                    ComponentPosition::SideBar(_) => (true, sidebar),
                    ComponentPosition::Popup(_) => (true, body),
                    ComponentPosition::Body { id, .. } => (self.current_body_component == id, body),
                };

                if should_render {
                    if let Err(err) = component.draw(frame, area) {
                        let _ = self
                            .action_tx
                            .send(Action::Error(format!("Failed to draw: {:?}", err)));
                    }
                }
            }
        })?;
        Ok(())
    }
}
