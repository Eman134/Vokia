use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use super::artifact_kinds::{find_kind, KIND_PLAN};
use super::artifacts::{json_list, json_text, read_plan_artifact, Artifact, ARTIFACT_ID_PREFIX};
use super::backlog::ensure_project;
use super::connection::Db;
use super::error::DbError;

pub const TITLE_MAX_CHARS: usize = 200;
pub const SUMMARY_MAX_CHARS: usize = 500;
pub const BODY_MAX_CHARS: usize = 100_000;

const FENCE: &str = "```";
const CHECK_OPEN: &str = "- [ ]";
const CHECK_DONE_LOWER: &str = "- [x]";
const CHECK_DONE_UPPER: &str = "- [X]";
const CHECK_MARK_OFFSET: usize = 3;

const NOW_MS: &str = "CAST(unixepoch('subsec') * 1000 AS INTEGER)";

const DOCUMENT_COLUMNS: &str = "id, kind, title, summary, body, tags, author, created_at, updated_at";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactDocument {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub summary: String,
    pub body: String,
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactSummary {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub summary: String,
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item_id: Option<String>,
    pub updated_at: i64,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum ArtifactDetail {
    Plan(Artifact),
    Document(ArtifactDocument),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewArtifactDocument {
    pub kind: String,
    pub title: String,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub author: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactDocumentPatch {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub expected_updated_at: Option<i64>,
}

impl Db {
    pub fn create_artifact_document(
        &self,
        project_id: i64,
        new: NewArtifactDocument,
    ) -> Result<ArtifactDocument, DbError> {
        let kind = find_kind(&new.kind)
            .ok_or_else(|| DbError::UnknownArtifactKind(new.kind.clone()))?;
        let title = valid_title(&new.title)?;
        let summary = new.summary.unwrap_or_default();
        let body = new.body.unwrap_or_else(|| kind.template.to_string());
        check_length("resumo", &summary, SUMMARY_MAX_CHARS)?;
        check_length("corpo", &body, BODY_MAX_CHARS)?;
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        ensure_project(&tx, project_id)?;
        let seq: i64 = tx.query_row(
            "SELECT COALESCE(MAX(seq), 0) + 1 FROM artifact_documents WHERE project_id = ?1 AND kind = ?2",
            params![project_id, kind.kind],
            |row| row.get(0),
        )?;
        let id = format!("{}-{seq}", kind.kind);
        tx.execute(
            &format!(
                "INSERT INTO artifact_documents \
                 (project_id, id, kind, seq, title, summary, body, tags, author, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, {NOW_MS}, {NOW_MS})"
            ),
            params![
                project_id,
                id,
                kind.kind,
                seq,
                title,
                summary,
                body,
                json_text(&new.tags)?,
                new.author,
            ],
        )?;
        let document = read_document(&tx, project_id, &id)?
            .ok_or_else(|| DbError::ArtifactNotFound(id.clone()))?;
        tx.commit()?;
        Ok(document)
    }

    pub fn update_artifact_document(
        &self,
        project_id: i64,
        id: &str,
        patch: ArtifactDocumentPatch,
    ) -> Result<ArtifactDocument, DbError> {
        reject_plan(id)?;
        if patch.title.is_none()
            && patch.summary.is_none()
            && patch.body.is_none()
            && patch.tags.is_none()
        {
            return Err(DbError::EmptyArtifactPatch);
        }
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        ensure_project(&tx, project_id)?;
        let current = read_document(&tx, project_id, id)?
            .ok_or_else(|| DbError::ArtifactNotFound(id.to_string()))?;
        if patch.expected_updated_at.is_some_and(|expected| expected != current.updated_at) {
            return Err(DbError::ArtifactConflict(id.to_string()));
        }
        let title = match &patch.title {
            Some(title) => valid_title(title)?,
            None => current.title,
        };
        let summary = patch.summary.unwrap_or(current.summary);
        let body = patch.body.unwrap_or(current.body);
        let tags = patch.tags.unwrap_or(current.tags);
        check_length("resumo", &summary, SUMMARY_MAX_CHARS)?;
        check_length("corpo", &body, BODY_MAX_CHARS)?;
        tx.execute(
            &format!(
                "UPDATE artifact_documents SET title = ?3, summary = ?4, body = ?5, tags = ?6, \
                 updated_at = MAX({NOW_MS}, updated_at + 1) WHERE project_id = ?1 AND id = ?2"
            ),
            params![project_id, id, title, summary, body, json_text(&tags)?],
        )?;
        let document = read_document(&tx, project_id, id)?
            .ok_or_else(|| DbError::ArtifactNotFound(id.to_string()))?;
        tx.commit()?;
        Ok(document)
    }

    pub fn delete_artifact_document(&self, project_id: i64, id: &str) -> Result<(), DbError> {
        reject_plan(id)?;
        let conn = self.conn()?;
        ensure_project(&conn, project_id)?;
        let removed = conn.execute(
            "DELETE FROM artifact_documents WHERE project_id = ?1 AND id = ?2",
            params![project_id, id],
        )?;
        if removed == 0 {
            return Err(DbError::ArtifactNotFound(id.to_string()));
        }
        Ok(())
    }

    // O índice conta só os marcadores fora de blocos de código, na ordem do corpo.
    pub fn set_artifact_check(
        &self,
        project_id: i64,
        id: &str,
        index: usize,
        done: bool,
    ) -> Result<ArtifactDocument, DbError> {
        reject_plan(id)?;
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        ensure_project(&tx, project_id)?;
        let current = read_document(&tx, project_id, id)?
            .ok_or_else(|| DbError::ArtifactNotFound(id.to_string()))?;
        let body = toggle_check(&current.body, index, done)
            .ok_or(DbError::ArtifactCheckNotFound(index))?;
        tx.execute(
            &format!(
                "UPDATE artifact_documents SET body = ?3, \
                 updated_at = MAX({NOW_MS}, updated_at + 1) WHERE project_id = ?1 AND id = ?2"
            ),
            params![project_id, id, body],
        )?;
        let document = read_document(&tx, project_id, id)?
            .ok_or_else(|| DbError::ArtifactNotFound(id.to_string()))?;
        tx.commit()?;
        Ok(document)
    }

    pub fn list_artifact_summaries(&self, project_id: i64) -> Result<Vec<ArtifactSummary>, DbError> {
        let conn = self.conn()?;
        ensure_project(&conn, project_id)?;
        let mut statement = conn.prepare(&format!(
            "SELECT id, '{KIND_PLAN}' AS kind, title, summary, tags, author, item_id, updated_at \
             FROM artifacts WHERE project_id = ?1 \
             UNION ALL \
             SELECT id, kind, title, summary, tags, author, NULL, updated_at \
             FROM artifact_documents WHERE project_id = ?1 \
             ORDER BY updated_at DESC"
        ))?;
        let rows = statement.query_map([project_id], |row| Ok(summary_from_row(row)))?;
        rows.map(|row| -> Result<ArtifactSummary, DbError> { row? })
            .collect()
    }

    pub fn get_artifact(&self, project_id: i64, id: &str) -> Result<ArtifactDetail, DbError> {
        let conn = self.conn()?;
        ensure_project(&conn, project_id)?;
        let detail = if id.starts_with(ARTIFACT_ID_PREFIX) {
            read_plan_artifact(&conn, project_id, id)?.map(ArtifactDetail::Plan)
        } else {
            read_document(&conn, project_id, id)?.map(ArtifactDetail::Document)
        };
        detail.ok_or_else(|| DbError::ArtifactNotFound(id.to_string()))
    }
}

fn reject_plan(id: &str) -> Result<(), DbError> {
    if id.starts_with(ARTIFACT_ID_PREFIX) {
        return Err(DbError::ArtifactReadOnly(id.to_string()));
    }
    Ok(())
}

fn valid_title(title: &str) -> Result<String, DbError> {
    let trimmed = title.trim();
    if trimmed.is_empty() {
        return Err(DbError::EmptyArtifactTitle);
    }
    check_length("título", trimmed, TITLE_MAX_CHARS)?;
    Ok(trimmed.to_string())
}

fn check_length(field: &'static str, text: &str, max: usize) -> Result<(), DbError> {
    if text.chars().count() > max {
        return Err(DbError::ArtifactFieldTooLong { field, max });
    }
    Ok(())
}

fn read_document(
    conn: &Connection,
    project_id: i64,
    id: &str,
) -> Result<Option<ArtifactDocument>, DbError> {
    conn.query_row(
        &format!("SELECT {DOCUMENT_COLUMNS} FROM artifact_documents WHERE project_id = ?1 AND id = ?2"),
        params![project_id, id],
        |row| Ok(document_from_row(row)),
    )
    .optional()?
    .transpose()
}

fn document_from_row(row: &Row<'_>) -> Result<ArtifactDocument, DbError> {
    let id: String = row.get("id")?;
    Ok(ArtifactDocument {
        kind: row.get("kind")?,
        title: row.get("title")?,
        summary: row.get("summary")?,
        body: row.get("body")?,
        tags: json_list(row, "tags", &id)?,
        author: row.get("author")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        id,
    })
}

fn summary_from_row(row: &Row<'_>) -> Result<ArtifactSummary, DbError> {
    let id: String = row.get("id")?;
    Ok(ArtifactSummary {
        kind: row.get("kind")?,
        title: row.get("title")?,
        summary: row.get("summary")?,
        tags: json_list(row, "tags", &id)?,
        author: row.get("author")?,
        item_id: row.get("item_id")?,
        updated_at: row.get("updated_at")?,
        id,
    })
}

fn is_check_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    [CHECK_OPEN, CHECK_DONE_LOWER, CHECK_DONE_UPPER]
        .iter()
        .any(|marker| trimmed.starts_with(marker))
}

// Troca o índice-ésimo marcador "- [ ]"/"- [x]" fora de blocos cercados por crases; None se o índice não existe.
fn toggle_check(body: &str, index: usize, done: bool) -> Option<String> {
    let mut in_fence = false;
    let mut seen = 0;
    let mut out = String::with_capacity(body.len());
    let mut changed = false;
    for line in body.split_inclusive('\n') {
        if line.trim_start().starts_with(FENCE) {
            in_fence = !in_fence;
        } else if !in_fence && is_check_line(line) {
            if seen == index {
                let mark_at = line.len() - line.trim_start().len() + CHECK_MARK_OFFSET;
                out.push_str(&line[..mark_at]);
                out.push(if done { 'x' } else { ' ' });
                out.push_str(&line[mark_at + 1..]);
                seen += 1;
                changed = true;
                continue;
            }
            seen += 1;
        }
        out.push_str(line);
    }
    changed.then_some(out)
}

#[cfg(test)]
#[path = "../../tests/artifact_documents_tests.rs"]
mod tests;
