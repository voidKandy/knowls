use std::path::PathBuf;
use surrealdb::{sql::Thing, RecordId};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum KnowledgeId {
    LocalFile(PathBuf),
    /// This URI type is not specifically for LSP related stuff
    /// Only using this because `http::Uri` doesn't implement Serialize or Deserialize
    Remote(lsp_types::Uri),
}

impl ToString for KnowledgeId {
    fn to_string(&self) -> String {
        match self {
            Self::LocalFile(p) => p.to_str().unwrap().to_string(),
            Self::Remote(uri) => uri.to_string(),
        }
    }
}

impl From<PathBuf> for KnowledgeId {
    fn from(value: PathBuf) -> Self {
        Self::LocalFile(value)
    }
}

impl From<lsp_types::Uri> for KnowledgeId {
    fn from(value: lsp_types::Uri) -> Self {
        Self::Remote(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Knowledge {
    /// Not present until knowledge is inserted into Database
    // pub id: Option<Thing>,
    pub kid: KnowledgeId,
    pub content: String,
}

impl Knowledge {
    // pub fn record_id(&self) -> Option<RecordId> {
    //     let thing = self.id?;
    //     Some(RecordId::from((thing.tb, thing.id)))
    // }
    // Eventually the content of knowledge should be built using the specific knowledge ID
    pub fn new(kid: impl Into<KnowledgeId>, content: impl ToString) -> Self {
        Self {
            // id: None,
            kid: kid.into(),
            content: content.to_string(),
        }
    }
}
