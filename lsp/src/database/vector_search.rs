use super::*;
use anyhow::anyhow;
use models::{block::DBBlock, DatabaseStruct, FieldQuery, QueryBuilder};

impl DBBlock {
    pub async fn all_with_no_embeddings(db: &Database) -> DatabaseResult<Vec<DBBlock>> {
        let field_query = FieldQuery::new("content_embedding", Option::<Vec<f32>>::None)?;
        let mut query = QueryBuilder::begin();
        let select = Self::select(Some(&field_query), Some(Self::db_id()))?;
        query.push(&select);

        let all: Vec<DBBlock> = db.client.query(query.end()).await?.take(0)?;
        Ok(all)
    }

    pub async fn get_relavent(
        db: &Database,
        embedding: Vec<f32>,
        threshold: f32,
    ) -> DatabaseResult<Vec<Self>> {
        let mut all_with_no_embeddings = Self::all_with_no_embeddings(db).await?;
        if !all_with_no_embeddings.is_empty() {
            debug!(
                "have to generate embeddings for the following documents: {:?}.",
                all_with_no_embeddings
                    .iter()
                    .map(|u| u.uri.as_str())
                    .collect::<Vec<&str>>(),
            );

            DBBlock::embed_all(&mut all_with_no_embeddings)?;
            let mut q = QueryBuilder::begin();
            for block in all_with_no_embeddings {
                q.push(&Self::update(block.thing(), &block)?)
            }
            db.client
                .query(q.end())
                .await
                .expect("failed to query transaction");
        }

        let query = format!("SELECT * FROM {} WHERE vector::similarity::cosine($this.content_embedding, $embedding) > {};", DBBlock::db_id(), threshold );
        let mut response = db
            .client
            .query(query)
            .bind(("embedding", embedding))
            .await?;
        let chunks: Vec<DBBlock> = response.take(0)?;
        Ok(chunks)
    }

    #[tracing::instrument(name = "embedding all document chunks")]
    pub fn embed_all(vec: &mut Vec<DBBlock>) -> anyhow::Result<()> {
        let all_embeds = crate::embeddings::get_passage_embeddings(
            vec.iter().map(|ch| ch.content.as_str()).collect(),
        )?;

        if all_embeds.len() != vec.len() {
            return Err(anyhow!(
                "expected {} embeddings, got {}",
                vec.len(),
                all_embeds.len()
            ));
        }

        let mut embiter = all_embeds.into_iter().enumerate();
        while let Some((i, emb)) = embiter.next() {
            vec[i].content_embedding = Some(emb);
        }

        Ok(())
    }
}
