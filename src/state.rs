use crate::{
    agents::{AgentID, Agents},
    config::Config,
    database::{
        models::{agent_memories::DBAgentMemory, block::DBBlock, DBItem},
        Database,
    },
    interact::{
        agent::uri_agent_role,
        parsing::{
            comments::ParsedComment,
            language_ext_from_uri,
            lexer::Lexer,
            tokens::{vec::TokenVec, Token},
        },
        InteractVar,
    },
    other_err, MainResult,
};
use lsp_types::Uri;
use std::{collections::HashMap, sync::Arc};
use surrealdb::sql::{Id, Thing};
use tokio::sync::RwLock;
use tracing::warn;

pub struct SharedState<'i>(pub Arc<RwLock<LspState<'i>>>);

#[derive(Debug)]
pub struct LspState<'i> {
    pub attached: Option<tokio::net::unix::SocketAddr>,
    // pub registry: InteractRegistry,
    pub documents: HashMap<Uri, TokenVec<'i>>,
    pub database: Option<Database>,
    pub agents: Agents,
}

impl<'i> LspState<'i> {
    #[tracing::instrument(name = "initializing state", skip_all)]
    pub async fn new(config: Config) -> MainResult<Self> {
        let database = match config.database {
            Some(db_config) => Some(Database::new(db_config).await?),
            None => None,
        };

        warn!("got database");

        let mut agents = Agents::from(config.model);
        warn!("got agents");
        if let Some(ref agents_config) = &config.agents {
            for (agent_id, agent_settings) in agents_config.clone().into_iter() {
                match agent_id {
                    AgentID::Uri(uri_str) => {
                        warn!("Did not expect to encounter a uri agent here, encountered: {uri_str:#?}")
                    }
                    AgentID::Global => {
                        let global_agent = agents.get_agent_mut(agent_id).expect("No global?");
                        agent_settings.change_agent(global_agent);
                    }
                    AgentID::Char(char) => {
                        agents.create_custom_agent(char, agent_settings.sys_prompt);
                    }
                }
            }
        }

        let state = Self {
            attached: None,
            documents: HashMap::new(),
            database,
            agents,
        };

        warn!("initialized state: {state:#?}");
        Ok(state)
    }

    pub async fn save_agent_memories_to_database(&self) -> MainResult<()> {
        let global_cache = &self
            .agents
            .get_agent_ref(AgentID::Global)
            .expect("No global agent?")
            .cache;

        let db = self
            .database
            .as_ref()
            .ok_or(other_err!("Database not present"))?;

        let global = DBAgentMemory::new(&AgentID::Global, global_cache.clone());

        let content = serde_json::to_value(&global).unwrap()["id"].take();

        let _: Option<DBAgentMemory> = db
            .client
            .upsert(global.record_id())
            .content(content)
            .await?;

        for (id, agent) in self.agents.iter_agents() {
            let mem = DBAgentMemory::new(&id, agent.cache.clone());

            let _: Option<DBAgentMemory> = db
                .client
                .upsert(mem.record_id())
                .content(mem.content_without_id().unwrap())
                .await?;
        }

        Ok(())
    }

    pub async fn save_docs_to_database(&self) -> MainResult<()> {
        let db = self
            .database
            .as_ref()
            .ok_or(other_err!("Database not present"))?;

        for (uri, tokens) in &self.documents {
            for b in DBBlock::from_tokens(&tokens, uri.clone()) {
                let _: Option<DBBlock> = db
                    .client
                    .upsert(b.record_id())
                    .content(b.content_without_id().unwrap())
                    .await?;
            }
        }

        Ok(())
    }

    pub fn update_doc_and_agents_from_text(&mut self, uri: Uri, text: &str) -> MainResult<()> {
        self.agents.update_or_create_doc_agent(&uri, &text);

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
                        *c != *comment
                    } else {
                        false
                    }
                })
            {
                if let Some(agent) = self.agents.get_agent_mut(agent_id.as_ref().unwrap()) {
                    let role = uri_agent_role(&uri);
                    warn!("wiping messages with role: {role:#?} from agent: {agent_id:#?}");
                    agent.cache.mut_filter_by(&role, false);
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
    #[tracing::instrument(name = "initializing shared state", skip_all)]
    pub async fn init(config: Config) -> MainResult<Self> {
        Ok(Self::new(LspState::new(config).await?))
    }

    pub fn new(state: LspState<'i>) -> Self {
        Self(Arc::new(RwLock::new(state)))
    }
    // pub fn get_read(&self) -> MainResult<RwLockReadGuard<'_, LspState>> {
    //     match self.0.try_read() {
    //         Ok(g) => Ok(g),
    //         Err(e) => Err(e.into()),
    //     }
    // }
    //
    // pub fn get_write(&mut self) -> MainResult<RwLockWriteGuard<'_, LspState>> {
    //     match self.0.try_write() {
    //         Ok(g) => Ok(g),
    //         Err(e) => Err(e.into()),
    //     }
    // }
}
