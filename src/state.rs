use crate::{
    agents::{AgentID, Agents},
    config::Config,
    database::{
        models::{agent_memories::DBAgentMemory, block::DBBlock, DBItem},
        Database,
    },
    interact::{
        agent::{push_interact_role, AgentInteract},
        execution::InteractDocumentInfo,
        logic::LspMessageInteract,
        parsing::{
            comments::ParsedComment,
            language_ext_from_uri,
            lexer::Lexer,
            tokens::{vec::TokenVec, Token},
        },
        InteractVar,
    },
    other_err,
    util::Diff,
    MainResult,
};
use espionox::agents::Agent;
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
            Some(db_config) => Some(
                Database::new(db_config)
                    .await
                    .expect("failed to get database"),
            ),
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

        if let Some((diff, old_tokens)) = self
            .documents
            .insert(uri.clone(), new_tokens.clone())
            .and_then(|old_tokens| {
                let diff = Diff::get_diffs(&old_tokens, &new_tokens);

                let push_int_ctx = old_tokens
                    .into_iter()
                    .filter_map(|(i, cmt)| {
                        if let Some(&InteractVar::Agent(AgentInteract::Push)) =
                            cmt.interact.as_ref().and_then(|i| Some(&i.variant))
                        {
                            let id = cmt
                                .interact
                                .as_ref()
                                .unwrap()
                                .parsed_args
                                .first()
                                .and_then(|arg| {
                                    arg.as_char()
                                        .and_then(|ch| Some(AgentID::from((&uri, *ch))))
                                })
                                .unwrap();
                            Some((i, id))
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<(usize, AgentID)>>();

                for (i, id) in push_int_ctx {
                    let role = push_interact_role(&uri, i);
                    if let Some(Diff::Change(_, Token::Block(block))) = diff.get(i + 1) {
                        warn!("block after push interact changed, updating");
                        if let Some(ref mut a) = self.agents.get_agent_mut(id) {
                            a.cache.mut_filter_by(&role, false);
                            a.cache.push(espionox::prelude::Message {
                                role: role.clone(),
                                content: block.to_string(),
                            });
                        }
                    }
                }

                Some((diff, old_tokens))
            })
        {
            for d in diff.iter() {
                let idx = match d {
                    Diff::Delete(idx) => idx,
                    Diff::Insert(idx, _) => idx,
                    Diff::Change(idx, _) => idx,
                };

                if let Some(interact) = old_tokens.get(*idx).as_ref().and_then(|t| {
                    if let Token::Comment(c) = t {
                        c.interact.to_owned()
                    } else {
                        None
                    }
                }) {
                    let doc_info = InteractDocumentInfo {
                        tokens: &old_tokens,
                        my_pos: *idx,
                        uri: &uri,
                    };
                    match interact.variant {
                        InteractVar::DB(_) => {}
                        InteractVar::Agent(int) => {
                            let agent_id = interact.parsed_args.first().and_then(|arg| {
                                arg.as_char()
                                    .and_then(|ch| Some(AgentID::from((&uri, *ch))))
                            });
                            if let Some(agent) = self.agents.get_agent_mut(agent_id.unwrap()) {
                                AgentInteract::push_interact_diff_handle(&int, agent, d, doc_info)?;
                            }
                        }
                    };
                }
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
