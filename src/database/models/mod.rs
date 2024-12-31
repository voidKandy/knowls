use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

use crate::{other_err, MainResult};

pub mod agent_memories;
pub mod block;

pub trait DBItem: Serialize + for<'de> Deserialize<'de> {
    const DB_ID: &str;
    fn thing(&self) -> &Thing;

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
