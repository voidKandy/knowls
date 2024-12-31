use super::Database;
use crate::{
    embeddings::{embed_sentences, EmbeddingFloat},
    other_err, MainResult,
};
use serde::{Deserialize, Serialize};
use surrealdb::{sql::Thing, RecordId};
pub mod agent_memories;
pub mod block;

pub trait DBItem: std::fmt::Debug + Serialize + for<'de> Deserialize<'de> {
    const DB_ID: &str;
    fn thing(&self) -> &Thing;

    fn record_id(&self) -> RecordId {
        let id = &self.thing().id.to_raw();
        RecordId::from_table_key(Self::DB_ID, id)
    }

    fn content_without_id(&self) -> MainResult<serde_json::Value> {
        let val = serde_json::to_value(self).unwrap();
        let mut map = val
            .as_object()
            .ok_or(other_err!("failed to coerce serde_json::Value to a map"))?
            .to_owned();
        map.remove("id");
        let val = serde_json::to_value(map).expect("map should not fail to serialize");
        Ok(val)
    }
}

#[allow(async_fn_in_trait)]
pub trait EmbeddedDBItem: DBItem {
    /// the name of this field *MUST* be 'embedding' for the queries to work
    fn embedding(&self) -> Option<&[EmbeddingFloat]>;
    /// Should update self to include given embedding
    fn update_with_embedding(&mut self, embedding: Vec<EmbeddingFloat>);
    fn content_to_embed(&self) -> &str;
    fn embed_my_content(&self) -> MainResult<Vec<EmbeddingFloat>> {
        let content = self.content_to_embed();
        let vec = embed_sentences(vec![&content])?;
        vec.into_iter()
            .next()
            .ok_or(other_err!("produced empty embeddings"))
    }

    fn embed_batch(vec: Vec<&Self>) -> MainResult<Vec<Vec<EmbeddingFloat>>> {
        let all_content: Vec<&str> = vec.iter().map(|v| v.content_to_embed()).collect();
        embed_sentences(all_content)
    }

    // this could eventually be made much more efficient with the use of an SQL transaction
    /// This will take a vector of the trait object, embed all of them, and then *upsert* to the DB
    /// Since this uses *upsert*, it can be used to create instances in the database
    #[tracing::instrument(name = "embed batch and upsert", skip_all)]
    async fn embed_batch_and_upsert(mut vec: Vec<Self>, db: &Database) -> MainResult<Vec<Self>> {
        let all_content: Vec<&str> = vec.iter().map(|v| v.content_to_embed()).collect();
        let embeds = embed_sentences(all_content)?;

        let mut all = vec![];
        tracing::warn!("embedded {} instances", embeds.len());
        for (i, emb) in embeds.into_iter().enumerate() {
            let val: &mut Self = vec
                .iter_mut()
                .nth(i)
                .expect("embeddings vector is larger than input vector");
            val.update_with_embedding(emb);
            let r: Option<Self> = db
                .client
                .upsert(vec[i].record_id())
                .content(vec[i].content_without_id().unwrap())
                .await
                .unwrap();
            all.push(r.ok_or(other_err!("failed to update {:#?}", vec[i]))?);
            tracing::warn!("pushed to all: {all:#?}");
        }
        Ok(all)
    }

    async fn query_for_similar(
        db: &Database,
        comparison_vec: Vec<EmbeddingFloat>,
        threshold: f32,
    ) -> MainResult<Vec<Self>> {
        // there are many more similarity algos: https://surrealdb.com/docs/surrealql/functions/database/vector
        let query = format!(
            r#"
            LET $all = SELECT * FROM {} WHERE type::is::array($this.embedding);
            SELECT * FROM $all WHERE vector::similarity::cosine($this.embedding, $embedding) > {};"#,
            Self::DB_ID,
            threshold
        );
        let mut response = db
            .client
            .query(query)
            .bind(("embedding", comparison_vec))
            .await?;
        // need to take 1 because the above query has 2 queries, the first of which returns NONE
        let all: Vec<Self> = response.take(1)?;
        Ok(all)
    }
}
