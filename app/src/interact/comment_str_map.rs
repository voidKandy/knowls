use super::{InteractError, InteractResult};
use std::{collections::HashMap, sync::LazyLock};

#[derive(Debug, Clone)]
pub(super) struct CommentStrInfo<'i> {
    singleline: &'i str,
    multiline: Option<MultilineCommentInfo<'i>>,
}

#[derive(Debug, Clone)]
pub(super) struct MultilineCommentInfo<'i> {
    start: &'i str,
    end: &'i str,
}

impl<'i> CommentStrInfo<'i> {
    pub fn singleline(&self) -> &str {
        &self.singleline
    }

    pub fn multiline_start(&self) -> Option<&str> {
        Some(&self.multiline.as_ref()?.start)
    }

    pub fn multiline_end(&self) -> Option<&str> {
        Some(&self.multiline.as_ref()?.end)
    }
    /// at which char idx does the multiline start differ from a singleline
    pub fn difference_idx(&self) -> Option<usize> {
        let multi = self.multiline_start()?;
        let single = self.singleline();

        for (i, (mchar, schar)) in multi.chars().zip(single.chars()).enumerate() {
            if mchar != schar {
                return Some(i);
            }
        }

        tracing::error!("start to multiline is the same as a singleline");
        None
    }
}

pub(super) fn get_comment_string_info(ext: &str) -> InteractResult<CommentStrInfo> {
    let m = COMMENT_EXTENSION_MAP;
    let map = LazyLock::force(&m);
    let comment_str = map
        .get(ext)
        .ok_or(InteractError::UnhandledLanguageExtension(ext.to_string()))?;
    Ok(comment_str.clone())
}

const COMMENT_EXTENSION_MAP: LazyLock<HashMap<&str, CommentStrInfo>> = LazyLock::new(|| {
    let mut map = HashMap::new();

    map.insert(
        "rs",
        CommentStrInfo {
            singleline: "//",
            multiline: Some(MultilineCommentInfo {
                start: "/*",
                end: "*/",
            }),
        },
    );

    map.insert(
        "c",
        CommentStrInfo {
            singleline: "//",
            multiline: Some(MultilineCommentInfo {
                start: "/*",
                end: "*/",
            }),
        },
    );

    map.insert(
        "cpp",
        CommentStrInfo {
            singleline: "//",
            multiline: Some(MultilineCommentInfo {
                start: "/*",
                end: "*/",
            }),
        },
    );

    map.insert(
        "java",
        CommentStrInfo {
            singleline: "//",
            multiline: Some(MultilineCommentInfo {
                start: "/*",
                end: "*/",
            }),
        },
    );

    map.insert(
        "js",
        CommentStrInfo {
            singleline: "//",
            multiline: Some(MultilineCommentInfo {
                start: "/*",
                end: "*/",
            }),
        },
    );

    map.insert(
        "py",
        CommentStrInfo {
            singleline: "#",
            multiline: None,
        },
    );

    map.insert(
        "rb",
        CommentStrInfo {
            singleline: "#",
            multiline: None,
        },
    );

    map.insert(
        "php",
        CommentStrInfo {
            singleline: "//",
            multiline: Some(MultilineCommentInfo {
                start: "/*",
                end: "*/",
            }),
        },
    );

    map.insert(
        "cs",
        CommentStrInfo {
            singleline: "//",
            multiline: Some(MultilineCommentInfo {
                start: "/*",
                end: "*/",
            }),
        },
    );

    map.insert(
        "swift",
        CommentStrInfo {
            singleline: "//",
            multiline: Some(MultilineCommentInfo {
                start: "/*",
                end: "*/",
            }),
        },
    );

    map.insert(
        "kt",
        CommentStrInfo {
            singleline: "//",
            multiline: Some(MultilineCommentInfo {
                start: "/*",
                end: "*/",
            }),
        },
    );

    map.insert(
        "pl",
        CommentStrInfo {
            singleline: "#",
            multiline: None,
        },
    );

    map.insert(
        "sh",
        CommentStrInfo {
            singleline: "#",
            multiline: None,
        },
    );

    map.insert(
        "lua",
        CommentStrInfo {
            singleline: "--",
            multiline: Some(MultilineCommentInfo {
                start: "--[[",
                end: "]]",
            }),
        },
    );

    map.insert(
        "hs",
        CommentStrInfo {
            singleline: "--",
            multiline: None,
        },
    );

    map.insert(
        "erl",
        CommentStrInfo {
            singleline: "%",
            multiline: None,
        },
    );

    map.insert(
        "ex",
        CommentStrInfo {
            singleline: "#",
            multiline: None,
        },
    );

    map.insert(
        "html",
        CommentStrInfo {
            singleline: "<!--",
            multiline: None,
        },
    );

    map.insert(
        "xml",
        CommentStrInfo {
            singleline: "<!--",
            multiline: None,
        },
    );

    map.insert(
        "sql",
        CommentStrInfo {
            singleline: "--",
            multiline: None,
        },
    );

    map.insert(
        "v",
        CommentStrInfo {
            singleline: "//",
            multiline: Some(MultilineCommentInfo {
                start: "/*",
                end: "*/",
            }),
        },
    );

    map.insert(
        "go",
        CommentStrInfo {
            singleline: "//",
            multiline: Some(MultilineCommentInfo {
                start: "/*",
                end: "*/",
            }),
        },
    );

    map.insert(
        "d",
        CommentStrInfo {
            singleline: "//",
            multiline: Some(MultilineCommentInfo {
                start: "/*",
                end: "*/",
            }),
        },
    );

    map.insert(
        "scala",
        CommentStrInfo {
            singleline: "//",
            multiline: Some(MultilineCommentInfo {
                start: "/*",
                end: "*/",
            }),
        },
    );

    map.insert(
        "sh",
        CommentStrInfo {
            singleline: "#",
            multiline: None,
        },
    );

    map.insert(
        "r",
        CommentStrInfo {
            singleline: "#",
            multiline: None,
        },
    );

    map.insert(
        "cob",
        CommentStrInfo {
            singleline: "*",
            multiline: None,
        },
    );

    map.insert(
        "f90",
        CommentStrInfo {
            singleline: "!",
            multiline: None,
        },
    );

    map
});

mod tests {
    use super::get_comment_string_info;

    #[test]
    fn diff_idx_works() {
        let info = get_comment_string_info("rs").unwrap();
        assert_eq!(info.difference_idx(), Some(1));
    }
}
