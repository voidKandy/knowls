pub mod config;
pub mod models;
pub mod query_builder;

use config::{DatabaseConfig, Protocol};
use knowls::{other_err, MainResult};
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
        if let Protocol::Mem = self.protocol {
            return ("mem://", config).into_endpoint();
        }
        let url = format!("{}://{}:{}", self.protocol.as_str(), self.host, self.port);
        (url, config).into_endpoint()
    }
}

impl Database {
    #[tracing::instrument(name = "init database")]
    pub async fn new(config: DatabaseConfig) -> MainResult<Self> {
        let client = any::connect(&config)
            .await
            .map_err(|err| other_err!("failed to connect to database: {err}"))?;
        client.use_ns(&config.namespace).await?;
        client.use_db(&config.database).await?;
        client
            .signin(root_credentials(&config))
            .await
            .map_err(|err| other_err!("failed to signin to database: {err}"))?;
        Ok(Self { config, client })
    }

    pub fn config(&self) -> &DatabaseConfig {
        &self.config
    }
}
