use std::collections::HashMap;

use espionox::{agents::Agent, prelude::Message};
use serde::{Deserialize, Serialize};

use crate::agents::{AgentID, ASSISTANT_AGENT_SYSTEM_PROMPT};

pub type AgentConfigFromFile = HashMap<String, AgentSettingsFromFile>;
pub type AgentConfig = HashMap<AgentID, AgentSettings>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentSettings {
    pub sys_prompt: String,
}

impl AgentSettings {
    pub fn change_agent(&self, agent: &mut Agent) {
        match agent.cache.mut_system_prompt_content() {
            Some(content) => *content = self.sys_prompt.clone(),
            None => agent.cache.push(Message::new_system(&self.sys_prompt)),
        }
    }
}

impl Default for AgentSettings {
    fn default() -> Self {
        let sys_prompt = ASSISTANT_AGENT_SYSTEM_PROMPT.to_string();
        Self { sys_prompt }
    }
}

impl From<AgentSettingsFromFile> for AgentSettings {
    fn from(value: AgentSettingsFromFile) -> Self {
        Self {
            sys_prompt: value.sys_prompt.unwrap_or(Self::default().sys_prompt),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentSettingsFromFile {
    pub sys_prompt: Option<String>,
}
