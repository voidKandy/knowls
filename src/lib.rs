pub mod agents;
pub mod config;
pub mod database;
pub mod error;
pub mod handle;
pub mod interact;
pub mod sockets;
pub mod state;
pub mod telemetry;
pub mod ui;
pub(crate) mod util;

pub mod embeddings {

    use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};

    pub fn embed_sentences(sentences: Vec<&str>) -> anyhow::Result<Vec<Vec<f32>>> {
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

        println!("Embeddings length: {}", embeddings.len()); // -> Embeddings length: 4
        println!("Embedding dimension: {}", embeddings[0].len()); // -> Embedding dimension: 384
        Ok(embeddings)
    }

    mod tests {}
}
