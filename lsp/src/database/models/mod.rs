pub mod agent_memories;
pub mod block;
use super::error::{DatabaseError, DatabaseResult};
use crate::util::OneOf;
use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;
use tracing::debug;

#[derive(Debug, Clone)]
pub struct FieldQuery {
    name: String,
    val: serde_json::Value,
}

impl FieldQuery {
    pub fn new(name: &str, val: impl Serialize) -> DatabaseResult<Self> {
        Ok(Self {
            name: name.to_string(),
            val: serde_json::to_value(val)?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct QueryBuilder(String);
impl QueryBuilder {
    pub fn begin() -> Self {
        Self("BEGIN TRANSACTION;".to_string())
    }
    pub fn push(&mut self, query: &str) {
        self.0 = format!("{} {}", self.0, query);
    }
    #[tracing::instrument(name = "ending transaction building", skip(self))]
    pub fn end(mut self) -> String {
        self.push("COMMIT TRANSACTION;");
        tracing::warn!("TRANSACTION STRING: {}", self.0);
        self.0
    }
}

impl<'l> IntoOneOf<'l, Thing, FieldQuery> for Thing {
    fn one_of(me: &'l Self) -> OneOf<&'l Thing, &'l FieldQuery> {
        OneOf::Left(me)
    }
}

impl<'l> IntoOneOf<'l, Thing, FieldQuery> for FieldQuery {
    fn one_of(me: &'l Self) -> OneOf<&'l Thing, &'l FieldQuery> {
        OneOf::Right(me)
    }
}

pub trait IntoOneOf<'l, L, R> {
    fn one_of(me: &'l Self) -> OneOf<&'l L, &'l R>;
}

/// P is a params object. Should contain options of every possible field in Self
pub trait DatabaseStruct<'l, P>:
    Serialize + for<'de> Deserialize<'de> + Sized + IntoOneOf<'l, Self, P> + 'l
where
    P: Serialize + IntoOneOf<'l, Self, P> + 'l,
{
    /// table id of the struct
    fn db_id() -> &'static str;
    /// for easy access to ID from reference to trait object
    fn thing(&self) -> &Thing;
    fn content(oneof: &'l impl IntoOneOf<'l, Self, P>) -> DatabaseResult<String>;
    fn merge(oneof: &'l impl IntoOneOf<'l, Self, P>) -> DatabaseResult<String> {
        let str = Self::content(oneof)?;
        if !str.contains("CONTENT") {
            return Err(DatabaseError::Undefined(anyhow!(
                "invalid content statement: {}",
                str
            )));
        }
        Ok(format!(
            "MERGE {}",
            str.to_string().trim_start_matches("CONTENT")
        ))
    }

    fn upsert(params: &'l P) -> DatabaseResult<String>;

    /// Updates by either ID (Single) or matching field value (Multiple)
    fn update(
        oneof: &'l impl IntoOneOf<'l, Thing, FieldQuery>,
        params: &'l impl IntoOneOf<'l, Self, P>,
    ) -> DatabaseResult<String> {
        let q = match IntoOneOf::<Thing, FieldQuery>::one_of(oneof) {
            OneOf::Left(thing) => format!(
                "UPDATE {}:{} {};",
                Self::db_id(),
                thing.id,
                Self::merge(params)?
            ),
            OneOf::Right(fq) => format!(
                "UPDATE {} {} WHERE {} = {};",
                Self::db_id(),
                Self::merge(params)?,
                fq.name,
                fq.val
            ),
        };
        debug!("update query: {}", q);
        Ok(q)
    }

    /// Deletes by either ID (Single) or matching field value (Multiple)
    fn delete(oneof: &'l impl IntoOneOf<'l, Thing, FieldQuery>) -> DatabaseResult<String> {
        match IntoOneOf::<Thing, FieldQuery>::one_of(oneof) {
            OneOf::Left(thing) => Ok(format!("DELETE {}:{};", Self::db_id(), thing.id,)),
            OneOf::Right(fq) => Ok(format!(
                "DELETE {} WHERE {} = {};",
                Self::db_id(),
                fq.name,
                fq.val
            )),
        }
    }

    /// Selects by either ID (Single) or matching field value (Multiple)
    fn select(
        oneof: Option<&'l impl IntoOneOf<'l, Thing, FieldQuery>>,
        fieldname: Option<&str>,
    ) -> DatabaseResult<String> {
        match oneof.and_then(|of| Some(IntoOneOf::<Thing, FieldQuery>::one_of(of))) {
            Some(OneOf::Left(thing)) => Ok(format!(
                "SELECT {} FROM {}:{};",
                fieldname.unwrap_or("*"),
                Self::db_id(),
                thing.id,
            )),
            Some(OneOf::Right(fq)) => Ok(format!(
                "SELECT {} FROM {} WHERE {} = {};",
                fieldname.unwrap_or("*"),
                Self::db_id(),
                fq.name,
                fq.val
            )),
            None => Ok(format!(
                "SELECT {} FROM {};",
                fieldname.unwrap_or("*"),
                Self::db_id(),
            )),
        }
    }
}
