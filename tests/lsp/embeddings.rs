use crate::helpers::test_state;

#[tokio::test]
async fn embeddings_work() {
    let out = espx_lsp_server::embeddings::embed_sentences(vec![
        "this is a sentece about dogs",
        "this is a sentece about aliens",
        "this is a sentece about buildings",
        "this is a sentece about cats",
    ])
    .unwrap();

    println!("out: {out:#?}");
    assert!(false)
}

use serde::{Deserialize, Serialize};
use surrealdb::sql::{Id, Thing};
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct EmbeddedSentence {
    content: String,
    embedding: Vec<Vec<f32>>,
}

#[tokio::test]
async fn db_works() {
    let state = test_state(true).await;
    let mut w = state.0.write().await;
    let db = w.database.as_mut().unwrap();
    // db.init_thread().await.unwrap();

    let sentences = vec![
        "this is a sentece about dogs",
        "this is a sentece about aliens",
        "this is a sentece about buildings",
        "this is a sentece about cats",
    ];
    let out = espx_lsp_server::embeddings::embed_sentences(sentences.clone());
    // let thread = db.thread.as_mut().unwrap();
    // for (c, embedding) in sentences.into_iter().zip(out) {
    //     let sentence = EmbeddedSentence {
    //         content: c.to_string(),
    //         embedding,
    //     };
    //     let _ = thread
    //         .client
    //         .create::<Option<EmbeddedSentence>>("sentence")
    //         .content(sentence);
    // }
}
