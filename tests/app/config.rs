use espx_lsp_server::{
    agents::AgentID,
    config::{
        agents::{AgentConfig, AgentSettings},
        database::{DatabaseConfig, DFLT_PORT},
        espx::{ModelConfig, ModelProvider},
        Config, ConfigFromFile,
    },
    state::LspState,
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

            [agents]
             [agents._]
                sys_prompt = "you are batman"
             [agents.c]
             [agents.b]
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

#[tokio::test]
async fn config_builds_correctly() {
    let mut agents: AgentConfig = HashMap::new();
    agents.insert(AgentID::Char('c'), AgentSettings::default());
    agents.insert(
        AgentID::Char('b'),
        AgentSettings {
            sys_prompt: "prompt".to_string(),
        },
    );

    agents.insert(
        AgentID::Global,
        AgentSettings {
            sys_prompt: "you are batman".to_string(),
        },
    );
    let expected = Config {
        pwd: pwd(),
        model: Some(ModelConfig {
            provider: ModelProvider::Anthropic,
            api_key: "invalid".to_owned(),
        }),
        agents: Some(agents),
        database: Some(DatabaseConfig {
            namespace: "espx".to_owned(),
            database: "espx".to_owned(),
            user: "root".to_owned(),
            pass: "root".to_owned(),
            port: DFLT_PORT.to_owned(),
        }),
    };

    let mut cfg = test_config(true).unwrap();
    cfg.model.as_mut().and_then(|mcfg| {
        mcfg.api_key = "invalid".to_string();
        Some(mcfg)
    });

    assert_eq!(expected, cfg);

    let state = LspState::new(cfg).await.unwrap();
    let global_agent = state
        .agents
        .as_ref()
        .unwrap()
        .get_agent_ref(AgentID::Global)
        .unwrap();
    let global_agent_sys_prompt_content = global_agent.cache.ref_system_prompt_content().unwrap();
    assert_eq!(global_agent_sys_prompt_content, "you are batman");
}
