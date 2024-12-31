use espionox::prelude::MessageStack;
use serde::{Deserialize, Serialize};
use surrealdb::sql::{Id, Thing};

use crate::agents::AgentID;

use super::DBItem;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DBAgentMemory {
    id: Thing,
    pub messages: MessageStack,
}

impl DBAgentMemory {
    pub fn new(id: &AgentID, m: impl Into<MessageStack>) -> Self {
        let id = Id::from(id);
        let id = Thing::from((Self::DB_ID, id));
        Self {
            id,
            messages: m.into(),
        }
    }
}

impl From<&AgentID> for Id {
    fn from(value: &AgentID) -> Self {
        match value {
            AgentID::Global => Self::String("Global".to_string()),
            AgentID::Uri(uri_str) => Self::String(uri_str.clone()),
            AgentID::Char(char) => Self::String(char.to_string()),
        }
    }
}

impl DBItem for DBAgentMemory {
    const DB_ID: &str = "agent_memory";
    fn thing(&self) -> &Thing {
        &self.id
    }
}
