// use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
// use tracing::debug;
//
// pub fn get_passage_embeddings(texts: Vec<&str>) -> anyhow::Result<Vec<Vec<f32>>> {
//     // With default InitOptions
//     // let model = TextEmbedding::try_new(Default::default())?;
//
//     // With custom InitOptions
//     //
//     let mut options = InitOptions::default();
//     options.model_name = EmbeddingModel::AllMiniLML6V2;
//     options.show_download_progress = false;
//
//     let model = TextEmbedding::try_new(options)?;
//     debug!("LOADED EMBEDDING MODEL");
//
//     // Generate embeddings with the default batch size, 256
//     let mut to_feed_model = vec![];
//     texts.into_iter().for_each(|t| {
//         to_feed_model.push(format!("passage: {}", t));
//     });
//
//     debug!("Getting Embeddings");
//     let embeddings = model.embed(to_feed_model, None)?;
//
//     debug!("Embeddings length: {}", embeddings.len()); // -> Embeddings length: 4
//     debug!("Embedding dimension: {}", embeddings[0].len()); // -> Embedding dimension: 384
//     Ok(embeddings)
// }
