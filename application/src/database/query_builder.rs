use knowls::{other_err, util::oneof::OneOf, MainResult};
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;
#[derive(Debug, Clone)]
pub struct FieldQuery {
    name: String,
    val: serde_json::Value,
}

impl FieldQuery {
    pub fn new(name: &str, val: impl Serialize) -> MainResult<Self> {
        Ok(Self {
            name: name.to_string(),
            val: serde_json::to_value(val)?,
        })
    }
}

impl FieldQuery {
    /// Updates by either ID (Single) or matching field value (Multiple)
    /// Deletes by either ID (Single) or matching field value (Multiple)
    pub fn delete(&self, db_id: &str) -> String {
        format!("DELETE {} WHERE {} = {};", db_id, self.name, self.val)
    }

    /// Selects by either ID (Single) or matching field value (Multiple)
    pub fn select(self, db_id: &str, fieldname: Option<&str>) -> String {
        format!(
            "SELECT {} FROM {} WHERE {} = {};",
            fieldname.unwrap_or("*"),
            db_id,
            self.name,
            self.val
        )
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
    fn content(oneof: &'l impl IntoOneOf<'l, Self, P>) -> MainResult<String>;
    fn merge(oneof: &'l impl IntoOneOf<'l, Self, P>) -> MainResult<String> {
        let str = Self::content(oneof)?;
        if !str.contains("CONTENT") {
            return Err(other_err!("invalid content statement: {}", str));
        }
        Ok(format!(
            "MERGE {}",
            str.to_string().trim_start_matches("CONTENT")
        ))
    }

    fn upsert(params: &'l P) -> MainResult<String>;
}
