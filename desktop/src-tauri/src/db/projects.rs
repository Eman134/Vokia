use rusqlite::{params, ErrorCode, Row};
use serde::Serialize;

use crate::permissions::PermissionLevel;

use super::connection::Db;
use super::error::DbError;

const PROJECT_COLUMNS: &str =
    "id, name, path, selected, created_at, symbol, description, origin_kind, origin_repo, pinned, \
     (SELECT MAX(created_at) FROM messages WHERE project_id = projects.id), permission_level";

// item_plans antes de backlog_items: a chave estrangeira deles aponta para backlog_items.
const PROJECT_CHILD_TABLES: &[&str] = &[
    "item_plans",
    "backlog_items",
    "messages",
    "agent_pauses",
    "permission_approvals",
    "agent_sessions",
    "channels",
];

pub const ORIGIN_FOLDER: &str = "folder";
pub const ORIGIN_GITHUB: &str = "github";

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: i64,
    pub name: String,
    pub path: String,
    pub selected: bool,
    pub created_at: i64,
    pub symbol: String,
    pub description: String,
    pub origin_kind: String,
    pub origin_repo: Option<String>,
    pub pinned: bool,
    pub last_activity_at: Option<i64>,
    pub permission_level: PermissionLevel,
}

fn project_from_row(row: &Row<'_>) -> rusqlite::Result<Project> {
    let level: String = row.get(11)?;
    let permission_level = PermissionLevel::parse(&level).ok_or_else(|| {
        rusqlite::Error::InvalidColumnType(
            11,
            "permission_level".into(),
            rusqlite::types::Type::Text,
        )
    })?;
    Ok(Project {
        id: row.get(0)?,
        name: row.get(1)?,
        path: row.get(2)?,
        selected: row.get(3)?,
        created_at: row.get(4)?,
        symbol: row.get(5)?,
        description: row.get(6)?,
        origin_kind: row.get(7)?,
        origin_repo: row.get(8)?,
        pinned: row.get(9)?,
        last_activity_at: row.get(10)?,
        permission_level,
    })
}

pub struct NewProject<'a> {
    pub name: &'a str,
    pub symbol: &'a str,
    pub description: &'a str,
    pub path: &'a str,
    pub origin_kind: &'a str,
    pub origin_repo: Option<&'a str>,
}

fn validate_project_fields(name: &str, symbol: &str) -> Result<(), DbError> {
    if name.trim().is_empty() {
        return Err(DbError::InvalidProject(
            "O nome do projeto não pode ser vazio.",
        ));
    }
    if symbol.trim().is_empty() {
        return Err(DbError::InvalidProject(
            "O ícone do projeto não pode ser vazio.",
        ));
    }
    Ok(())
}

impl NewProject<'_> {
    pub fn validate(&self) -> Result<(), DbError> {
        validate_project_fields(self.name, self.symbol)?;
        if self.path.trim().is_empty() {
            return Err(DbError::InvalidProject(
                "A pasta do projeto não pode ser vazia.",
            ));
        }
        Ok(())
    }
}

impl Db {
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

    pub fn selected_project(&self) -> Result<Project, DbError> {
        let conn = self.conn()?;
        conn.query_row(
            &format!("SELECT {PROJECT_COLUMNS} FROM projects WHERE selected = 1"),
            [],
            project_from_row,
        )
        .map_err(|error| match error {
            rusqlite::Error::QueryReturnedNoRows => DbError::NoSelectedProject,
            other => DbError::Sqlite(other),
        })
    }

    pub fn project(&self, id: i64) -> Result<Project, DbError> {
        let conn = self.conn()?;
        conn.query_row(
            &format!("SELECT {PROJECT_COLUMNS} FROM projects WHERE id = ?1"),
            [id],
            project_from_row,
        )
        .map_err(|error| match error {
            rusqlite::Error::QueryReturnedNoRows => DbError::ProjectNotFound(id),
            other => DbError::Sqlite(other),
        })
    }

    pub fn insert_project(&self, new: &NewProject) -> Result<Project, DbError> {
        new.validate()?;
        let path = new.path.trim();
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO projects(name, path, symbol, description, origin_kind, origin_repo)              VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                new.name.trim(),
                path,
                new.symbol.trim(),
                new.description.trim(),
                new.origin_kind,
                new.origin_repo
            ],
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

    pub fn update_project(
        &self,
        id: i64,
        name: &str,
        symbol: &str,
        description: &str,
    ) -> Result<Project, DbError> {
        validate_project_fields(name, symbol)?;
        let conn = self.conn()?;
        let changed = conn.execute(
            "UPDATE projects SET name = ?1, symbol = ?2, description = ?3 WHERE id = ?4",
            params![name.trim(), symbol.trim(), description.trim(), id],
        )?;
        if changed == 0 {
            return Err(DbError::ProjectNotFound(id));
        }
        let project = conn.query_row(
            &format!("SELECT {PROJECT_COLUMNS} FROM projects WHERE id = ?1"),
            [id],
            project_from_row,
        )?;
        Ok(project)
    }

    pub fn delete_project(&self, id: i64) -> Result<(), DbError> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        for table in PROJECT_CHILD_TABLES {
            tx.execute(&format!("DELETE FROM {table} WHERE project_id = ?1"), [id])?;
        }
        if tx.execute("DELETE FROM projects WHERE id = ?1", [id])? == 0 {
            return Err(DbError::ProjectNotFound(id));
        }
        tx.execute(
            "UPDATE projects SET selected = 1 WHERE id = (SELECT MIN(id) FROM projects) \
             AND NOT EXISTS (SELECT 1 FROM projects WHERE selected = 1)",
            [],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn erase_all(&self) -> Result<(), DbError> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        for table in PROJECT_CHILD_TABLES.iter().chain(&["projects", "settings"]) {
            tx.execute(&format!("DELETE FROM {table}"), [])?;
        }
        tx.commit()?;
        Ok(())
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

    pub fn pin_project(&self, id: Option<i64>) -> Result<(), DbError> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        tx.execute("UPDATE projects SET pinned = 0 WHERE pinned = 1", [])?;
        if let Some(id) = id {
            let path: String = tx
                .query_row("SELECT path FROM projects WHERE id = ?1", [id], |row| {
                    row.get(0)
                })
                .map_err(|error| match error {
                    rusqlite::Error::QueryReturnedNoRows => DbError::ProjectNotFound(id),
                    other => DbError::Sqlite(other),
                })?;
            if path.trim().is_empty() {
                return Err(DbError::ProjectHasNoFolder);
            }
            tx.execute("UPDATE projects SET pinned = 1 WHERE id = ?1", [id])?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn set_permission_level(&self, id: i64, level: PermissionLevel) -> Result<(), DbError> {
        let conn = self.conn()?;
        let changed = conn.execute(
            "UPDATE projects SET permission_level = ?1 WHERE id = ?2",
            params![level.as_str(), id],
        )?;
        if changed == 0 {
            return Err(DbError::ProjectNotFound(id));
        }
        Ok(())
    }
}
