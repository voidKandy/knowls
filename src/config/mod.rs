pub mod agents;
pub mod database;
pub mod espx;
use agents::{AgentConfig, AgentConfigFromFile, AgentSettings};
use database::{DatabaseConfig, DatabaseConfigFromFile};
use espx::ModelConfig;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs::{self},
    path::{Path, PathBuf},
    str::FromStr,
    sync::LazyLock,
};
use toml;
use tracing::{debug, warn};

use crate::agents::AgentID;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct Config {
    pub pwd: PathBuf,
    pub model: Option<ModelConfig>,
    pub database: Option<DatabaseConfig>,
    pub agents: Option<AgentConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct ConfigFromFile {
    model: Option<ModelConfig>,
    database: Option<DatabaseConfigFromFile>,
    agents: Option<AgentConfigFromFile>,
}

impl From<(ConfigFromFile, PathBuf)> for Config {
    fn from((cfg, pwd): (ConfigFromFile, PathBuf)) -> Self {
        let agents: Option<HashMap<AgentID, AgentSettings>> = {
            if cfg.agents.is_none() || cfg.agents.as_ref().is_some_and(|hm| hm.is_empty()) {
                None
            } else {
                let mut map = HashMap::new();

                for (char, settings) in cfg.agents.unwrap() {
                    match AgentID::try_from_char(char) {
                        Some(id) => {
                            let _ = map.insert(id, settings.into());
                        }
                        None => {
                            warn!("config does not support configuring ^ agent, skipping");
                            continue;
                        }
                    }
                }
                Some(map)
            }
        };

        Config {
            pwd,
            model: cfg.model,
            database: cfg.database.and_then(|db| Some(db.into())),
            agents,
        }
    }
}

pub const GLOBAL_SYS_CONFIG: LazyLock<PathBuf> = LazyLock::new(|| {
    let home = std::env::var("HOME").expect("No $HOME env variable");
    let path_str = format!("{home}/.espx/config.toml");
    let path = PathBuf::from_str(&path_str).expect("could not build path buf");
    if !path.exists() {
        panic!("{path:#?} does not exist!");
    }
    path
});

impl Config {
    pub fn init_from_pwd() -> Self {
        let pwd = std::env::current_dir()
            .expect("failed to get current dir")
            .canonicalize()
            .expect("failed to canonicalize pwd");
        debug!("pwd: {:?}", pwd);
        let mut config_file_path = pwd.clone();
        config_file_path.push(Path::new("espx-ls.toml"));

        let content = fs::read_to_string(config_file_path).unwrap_or(String::new());
        let cnfg: ConfigFromFile = match toml::from_str(&content) {
            Ok(c) => c,
            Err(err) => panic!("CONFIG ERROR: {:?}", err),
        };

        Config::from((cnfg, pwd))
    }

    /// returns none if the global config file does not exist
    pub fn init_from_global_config() -> Option<Self> {
        let _p = GLOBAL_SYS_CONFIG;
        let path = LazyLock::force(&_p);
        warn!("Building config from: {path:#?}");
        if !path.exists() {
            return None;
        }
        let content = fs::read_to_string(path.clone()).expect("unexpected error");
        let cnfg: ConfigFromFile = match toml::from_str(&content) {
            Ok(c) => c,
            Err(err) => panic!("CONFIG ERROR: {:?}", err),
        };

        let pwd: PathBuf = Into::<PathBuf>::into(path.parent().unwrap())
            .canonicalize()
            .expect("could not canonicalize config pwd");

        Some(Config::from((cnfg, pwd)))
    }

    fn espx_ls_dir(&self) -> PathBuf {
        let mut path = self.pwd.clone();
        path.push(PathBuf::from("data"));
        debug!("espx ls folder path: {:?}", path);
        if !path.exists() {
            fs::create_dir(&path).expect("failed to make .espx-ls directory");
        }
        path
    }

    pub fn conversation_file(&self) -> PathBuf {
        let mut path = self.espx_ls_dir();
        path.push(PathBuf::from("conversation.md"));
        if !path.exists() {
            fs::File::create_new(&path).expect("failed to create conversation file");
        }
        path
    }

    pub fn database_directory(&self) -> PathBuf {
        let mut path = self.espx_ls_dir();
        path.push(PathBuf::from("db.surql"));
        path
    }
}
