use rusqlite::{params, Row};
use serde::{Deserialize, Serialize};

use crate::events::Agent;
use crate::state::RequestContext;

use super::connection::Db;
use super::error::DbError;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct QueuedRequestRecord {
    pub id: i64,
    pub project_id: i64,
    pub to: Agent,
    pub text: String,
    pub channel_id: Option<i64>,
    pub item_id: Option<String>,
    pub attachments: Vec<String>,
    pub position: i64,
}

#[derive(Clone, Debug)]
pub struct PersistedFlowJob {
    pub id: i64,
    pub project_id: i64,
    pub agent: Agent,
    pub text: String,
    pub context: RequestContext,
}

fn parse_stored_agent(value: &str, col: usize) -> rusqlite::Result<Agent> {
    Agent::from_stored(value).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            col,
            rusqlite::types::Type::Text,
            format!("agente inválido: {value}").into(),
        )
    })
}

fn queued_request_from_row(row: &Row<'_>) -> rusqlite::Result<QueuedRequestRecord> {
    let attachments_json: String = row.get(6)?;
    let attachments: Vec<String> = serde_json::from_str(&attachments_json).unwrap_or_default();
    let raw_agent: String = row.get(2)?;
    Ok(QueuedRequestRecord {
        id: row.get(0)?,
        project_id: row.get(1)?,
        to: parse_stored_agent(&raw_agent, 2)?,
        text: row.get(3)?,
        channel_id: row.get(4)?,
        item_id: row.get(5)?,
        attachments,
        position: row.get(7)?,
    })
}

fn flow_job_from_row(row: &Row<'_>) -> rusqlite::Result<PersistedFlowJob> {
    let context_json: String = row.get(4)?;
    let context: RequestContext = serde_json::from_str(&context_json).unwrap_or_default();
    let raw_agent: String = row.get(2)?;
    Ok(PersistedFlowJob {
        id: row.get(0)?,
        project_id: row.get(1)?,
        agent: parse_stored_agent(&raw_agent, 2)?,
        text: row.get(3)?,
        context,
    })
}

impl Db {
    pub fn list_queued_requests(&self) -> Result<Vec<QueuedRequestRecord>, DbError> {
        let conn = self.conn()?;
        let mut statement = conn.prepare(
            "SELECT id, project_id, to_agent, text, channel_id, item_id, attachments, position
             FROM queued_requests
             ORDER BY position ASC, id ASC",
        )?;
        let rows = statement.query_map([], queued_request_from_row)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn enqueue_queued_request(
        &self,
        project_id: i64,
        to: &Agent,
        text: &str,
        channel_id: Option<i64>,
        item_id: Option<&str>,
        attachments: &[String],
    ) -> Result<QueuedRequestRecord, DbError> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let position: i64 = tx.query_row(
            "SELECT COALESCE(MAX(position), 0) + 1 FROM queued_requests",
            [],
            |row| row.get(0),
        )?;
        let attachments_json = serde_json::to_string(attachments).unwrap_or_else(|_| "[]".into());
        tx.execute(
            "INSERT INTO queued_requests (project_id, to_agent, text, channel_id, item_id, attachments, position)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                project_id,
                to.as_str(),
                text,
                channel_id,
                item_id,
                attachments_json,
                position
            ],
        )?;
        let id = tx.last_insert_rowid();
        tx.commit()?;
        Ok(QueuedRequestRecord {
            id,
            project_id,
            to: to.clone(),
            text: text.to_string(),
            channel_id,
            item_id: item_id.map(str::to_string),
            attachments: attachments.to_vec(),
            position,
        })
    }

    pub fn update_queued_request(&self, id: i64, text: &str) -> Result<bool, DbError> {
        let conn = self.conn()?;
        let updated = conn.execute(
            "UPDATE queued_requests SET text = ?2 WHERE id = ?1",
            params![id, text],
        )?;
        Ok(updated > 0)
    }

    pub fn move_queued_request(&self, id: i64, direction: &str) -> Result<bool, DbError> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let entries: Vec<(i64, i64)> = {
            let mut statement =
                tx.prepare("SELECT id, position FROM queued_requests ORDER BY position ASC, id ASC")?;
            let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
            rows.collect::<Result<_, _>>()?
        };
        let Some(index) = entries.iter().position(|(row_id, _)| *row_id == id) else {
            return Ok(false);
        };
        let neighbor = match direction {
            "up" if index > 0 => index - 1,
            "down" if index + 1 < entries.len() => index + 1,
            _ => return Ok(false),
        };
        let (id_a, pos_a) = entries[index];
        let (id_b, pos_b) = entries[neighbor];
        tx.execute(
            "UPDATE queued_requests SET position = ?2 WHERE id = ?1",
            params![id_a, pos_b],
        )?;
        tx.execute(
            "UPDATE queued_requests SET position = ?2 WHERE id = ?1",
            params![id_b, pos_a],
        )?;
        tx.commit()?;
        Ok(true)
    }

    pub fn remove_queued_request(&self, id: i64) -> Result<bool, DbError> {
        let conn = self.conn()?;
        let deleted = conn.execute("DELETE FROM queued_requests WHERE id = ?1", [id])?;
        Ok(deleted > 0)
    }

    pub fn enqueue_flow_job(
        &self,
        project_id: i64,
        agent: &Agent,
        text: &str,
        context: &RequestContext,
    ) -> Result<i64, DbError> {
        let conn = self.conn()?;
        let context_json = serde_json::to_string(context).unwrap_or_else(|_| "{}".into());
        conn.execute(
            "INSERT INTO flow_queue (project_id, agent, text, context_json) VALUES (?1, ?2, ?3, ?4)",
            params![project_id, agent.as_str(), text, context_json],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn remove_flow_job(&self, id: i64) -> Result<(), DbError> {
        let conn = self.conn()?;
        conn.execute("DELETE FROM flow_queue WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn clear_flow_queue_for_session(
        &self,
        project_id: i64,
        agent: &Agent,
    ) -> Result<(), DbError> {
        let conn = self.conn()?;
        conn.execute(
            "DELETE FROM flow_queue WHERE project_id = ?1 AND agent = ?2",
            params![project_id, agent.as_str()],
        )?;
        Ok(())
    }

    pub fn list_flow_queue(&self) -> Result<Vec<PersistedFlowJob>, DbError> {
        let conn = self.conn()?;
        let mut statement = conn.prepare(
            "SELECT id, project_id, agent, text, context_json
             FROM flow_queue
             ORDER BY id ASC",
        )?;
        let rows = statement.query_map([], flow_job_from_row)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    struct TempDir(PathBuf);

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn test_db() -> (TempDir, Db, i64) {
        let id = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "vokia-queued-requests-test-{}-{id}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let db = Db::open(&dir).unwrap();
        let project_id = {
            let conn = db.conn().unwrap();
            conn.execute(
                "INSERT INTO projects (name, path, selected) VALUES ('Vokia', ?1, 1)",
                [dir.to_str().unwrap()],
            )
            .unwrap();
            conn.last_insert_rowid()
        };
        (TempDir(dir), db, project_id)
    }

    #[test]
    fn queued_requests_crud_and_reorder() {
        let (_dir, db, project_id) = test_db();
        let first = db
            .enqueue_queued_request(
                project_id,
                &Agent::from_stored("kairo").unwrap(),
                "Primeiro pedido",
                Some(2),
                Some("010"),
                &["att-1".into()],
            )
            .unwrap();
        let second = db
            .enqueue_queued_request(
                project_id,
                &Agent::from_stored("vexa").unwrap(),
                "Segundo pedido",
                None,
                None,
                &[],
            )
            .unwrap();

        let listed = db.list_queued_requests().unwrap();
        assert_eq!(listed, vec![first.clone(), second.clone()]);

        assert!(db.update_queued_request(first.id, "Primeiro editado").unwrap());
        assert!(db.move_queued_request(second.id, "up").unwrap());

        let reordered = db.list_queued_requests().unwrap();
        assert_eq!(reordered[0].id, second.id);
        assert_eq!(reordered[1].id, first.id);
        assert_eq!(reordered[1].text, "Primeiro editado");

        assert!(db.remove_queued_request(second.id).unwrap());
        let remaining = db.list_queued_requests().unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, first.id);
    }

    #[test]
    fn flow_queue_persists_and_clears() {
        let (_dir, db, project_id) = test_db();
        let mut ctx = RequestContext::new(Some(7), Some("042".into()), Some(3));
        ctx.attachments = vec!["a.png".into()];
        let id1 = db
            .enqueue_flow_job(
                project_id,
                &Agent::from_stored("kairo").unwrap(),
                "Job 1",
                &ctx,
            )
            .unwrap();
        let id2 = db
            .enqueue_flow_job(
                project_id,
                &Agent::from_stored("kairo").unwrap(),
                "Job 2",
                &RequestContext::default(),
            )
            .unwrap();

        let jobs = db.list_flow_queue().unwrap();
        assert_eq!(jobs.len(), 2);
        assert_eq!(jobs[0].id, id1);
        assert_eq!(jobs[0].text, "Job 1");
        assert_eq!(jobs[0].context.item_id.as_deref(), Some("042"));
        assert_eq!(jobs[0].context.attachments, vec!["a.png"]);
        assert_eq!(jobs[1].id, id2);

        db.remove_flow_job(id1).unwrap();
        assert_eq!(db.list_flow_queue().unwrap().len(), 1);

        db.clear_flow_queue_for_session(project_id, &Agent::from_stored("kairo").unwrap())
            .unwrap();
        assert!(db.list_flow_queue().unwrap().is_empty());
    }
}
