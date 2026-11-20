mod error;

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::db::Db;
use crate::mcp::random_hex;

pub use error::AttachmentError;

pub const ATTACHMENTS_DIR: &str = "attachments";
pub const ATTACHMENT_MAX_BYTES: u64 = 20_971_520;
pub const ATTACHMENTS_PER_MESSAGE: usize = 10;
pub const ATTACHMENTS_HEADER: &str = "Anexos do usuário:";
pub const ATTACHMENTS_FOOTER: &str = "Leia um anexo com a ferramenta Read só quando ele for necessário. Ao delegar, inclua no pedido o caminho dos anexos de que o outro agente precisar.";

const ID_BYTES: usize = 8;
const FALLBACK_NAME: &str = "anexo";
const NAME_MAX_CHARS: usize = 120;
const BYTES_PER_KB: u64 = 1024;
const BYTES_PER_MB: u64 = BYTES_PER_KB * 1024;
const FORBIDDEN_NAME_CHARS: [char; 7] = ['<', '>', ':', '"', '|', '?', '*'];
const RESERVED_DEVICE_NAMES: [&str; 4] = ["CON", "PRN", "AUX", "NUL"];
const NUMBERED_DEVICES: [&str; 2] = ["COM", "LPT"];

const KIND_IMAGE: &str = "image";
const KIND_PDF: &str = "pdf";
const KIND_FILE: &str = "file";
const IMAGE_EXTENSIONS: [&str; 5] = ["png", "jpg", "jpeg", "gif", "webp"];
const PDF_EXTENSION: &str = "pdf";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub size: u64,
    pub path: String,
}

pub fn project_dir(data_dir: &Path, project_id: i64) -> Result<PathBuf, AttachmentError> {
    let dir = project_path(data_dir, project_id);
    fs::create_dir_all(&dir).map_err(|error| io_error("criar a pasta de anexos", &dir, error))?;
    Ok(dir)
}

pub fn save_file(
    db: &Db,
    data_dir: &Path,
    project_id: i64,
    source: &Path,
) -> Result<Attachment, AttachmentError> {
    let not_a_file = || AttachmentError::NotAFile(source.display().to_string());
    let metadata = match fs::metadata(source) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Err(not_a_file()),
        Err(error) => return Err(io_error("ler o arquivo", source, error)),
    };
    if !metadata.is_file() {
        return Err(not_a_file());
    }
    // Só o último segmento: o nome original nunca vira caminho.
    let name = source
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .ok_or_else(not_a_file)?;
    ensure_fits(&name, metadata.len())?;
    store(db, data_dir, project_id, &name, |target| {
        fs::copy(source, target)
            .map_err(|error| io_error("copiar o arquivo", source, error))
    })
}

pub fn save_bytes(
    db: &Db,
    data_dir: &Path,
    project_id: i64,
    name: &str,
    bytes: &[u8],
) -> Result<Attachment, AttachmentError> {
    if bytes.is_empty() {
        return Err(AttachmentError::Empty(name.to_string()));
    }
    ensure_fits(name, bytes.len() as u64)?;
    store(db, data_dir, project_id, name, |target| {
        fs::write(target, bytes).map_err(|error| io_error("gravar o anexo", target, error))?;
        Ok(bytes.len() as u64)
    })
}

pub fn discard(
    db: &Db,
    data_dir: &Path,
    project_id: i64,
    id: &str,
) -> Result<(), AttachmentError> {
    // Apagar a linha primeiro garante que só um id conhecido vira caminho de pasta.
    if !db.delete_attachment(project_id, id)? {
        return Err(AttachmentError::NotFound(id.to_string()));
    }
    remove_dir(&project_path(data_dir, project_id).join(id))
}

pub fn find_for_send(
    db: &Db,
    project_id: i64,
    ids: &[String],
) -> Result<Vec<Attachment>, AttachmentError> {
    if ids.len() > ATTACHMENTS_PER_MESSAGE {
        return Err(AttachmentError::TooMany {
            limit: ATTACHMENTS_PER_MESSAGE,
        });
    }
    let found = db.find_attachments(project_id, ids)?;
    for id in ids {
        if !found.iter().any(|attachment| &attachment.id == id) {
            return Err(AttachmentError::NotFound(id.clone()));
        }
    }
    Ok(found)
}

pub fn block(attachments: &[Attachment]) -> String {
    let mut lines = vec![ATTACHMENTS_HEADER.to_string()];
    lines.extend(attachments.iter().map(|attachment| {
        format!(
            "- {} ({}, {}): {}",
            attachment.name,
            kind_label(&attachment.kind),
            format_size(attachment.size),
            attachment.path.replace('\\', "/")
        )
    }));
    lines.push(ATTACHMENTS_FOOTER.to_string());
    lines.join("\n")
}

pub fn append_block(text: &str, attachments: &[Attachment]) -> String {
    if attachments.is_empty() {
        return text.to_string();
    }
    let text = text.trim();
    if text.is_empty() {
        return block(attachments);
    }
    format!("{text}\n\n{}", block(attachments))
}

pub fn purge_unsent(db: &Db, data_dir: &Path) -> Result<(), AttachmentError> {
    for (project_id, id) in db.list_unsent_attachments()? {
        remove_dir(&project_path(data_dir, project_id).join(id))?;
    }
    Ok(db.delete_unsent_attachments()?)
}

pub fn purge_project(db: &Db, data_dir: &Path, project_id: i64) -> Result<(), AttachmentError> {
    db.delete_project_attachments(project_id)?;
    remove_dir(&project_path(data_dir, project_id))
}

pub fn purge_sent(db: &Db, data_dir: &Path, project_id: i64) -> Result<(), AttachmentError> {
    for id in db.list_sent_attachment_ids(project_id)? {
        remove_dir(&project_path(data_dir, project_id).join(id))?;
    }
    Ok(db.delete_sent_attachments(project_id)?)
}

fn project_path(data_dir: &Path, project_id: i64) -> PathBuf {
    data_dir.join(ATTACHMENTS_DIR).join(project_id.to_string())
}

fn ensure_fits(name: &str, size: u64) -> Result<(), AttachmentError> {
    if size > ATTACHMENT_MAX_BYTES {
        return Err(AttachmentError::TooLarge {
            name: name.to_string(),
            size,
        });
    }
    Ok(())
}

fn store(
    db: &Db,
    data_dir: &Path,
    project_id: i64,
    name: &str,
    write: impl FnOnce(&Path) -> Result<u64, AttachmentError>,
) -> Result<Attachment, AttachmentError> {
    let id = random_hex(ID_BYTES)?;
    let name = sanitize_name(name);
    let dir = project_dir(data_dir, project_id)?.join(&id);
    fs::create_dir_all(&dir).map_err(|error| io_error("criar a pasta do anexo", &dir, error))?;
    let target = dir.join(&name);
    let result = write(&target).and_then(|size| {
        let attachment = Attachment {
            kind: classify(&name).to_string(),
            id,
            name,
            size,
            path: target.to_string_lossy().into_owned(),
        };
        db.insert_attachment(project_id, &attachment)?;
        Ok(attachment)
    });
    if result.is_err() {
        // Limpeza best effort: o erro original é o que importa ao chamador.
        let _ = fs::remove_dir_all(&dir);
    }
    result
}

fn remove_dir(dir: &Path) -> Result<(), AttachmentError> {
    match fs::remove_dir_all(dir) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => {
            Err(io_error("apagar a pasta do anexo", dir, error))
        }
        _ => Ok(()),
    }
}

fn io_error(action: &str, path: &Path, error: io::Error) -> AttachmentError {
    AttachmentError::Io(format!("{action} ({})", path.display()), error)
}

fn split_extension(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(index) if index > 0 => name.split_at(index),
        _ => (name, ""),
    }
}

fn is_reserved_device(stem: &str) -> bool {
    let base = stem.split('.').next().unwrap_or("").to_uppercase();
    RESERVED_DEVICE_NAMES.contains(&base.as_str())
        || NUMBERED_DEVICES.iter().any(|device| {
            base.strip_prefix(device)
                .is_some_and(|number| matches!(number.as_bytes(), [b'1'..=b'9']))
        })
}

fn sanitize_name(raw: &str) -> String {
    let last_segment = raw.rsplit(['/', '\\']).next().unwrap_or("");
    let cleaned: String = last_segment
        .chars()
        .map(|c| {
            if FORBIDDEN_NAME_CHARS.contains(&c) || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    let cleaned = cleaned.trim_end_matches(['.', ' ']);
    let (stem, extension) = split_extension(cleaned);
    let stem = if stem.is_empty() || is_reserved_device(stem) {
        FALLBACK_NAME
    } else {
        stem
    };
    let extension_chars = extension.chars().count();
    if extension_chars >= NAME_MAX_CHARS {
        return format!("{stem}{extension}").chars().take(NAME_MAX_CHARS).collect();
    }
    let stem: String = stem.chars().take(NAME_MAX_CHARS - extension_chars).collect();
    format!("{stem}{extension}")
}

fn classify(name: &str) -> &'static str {
    let (_, extension) = split_extension(name);
    let extension = extension.trim_start_matches('.').to_lowercase();
    if IMAGE_EXTENSIONS.contains(&extension.as_str()) {
        KIND_IMAGE
    } else if extension == PDF_EXTENSION {
        KIND_PDF
    } else {
        KIND_FILE
    }
}

fn kind_label(kind: &str) -> &'static str {
    match kind {
        KIND_IMAGE => "imagem",
        KIND_PDF => "PDF",
        _ => "arquivo",
    }
}

fn format_size(bytes: u64) -> String {
    if bytes < BYTES_PER_KB {
        format!("{bytes} B")
    } else if bytes < BYTES_PER_MB {
        format!("{} KB", bytes / BYTES_PER_KB)
    } else {
        format!("{:.1} MB", bytes as f64 / BYTES_PER_MB as f64).replace('.', ",")
    }
}
