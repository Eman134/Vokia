use std::path::{Path, PathBuf};

use serde::Deserialize;
use tauri::ipc::{InvokeBody, Request};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

use crate::attachments::{self, Attachment, AttachmentError, ATTACHMENTS_PER_MESSAGE};
use crate::db::{Db, DbState};

const PROJECT_HEADER: &str = "x-vokia-project";
const NAME_HEADER: &str = "x-vokia-name";
const RAW_BODY_EXPECTED: &str = "corpo binário esperado";

#[tauri::command]
pub async fn pick_attachment_files(app: AppHandle) -> Vec<String> {
    app.dialog()
        .file()
        .blocking_pick_files()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|path| path.into_path().ok())
        .map(|path| path.to_string_lossy().into_owned())
        .collect()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SaveAttachmentFilesArgs {
    pub project_id: i64,
    pub paths: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiscardAttachmentArgs {
    pub project_id: i64,
    pub id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReadAttachmentPreviewArgs {
    pub path: String,
}

pub(crate) async fn save_attachment_files_exec(
    app: &AppHandle,
    args: SaveAttachmentFilesArgs,
) -> Result<Vec<Attachment>, String> {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| error.to_string())?;
    let data_dir = data_dir(app)?;
    if args.paths.len() > ATTACHMENTS_PER_MESSAGE {
        return Err(AttachmentError::TooMany {
            limit: ATTACHMENTS_PER_MESSAGE,
        }
        .to_string());
    }
    let mut saved = Vec::with_capacity(args.paths.len());
    for path in &args.paths {
        match attachments::save_file(db, &data_dir, args.project_id, Path::new(path)) {
            Ok(attachment) => saved.push(attachment),
            Err(error) => {
                discard_all(db, &data_dir, args.project_id, &saved);
                return Err(error.to_string());
            }
        }
    }
    Ok(saved)
}

pub(crate) async fn discard_attachment_exec(
    app: &AppHandle,
    args: DiscardAttachmentArgs,
) -> Result<(), String> {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| error.to_string())?;
    attachments::discard(db, &data_dir(app)?, args.project_id, &args.id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn save_attachment_files(
    app: AppHandle,
    project_id: i64,
    paths: Vec<String>,
) -> Result<Vec<Attachment>, String> {
    save_attachment_files_exec(&app, SaveAttachmentFilesArgs { project_id, paths }).await
}

#[tauri::command]
pub async fn save_attachment_bytes(
    app: AppHandle,
    state: State<'_, DbState>,
    request: Request<'_>,
) -> Result<Attachment, String> {
    let InvokeBody::Raw(bytes) = request.body() else {
        return Err(RAW_BODY_EXPECTED.to_string());
    };
    let project_id = header(&request, PROJECT_HEADER)?
        .parse::<i64>()
        .map_err(|_| header_error(PROJECT_HEADER))?;
    let name =
        percent_decode(header(&request, NAME_HEADER)?).ok_or_else(|| header_error(NAME_HEADER))?;
    let db = state.require().map_err(|error| error.to_string())?;
    attachments::save_bytes(db, &data_dir(&app)?, project_id, &name, bytes)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn discard_attachment(app: AppHandle, project_id: i64, id: String) -> Result<(), String> {
    discard_attachment_exec(&app, DiscardAttachmentArgs { project_id, id }).await
}

pub(crate) fn read_attachment_preview_exec(
    app: &AppHandle,
    args: ReadAttachmentPreviewArgs,
) -> Result<String, String> {
    attachments::read_image_data_url(&data_dir(app)?, Path::new(&args.path))
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn read_attachment_preview(app: AppHandle, path: String) -> Result<String, String> {
    read_attachment_preview_exec(&app, ReadAttachmentPreviewArgs { path })
}

// A operação principal já deu certo; sobra de arquivo é varrida na próxima abertura do app.
pub fn purge_project_files(app: &AppHandle, db: &Db, project_id: i64) {
    if let Ok(data_dir) = app.path().app_data_dir() {
        let _ = attachments::purge_project(db, &data_dir, project_id);
    }
}

// Anexos ainda no composer não pertencem à conversa apagada e o front ainda guarda seus ids.
pub fn purge_sent_files(app: &AppHandle, db: &Db, project_id: i64) {
    if let Ok(data_dir) = app.path().app_data_dir() {
        let _ = attachments::purge_sent(db, &data_dir, project_id);
    }
}

fn data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path().app_data_dir().map_err(|error| error.to_string())
}

// Falha ao desfazer deixa só pastas já sem linha útil; o erro que importa é o do anexo que falhou.
fn discard_all(db: &Db, data_dir: &Path, project_id: i64, saved: &[Attachment]) {
    for attachment in saved {
        let _ = attachments::discard(db, data_dir, project_id, &attachment.id);
    }
}

fn header<'a>(request: &'a Request<'_>, name: &str) -> Result<&'a str, String> {
    request
        .headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| header_error(name))
}

fn header_error(name: &str) -> String {
    format!("cabeçalho {name} ausente ou inválido")
}

fn percent_decode(encoded: &str) -> Option<String> {
    let bytes = encoded.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hex = encoded.get(index + 1..index + 3)?;
            decoded.push(u8::from_str_radix(hex, 16).ok()?);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).ok()
}
