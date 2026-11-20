use rusqlite::{params, OptionalExtension};

use crate::attachments::Attachment;

use super::connection::Db;
use super::error::DbError;

const NOW_MS_SQL: &str = "CAST(unixepoch('subsec') * 1000 AS INTEGER)";

impl Db {
    pub fn insert_attachment(
        &self,
        project_id: i64,
        attachment: &Attachment,
    ) -> Result<(), DbError> {
        let conn = self.conn()?;
        let inserted = conn.execute(
            &format!(
                "INSERT INTO attachments (project_id, id, name, kind, size, path, created_at) \
                 SELECT id, ?2, ?3, ?4, ?5, ?6, {NOW_MS_SQL} FROM projects WHERE id = ?1"
            ),
            params![
                project_id,
                attachment.id,
                attachment.name,
                attachment.kind,
                attachment.size as i64,
                attachment.path
            ],
        )?;
        if inserted == 0 {
            return Err(DbError::ProjectNotFound(project_id));
        }
        Ok(())
    }

    // Devolve os anexos achados, na ordem dos ids; id desconhecido é simplesmente omitido.
    pub fn find_attachments(
        &self,
        project_id: i64,
        ids: &[String],
    ) -> Result<Vec<Attachment>, DbError> {
        let conn = self.conn()?;
        let mut found = Vec::with_capacity(ids.len());
        for id in ids {
            let attachment = conn
                .query_row(
                    "SELECT id, name, kind, size, path FROM attachments                      WHERE project_id = ?1 AND id = ?2",
                    params![project_id, id],
                    |row| {
                        Ok(Attachment {
                            id: row.get(0)?,
                            name: row.get(1)?,
                            kind: row.get(2)?,
                            size: row.get::<_, i64>(3)? as u64,
                            path: row.get(4)?,
                        })
                    },
                )
                .optional()?;
            found.extend(attachment);
        }
        Ok(found)
    }

    pub fn mark_attachments_sent(&self, project_id: i64, ids: &[String]) -> Result<(), DbError> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        for id in ids {
            tx.execute(
                &format!(
                    "UPDATE attachments SET sent_at = {NOW_MS_SQL}                      WHERE project_id = ?1 AND id = ?2"
                ),
                params![project_id, id],
            )?;
        }
        Ok(tx.commit()?)
    }

    pub fn list_sent_attachment_ids(&self, project_id: i64) -> Result<Vec<String>, DbError> {
        let conn = self.conn()?;
        let mut statement = conn
            .prepare("SELECT id FROM attachments WHERE project_id = ?1 AND sent_at IS NOT NULL")?;
        let rows = statement.query_map([project_id], |row| row.get(0))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn delete_sent_attachments(&self, project_id: i64) -> Result<(), DbError> {
        let conn = self.conn()?;
        conn.execute(
            "DELETE FROM attachments WHERE project_id = ?1 AND sent_at IS NOT NULL",
            [project_id],
        )?;
        Ok(())
    }

    pub fn delete_attachment(&self, project_id: i64, id: &str) -> Result<bool, DbError> {
        let conn = self.conn()?;
        let deleted = conn.execute(
            "DELETE FROM attachments WHERE project_id = ?1 AND id = ?2",
            params![project_id, id],
        )?;
        Ok(deleted > 0)
    }

    pub fn list_unsent_attachments(&self) -> Result<Vec<(i64, String)>, DbError> {
        let conn = self.conn()?;
        let mut statement =
            conn.prepare("SELECT project_id, id FROM attachments WHERE sent_at IS NULL")?;
        let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn delete_unsent_attachments(&self) -> Result<(), DbError> {
        let conn = self.conn()?;
        conn.execute("DELETE FROM attachments WHERE sent_at IS NULL", [])?;
        Ok(())
    }

    pub fn delete_project_attachments(&self, project_id: i64) -> Result<(), DbError> {
        let conn = self.conn()?;
        conn.execute(
            "DELETE FROM attachments WHERE project_id = ?1",
            [project_id],
        )?;
        Ok(())
    }
}
