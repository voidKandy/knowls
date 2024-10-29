use crate::config::espx::{ModelConfig, ModelProvider};
use espionox::{
    agents::{memory::OtherRoleTo, Agent},
    language_models::completions::{
        anthropic::builder::AnthropicCompletionModel, openai::builder::OpenAiCompletionModel,
        CompletionModel, CompletionProvider, ModelParameters,
    },
    prelude::{Message, MessageRole},
};

pub const ASSISTANT_AGENT_SYSTEM_PROMPT: &str = r#"
You are an AI assistant in NeoVim. You will be provided with the user's codebase, as well as their most recent changes to the current file
answer their queries to the best of your ability. Your response should consider the language of the user's codebase and current document.
"#;

pub const SUMMARIZER_AGENT_SYSTEM_PROMPT: &str = r#"
    You are a state of the art high quality code summary generator. 
    You will be provided with chunks of code that you must summarize.
    Please be thorough in your summaries.
"#;

pub(super) fn summarizer(cfg: &ModelConfig) -> Agent {
    let provider: CompletionProvider = match cfg.provider {
        ModelProvider::OpenAi => OpenAiCompletionModel::Gpt3.into(),
        ModelProvider::Anthropic => AnthropicCompletionModel::Haiku.into(),
    };
    let params = ModelParameters::default();
    Agent::new(
        Some(SUMMARIZER_AGENT_SYSTEM_PROMPT),
        CompletionModel::new(provider, params, &cfg.api_key),
    )
}

pub(super) fn global(cfg: &ModelConfig) -> Agent {
    let provider: CompletionProvider = match cfg.provider {
        ModelProvider::OpenAi => OpenAiCompletionModel::Gpt4.into(),
        ModelProvider::Anthropic => AnthropicCompletionModel::Sonnet.into(),
    };
    let params = ModelParameters::default();
    Agent::new(
        Some(ASSISTANT_AGENT_SYSTEM_PROMPT),
        CompletionModel::new(provider, params, &cfg.api_key),
    )
}

pub fn doc_control_role() -> MessageRole {
    MessageRole::Other {
        alias: "DOCUMENT_CONTROL".to_owned(),
        coerce_to: OtherRoleTo::User,
    }
}

pub(super) fn document(cfg: &ModelConfig, doc_content: &str) -> Agent {
    let provider: CompletionProvider = match cfg.provider {
        ModelProvider::OpenAi => OpenAiCompletionModel::Gpt4.into(),
        ModelProvider::Anthropic => AnthropicCompletionModel::Sonnet.into(),
    };
    let params = ModelParameters::default();
    let mut agent = Agent::new(
        Some(ASSISTANT_AGENT_SYSTEM_PROMPT),
        CompletionModel::new(provider, params, &cfg.api_key),
    );
    let role = doc_control_role();

    agent.cache.push(Message {
        role,
        content: doc_content.to_owned(),
    });

    agent
}

pub(super) fn custom(cfg: &ModelConfig, sys_prompt: String) -> Agent {
    let provider: CompletionProvider = match cfg.provider {
        ModelProvider::OpenAi => OpenAiCompletionModel::Gpt4.into(),
        ModelProvider::Anthropic => AnthropicCompletionModel::Sonnet.into(),
    };

    let params = ModelParameters::default();
    let agent = Agent::new(
        Some(&sys_prompt),
        CompletionModel::new(provider, params, &cfg.api_key),
    );

    agent
}
