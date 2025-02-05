use surrealdb::sql::Thing;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Knowledge {
    /// will change down the line
    pub id: String,
    pub content: String,
}
