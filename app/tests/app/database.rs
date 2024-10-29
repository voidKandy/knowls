use crate::{
    helpers::{test_state, TEST_TRACING},
    test_docs::*,
};
use espionox::prelude::{Message, MessageStack};
use espx_app::{
    database::models::{
        agent_memories::{DBAgentID, DBAgentMemory, DBAgentMemoryParams},
        block::{block_params_from, DBBlock, DBBlockParams},
        DatabaseStruct, FieldQuery, QueryBuilder,
    },
    embeddings,
    interact::lexer::Lexer,
};
use std::{io::Read, sync::LazyLock};
use tracing::warn;

#[tokio::test]
async fn health_test() {
    LazyLock::force(&TEST_TRACING);
    let mut state = test_state(true);
    let mut w = state.get_write().unwrap();
    w.database.as_mut().unwrap().init_thread().await.unwrap();
    if let Err(err) = w
        .database
        .as_ref()
        .unwrap()
        .thread
        .as_ref()
        .unwrap()
        .client
        .health()
        .await
    {
        panic!("unhealthy database: {err:#?}")
    }

    let mut buf = String::new();
    w.database
        .as_mut()
        .unwrap()
        .thread
        .as_mut()
        .unwrap()
        .stdout
        .read_to_string(&mut buf)
        .unwrap();
    warn!("Gt buf: {buf}");
    assert!(!buf.is_empty());
}

// #[tokio::test]
// async fn get_relavent_blocks() {
//     let state = test_state(true).await;
//     let r = state.get_read().unwrap();
//     let db = r.database.as_ref().unwrap();
//     let (uri, _) = test_doc_1();
//
//     let relavent_blocks = vec![
//         DBBlockParams::new(uri.clone(), 0, Some("i ate a sandwich".to_owned())),
//         DBBlockParams::new(uri.clone(), 1, Some("i ate a watermelon".to_owned())),
//         DBBlockParams::new(uri.clone(), 2, Some("i ate a burrito".to_owned())),
//         DBBlockParams::new(uri.clone(), 3, Some("i ate some beans".to_owned())),
//     ];
//
//     let irrelavent_blocks = vec![
//         DBBlockParams::new(uri.clone(), 4, Some("walking to the store".to_owned())),
//         DBBlockParams::new(uri.clone(), 5, Some("walking to church".to_owned())),
//         DBBlockParams::new(uri.clone(), 6, Some("walking home".to_owned())),
//     ];
//
//     let mut q = QueryBuilder::begin();
//     for b in relavent_blocks.iter() {
//         q.push(&DBBlock::upsert(b).unwrap())
//     }
//     for b in irrelavent_blocks.iter() {
//         q.push(&DBBlock::upsert(b).unwrap())
//     }
//     db.client.query(q.end()).await.unwrap();
//
//     let embedding = embeddings::get_passage_embeddings(vec!["eating is involved"])
//         .unwrap()
//         .into_iter()
//         .next()
//         .unwrap();
//
//     let relavent = DBBlock::get_relavent(db, embedding, 0.5).await.unwrap();
//
//     let r_contents = relavent
//         .iter()
//         .map(|b| b.content.as_str())
//         .collect::<Vec<&str>>();
//     for b in relavent_blocks.iter() {
//         if !r_contents.contains(&b.content.as_ref().unwrap().as_str()) {
//             panic!(
//                 "returned relavent blocks should contain: {b:#?}\nreturned: {relavent_blocks:#?}"
//             )
//         }
//     }
//     for b in irrelavent_blocks.iter() {
//         if r_contents.contains(&b.content.as_ref().unwrap().as_str()) {
//             panic!(
//                 "returned relavent blocks should not contain: {b:#?}\nreturned: {relavent_blocks:#?}"
//             )
//         }
//     }
// }

#[tokio::test]
async fn tokens_crud_test() {
    LazyLock::force(&TEST_TRACING);
    let mut state = test_state(true);
    let mut w = state.get_write().unwrap();
    let all_test_docs = vec![
        test_doc_1(),
        test_doc_2(),
        test_doc_3(),
        test_doc_4(),
        test_doc_5(),
    ];

    let registry = w.registry.clone();
    let mut all_block_params = vec![];
    for (uri, content) in all_test_docs.iter() {
        let uri_str = uri.to_string();

        let ext = uri_str
            .rsplit_once('.')
            .expect("uri does not have extension")
            .1;
        let mut lexer = Lexer::new(&content, ext);
        let tokens = lexer.lex_input(&registry);

        all_block_params.push(block_params_from(&tokens, uri.clone()));
    }

    let db = w.database.as_mut().unwrap();
    db.init_thread().await.unwrap();
    let _: Vec<DBBlock> = db
        .thread
        .as_ref()
        .unwrap()
        .client
        .delete(DBBlock::db_id())
        .await
        .unwrap();

    let mut query = QueryBuilder::begin();
    for document_params in all_block_params.iter() {
        for params in document_params {
            query.push(&DBBlock::upsert(params).unwrap());
        }
    }
    db.thread
        .as_ref()
        .unwrap()
        .client
        .query(query.end())
        .await
        .unwrap();

    let all: Vec<DBBlock> = db
        .thread
        .as_ref()
        .unwrap()
        .client
        .select(DBBlock::db_id())
        .await
        .unwrap();
    assert_eq!(
        all.len(),
        all_block_params
            .iter()
            .fold(0, |acc, vec| { acc + vec.len() })
    );

    let mut query = QueryBuilder::begin();
    let first_doc_uri = all_test_docs.iter().nth(0).cloned().unwrap().0;
    let len_not_that_uri = all_block_params.iter().fold(0, |acc, vec| {
        acc + vec
            .iter()
            .filter(|p| p.uri.as_ref() != Some(&first_doc_uri))
            .count()
    });

    query.push(&DBBlock::delete(&FieldQuery::new("uri", first_doc_uri).unwrap()).unwrap());

    db.thread
        .as_ref()
        .unwrap()
        .client
        .query(query.end())
        .await
        .unwrap();

    let all: Vec<DBBlock> = db
        .thread
        .as_ref()
        .unwrap()
        .client
        .select(DBBlock::db_id())
        .await
        .unwrap();
    assert_eq!(all.len(), len_not_that_uri);

    let second_doc_uri = all_test_docs.iter().nth(1).cloned().unwrap().0;
    let fq = FieldQuery::new("uri", second_doc_uri).unwrap();

    let all_to_update: Vec<DBBlock> = db
        .thread
        .as_ref()
        .unwrap()
        .client
        .query(DBBlock::select(Some(&fq), None).unwrap())
        .await
        .unwrap()
        .take(0)
        .unwrap();

    let mut q = QueryBuilder::begin();

    let new_content = "All blocks of this uri have the same content now".to_string();

    for mut dbb in all_to_update {
        dbb.content = new_content.to_owned();
        q.push(&DBBlock::update(dbb.thing(), &dbb).unwrap());
    }
    let updated: Vec<DBBlock> = db
        .thread
        .as_ref()
        .unwrap()
        .client
        .query(&q.end())
        .await
        .unwrap()
        .take(0)
        .unwrap();

    for block in updated {
        assert_eq!(block.content.as_str(), new_content.as_str());
    }
    let _: Vec<DBBlock> = db
        .thread
        .as_ref()
        .unwrap()
        .client
        .delete(DBBlock::db_id())
        .await
        .unwrap();
}

#[tokio::test]
async fn memories_crud_test() {
    LazyLock::force(&TEST_TRACING);
    let mut state = test_state(true);
    let mut w = state.get_write().unwrap();

    let db = w.database.as_mut().unwrap();
    db.init_thread().await.unwrap();
    let thread = db.thread.as_ref().unwrap();
    let _: Vec<DBAgentMemory> = thread.client.delete(DBAgentMemory::db_id()).await.unwrap();

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

    let mut all_params = vec![];

    all_params.push(DBAgentMemoryParams::new(&agent_char_1, Some(&test_mems_1)));
    all_params.push(DBAgentMemoryParams::new(&agent_uri_2, Some(&test_mems_2)));

    let mut q = QueryBuilder::begin();

    for param in all_params.iter() {
        q.push(&DBAgentMemory::upsert(&param).unwrap())
    }

    thread.client.query(q.end()).await.unwrap();

    let all: Vec<DBAgentMemory> = thread.client.select(DBAgentMemory::db_id()).await.unwrap();

    assert_eq!(all.len(), all_params.len(),);

    let agent_2: DBAgentMemory = thread
        .client
        .select((
            DBAgentMemory::db_id(),
            DBAgentID::from(&agent_uri_2).to_string(),
        ))
        .await
        .unwrap()
        .unwrap();

    assert_eq!(agent_2.messages, test_mems_2);
    // let mut q = QueryBuilder::begin();

    // q.push(&DBBlock::delete(&FieldQuery::new("uri", ).unwrap()).unwrap());
}
