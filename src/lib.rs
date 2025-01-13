pub mod agents;
pub mod client;
pub mod config;
pub mod database;
pub mod error;
pub mod interact;
pub mod knowledge;
pub mod rpc;
pub mod server;
// pub mod state;
pub mod trace;
pub mod ui;
pub mod util;

macro_rules! other_err {
    ($($arg:tt)*) => ({
        Into::<crate::MainErr>::into(std::io::Error::other(format!($($arg)*)))
    });
}
pub(crate) use other_err;
pub type MainErr = Box<dyn std::error::Error + Send + Sync + 'static>;
pub type MainResult<T> = std::result::Result<T, MainErr>;
#[allow(unused_macros)]
#[macro_export]
macro_rules! trace_panics {
    () => {
        std::panic::set_hook(Box::new(|v| {
            let payload = v.payload();
            let str = payload.downcast_ref::<String>().cloned().unwrap_or(
                payload
                    .downcast_ref::<&'static str>()
                    .map(|str| str.to_string())
                    .unwrap_or("Any".to_string()),
            );

            tracing::error!("thread panicked at: {:#?}\npayload: {str:#?}", v.location(),)
        }));
    };
}

pub mod embeddings {

    pub type EmbeddingFloat = f32;
    use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};

    use crate::MainResult;

    pub fn embed_sentences(sentences: Vec<&str>) -> MainResult<Vec<Vec<f32>>> {
        // With default InitOptions
        let model = TextEmbedding::try_new(Default::default())?;

        // With custom InitOptions
        let model = TextEmbedding::try_new(
            InitOptions::new(EmbeddingModel::AllMiniLML6V2).with_show_download_progress(true),
        )?;

        // let documents = vec![
        //     "passage: Hello, World!",
        //     "query: Hello, World!",
        //     "passage: This is an example passage.",
        //     // You can leave out the prefix but it's recommended
        //     "fastembed-rs is licensed under Apache  2.0",
        // ];

        // Generate embeddings with the default batch size, 256
        let embeddings = model.embed(sentences, None)?;

        tracing::warn!("Embeddings length: {}", embeddings.len()); // -> Embeddings length: 4
        tracing::warn!("Embedding dimension: {}", embeddings[0].len()); // -> Embedding dimension: 384
        Ok(embeddings)
    }
}
