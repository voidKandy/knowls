use crate::{database::models::Knowledge, tui::config::get_data_dir};
use color_eyre::owo_colors::OwoColorize;
use knowls::{other_err, MainResult};
use std::{io::Write, path::PathBuf, sync::LazyLock};

const KNOWLEDGE_FOLDER: LazyLock<PathBuf> = LazyLock::new(|| {
    let mut path = get_data_dir().clone();
    path.push("knowledge/");
    if !path.exists() {
        if let Err(e) = std::fs::create_dir_all(&path) {
            tracing::error!("Could not create knowledge folder: {e:#?}")
        }
    }

    path
});

pub(super) fn knowledge_document(k: &Knowledge) -> MainResult<std::path::PathBuf> {
    let mut path = LazyLock::force(&KNOWLEDGE_FOLDER).clone();
    let sanitized = k.kid.to_string().replace("/", "_");
    path.push(format!("{sanitized}.md"));
    tracing::warn!("creating file: {path:#?}");
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .open(&path)
        .expect("failed to create file");
    file.write(&k.content.as_bytes())?;

    Ok(path)
}
