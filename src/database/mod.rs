pub mod models;

use crate::{config::database::DatabaseConfig, MainResult};
use serde::Deserialize;
use surrealdb::{
    engine::any::{self, Any, IntoEndpoint},
    opt::auth::Root,
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

fn root_credentials(cfg: &DatabaseConfig) -> Root {
    surrealdb::opt::auth::Root {
        username: &cfg.user,
        password: &cfg.pass,
    }
}

impl IntoEndpoint for &DatabaseConfig {
    fn into_endpoint(self) -> surrealdb::Result<surrealdb::opt::Endpoint> {
        let config = surrealdb::opt::Config::new().user(root_credentials(&self));
        let url = format!("{}://{}:{}", self.protocol.as_str(), self.host, self.port);
        (url, config).into_endpoint()
    }
}

impl Database {
    pub async fn new(config: DatabaseConfig) -> MainResult<Self> {
        let client = any::connect(&config).await?;
        client.use_ns(&config.namespace).await?;
        client.use_db(&config.database).await?;
        client.signin(root_credentials(&config)).await?;
        Ok(Self { config, client })
    }

    pub fn config(&self) -> &DatabaseConfig {
        &self.config
    }
}
