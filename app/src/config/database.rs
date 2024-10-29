use serde::{Deserialize, Serialize};
use tracing::warn;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct DatabaseConfig {
    pub namespace: String,
    pub database: String,
    pub user: String,
    pub pass: String,
    pub port: String,
}

const DFLT_NAMESPACE: &str = "namespace";
const DFLT_DATABASE: &str = "database";
const DFLT_USER: &str = "user";
const DFLT_PASS: &str = "pass";
const DFLT_PORT: &str = "19917";

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            namespace: DFLT_NAMESPACE.to_string(),
            database: DFLT_DATABASE.to_string(),
            user: DFLT_USER.to_string(),
            pass: DFLT_PASS.to_string(),
            port: DFLT_PORT.to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub(super) struct DatabaseConfigFromFile {
    namespace: Option<String>,
    database: Option<String>,
    user: Option<String>,
    pass: Option<String>,
    port: Option<String>,
}

impl Into<DatabaseConfig> for DatabaseConfigFromFile {
    fn into(self) -> DatabaseConfig {
        DatabaseConfig {
            namespace: self.namespace.unwrap_or_else(|| {
                warn!("namespace not provided, using default");
                DFLT_NAMESPACE.into()
            }),
            database: self.database.unwrap_or_else(|| {
                warn!("database not provided, using default");
                DFLT_DATABASE.into()
            }),
            user: self.user.unwrap_or_else(|| {
                warn!("user not provided, using default");
                DFLT_USER.into()
            }),
            pass: self.pass.unwrap_or_else(|| {
                warn!("pass not provided, using default");
                DFLT_PASS.into()
            }),
            port: self.port.unwrap_or_else(|| {
                warn!("port not provided, using default");
                DFLT_PORT.into()
            }),
        }
    }
}
