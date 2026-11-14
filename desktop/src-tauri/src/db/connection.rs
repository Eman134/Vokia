use std::fs;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use rusqlite::Connection;

use super::error::{classify, DbError, DbProblem};
use super::migrations::migrate;

pub const DB_FILE: &str = "vokia.db";

pub struct Db {
    conn: Mutex<Connection>,
}

impl Db {
    pub fn open(dir: &Path) -> Result<Db, DbError> {
        fs::create_dir_all(dir).map_err(|error| DbError::Io(dir.to_path_buf(), error))?;
        let path = dir.join(DB_FILE);
        let mut conn = Connection::open(&path).map_err(|error| classify(&path, error))?;
        // SQLite cai para modo somente leitura sem erro na abertura; a falha só apareceria na primeira escrita.
        if conn
            .is_readonly("main")
            .map_err(|error| classify(&path, error))?
        {
            return Err(DbError::ReadOnly(path));
        }
        conn.pragma_update(None, "foreign_keys", true)
            .map_err(|error| classify(&path, error))?;
        migrate(&mut conn).map_err(|error| classify(&path, error))?;
        Ok(Db {
            conn: Mutex::new(conn),
        })
    }

    pub(crate) fn conn(&self) -> Result<MutexGuard<'_, Connection>, DbError> {
        self.conn.lock().map_err(|_| DbError::Poisoned)
    }
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
