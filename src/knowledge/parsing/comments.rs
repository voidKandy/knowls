use super::comment_str_map::COMMENT_EXTENSION_MAP;
use lsp_types::Range;
use serde::{Deserialize, Serialize};
use std::{fmt::Debug, marker::PhantomData, sync::LazyLock};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedComment {
    pub content: String,
    pub range: Range,
}

impl ParsedComment {
    pub fn new(content: &str, range: Range) -> Self {
        Self {
            content: content.to_string().trim().to_string(),
            range,
        }
    }

    pub fn range_without_comment(&self, ext: &str) -> Option<Range> {
        if let Some(comment_info) = LazyLock::force(&COMMENT_EXTENSION_MAP).get(ext) {
            let chars_amt = comment_info.singleline().chars().count();
            let mut range = self.range.clone();
            range.start.character += chars_amt as u32;
            return Some(range);
        }
        None
    }

    pub fn content_without_comment(&self, ext: &str) -> Option<String> {
        if let Some(comment_info) = LazyLock::force(&COMMENT_EXTENSION_MAP).get(ext) {
            let chars_amt = comment_info.singleline().chars().count();
            let content = self.content.trim_start()[chars_amt..].to_string();
            return Some(content);
        }
        None
    }
}
