#[derive(Clone, Debug, serde::Deserialize)]
pub struct CompletionConfig {
    pub prefix: String,
    // pub postfix: Option<&'static str>,
}

impl Default for CompletionConfig {
    fn default() -> Self {
        Self {
            prefix: "@@$".to_string(),
        }
    }
}
