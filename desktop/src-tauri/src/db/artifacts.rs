use rusqlite::{params, Connection, Row};
use serde::Serialize;

use crate::plan_proposal::PlanDraft;

use super::backlog::ensure_project;
use super::connection::Db;
use super::error::DbError;

pub(super) const ARTIFACT_ID_PREFIX: &str = "plano-";
const CRITERION_ID_PREFIX: &str = "c";

const UPSERT_ARTIFACT_SQL: &str = "INSERT INTO artifacts (project_id, id, item_id, title, summary, \
    points, estimate, risks, tags, detail, author, created_at, updated_at) \
    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, \
    CAST(unixepoch('subsec') * 1000 AS INTEGER), CAST(unixepoch('subsec') * 1000 AS INTEGER)) \
    ON CONFLICT(project_id, id) DO UPDATE SET title = excluded.title, summary = excluded.summary, \
    points = excluded.points, estimate = excluded.estimate, risks = excluded.risks, \
    tags = excluded.tags, detail = excluded.detail, author = excluded.author, \
    updated_at = excluded.updated_at";

const INSERT_CRITERION_SQL: &str = "INSERT INTO artifact_criteria \
    (project_id, artifact_id, id, position, text, done) VALUES (?1, ?2, ?3, ?4, ?5, 0)";

const ARTIFACT_COLUMNS: &str = "id, item_id, title, summary, points, estimate, risks, tags, detail, \
    author, created_at, updated_at";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactCriterion {
    pub id: String,
    pub text: String,
    pub done: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Artifact {
    pub id: String,
    pub item_id: String,
    pub title: String,
    pub summary: String,
    pub points: Vec<String>,
    pub estimate: String,
    pub risks: Vec<String>,
    pub tags: Vec<String>,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    pub criteria: Vec<ArtifactCriterion>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl Db {
    pub fn list_artifacts(&self, project_id: i64) -> Result<Vec<Artifact>, DbError> {
        let conn = self.conn()?;
        ensure_project(&conn, project_id)?;
        read_plan_artifacts(&conn, project_id, None)
    }

    pub fn set_artifact_criterion(
        &self,
        project_id: i64,
        artifact_id: &str,
        criterion_id: &str,
        done: bool,
    ) -> Result<(), DbError> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        ensure_project(&tx, project_id)?;
        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM artifacts WHERE project_id = ?1 AND id = ?2)",
            params![project_id, artifact_id],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(DbError::ArtifactNotFound(artifact_id.to_string()));
        }
        let changed = tx.execute(
            "UPDATE artifact_criteria SET done = ?4 WHERE project_id = ?1 AND artifact_id = ?2 AND id = ?3",
            params![project_id, artifact_id, criterion_id, done],
        )?;
        if changed == 0 {
            return Err(DbError::CriterionNotFound(criterion_id.to_string()));
        }
        tx.execute(
            "UPDATE artifacts SET updated_at = CAST(unixepoch('subsec') * 1000 AS INTEGER) \
             WHERE project_id = ?1 AND id = ?2",
            params![project_id, artifact_id],
        )?;
        tx.commit()?;
        Ok(())
    }
}

// Com id, devolve só esse plano; sem id, todos na ordem de criação.
fn read_plan_artifacts(
    conn: &Connection,
    project_id: i64,
    id: Option<&str>,
) -> Result<Vec<Artifact>, DbError> {
    let mut statement = conn.prepare(&format!(
        "SELECT {ARTIFACT_COLUMNS} FROM artifacts \
         WHERE project_id = ?1 AND (?2 IS NULL OR id = ?2) ORDER BY created_at"
    ))?;
    let rows = statement.query_map(params![project_id, id], |row| Ok(artifact_from_row(row)))?;
    let mut artifacts = rows
        .map(|row| -> Result<Artifact, DbError> { row? })
        .collect::<Result<Vec<Artifact>, DbError>>()?;
    for artifact in &mut artifacts {
        artifact.criteria = read_criteria(conn, project_id, &artifact.id)?;
    }
    Ok(artifacts)
}

pub(super) fn read_plan_artifact(
    conn: &Connection,
    project_id: i64,
    id: &str,
) -> Result<Option<Artifact>, DbError> {
    Ok(read_plan_artifacts(conn, project_id, Some(id))?.into_iter().next())
}

pub(super) fn artifact_id(item_id: &str) -> String {
    format!("{ARTIFACT_ID_PREFIX}{item_id}")
}

pub(super) fn artifact_exists(conn: &Connection, project_id: i64, item_id: &str) -> rusqlite::Result<bool> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM artifacts WHERE project_id = ?1 AND item_id = ?2)",
        params![project_id, item_id],
        |row| row.get(0),
    )
}

// Grava o plano e recria os critérios (um por lote com meta): ajuste da proposta zera o que foi marcado.
pub(super) fn replace_artifact(
    conn: &Connection,
    project_id: i64,
    item_id: &str,
    title: &str,
    text: &str,
    author: Option<&str>,
    draft: &PlanDraft,
) -> rusqlite::Result<()> {
    let id = artifact_id(item_id);
    let points: Vec<&str> = draft.lots.iter().map(|lot| lot.title.as_str()).collect();
    conn.execute(
        UPSERT_ARTIFACT_SQL,
        params![
            project_id,
            id,
            item_id,
            title,
            draft.summary,
            json_text(&points)?,
            draft.estimate,
            json_text(&draft.risks)?,
            json_text(&draft.tags)?,
            text,
            author,
        ],
    )?;
    conn.execute(
        "DELETE FROM artifact_criteria WHERE project_id = ?1 AND artifact_id = ?2",
        params![project_id, id],
    )?;
    for lot in draft.lots.iter().filter(|lot| !lot.meta.is_empty()) {
        conn.execute(
            INSERT_CRITERION_SQL,
            params![project_id, id, format!("{CRITERION_ID_PREFIX}{}", lot.n), lot.n, lot.meta],
        )?;
    }
    Ok(())
}

// Os critérios saem junto pela chave estrangeira (ON DELETE CASCADE).
pub(super) fn delete_artifact(conn: &Connection, project_id: i64, item_id: &str) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM artifacts WHERE project_id = ?1 AND item_id = ?2",
        params![project_id, item_id],
    )?;
    Ok(())
}

pub(super) fn json_text<T: Serialize + ?Sized>(value: &T) -> rusqlite::Result<String> {
    serde_json::to_string(value).map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))
}

fn artifact_from_row(row: &Row<'_>) -> Result<Artifact, DbError> {
    let item_id: String = row.get("item_id")?;
    Ok(Artifact {
        id: row.get("id")?,
        title: row.get("title")?,
        summary: row.get("summary")?,
        points: json_list(row, "points", &item_id)?,
        estimate: row.get("estimate")?,
        risks: json_list(row, "risks", &item_id)?,
        tags: json_list(row, "tags", &item_id)?,
        detail: row.get("detail")?,
        author: row.get("author")?,
        criteria: Vec::new(),
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        item_id,
    })
}

pub(super) fn json_list(row: &Row<'_>, column: &str, item_id: &str) -> Result<Vec<String>, DbError> {
    let raw: String = row.get(column)?;
    serde_json::from_str(&raw).map_err(|error| DbError::BacklogItemData {
        item_id: item_id.to_string(),
        reason: format!("{column}: {error}"),
    })
}

fn read_criteria(conn: &Connection, project_id: i64, artifact_id: &str) -> Result<Vec<ArtifactCriterion>, DbError> {
    let mut statement = conn.prepare(
        "SELECT id, text, done FROM artifact_criteria \
         WHERE project_id = ?1 AND artifact_id = ?2 ORDER BY position",
    )?;
    let rows = statement.query_map(params![project_id, artifact_id], |row| {
        Ok(ArtifactCriterion {
            id: row.get(0)?,
            text: row.get(1)?,
            done: row.get(2)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(DbError::from)
}

#[cfg(test)]
#[path = "../../tests/plan_artifacts_tests.rs"]
mod tests;
