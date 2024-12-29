use serde::{Deserialize, Serialize};
use surrealdb::{engine::any::IntoEndpoint, opt::auth::Root};
use tracing::warn;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct DatabaseConfig {
    pub namespace: String,
    pub database: String,
    pub user: String,
    pub pass: String,
    pub port: String,
    pub host: String,
    pub protocol: Protocol,
}

/// https://docs.rs/surrealdb/latest/surrealdb/engine/any/fn.connect.html
/// Surreal supports more, but I've opted to only allow these
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Protocol {
    Ws,
    Wss,
    Http,
    Https,
    /// in-memory instance
    Mem,
    /// file-backed instance (currently uses RocksDB)
    File,
    /// RocksDB-backed instance
    Rocksdb,
    /// SurrealKV-backed instance
    Surrealkv,
}
const ALL_PROTOCOLS: &[&'static Protocol] = &[
    &Protocol::Ws,
    &Protocol::Wss,
    &Protocol::Http,
    &Protocol::Https,
    &Protocol::Mem,
    &Protocol::File,
    &Protocol::Rocksdb,
    &Protocol::Surrealkv,
];

// We need this static arr to return for when deserialize errors
const ALL_PROTOCOLS_STR: &[&'static str] = &[
    &Protocol::Ws.as_str(),
    &Protocol::Ws.as_str(),
    &Protocol::Http.as_str(),
    &Protocol::Https.as_str(),
    &Protocol::Mem.as_str(),
    &Protocol::File.as_str(),
    &Protocol::Rocksdb.as_str(),
    &Protocol::Surrealkv.as_str(),
];

impl<'de> Deserialize<'de> for Protocol {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let string = String::deserialize(deserializer)?;

        match ALL_PROTOCOLS
            .iter()
            .position(|v| v.as_str() == string.as_str())
        {
            Some(i) => Ok(ALL_PROTOCOLS[i].clone()),
            None => Err(serde::de::Error::unknown_variant(
                &string,
                ALL_PROTOCOLS_STR,
            )),
        }
    }
}

impl Serialize for Protocol {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let str = self.as_str();
        Serialize::serialize(str, serializer)
    }
}

impl Protocol {
    const fn as_str(&self) -> &'static str {
        match self {
            Self::Ws => "ws",
            Self::Wss => "wss",
            Self::Http => "http",
            Self::Https => "https",
            Self::Mem => "mem",
            Self::File => "file",
            Self::Rocksdb => "rocksdb",
            Self::Surrealkv => "surrealkv",
        }
    }
}

impl IntoEndpoint for &DatabaseConfig {
    fn into_endpoint(self) -> surrealdb::Result<surrealdb::opt::Endpoint> {
        let root = Root {
            username: &self.user,
            password: &self.pass,
        };
        let config = surrealdb::opt::Config::new().user(root);
        let url = format!("{}://{}:{}", self.protocol.as_str(), self.host, self.port);
        (url, config).into_endpoint()
    }
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            namespace: "namespace".to_string(),
            database: "database".to_string(),
            user: "user".to_string(),
            pass: "pass".to_string(),
            port: "19917".to_string(),
            host: "127.0.0.1".to_string(),
            protocol: Protocol::Mem,
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
    host: Option<String>,
    protocol: Option<Protocol>,
}

impl Into<DatabaseConfig> for DatabaseConfigFromFile {
    fn into(self) -> DatabaseConfig {
        let d = DatabaseConfig::default();
        DatabaseConfig {
            namespace: self.namespace.unwrap_or_else(|| {
                warn!("namespace not provided, using default");
                d.namespace
            }),
            database: self.database.unwrap_or_else(|| {
                warn!("database not provided, using default");
                d.database
            }),
            user: self.user.unwrap_or_else(|| {
                warn!("user not provided, using default");
                d.user
            }),
            pass: self.pass.unwrap_or_else(|| {
                warn!("pass not provided, using default");
                d.pass
            }),
            port: self.port.unwrap_or_else(|| {
                warn!("port not provided, using default");
                d.port
            }),
            host: self.host.unwrap_or_else(|| {
                warn!("host not provided, using default");
                d.host
            }),
            protocol: self.protocol.unwrap_or_else(|| {
                warn!("protocol not provided, using default");
                d.protocol
            }),
        }
    }
}
