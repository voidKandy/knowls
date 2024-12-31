use crate::{
    helpers::{test_state, TEST_TRACING},
    test_docs::*,
};
use espionox::prelude::{Message, MessageStack};
use espx_lsp_server::{
    agents::AgentID,
    database::{
        models::{agent_memories::DBAgentMemory, block::DBBlock, DBItem, EmbeddedDBItem},
        query_builder::{FieldQuery, QueryBuilder},
    },
    interact::parsing::lexer::Lexer,
};
use std::sync::LazyLock;
use surrealdb::RecordId;
use tracing::warn;

#[tokio::test]
async fn health_test() {
    LazyLock::force(&TEST_TRACING);
    let state = test_state(true).await;
    let w = state.0.try_write().unwrap();
    if let Err(err) = w.database.as_ref().unwrap().client.health().await {
        panic!("unhealthy database: {err:#?}")
    }
}

#[tokio::test]
async fn get_relavent_blocks() {
    let state = test_state(true).await;
    let r = state.0.read().await;
    let db = r.database.as_ref().unwrap();
    let (uri, _) = test_doc_1();

    let _: Vec<DBBlock> = db.client.delete(DBBlock::DB_ID).await.unwrap();

    let relavent_blocks = vec![
        DBBlock::new(uri.clone(), 0, "i ate a sandwich".to_owned()),
        DBBlock::new(uri.clone(), 1, "i ate a watermelon".to_owned()),
        DBBlock::new(uri.clone(), 2, "i ate a burrito".to_owned()),
        DBBlock::new(uri.clone(), 3, "i ate some beans".to_owned()),
    ];

    let irrelavent_blocks = vec![
        DBBlock::new(uri.clone(), 4, "walking to the store".to_owned()),
        DBBlock::new(uri.clone(), 5, "walking to church".to_owned()),
        DBBlock::new(uri.clone(), 6, "walking home".to_owned()),
    ];

    let upserted = DBBlock::embed_batch_and_upsert(relavent_blocks.clone(), db)
        .await
        .unwrap();

    assert_eq!(
        upserted.len(),
        relavent_blocks.len(),
        "upserted relavent blocks returned an unexpected amount"
    );

    let upserted = DBBlock::embed_batch_and_upsert(irrelavent_blocks.clone(), db)
        .await
        .unwrap();

    assert_eq!(
        upserted.len(),
        irrelavent_blocks.len(),
        "upserted irrelavent blocks returned an unexpected amount"
    );

    let embedding = espx_lsp_server::embeddings::embed_sentences(vec!["I ate something"])
        .unwrap()
        .into_iter()
        .next()
        .unwrap();

    let similar = DBBlock::query_for_similar(db, embedding, 0.4)
        .await
        .unwrap();

    let similar: Vec<DBBlock> = similar
        .into_iter()
        .map(|mut s| {
            s.embedding = None;
            s
        })
        .collect();

    assert_eq!(similar, relavent_blocks);
}

#[tokio::test]
async fn tokens_crud_test() {
    LazyLock::force(&TEST_TRACING);
    let state = test_state(true).await;
    let mut w = state.0.try_write().unwrap();
    let all_test_docs = vec![
        test_doc_1(),
        test_doc_2(),
        test_doc_3(),
        test_doc_4(),
        test_doc_5(),
    ];

    let mut all_blocks: Vec<Vec<DBBlock>> = vec![];
    let db = w.database.as_mut().unwrap();
    let _: Vec<DBBlock> = db.client.delete("block").await.unwrap();

    for (uri, content) in all_test_docs.iter() {
        let uri_str = uri.to_string();

        let ext = uri_str
            .rsplit_once('.')
            .expect("uri does not have extension")
            .1;
        let mut lexer = Lexer::new(&content, ext);
        let tokens = lexer.lex_input();

        let blocks = DBBlock::from_tokens(&tokens, uri.clone());

        let mut ret_blocks = vec![];
        for b in blocks {
            let content = b.content_without_id().unwrap();
            let ret: Option<DBBlock> = db
                .client
                .upsert(b.record_id())
                .content(content)
                .await
                .unwrap();
            warn!("upserted: {ret:#?}");
            ret_blocks.push(ret.unwrap())
        }
        all_blocks.push(ret_blocks);
    }

    let all: Vec<DBBlock> = db.client.select("block").await.unwrap();
    assert_eq!(
        all.len(),
        all_blocks.iter().fold(0, |acc, vec| { acc + vec.len() })
    );
    warn!("blocks in database {all:#?}");

    let first_doc_uri = all_test_docs.iter().nth(0).cloned().unwrap().0;
    let all_that_uri: Vec<RecordId> = all_blocks
        .iter()
        .find(|vec| vec[0].uri == first_doc_uri)
        .unwrap()
        .iter()
        .map(|b| b.record_id())
        .collect();

    // query.push(&DBBlock::delete(&::new("uri", first_doc_uri).unwrap()).unwrap());

    // db.client.query(query.end()).await.unwrap();

    let expected_amt: usize = all_blocks.len() - all_that_uri.len();
    for id in all_that_uri {
        warn!("deleting block with id {}", id.to_string());
        let b: Option<DBBlock> = db.client.delete(id).await.unwrap();
        assert!(b.is_some(), "failed to delete");
    }

    let all: Vec<DBBlock> = db.client.select("block").await.unwrap();
    assert_eq!(all.len(), expected_amt);

    let second_doc_uri = all_test_docs.iter().nth(1).cloned().unwrap().0;

    let mut q = QueryBuilder::begin();
    let fq = FieldQuery::new("uri", second_doc_uri).unwrap();
    let select = fq.select("uri", None);
    q.push(&select);
    let all_to_update: Vec<DBBlock> = db.client.query(q.end()).await.unwrap().take(0).unwrap();
    let new_content = "All blocks of this uri have the same content now".to_string();

    let mut updated: Vec<DBBlock> = vec![];
    for mut dbb in all_to_update {
        dbb.content = new_content.to_owned();
        let content = dbb.content_without_id().unwrap();
        let block: Option<DBBlock> = db
            .client
            .update(dbb.record_id())
            .content(content)
            .await
            .unwrap();
        updated.push(block.unwrap());
    }

    for block in updated {
        assert_eq!(block.content.as_str(), new_content.as_str());
    }
    let _: Vec<DBBlock> = db.client.delete("block").await.unwrap();
}

#[tokio::test]
async fn memories_crud_test() {
    LazyLock::force(&TEST_TRACING);
    let mut state = test_state(true).await;
    let mut w = state.0.try_write().unwrap();

    let db = w.database.as_mut().unwrap();
    let _: Vec<DBAgentMemory> = db.client.delete("agent_memory").await.unwrap();

    let test_mems_1: MessageStack = vec![
        Message::new_system("some system prompt"),
        Message::new_user("some user messagee "),
        Message::new_user("another user messagee "),
        Message::new_user("another another user messagee "),
    ]
    .into();

    let test_mems_2: MessageStack = vec![
        Message::new_system("some system prompt 2"),
        Message::new_user("some user messagee 2"),
        Message::new_user("another user messagee 2"),
        Message::new_user("another another user message 2"),
    ]
    .into();

    let agent_char_1 = 'c';
    let (agent_uri_2, _) = test_doc_1();
    let id = AgentID::Char(agent_char_1);
    let memory = DBAgentMemory::new(&id, test_mems_1);

    let _: Option<DBAgentMemory> = db
        .client
        .upsert(("agent_memory", id.to_string()))
        .content(memory.content_without_id().unwrap())
        .await
        .unwrap();

    let id = AgentID::from(&agent_uri_2);
    let memory = DBAgentMemory::new(&id, test_mems_2.clone());

    let _: Option<DBAgentMemory> = db
        .client
        .upsert(("agent_memory", id.to_string()))
        .content(memory.content_without_id().unwrap())
        .await
        .unwrap();

    let all: Vec<DBAgentMemory> = db.client.select("agent_memory").await.unwrap();

    assert_eq!(all.len(), 2);

    warn!("all mems in db: {all:#?}");

    let agent_2: DBAgentMemory = db
        .client
        .select(("agent_memory", AgentID::from(&agent_uri_2).to_string()))
        .await
        .unwrap()
        .unwrap();

    assert_eq!(agent_2.messages, test_mems_2);
}
