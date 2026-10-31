use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use rusqlite::{params, Connection, ErrorCode, Row};
use serde::Serialize;

use crate::events::{Speaker, VokiaEvent};

pub const DB_FILE: &str = "vokia.db";
const DEFAULT_PROJECT_NAME: &str = "Projeto padrão";
const DEFAULT_PROJECT_PATH: &str = "";
const HISTORY_LIMIT: i64 = 500;
const PROJECT_COLUMNS: &str = "id, name, path, selected, created_at";

// Cada item é uma migração; a versão é a posição + 1. Nunca editar um item já publicado: acrescentar outro.
const MIGRATIONS: &[&str] = &["
    CREATE TABLE projects (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        name TEXT NOT NULL,
        path TEXT NOT NULL UNIQUE,
        selected INTEGER NOT NULL DEFAULT 0,
        created_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000 AS INTEGER))
    );
    CREATE UNIQUE INDEX projects_one_selected ON projects(selected) WHERE selected = 1;
    CREATE TABLE messages (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        project_id INTEGER NOT NULL REFERENCES projects(id),
        from_agent TEXT NOT NULL,
        to_agent TEXT NOT NULL,
        text TEXT NOT NULL,
        created_at INTEGER NOT NULL
    );
    CREATE INDEX messages_by_project ON messages(project_id, id);
"];

#[derive(Debug)]
pub enum DbError {
    Io(PathBuf, std::io::Error),
    Corrupt(PathBuf),
    Inaccessible(PathBuf, rusqlite::Error),
    Sqlite(rusqlite::Error),
    Poisoned,
    Unavailable(String),
    NoSelectedProject,
    ProjectNotFound(i64),
    ProjectPathTaken(String),
    InvalidProject(&'static str),
    InvalidSpeaker(String),
}

impl fmt::Display for DbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DbError::Io(path, error) => write!(
                f,
                "Não foi possível criar a pasta de dados {}: {error}",
                path.display()
            ),
            DbError::Corrupt(path) => write!(
                f,
                "O banco de dados {} está corrompido ou não é um banco válido.",
                path.display()
            ),
            DbError::Inaccessible(path, error) => write!(
                f,
                "Sem acesso ao banco de dados {}: {error}",
                path.display()
            ),
            DbError::Sqlite(error) => write!(f, "Erro no banco de dados: {error}"),
            DbError::Poisoned => write!(
                f,
                "O banco de dados ficou inconsistente após uma falha anterior."
            ),
            DbError::Unavailable(reason) => write!(f, "{reason}"),
            DbError::NoSelectedProject => write!(f, "Nenhum projeto está selecionado."),
            DbError::ProjectNotFound(id) => write!(f, "O projeto {id} não existe."),
            DbError::ProjectPathTaken(path) => {
                write!(f, "Já existe um projeto na pasta {path}.")
            }
            DbError::InvalidProject(reason) => write!(f, "{reason}"),
            DbError::InvalidSpeaker(value) => {
                write!(f, "O histórico guarda um remetente desconhecido: {value}.")
            }
        }
    }
}

impl From<rusqlite::Error> for DbError {
    fn from(error: rusqlite::Error) -> Self {
        DbError::Sqlite(error)
    }
}

fn classify(path: &Path, error: rusqlite::Error) -> DbError {
    match error.sqlite_error_code() {
        Some(ErrorCode::NotADatabase | ErrorCode::DatabaseCorrupt) => {
            DbError::Corrupt(path.to_path_buf())
        }
        Some(ErrorCode::CannotOpen | ErrorCode::PermissionDenied | ErrorCode::ReadOnly) => {
            DbError::Inaccessible(path.to_path_buf(), error)
        }
        _ => DbError::Sqlite(error),
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DbProblem {
    pub reason: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: i64,
    pub name: String,
    pub path: String,
    pub selected: bool,
    pub created_at: i64,
}

fn project_from_row(row: &Row<'_>) -> rusqlite::Result<Project> {
    Ok(Project {
        id: row.get(0)?,
        name: row.get(1)?,
        path: row.get(2)?,
        selected: row.get(3)?,
        created_at: row.get(4)?,
    })
}

pub struct Db {
    conn: Mutex<Connection>,
}

impl Db {
    pub fn open(dir: &Path) -> Result<Db, DbError> {
        fs::create_dir_all(dir).map_err(|error| DbError::Io(dir.to_path_buf(), error))?;
        let path = dir.join(DB_FILE);
        let mut conn = Connection::open(&path).map_err(|error| classify(&path, error))?;
        conn.pragma_update(None, "foreign_keys", true)
            .map_err(|error| classify(&path, error))?;
        migrate(&mut conn).map_err(|error| classify(&path, error))?;
        ensure_default_project(&conn).map_err(|error| classify(&path, error))?;
        Ok(Db {
            conn: Mutex::new(conn),
        })
    }

    fn conn(&self) -> Result<MutexGuard<'_, Connection>, DbError> {
        self.conn.lock().map_err(|_| DbError::Poisoned)
    }

    pub fn list_projects(&self) -> Result<Vec<Project>, DbError> {
        let conn = self.conn()?;
        let mut statement = conn.prepare(&format!(
            "SELECT {PROJECT_COLUMNS} FROM projects ORDER BY id"
        ))?;
        let projects = statement
            .query_map([], project_from_row)?
            .collect::<rusqlite::Result<Vec<Project>>>()?;
        Ok(projects)
    }

    pub fn add_project(&self, name: &str, path: &str) -> Result<Project, DbError> {
        let name = name.trim();
        let path = path.trim();
        if name.is_empty() {
            return Err(DbError::InvalidProject(
                "O nome do projeto não pode ser vazio.",
            ));
        }
        if path.is_empty() {
            return Err(DbError::InvalidProject(
                "A pasta do projeto não pode ser vazia.",
            ));
        }
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO projects(name, path) VALUES (?1, ?2)",
            params![name, path],
        )
        .map_err(|error| match error.sqlite_error_code() {
            Some(ErrorCode::ConstraintViolation) => DbError::ProjectPathTaken(path.to_string()),
            _ => DbError::Sqlite(error),
        })?;
        let project = conn.query_row(
            &format!("SELECT {PROJECT_COLUMNS} FROM projects WHERE id = ?1"),
            [conn.last_insert_rowid()],
            project_from_row,
        )?;
        Ok(project)
    }

    pub fn select_project(&self, id: i64) -> Result<(), DbError> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        tx.execute("UPDATE projects SET selected = 0 WHERE selected = 1", [])?;
        let changed = tx.execute("UPDATE projects SET selected = 1 WHERE id = ?1", [id])?;
        if changed == 0 {
            return Err(DbError::ProjectNotFound(id));
        }
        tx.commit()?;
        Ok(())
    }

    pub fn insert_message(
        &self,
        from: Speaker,
        to: Speaker,
        text: &str,
        time: u64,
    ) -> Result<(), DbError> {
        let conn = self.conn()?;
        let inserted = conn.execute(
            "INSERT INTO messages(project_id, from_agent, to_agent, text, created_at) \
             SELECT id, ?1, ?2, ?3, ?4 FROM projects WHERE selected = 1",
            params![from.as_str(), to.as_str(), text, time as i64],
        )?;
        if inserted == 0 {
            return Err(DbError::NoSelectedProject);
        }
        Ok(())
    }

    pub fn load_history(&self, project_id: i64) -> Result<Vec<VokiaEvent>, DbError> {
        let conn = self.conn()?;
        let mut statement = conn.prepare(
            "SELECT from_agent, to_agent, text, created_at FROM ( \
                 SELECT id, from_agent, to_agent, text, created_at FROM messages \
                 WHERE project_id = ?1 ORDER BY id DESC LIMIT ?2 \
             ) ORDER BY id",
        )?;
        let rows = statement.query_map(params![project_id, HISTORY_LIMIT], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })?;
        rows.map(|row| -> Result<VokiaEvent, DbError> {
            let (from, to, text, time) = row?;
            Ok(VokiaEvent::Message {
                from: parse_speaker(&from)?,
                to: parse_speaker(&to)?,
                text,
                time: time as u64,
            })
        })
        .collect()
    }
}

fn parse_speaker(value: &str) -> Result<Speaker, DbError> {
    Speaker::parse(value).ok_or_else(|| DbError::InvalidSpeaker(value.to_string()))
}

fn migrate(conn: &mut Connection) -> rusqlite::Result<()> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL)")?;
    let current: i64 = conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_version",
        [],
        |row| row.get(0),
    )?;
    for (index, sql) in MIGRATIONS.iter().enumerate().skip(current as usize) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.execute(
            "INSERT INTO schema_version(version) VALUES (?1)",
            [index as i64 + 1],
        )?;
        tx.commit()?;
    }
    Ok(())
}

fn ensure_default_project(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO projects(name, path, selected) \
         SELECT ?1, ?2, 1 WHERE NOT EXISTS (SELECT 1 FROM projects)",
        params![DEFAULT_PROJECT_NAME, DEFAULT_PROJECT_PATH],
    )?;
    Ok(())
}

pub struct DbState(Result<Db, DbProblem>);

impl DbState {
    pub fn open(dir: &Path) -> DbState {
        DbState(Db::open(dir).map_err(|error| DbProblem {
            reason: error.to_string(),
        }))
    }

    pub fn problem(&self) -> Option<DbProblem> {
        self.0.as_ref().err().cloned()
    }

    pub fn db(&self) -> Option<&Db> {
        self.0.as_ref().ok()
    }

    pub fn require(&self) -> Result<&Db, DbError> {
        self.0
            .as_ref()
            .map_err(|problem| DbError::Unavailable(problem.reason.clone()))
    }
}
