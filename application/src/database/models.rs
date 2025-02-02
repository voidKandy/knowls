use surrealdb::sql::Thing;

#[derive(Debug, Clone)]
pub struct Knowledge {
    /// will change down the line
    pub id: String,
    pub content: String,
}
