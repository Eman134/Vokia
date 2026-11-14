use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager};

use crate::state::AppState;
use crate::task::TaskRecord;

use super::error::FlowError;

const ARCHIVE_LIMIT: usize = 20;
const ARCHIVE_FILE: &str = "tasks-archive.json";

pub(super) fn archive_task(
    app: &AppHandle,
    state: &AppState,
    record: TaskRecord,
) -> Result<(), FlowError> {
    let mut archive = state
        .archive
        .lock()
        .map_err(|_| FlowError::ArchivePoisoned)?;
    archive.push(record);
    if archive.len() > ARCHIVE_LIMIT {
        archive.remove(0);
    }
    write_archive(&archive_path(app)?, &archive)
}

fn archive_path(app: &AppHandle) -> Result<PathBuf, FlowError> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(FlowError::ArchivePath)?
        .join(ARCHIVE_FILE))
}

fn write_archive(path: &Path, records: &[TaskRecord]) -> Result<(), FlowError> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(FlowError::ArchiveIo)?;
    }
    let json = serde_json::to_vec(records).map_err(FlowError::ArchiveJson)?;
    fs::write(path, json).map_err(FlowError::ArchiveIo)
}

pub(crate) fn remove_archive(app: &AppHandle, state: &AppState) -> Result<(), FlowError> {
    state
        .archive
        .lock()
        .map_err(|_| FlowError::ArchivePoisoned)?
        .clear();
    match fs::remove_file(archive_path(app)?) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(FlowError::ArchiveIo(error)),
    }
}
