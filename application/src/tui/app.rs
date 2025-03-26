use super::{
    action::Action,
    components::{
        connections::ConnectionsComponent, database::DatabaseComponent, fps::FpsCounter,
        help::HelpComponent, home::Home, knowledge::KnowledgeComponent, Component, ComponentId,
        PageComponent, BODY_LAYOUT, OUTER_VERTICAL_LAYOUT,
    },
    config::Config,
    tui::{Event, Tui},
};
use crate::{
    database::{models::Knowledge, Database, Record},
    state::{SharedState, State},
    tui::config::key_event_to_string,
};
use color_eyre::Result;
use crossterm::{
    event::KeyEvent,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use knowls::{util::oneof::OneOf, MainResult};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout},
    prelude::{CrosstermBackend, Rect},
    style::{Style, Stylize},
    text::Span,
    widgets::{Paragraph, WidgetRef},
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use std::{
    fs::OpenOptions, io::Write, path::PathBuf, process::Command, str::FromStr, sync::LazyLock,
};
use tokio::{
    net::{TcpListener, ToSocketAddrs},
    sync::{mpsc, RwLock},
    task::JoinHandle,
};
use tracing::{debug, info};

pub struct App {
    config: Config,
    tick_rate: f64,
    frame_rate: f64,
    editor_open: bool,
    /// Component to be rendered in `Normal` Mode
    home: Home,
    help: HelpComponent,
    /// Components rendered in the sidebar
    components: Vec<Box<dyn Component>>,
    /// Components rendered in the body
    page_components: Vec<Box<dyn PageComponent>>,
    should_quit: bool,
    should_suspend: bool,
    mode: Mode,
    last_tick_key_events: Vec<KeyEvent>,
    action_tx: mpsc::UnboundedSender<Action>,
    action_rx: mpsc::UnboundedReceiver<Action>,

    state: SharedState,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Mode {
    #[default]
    Normal,
    Help(Option<ComponentId>),
    Component(ComponentId),
}

type Terminal = ratatui::Terminal<CrosstermBackend<std::io::Stdout>>;

impl App {
    pub async fn new(
        config: Config,
        state: SharedState,
        tick_rate: f64,
        frame_rate: f64,
        // rpc_listen_addr: impl ToSocketAddrs,
        // database: Database,
    ) -> MainResult<Self> {
        let (action_tx, action_rx) = mpsc::unbounded_channel();
        // let listener = TcpListener::bind(rpc_listen_addr).await?.into();
        let r = state.read().await;

        let page_components: Vec<Box<dyn PageComponent>> = vec![
            Box::new(KnowledgeComponent::from(&r)),
            Box::new(ConnectionsComponent::from(&r)),
        ];

        let components: Vec<Box<dyn Component>> = vec![
            Box::new(FpsCounter::default()),
            Box::new(DatabaseComponent::from(&r)),
        ];
        drop(r);

        Ok(Self {
            editor_open: false,
            tick_rate,
            frame_rate,
            home: Home::new(),
            help: HelpComponent::default(),
            components,
            page_components,
            state,
            should_quit: false,
            should_suspend: false,
            config,
            mode: Mode::Normal,
            last_tick_key_events: Vec::new(),
            action_tx,
            action_rx,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        let mut tui = Tui::new()?
            // .mouse(true) // uncomment this line to enable mouse support
            .tick_rate(self.tick_rate)
            .frame_rate(self.frame_rate);
        tui.enter()?;

        for component in self.page_components.iter() {
            self.config.add_component_bindings(component);
        }

        self.help.register_action_handler(self.action_tx.clone())?;
        self.help.register_config_handler(self.config.clone())?;
        self.help.init(tui.size()?)?;

        for component in self.components.iter_mut() {
            component.register_action_handler(self.action_tx.clone())?;
            component.register_config_handler(self.config.clone())?;
            component.init(tui.size()?)?;
        }

        let action_tx = self.action_tx.clone();

        loop {
            // tokio::select! {
            //     result =  => {
            //         result?
            //     },
            //     Ok((addr, conn_info)) = self.rpc_listener.accept() => {
            //         self.state.connections.insert(addr.to_string(),conn_info);
            //     },
            // }

            self.handle_events(&mut tui).await?;
            if !self.editor_open {
                self.handle_actions(&mut tui).await?;
            }

            // self.state
            //     .manage_connections()
            //     .await
            //     .expect("failed to manage connections");
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

    fn render_header(&self, area: Rect, buf: &mut Buffer) {
        let normal_map = self
            .config
            .keybindings
            .get(&crate::tui::app::Mode::Normal)
            .expect("should have normal map");

        let msg = normal_map
            .iter()
            .filter_map(|(k, v)| {
                if let Action::ChangeMode(Mode::Component(id)) = v {
                    Some((k, id))
                } else {
                    None
                }
            })
            .collect::<Vec<(&Vec<KeyEvent>, &ComponentId)>>()
            .into_iter()
            .fold(String::from("? for help | "), |acc, (keys, id)| {
                let mut all_keys_str = keys.iter().fold(String::from("["), |acc, k| {
                    format!("{acc}{}, ", key_event_to_string(k))
                });
                // pop off trailing ', '
                all_keys_str.pop();
                all_keys_str.pop();
                all_keys_str.push(']');

                format!("{acc} {all_keys_str} for {}", id.as_ref())
            });
        let span = Span::styled(msg, Style::new().dim());
        let paragraph = Paragraph::new(span).left_aligned();
        paragraph.render_ref(area, buf);
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
        if let Mode::Component(id) = &mut self.mode {
            let component = self
                .page_components
                .iter_mut()
                .find(|c| c.id() == *id)
                .expect("should have gotten a component here");
            match component.handle_key_event(key) {
                Ok(Some(action)) => action_tx.send(action)?,
                Err(err) => self.action_tx.send(Action::Error(format!(
                    "Component failed handle key event : {:?}",
                    err
                )))?,
                Ok(None) => {}
            }
        }

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
                    let mut w = self.state.write().await;
                    let r: Option<Record<Knowledge>> = w
                        .database
                        .client
                        .create("knowledge")
                        .content(knowledge)
                        .await?;
                    let r = r.unwrap();
                    w.knowledge.insert(r.id, r.obj);
                    // this will not get passed to component action handler
                    return Ok(());
                }
                Action::ChangeMode(mode) => {
                    self.mode = mode;
                    return Ok(());
                }
                _ => {}
            }

            // Components are only updated if its possible to get state read immediately
            // This prevents the ui from blocking when something is mutating state
            // I might come to regret this but probably not
            if let Ok(r) = self.state.try_read() {
                for component in self.components.iter_mut() {
                    if let Some(action) = component.update(&r, action.clone())? {
                        self.action_tx.send(action)?
                    };
                }
                for component in self.page_components.iter_mut() {
                    if let Some(action) = component.update(&r, action.clone())? {
                        self.action_tx.send(action)?
                    };
                }
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
            self.render_header(header, frame.buffer_mut());
            match &self.mode {
                // page components are only rendered if they are the current mode
                Mode::Component(id) => {
                    let component = self
                        .page_components
                        .iter_mut()
                        .find(|c| c.id() == *id)
                        .expect("could not get component");
                    if let Err(err) = component.draw(frame, body) {
                        let _ = self
                            .action_tx
                            .send(Action::Error(format!("Failed to draw: {:?}", err)));
                    }
                }
                Mode::Help(comp) => {
                    self.help.component_mode = comp.clone();
                    if let Err(err) = self.help.draw(frame, body) {
                        let _ = self
                            .action_tx
                            .send(Action::Error(format!("Failed to draw: {:?}", err)));
                    }
                }
                Mode::Normal => {
                    if let Err(err) = self.home.draw(frame, body) {
                        let _ = self
                            .action_tx
                            .send(Action::Error(format!("Failed to draw: {:?}", err)));
                    }
                }
            };

            let constraints = (0..self.components.len())
                .into_iter()
                .fold(vec![], |mut acc, _| {
                    acc.push(Constraint::Fill(1));
                    acc
                });
            let sidebar_split = Layout::vertical(constraints)
                .flex(ratatui::layout::Flex::Start)
                .split(sidebar);

            for (i, component) in self.components.iter_mut().enumerate() {
                let sidebar_area = sidebar_split[i];

                if let Err(err) = component.draw(frame, sidebar_area) {
                    let _ = self
                        .action_tx
                        .send(Action::Error(format!("Failed to draw: {:?}", err)));
                }
            }
        })?;
        Ok(())
    }
}
