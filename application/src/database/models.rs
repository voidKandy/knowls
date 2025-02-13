use std::{ops::Add, path::PathBuf, sync::LazyLock};

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
    pub kid: KnowledgeId,
    pub content: String,
}

fn increment_and_return_char_counter() -> u8 {
    static COUNTER: std::sync::OnceLock<std::sync::Mutex<u8>> = std::sync::OnceLock::new();
    let mut mutex = COUNTER
        .get_or_init(|| std::sync::Mutex::new(0))
        .lock()
        .unwrap();
    *mutex = mutex.add(1);
    *mutex
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
