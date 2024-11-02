use super::{DatabaseStruct, IntoOneOf};
use crate::{agents::AgentID, database::error::DatabaseError, util::OneOf};
use anyhow::anyhow;
use base64::{prelude::BASE64_URL_SAFE_NO_PAD, Engine};
use espionox::{
    agents::memory::MessageStackRef,
    prelude::{Message, MessageStack},
};
use lsp_types::Uri;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use surrealdb::sql::Thing;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DBAgentMemory {
    pub id: Thing,
    pub messages: MessageStack,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum DBAgentID {
    Global,
    Char(char),
    EncodedUri(String),
}

impl TryFrom<Thing> for DBAgentID {
    type Error = anyhow::Error;
    fn try_from(value: Thing) -> Result<Self, Self::Error> {
        match value.id {
            surrealdb::sql::Id::String(string) => {
                if string.chars().count() == 1 {
                    Ok(Self::Char(string.chars().next().unwrap()))
                } else {
                    // let uri = Uri::from_str(&string)?;
                    Ok(Self::EncodedUri(string))
                }
            }
            other => Err(anyhow!("{other:?} cannot be turned into an AgentID")),
        }
    }
}

impl From<&char> for DBAgentID {
    fn from(value: &char) -> Self {
        Self::Char(value.to_owned())
    }
}

impl From<&Uri> for DBAgentID {
    fn from(value: &Uri) -> Self {
        let encoded = BASE64_URL_SAFE_NO_PAD.encode(value.as_str());
        Self::EncodedUri(encoded)
    }
}

impl From<&AgentID> for DBAgentID {
    fn from(value: &AgentID) -> Self {
        match value {
            AgentID::Global => Self::Global,
            AgentID::Uri(uri_str) => {
                let encoded = BASE64_URL_SAFE_NO_PAD.encode(uri_str.as_str());
                Self::EncodedUri(encoded)
            }
            AgentID::Char(char) => Self::Char(*char),
        }
    }
}

impl ToString for DBAgentID {
    fn to_string(&self) -> String {
        match self {
            Self::Global => AgentID::Global.to_string(),
            Self::EncodedUri(uri) => uri.to_string(),
            Self::Char(char) => char.to_string(),
        }
    }
}

impl DBAgentID {
    fn decode_uri(&self) -> anyhow::Result<Option<Uri>> {
        if let Self::EncodedUri(encoded) = self {
            let uri_str = BASE64_URL_SAFE_NO_PAD
                .decode(encoded)?
                .into_iter()
                .map(|int| int as char)
                .collect::<String>();
            let uri = Uri::from_str(&uri_str)?;
            return Ok(Some(uri));
        }
        Ok(None)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DBAgentMemoryParams<'stack> {
    id: DBAgentID,
    messages: Option<MessageStackRef<'stack>>,
}

impl<'stack> DBAgentMemoryParams<'stack> {
    pub fn new(into_id: impl Into<DBAgentID>, messages: Option<&'stack MessageStack>) -> Self {
        let id = Into::<DBAgentID>::into(into_id);
        let messages = messages.and_then(|mstack| {
            let vec = mstack.as_ref().iter().collect::<Vec<&Message>>();
            Some(MessageStackRef::from(vec))
        });
        Self { id, messages }
    }
}

impl<'l> IntoOneOf<'l, DBAgentMemory, DBAgentMemoryParams<'l>> for DBAgentMemory {
    fn one_of(me: &'l Self) -> OneOf<&'l DBAgentMemory, &'l DBAgentMemoryParams> {
        OneOf::Left(me)
    }
}

impl<'l> IntoOneOf<'l, DBAgentMemory, DBAgentMemoryParams<'l>> for DBAgentMemoryParams<'l> {
    fn one_of(me: &'l Self) -> OneOf<&'l DBAgentMemory, &'l DBAgentMemoryParams> {
        OneOf::Right(me)
    }
}

impl<'l> DatabaseStruct<'l, DBAgentMemoryParams<'l>> for DBAgentMemory {
    fn db_id() -> &'static str {
        "memories"
    }

    fn thing(&self) -> &Thing {
        &self.id
    }

    fn content(
        oneof: &'l impl IntoOneOf<'l, Self, DBAgentMemoryParams<'l>>,
    ) -> crate::database::error::DatabaseResult<String> {
        match IntoOneOf::<Self, DBAgentMemoryParams>::one_of(oneof) {
            OneOf::Left(me) => Ok(format!(
                r#"CONTENT {{
                messages: {},
                }}"#,
                serde_json::to_value(&me.messages)?,
            )),
            OneOf::Right(params) => {
                let messages_string = {
                    if let Some(messages) = &params.messages {
                        format!(",messages: {}", serde_json::to_value(messages)?)
                    } else {
                        String::new()
                    }
                };

                Ok(format!(
                    r#"CONTENT {{ {} }}"#,
                    [messages_string].join(" ").trim().trim_start_matches(','),
                ))
            }
        }
    }

    fn upsert(params: &DBAgentMemoryParams) -> crate::database::error::DatabaseResult<String> {
        if params.messages.is_none() {
            return Err(DatabaseError::DbStruct(format!(
                "All fields need to be Some for a create statement, got: {:?} {:?} ",
                params.id, params.messages,
            )));
        }

        let id = params.id.to_string();

        Ok(format!(
            "UPSERT {}:{id} {};",
            Self::db_id(),
            Self::content(params)?
        ))
    }
}
