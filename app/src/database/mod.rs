pub mod error;
pub mod models;
mod thread;
use std::path::PathBuf;

// pub mod vector_search;
use self::error::DatabaseResult;
use crate::config::{database::DatabaseConfig, Config};
use error::DatabaseError;
use serde::Deserialize;
use surrealdb::{
    engine::local::{Db, RocksDb},
    opt::{auth::Root, Config as SurConfig},
    sql::Thing,
    Surreal,
};
use thread::DatabaseThread;
use tracing::debug;

#[derive(Debug)]
pub struct Database {
    pub config: DatabaseConfig,
    pub path: PathBuf,
    pub thread: Option<DatabaseThread>,
    // pub client: Surreal<Db>,
}

#[derive(Debug, Deserialize)]
pub struct Record {
    #[allow(dead_code)]
    id: Thing,
}

impl Database {
    pub fn new(config: &Config) -> Option<Self> {
        Some(Self {
            config: config.database.as_ref().cloned()?,
            path: config.database_directory(),
            thread: None,
        })
    }
    #[tracing::instrument(name = "initialize database connection", skip_all)]
    pub async fn init_thread(&mut self) -> DatabaseResult<()> {
        let thread = DatabaseThread::try_init(
            self.config.clone(),
            self.path
                .to_str()
                .expect("could not get str from path")
                .to_string(),
        )
        .await?;

        self.thread = Some(thread);

        Ok(())
    }
}
