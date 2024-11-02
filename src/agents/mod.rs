use std::{collections::HashMap, str::FromStr};
pub mod error;
use crate::{config::espx::ModelConfig, interact::id::GLOBAL_CHARACTER};
use anyhow::anyhow;
use error::AgentsError;
use espionox::{
    agents::{memory::MessageStackRef, Agent},
    prelude::Message,
};
pub use inits::{doc_control_role, ASSISTANT_AGENT_SYSTEM_PROMPT};
use lsp_types::{MarkedString, Uri};
use tracing::warn;
mod inits;

#[derive(Debug)]
pub struct Agents {
    pub config: ModelConfig,
    map: HashMap<AgentID, Agent>,
}

#[derive(Debug, Hash, Eq, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum AgentID {
    Global,
    Uri(String),
    Char(char),
}

impl From<&AgentID> for AgentID {
    fn from(value: &AgentID) -> Self {
        warn!("Cloning agent ID");
        value.to_owned()
    }
}

impl From<&Uri> for AgentID {
    fn from(value: &Uri) -> Self {
        Self::Uri(value.to_string())
    }
}

impl From<char> for AgentID {
    fn from(value: char) -> Self {
        match value {
            _ if value == *GLOBAL_CHARACTER.as_ref() => return Self::Global,
            _ => Self::Char(value),
        }
    }
}

impl std::fmt::Display for AgentID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let dis = match self {
            Self::Global => "Global".to_string(),
            Self::Uri(uri_str) => {
                let split = uri_str
                    .rsplitn(3, std::path::MAIN_SEPARATOR)
                    .collect::<Vec<&str>>();
                format!("{}{}{}", split[1], std::path::MAIN_SEPARATOR, split[0])
            }
            Self::Char(char) => {
                format!("Custom Agent({char})")
            }
        };
        write!(f, "{dis}")
    }
}

impl TryInto<Uri> for AgentID {
    type Error = AgentsError;
    fn try_into(self) -> Result<Uri, Self::Error> {
        if let AgentID::Uri(uri) = self {
            let val = Uri::from_str(&uri)
                .map_err(|err| anyhow!("Could not create uri from str: {err:#?}"))?;
            return Ok(val);
        }
        Err(AgentsError::IncorrectAgentIDVariant(self))
    }
}

impl TryInto<char> for AgentID {
    type Error = AgentsError;
    fn try_into(self) -> Result<char, Self::Error> {
        if let AgentID::Char(char) = self {
            return Ok(char);
        }
        Err(AgentsError::IncorrectAgentIDVariant(self))
    }
}

impl From<ModelConfig> for Agents {
    fn from(cfg: ModelConfig) -> Self {
        let mut map = HashMap::new();
        let global = self::inits::global(&cfg);
        map.insert(AgentID::Global, global);
        Self { config: cfg, map }
    }
}

pub fn message_stack_into_marked_string(mut stack: MessageStackRef<'_>) -> MarkedString {
    let mut content = String::new();
    while let Some(message) = stack.pop(None) {
        content.push_str(&format!(
            r#"
## {:?}
{}
        "#,
            message.role,
            message.content.trim_start()
        ));
    }

    MarkedString::LanguageString(lsp_types::LanguageString {
        language: "markdown".to_string(),
        value: content,
    })
}

impl Agents {
    pub fn get_agent_ref(&self, key: impl Into<AgentID>) -> Option<&Agent> {
        let id: AgentID = key.into();
        self.map.get(&id)
    }
    pub fn get_agent_mut(&mut self, key: impl Into<AgentID>) -> Option<&mut Agent> {
        let id: AgentID = key.into();
        self.map.get_mut(&id)
    }

    pub fn iter_agents(&self) -> std::collections::hash_map::Iter<'_, AgentID, Agent> {
        self.map.iter()
    }
    pub fn iter_agents_mut(&mut self) -> std::collections::hash_map::IterMut<'_, AgentID, Agent> {
        self.map.iter_mut()
    }

    pub fn insert_agent(&mut self, key: impl Into<AgentID>, agent: Agent) {
        let id: AgentID = key.into();
        self.map.insert(id, agent);
    }

    pub fn update_or_create_doc_agent(&mut self, uri: &Uri, doc_content: &str) {
        let role = doc_control_role();
        match self.get_agent_mut(uri) {
            Some(agent) => {
                agent.cache.mut_filter_by(&role, false);
                agent.cache.push(Message {
                    role,
                    content: doc_content.to_owned(),
                });
            }
            None => {
                let agent = self::inits::document(&self.config, doc_content);
                self.insert_agent(uri, agent);
            }
        }
    }

    pub fn create_custom_agent(&mut self, char: char, sys_prompt: String) {
        let agent = self::inits::custom(&self.config, sys_prompt);
        self.insert_agent(char, agent);
    }

    pub fn get_last_n_messages(agent: &Agent, n: usize) -> MessageStackRef {
        let messages: Vec<&Message> = agent.cache.as_ref().iter().rev().take(n).collect();
        MessageStackRef::from(messages)
    }
}
