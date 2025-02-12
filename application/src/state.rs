use crate::{
    database::{models::Knowledge, Database},
    rpc_handler::ConnectionInfo,
};
use std::{collections::HashMap, sync::Arc};
use surrealdb::RecordId;
use tokio::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

pub type StateReadGuard<'g> = RwLockReadGuard<'g, State>;
pub type StateWriteGuard<'g> = RwLockWriteGuard<'g, State>;
pub type SharedState = Arc<RwLock<State>>;
#[derive(Debug)]
pub struct State {
    pub database: Database,
    pub knowledge: HashMap<RecordId, Knowledge>,
    pub lsp_documents: HashMap<lsp_types::Uri, String>,
    pub connections: HashMap<String, ConnectionInfo>,
}

impl State {
    pub fn new(database: Database) -> Self {
        Self {
            database,
            knowledge: HashMap::new(),
            lsp_documents: HashMap::new(),
            connections: HashMap::new(),
        }
    }
}
