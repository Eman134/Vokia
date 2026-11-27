use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Deserializer, Serialize};

use super::backlog::ensure_project;
use super::connection::Db;
use super::error::DbError;

pub const SPRINT_STATUS_PLANNED: &str = "planejada";
pub const SPRINT_STATUS_ACTIVE: &str = "ativa";
pub const SPRINT_STATUS_CLOSED: &str = "encerrada";

const SPRINT_NAME_MAX_CHARS: usize = 120;
const SPRINT_GOAL_MAX_CHARS: usize = 1000;
const SPRINT_ID_PREFIX: &str = "sprint-";
const DONE_COLUMN: &str = "Done";
const NOW_MS: &str = "CAST(unixepoch('subsec') * 1000 AS INTEGER)";

const SPRINT_SELECT_SQL: &str = "
    SELECT s.id, s.name, s.goal, s.starts_at, s.ends_at, s.status, s.created_at, s.updated_at,
           COALESCE(COUNT(b.id), 0) AS total_items,
           COALESCE(SUM(CASE WHEN b.\"column\" = 'Done' THEN 1 ELSE 0 END), 0) AS done_items
    FROM sprints s
    LEFT JOIN backlog_items b
      ON b.project_id = s.project_id
     AND b.sprint_id = s.id
     AND b.archived_at IS NULL
     AND b.parent IS NULL";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sprint {
    pub id: String,
    pub name: String,
    pub goal: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub starts_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ends_at: Option<i64>,
    pub status: String,
    pub total_items: u32,
    pub done_items: u32,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewSprint {
    pub name: String,
    #[serde(default)]
    pub goal: Option<String>,
    #[serde(default)]
    pub starts_at: Option<i64>,
    #[serde(default)]
    pub ends_at: Option<i64>,
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SprintPatch {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub goal: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_nullable")]
    pub starts_at: Option<Option<i64>>,
    #[serde(default, deserialize_with = "deserialize_optional_nullable")]
    pub ends_at: Option<Option<i64>>,
    #[serde(default)]
    pub status: Option<String>,
}

// Distingue campo omitido (None) de campo enviado explicitamente como null (Some(None)) em patches JSON.
fn deserialize_optional_nullable<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

impl Db {
    pub fn list_sprints(&self, project_id: i64) -> Result<Vec<Sprint>, DbError> {
        let conn = self.conn()?;
        ensure_project(&conn, project_id)?;
        read_sprints(&conn, project_id)
    }

    pub fn create_sprint(&self, project_id: i64, new: NewSprint) -> Result<Sprint, DbError> {
        let name = valid_name(&new.name)?;
        let goal = valid_goal(new.goal.as_deref().unwrap_or_default())?;
        let status = valid_status(new.status.as_deref().unwrap_or(SPRINT_STATUS_PLANNED))?;
        valid_period(new.starts_at, new.ends_at)?;

        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        ensure_project(&tx, project_id)?;
        if status == SPRINT_STATUS_ACTIVE {
            ensure_no_other_active(&tx, project_id, None)?;
        }

        let id = next_sprint_id(&tx, project_id)?;
        tx.execute(
            &format!(
                "INSERT INTO sprints \
                 (project_id, id, name, goal, starts_at, ends_at, status, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, {NOW_MS}, {NOW_MS})"
            ),
            params![
                project_id,
                id,
                name,
                goal,
                new.starts_at,
                new.ends_at,
                status
            ],
        )?;
        let created = find_sprint(&tx, project_id, &id)?;
        tx.commit()?;
        Ok(created)
    }

    pub fn update_sprint(
        &self,
        project_id: i64,
        id: &str,
        patch: SprintPatch,
    ) -> Result<Sprint, DbError> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        ensure_project(&tx, project_id)?;
        let current = find_sprint(&tx, project_id, id)?;

        let name = match patch.name.as_deref() {
            Some(raw) => valid_name(raw)?,
            None => current.name,
        };
        let goal = match patch.goal.as_deref() {
            Some(raw) => valid_goal(raw)?,
            None => current.goal,
        };
        let starts_at = patch.starts_at.unwrap_or(current.starts_at);
        let ends_at = patch.ends_at.unwrap_or(current.ends_at);
        valid_period(starts_at, ends_at)?;

        let status = match patch.status.as_deref() {
            Some(raw) => valid_status(raw)?,
            None => current.status.as_str(),
        };
        if status == SPRINT_STATUS_ACTIVE && current.status != SPRINT_STATUS_ACTIVE {
            ensure_no_other_active(&tx, project_id, Some(id))?;
        }

        tx.execute(
            &format!(
                "UPDATE sprints SET name = ?3, goal = ?4, starts_at = ?5, ends_at = ?6, \
                 status = ?7, updated_at = {NOW_MS} \
                 WHERE project_id = ?1 AND id = ?2"
            ),
            params![project_id, id, name, goal, starts_at, ends_at, status],
        )?;
        let updated = find_sprint(&tx, project_id, id)?;
        tx.commit()?;
        Ok(updated)
    }

    pub fn close_sprint(
        &self,
        project_id: i64,
        id: &str,
        move_pending_to_sprint_id: Option<&str>,
    ) -> Result<Sprint, DbError> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        ensure_project(&tx, project_id)?;
        let _ = find_sprint(&tx, project_id, id)?;

        if let Some(target_id) = move_pending_to_sprint_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            if target_id == id {
                return Err(DbError::SprintClosed(id.to_string()));
            }
            let target = find_sprint(&tx, project_id, target_id)?;
            if target.status == SPRINT_STATUS_CLOSED {
                return Err(DbError::SprintClosed(target_id.to_string()));
            }
            tx.execute(
                "UPDATE backlog_items SET sprint_id = ?3 \
                 WHERE project_id = ?1 AND sprint_id = ?2 \
                   AND parent IS NULL AND archived_at IS NULL AND \"column\" <> ?4",
                params![project_id, id, target_id, DONE_COLUMN],
            )?;
        }

        tx.execute(
            &format!(
                "UPDATE sprints SET status = ?3, updated_at = {NOW_MS} \
                 WHERE project_id = ?1 AND id = ?2"
            ),
            params![project_id, id, SPRINT_STATUS_CLOSED],
        )?;
        let closed = find_sprint(&tx, project_id, id)?;
        tx.commit()?;
        Ok(closed)
    }

    pub fn delete_sprint(&self, project_id: i64, id: &str) -> Result<(), DbError> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        ensure_project(&tx, project_id)?;
        let _ = find_sprint(&tx, project_id, id)?;
        tx.execute(
            "UPDATE backlog_items SET sprint_id = NULL WHERE project_id = ?1 AND sprint_id = ?2",
            params![project_id, id],
        )?;
        tx.execute(
            "DELETE FROM sprints WHERE project_id = ?1 AND id = ?2",
            params![project_id, id],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn set_item_sprint(
        &self,
        project_id: i64,
        item_id: &str,
        sprint_id: Option<&str>,
    ) -> Result<(), DbError> {
        let normalized = sprint_id.map(str::trim).filter(|value| !value.is_empty());
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        ensure_project(&tx, project_id)?;
        if let Some(target_id) = normalized {
            let sprint = find_sprint(&tx, project_id, target_id)?;
            if sprint.status == SPRINT_STATUS_CLOSED {
                return Err(DbError::SprintClosed(target_id.to_string()));
            }
        }
        let changed = tx.execute(
            "UPDATE backlog_items SET sprint_id = ?3 WHERE project_id = ?1 AND id = ?2",
            params![project_id, item_id, normalized],
        )?;
        if changed == 0 {
            return Err(DbError::BacklogItemNotFound(item_id.to_string()));
        }
        tx.commit()?;
        Ok(())
    }
}

pub(super) fn read_sprints(conn: &Connection, project_id: i64) -> Result<Vec<Sprint>, DbError> {
    let mut statement = conn.prepare(&format!(
        "{SPRINT_SELECT_SQL}
         WHERE s.project_id = ?1
         GROUP BY s.project_id, s.id
         ORDER BY CASE s.status WHEN 'ativa' THEN 0 WHEN 'planejada' THEN 1 ELSE 2 END,
                  COALESCE(s.starts_at, s.created_at) DESC,
                  s.created_at DESC"
    ))?;
    let rows = statement.query_map([project_id], sprint_from_row)?;
    rows.collect::<rusqlite::Result<Vec<Sprint>>>()
        .map_err(DbError::from)
}

fn find_sprint(conn: &Connection, project_id: i64, id: &str) -> Result<Sprint, DbError> {
    conn.query_row(
        &format!(
            "{SPRINT_SELECT_SQL}
             WHERE s.project_id = ?1 AND s.id = ?2
             GROUP BY s.project_id, s.id"
        ),
        params![project_id, id],
        sprint_from_row,
    )
    .optional()?
    .ok_or_else(|| DbError::SprintNotFound(id.to_string()))
}

fn sprint_from_row(row: &Row<'_>) -> rusqlite::Result<Sprint> {
    Ok(Sprint {
        id: row.get("id")?,
        name: row.get("name")?,
        goal: row.get("goal")?,
        starts_at: row.get("starts_at")?,
        ends_at: row.get("ends_at")?,
        status: row.get("status")?,
        total_items: row.get("total_items")?,
        done_items: row.get("done_items")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn next_sprint_id(conn: &Connection, project_id: i64) -> Result<String, DbError> {
    let prefix_len = SPRINT_ID_PREFIX.len() as i64 + 1;
    let seq: i64 = conn.query_row(
        "SELECT COALESCE(MAX(CAST(SUBSTR(id, ?2) AS INTEGER)), 0) + 1 \
         FROM sprints WHERE project_id = ?1 AND id LIKE 'sprint-%'",
        params![project_id, prefix_len],
        |row| row.get(0),
    )?;
    Ok(format!("{SPRINT_ID_PREFIX}{seq}"))
}

fn ensure_no_other_active(
    conn: &Connection,
    project_id: i64,
    except_id: Option<&str>,
) -> Result<(), DbError> {
    let active_id: Option<String> = conn
        .query_row(
            "SELECT id FROM sprints WHERE project_id = ?1 AND status = ?2 AND (?3 IS NULL OR id <> ?3) LIMIT 1",
            params![project_id, SPRINT_STATUS_ACTIVE, except_id],
            |row| row.get(0),
        )
        .optional()?;
    if let Some(existing) = active_id {
        return Err(DbError::ActiveSprintExists(existing));
    }
    Ok(())
}

fn valid_name(raw: &str) -> Result<String, DbError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(DbError::EmptySprintName);
    }
    if trimmed.chars().count() > SPRINT_NAME_MAX_CHARS {
        return Err(DbError::SprintFieldTooLong {
            field: "nome",
            max: SPRINT_NAME_MAX_CHARS,
        });
    }
    Ok(trimmed.to_string())
}

fn valid_goal(raw: &str) -> Result<String, DbError> {
    let trimmed = raw.trim();
    if trimmed.chars().count() > SPRINT_GOAL_MAX_CHARS {
        return Err(DbError::SprintFieldTooLong {
            field: "objetivo",
            max: SPRINT_GOAL_MAX_CHARS,
        });
    }
    Ok(trimmed.to_string())
}

fn valid_status(raw: &str) -> Result<&'static str, DbError> {
    match raw.trim() {
        SPRINT_STATUS_PLANNED => Ok(SPRINT_STATUS_PLANNED),
        SPRINT_STATUS_ACTIVE => Ok(SPRINT_STATUS_ACTIVE),
        SPRINT_STATUS_CLOSED => Ok(SPRINT_STATUS_CLOSED),
        other => Err(DbError::InvalidSprintStatus(other.to_string())),
    }
}

fn valid_period(starts_at: Option<i64>, ends_at: Option<i64>) -> Result<(), DbError> {
    if let (Some(start), Some(end)) = (starts_at, ends_at) {
        if end < start {
            return Err(DbError::InvalidSprintPeriod);
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../tests/sprints_tests.rs"]
mod tests;
