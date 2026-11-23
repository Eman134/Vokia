use std::fmt;
use std::io;

use crate::db::DbError;
use crate::mcp::RandomnessUnavailable;

use super::{format_size, ATTACHMENT_MAX_BYTES, BYTES_PER_MB};

#[derive(Debug)]
pub enum AttachmentError {
    TooLarge { name: String, size: u64 },
    TooMany { limit: usize },
    Empty(String),
    NotAFile(String),
    NotFound(String),
    OutsideAttachmentsDir(String),
    UnsupportedImage(String),
    Io(String, io::Error),
    Randomness(RandomnessUnavailable),
    Db(DbError),
}

impl fmt::Display for AttachmentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AttachmentError::TooLarge { name, size } => write!(
                f,
                "O arquivo \"{name}\" tem {} e passa do limite de {} MB por anexo.",
                format_size(*size),
                ATTACHMENT_MAX_BYTES / BYTES_PER_MB
            ),
            AttachmentError::TooMany { limit } => {
                write!(f, "Só é possível enviar até {limit} anexos por mensagem.")
            }
            AttachmentError::Empty(name) => write!(f, "O arquivo \"{name}\" está vazio."),
            AttachmentError::NotAFile(path) => {
                write!(f, "\"{path}\" não existe ou não é um arquivo.")
            }
            AttachmentError::NotFound(id) => write!(f, "O anexo {id} não existe neste projeto."),
            AttachmentError::OutsideAttachmentsDir(path) => write!(
                f,
                "O caminho \"{path}\" não pertence à pasta de anexos do Vokia."
            ),
            AttachmentError::UnsupportedImage(path) => write!(
                f,
                "O arquivo \"{path}\" não é uma imagem suportada para pré-visualização."
            ),
            AttachmentError::Io(context, error) => write!(f, "Falha ao {context}: {error}"),
            AttachmentError::Randomness(error) => write!(f, "{error}"),
            AttachmentError::Db(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for AttachmentError {}

impl From<DbError> for AttachmentError {
    fn from(error: DbError) -> Self {
        AttachmentError::Db(error)
    }
}

impl From<RandomnessUnavailable> for AttachmentError {
    fn from(error: RandomnessUnavailable) -> Self {
        AttachmentError::Randomness(error)
    }
}
