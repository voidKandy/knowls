use espx_app::config::{
    database::DatabaseConfig,
    espx::{ModelConfig, ModelProvider},
    scopes::ScopeSettings,
    Config, ConfigFromFile,
};
use std::{collections::HashMap, path::PathBuf};
use tracing::warn;

pub fn test_config(database: bool) -> anyhow::Result<Config> {
    dotenv::dotenv().ok();
    let key = std::env::var("ANTHROPIC_KEY").unwrap();

    let database_str = match database {
        true => {
            r#"
            [database]
            namespace="espx" 
            database="espx"
            user="root"
            pass="root""#
        }
        false => "",
    };
    let input = format!(
        r#"
            [model]
            provider="Anthropic"
            api_key="{key}"

            {database_str}

            [scopes]
             [scopes.c]
             [scopes.b]
             sys_prompt = "prompt"

        "#
    );
    let cnfg: ConfigFromFile = match toml::from_str(&input) {
        Ok(c) => c,
        Err(err) => panic!("CONFIG ERROR: {:?}", err),
    };

    warn!("got from file config: {:?}", cnfg);
    Ok(Config::from((cnfg, pwd())))
}

fn pwd() -> PathBuf {
    std::env::current_dir().unwrap().canonicalize().unwrap()
}

#[test]
fn config_builds_correctly() {
    let mut scopes = HashMap::new();
    scopes.insert('c', ScopeSettings::default());
    scopes.insert(
        'b',
        ScopeSettings {
            sys_prompt: "prompt".to_string(),
        },
    );
    let expected = Config {
        pwd: pwd(),
        model: Some(ModelConfig {
            provider: ModelProvider::Anthropic,
            api_key: "invalid".to_owned(),
        }),
        scopes: Some(scopes),
        database: Some(DatabaseConfig {
            namespace: "espx".to_owned(),
            database: "espx".to_owned(),
            user: "root".to_owned(),
            pass: "root".to_owned(),
            port: "18181".to_owned(),
        }),
    };

    let mut cfg = test_config(true).unwrap();
    cfg.model.as_mut().and_then(|mcfg| {
        mcfg.api_key = "invalid".to_string();
        Some(mcfg)
    });

    assert_eq!(expected, cfg);
}
