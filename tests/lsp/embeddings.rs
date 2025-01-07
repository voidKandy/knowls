#[tokio::test]
async fn embeddings_work() {
    let out = espx_lsp_server::embeddings::embed_sentences(vec![
        "this is a sentece about dogs",
        "this is a sentece about aliens",
        "this is a sentece about buildings",
        "this is a sentece about cats",
    ])
    .unwrap();
}
