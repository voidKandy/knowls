pub mod connection;
use crate::{
    agents::{AgentID, Agents},
    config::Config,
    database::Database,
    knowledge::Knowledge,
};
use std::{collections::HashMap, sync::Arc};
use tokio::{
    net::{TcpListener, ToSocketAddrs},
    sync::{RwLock, RwLockWriteGuard},
    task::JoinHandle,
};

#[derive(Debug)]
pub struct Server {
    listener: TcpListener,
    state: Arc<RwLock<ServerState>>,
    connections: HashMap<String, JoinHandle<()>>,
}

pub type ServerStateWriteGuard<'g> = RwLockWriteGuard<'g, ServerState>;
pub type SharedState<'s> = Arc<RwLock<ServerState>>;
#[derive(Debug)]
pub struct ServerState {
    pub(crate) config: Config,
    pub(crate) db: Option<Database>,
    pub(crate) agents: Agents,
    pub(crate) knowledge: HashMap<surrealdb::sql::Id, Knowledge>,
}

impl Server {
    pub async fn new(config: Config, addr: impl ToSocketAddrs) -> Self {
        let listener = TcpListener::bind(addr).await.expect("could not bind addr");
        let state = ServerState::from_config(config).await;

        Self {
            listener,
            state: Arc::new(RwLock::new(state)),
            connections: HashMap::new(),
        }
    }

    #[tracing::instrument(name = "server main loop", skip_all)]
    pub async fn main_loop(&mut self) {
        loop {
            match self.listener.accept().await {
                Ok((stream, addr)) => {
                    tracing::warn!("connected: {addr:#?}");
                    let handle = connection::ConnectionThreadState::spawn_handle(
                        stream,
                        Arc::clone(&self.state),
                    );
                    self.connections.insert(addr.to_string(), handle);
                }
                Err(e) => tracing::warn!("couldn't accept connection: {e:?}"),
            }
            self.connections.retain(|c, v| {
                if v.is_finished() {
                    tracing::warn!("dropping connection to: {c:#?}");
                    false
                } else {
                    true
                }
            });
        }
    }
}

impl ServerState {
    pub async fn from_config(config: Config) -> Self {
        let db = match &config.database {
            Some(db_config) => Some(
                Database::new(db_config.clone())
                    .await
                    .expect("failed to get database"),
            ),
            None => None,
        };

        tracing::warn!("got database");

        let mut agents = Agents::from(&config.model);
        tracing::warn!("got agents");
        if let Some(ref agents_config) = &config.agents {
            for (agent_id, agent_settings) in agents_config.clone().into_iter() {
                match agent_id {
                    AgentID::Uri(uri_str) => {
                        tracing::warn!("Did not expect to encounter a uri agent here, encountered: {uri_str:#?}")
                    }
                    AgentID::Global => {
                        let mut global_agent = agents.remove(agent_id.clone()).expect("No global?");
                        agent_settings.change_agent(&mut global_agent);
                        agents.insert(agent_id, global_agent);
                    }
                    AgentID::Char(_) => {
                        let agent =
                            crate::agents::inits::custom(&config.model, agent_settings.sys_prompt);
                        agents.insert(agent_id, agent);
                    }
                }
            }
        }

        Self {
            db,
            agents,
            config,
            knowledge: HashMap::new(),
        }
    }
}
