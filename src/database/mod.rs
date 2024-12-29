pub mod models;

use crate::{config::database::DatabaseConfig, MainResult};
use serde::Deserialize;
use surrealdb::{
    engine::any::{self, Any},
    sql::Thing,
    Surreal,
};

#[derive(Debug)]
pub struct Database {
    config: DatabaseConfig,
    pub client: Surreal<Any>,
}

#[derive(Debug, Deserialize)]
pub struct Record {
    #[allow(dead_code)]
    id: Thing,
}

impl Database {
    pub async fn new(config: DatabaseConfig) -> MainResult<Self> {
        let client = any::connect(&config).await?;
        client.use_ns(&config.namespace).await?;
        client.use_db(&config.database).await?;
        Ok(Self { config, client })
    }

    pub fn config(&self) -> &DatabaseConfig {
        &self.config
    }
}
