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
        id::{
            InteractID, COMMAND_MASK, DOCUMENT_CHARACTER, GLOBAL_CHARACTER, GLOBAL_ID, PUSH_ID,
            SCOPE_MASK,
        },
        lexer::{Lexer, Token, TokenVec},
        registry::InteractRegistry,
    },
};
use anyhow::anyhow;
use espionox::{
    agents::{memory::OtherRoleTo, Agent},
    prelude::{Message, MessageRole},
};
use lsp_types::Uri;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use tracing::warn;

pub struct SharedState(Arc<RwLock<LspState>>);

#[derive(Debug)]
pub struct LspState {
    pub attached: Option<tokio::net::unix::SocketAddr>,
    pub documents: HashMap<Uri, TokenVec>,
    pub database: Option<Database>,
    pub registry: InteractRegistry,
    pub agents: Option<Agents>,
}

impl LspState {
    #[tracing::instrument(name = "initializing state")]
    pub fn new(mut config: Config) -> anyhow::Result<Self> {
        let database = Database::new(&config);
        // if let Some(db) = database.as_mut() {
        //     db.init_handle().await?;
        // }
        let mut agents = config.model.take().and_then(|cfg| Some(Agents::from(cfg)));
        let mut registry = InteractRegistry::default();
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
                        registry.register_scope(&char)?;
                        if let Some(agents) = agents.as_mut() {
                            agents.create_custom_agent(char, agent_settings.sys_prompt);
                        }
                    }
                }
            }
        }

        Ok(Self {
            attached: None,
            documents: HashMap::new(),
            registry,
            database,
            agents,
        })
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
            let global_char = self
                .registry
                .get_interact_char(GLOBAL_ID)
                .expect("no global agent in registry?");

            all_agent_params.push(DBAgentMemoryParams::new(
                global_char.as_ref(),
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

    pub fn agent_mut_from_interact_integer(
        &mut self,
        integer: u8,
        current_document_uri: &Uri,
    ) -> StateResult<&mut Agent> {
        let agents = self
            .agents
            .as_mut()
            .ok_or(anyhow!("agents not present in state"))?;
        let masked = integer & SCOPE_MASK;
        let char = self
            .registry
            .get_interact_char(InteractID::Scope(masked))
            .ok_or(anyhow!(
                "registry does not have char for id: {integer} with mask: {SCOPE_MASK}"
            ))?;
        match char {
            _ if char == &DOCUMENT_CHARACTER => Ok(agents
                .get_agent_mut(current_document_uri)
                .ok_or(StateError::AgentsNotPresent)?),
            _ if char == &GLOBAL_CHARACTER => Ok(agents
                .get_agent_mut(AgentID::Global)
                .ok_or(StateError::AgentsNotPresent)?),
            custom_character => Ok(agents
                .get_agent_mut(*custom_character.as_ref())
                .ok_or(StateError::AgentsNotPresent)?),
        }
    }

    pub fn update_doc_and_agents_from_text(&mut self, uri: Uri, text: String) -> StateResult<()> {
        if let Some(agents) = self.agents.as_mut() {
            agents.update_or_create_doc_agent(&uri, &text);
        }

        let uri_str = uri.as_str().to_string();
        let ext = &uri_str
            .rsplit_once('.')
            .expect("uri does not have extension")
            .1;
        let mut lexer = Lexer::new(&text, ext);
        let new_tokens = lexer.lex_input(&self.registry);
        let old_tokens = self.documents.get(&uri);
        let mut prev_existing_push_scopes = old_tokens
            .and_then(|tokens| {
                let mut all = vec![];
                for idx in tokens.comment_indices() {
                    let mut iter = tokens.as_ref().iter();
                    if let Token::Comment(comment) = iter.nth(*idx).unwrap() {
                        if let Some(integer) = comment.try_get_interact_integer().ok() {
                            if integer & COMMAND_MASK == *PUSH_ID.as_ref() {
                                all.push(integer & SCOPE_MASK)
                            }
                        }
                    }
                }
                Some(all)
            })
            .unwrap_or(vec![]);

        let role = MessageRole::Other {
            alias: uri.to_string(),
            coerce_to: OtherRoleTo::User,
        };

        for comment_idx in new_tokens.comment_indices() {
            let mut iter = new_tokens.as_ref().iter();
            if let Token::Comment(comment) = iter.nth(*comment_idx).unwrap() {
                warn!("comment: {comment:?}");
                if let Some(integer) = comment.try_get_interact_integer().ok() {
                    warn!("id: {integer:?}");
                    if let Some(idx) = prev_existing_push_scopes
                        .iter()
                        .position(|id| integer & SCOPE_MASK == *id)
                    {
                        prev_existing_push_scopes.remove(idx);
                    }
                    if let Some(agent) = self.agent_mut_from_interact_integer(integer, &uri).ok() {
                        agent.cache.mut_filter_by(&role, false);
                        if integer & COMMAND_MASK == *PUSH_ID.as_ref() {
                            warn!("command is push");
                            if let Some(Token::Block(block)) = iter.next() {
                                warn!("block: {block:?}");
                                warn!("got agent, updating");
                                agent.cache.push(Message {
                                    role: role.clone(),
                                    content: block.to_owned(),
                                });
                            }
                        }
                    }
                }
            }
        }

        for scope in prev_existing_push_scopes {
            if let Some(agent) = self.agent_mut_from_interact_integer(scope, &uri).ok() {
                warn!("cleaning agent for scope: {scope}");
                agent.cache.mut_filter_by(&role, false);
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

impl Clone for SharedState {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl SharedState {
    pub fn init(config: Config) -> anyhow::Result<Self> {
        Ok(Self(Arc::new(RwLock::new(LspState::new(config)?))))
    }
    pub fn get_read(&self) -> anyhow::Result<RwLockReadGuard<'_, LspState>> {
        match self.0.try_read() {
            Ok(g) => Ok(g),
            Err(e) => Err(e.into()),
        }
    }

    pub fn get_write(&mut self) -> anyhow::Result<RwLockWriteGuard<'_, LspState>> {
        match self.0.try_write() {
            Ok(g) => Ok(g),
            Err(e) => Err(e.into()),
        }
    }
}
