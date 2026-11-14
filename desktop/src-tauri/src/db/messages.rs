use rusqlite::params;

use crate::events::{Speaker, VokiaEvent};

use super::connection::Db;
use super::error::DbError;

const HISTORY_LIMIT: i64 = 500;

impl Db {
    pub fn clear_messages(&self, project_id: i64) -> Result<(), DbError> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let exists: bool = tx.query_row(
            "SELECT EXISTS (SELECT 1 FROM projects WHERE id = ?1)",
            [project_id],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(DbError::ProjectNotFound(project_id));
        }
        tx.execute("DELETE FROM messages WHERE project_id = ?1", [project_id])?;
        tx.execute(
            "DELETE FROM agent_sessions WHERE project_id = ?1",
            [project_id],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn insert_message(
        &self,
        project_id: i64,
        from: Speaker,
        to: Speaker,
        text: &str,
        time: u64,
    ) -> Result<(), DbError> {
        let conn = self.conn()?;
        let inserted = conn.execute(
            "INSERT INTO messages(project_id, from_agent, to_agent, text, created_at) \
             SELECT id, ?2, ?3, ?4, ?5 FROM projects WHERE id = ?1",
            params![project_id, from.as_str(), to.as_str(), text, time as i64],
        )?;
        if inserted == 0 {
            return Err(DbError::ProjectNotFound(project_id));
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
                project_id: Some(project_id),
            })
        })
        .collect()
    }
}

fn parse_speaker(value: &str) -> Result<Speaker, DbError> {
    Speaker::parse(value).ok_or_else(|| DbError::InvalidSpeaker(value.to_string()))
}
