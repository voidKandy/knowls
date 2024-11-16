use crate::{
    agents::{AgentID, Agents},
    config::Config,
    database::{
        error::DatabaseError,
        models::{
            agent_memories::{DBAgentMemory, DBAgentMemoryParams},
            block::{block_params_from, DBBlock},
            DatabaseStruct, QueryBuilder,
        },
        Database,
    },
    error::{StateError, StateResult},
    interact::{
        agent::uri_agent_role,
        parsing::{
            comments::ParsedComment,
            language_ext_from_uri,
            lexer::Lexer,
            tokens::{Token, TokenVec},
        },
        InteractVar,
    },
};
use lsp_types::Uri;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;
use tracing::warn;

pub struct SharedState<'i>(pub Arc<RwLock<LspState<'i>>>);

#[derive(Debug)]
pub struct LspState<'i> {
    pub attached: Option<tokio::net::unix::SocketAddr>,
    // pub registry: InteractRegistry,
    pub documents: HashMap<Uri, TokenVec<'i>>,
    pub database: Option<Database>,
    pub agents: Option<Agents>,
}

impl<'i> LspState<'i> {
    #[tracing::instrument(name = "initializing state")]
    pub fn new(mut config: Config) -> anyhow::Result<Self> {
        let database = Database::new(&config);
        // if let Some(db) = database.as_mut() {
        //     db.init_handle().await?;
        // }
        let mut agents = config.model.take().and_then(|cfg| Some(Agents::from(cfg)));
        // let mut registry = InteractRegistry::default();
        if let Some(ref agents_config) = &config.agents {
            for (agent_id, agent_settings) in agents_config.clone().into_iter() {
                match agent_id {
                    AgentID::Uri(uri_str) => {
                        warn!("Did not expect to encounter a uri agent here, encountered: {uri_str:#?}")
                    }
                    AgentID::Global => {
                        if let Some(agents) = agents.as_mut() {
                            let global_agent = agents.get_agent_mut(agent_id).expect("No global?");
                            agent_settings.change_agent(global_agent);
                        }
                    }
                    AgentID::Char(char) => {
                        // registry.register_scope(&char)?;
                        if let Some(agents) = agents.as_mut() {
                            agents.create_custom_agent(char, agent_settings.sys_prompt);
                        }
                    }
                }
            }
        }

        let state = Self {
            attached: None,
            documents: HashMap::new(),
            // registry,
            database,
            agents,
        };

        warn!("initialized state: {state:#?}");
        Ok(state)
    }

    async fn init_database_thread(&mut self) -> StateResult<()> {
        if let Some(db) = self.database.as_mut() {
            warn!("Initializing db thread");
            db.init_thread().await?;
        } else {
            warn!("No database present");
        }
        Ok(())
    }

    pub async fn save_agent_memories_to_database(&self) -> StateResult<()> {
        let mut all_agent_params = vec![];
        if let Some(agents) = &self.agents {
            let global_cache = &agents
                .get_agent_ref(AgentID::Global)
                .expect("No global agent?")
                .cache;
            // let global_char = self
            //     .registry
            //     .get_interact_char(GLOBAL_ID)
            //     .expect("no global agent in registry?");

            all_agent_params.push(DBAgentMemoryParams::new(
                &AgentID::Global,
                Some(&global_cache),
            ));

            for (id, agent) in agents.iter_agents() {
                let cache = &agent.cache;
                all_agent_params.push(DBAgentMemoryParams::new(id, Some(&cache)));
            }
        }

        let db = self
            .database
            .as_ref()
            .ok_or(StateError::DatabaseNotPresent)?;

        let mut q = QueryBuilder::begin();

        for param in all_agent_params {
            q.push(&DBAgentMemory::upsert(&param)?)
        }

        if let Some(thread) = db.thread.as_ref() {
            thread
                .client
                .query(q.end())
                .await
                .map_err(|err| StateError::from(DatabaseError::from(err)))?;
        }

        Ok(())
    }

    pub async fn save_docs_to_database(&self) -> StateResult<()> {
        let mut all_block_params = vec![];
        for (uri, tokens) in &self.documents {
            let mut params = block_params_from(&tokens, uri.clone());
            all_block_params.append(&mut params);
        }

        let db = self
            .database
            .as_ref()
            .ok_or(StateError::DatabaseNotPresent)?;

        let mut q = QueryBuilder::begin();

        for param in all_block_params {
            q.push(&DBBlock::upsert(&param)?)
        }

        if let Some(thread) = db.thread.as_ref() {
            thread
                .client
                .query(q.end())
                .await
                .map_err(|err| StateError::from(DatabaseError::from(err)))?;
        }
        Ok(())
    }

    pub fn update_doc_and_agents_from_text(&mut self, uri: Uri, text: &str) -> StateResult<()> {
        if let Some(agents) = self.agents.as_mut() {
            agents.update_or_create_doc_agent(&uri, &text);
        }

        let ext = language_ext_from_uri(&uri);
        let mut lexer = Lexer::new(&text, ext);
        let new_tokens = lexer.lex_input();
        let old_tokens = self.documents.get(&uri);

        let prev_push_interacts: Vec<(usize, &ParsedComment<'_>)> = old_tokens
            .and_then(|tokens| {
                Some(
                    tokens
                        .into_iter()
                        .filter_map(|(i, c)| {
                            if let Some(ref int) = c.interact {
                                if let InteractVar::AGENT_PUSH = int.variant {
                                    return Some((i, c));
                                }
                            }
                            None
                        })
                        .collect(),
                )
            })
            .unwrap_or(vec![]);

        for (i, comment) in prev_push_interacts {
            let agent_id = comment
                .interact
                .as_ref()
                .expect("should be some")
                .parsed_args
                .first()
                .and_then(|arg| {
                    arg.as_char()
                        .and_then(|ch| Some(AgentID::from((&uri, *ch))))
                });

            if agent_id.is_some() && new_tokens.get(i).is_none()
                || new_tokens.get(i).is_some_and(|t| {
                    warn!("token exists at matching place: {t:#?}");
                    if let Token::Comment(c) = t {
                        c != comment
                    } else {
                        false
                    }
                })
            {
                if let Some(agents) = self.agents.as_mut() {
                    if let Some(agent) = agents.get_agent_mut(agent_id.as_ref().unwrap()) {
                        let role = uri_agent_role(&uri);
                        warn!("wiping messages with role: {role:#?} from agent: {agent_id:#?}");
                        agent.cache.mut_filter_by(&role, false);
                    }
                }
            }
        }

        match self.documents.get_mut(&uri) {
            Some(tokens) => {
                *tokens = new_tokens;
            }
            None => {
                self.documents.insert(uri, new_tokens);
            }
        }

        Ok(())
    }
}

impl<'i> Clone for SharedState<'i> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl<'i> SharedState<'i> {
    pub fn init(config: Config) -> anyhow::Result<Self> {
        Ok(Self(Arc::new(RwLock::new(LspState::new(config)?))))
    }
    // pub fn get_read(&self) -> anyhow::Result<RwLockReadGuard<'_, LspState>> {
    //     match self.0.try_read() {
    //         Ok(g) => Ok(g),
    //         Err(e) => Err(e.into()),
    //     }
    // }
    //
    // pub fn get_write(&mut self) -> anyhow::Result<RwLockWriteGuard<'_, LspState>> {
    //     match self.0.try_write() {
    //         Ok(g) => Ok(g),
    //         Err(e) => Err(e.into()),
    //     }
    // }
}
