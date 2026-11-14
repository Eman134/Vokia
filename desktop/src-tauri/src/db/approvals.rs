use rusqlite::params;

use super::connection::Db;
use super::error::DbError;

impl Db {
    pub fn approved_count(
        &self,
        project_id: i64,
        tool: &str,
        command: &str,
    ) -> Result<u32, DbError> {
        let conn = self.conn()?;
        let found = conn.query_row(
            "SELECT approved_count FROM permission_approvals \
             WHERE project_id = ?1 AND tool = ?2 AND command = ?3",
            params![project_id, tool, command],
            |row| row.get(0),
        );
        match found {
            Ok(count) => Ok(count),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(0),
            Err(other) => Err(DbError::Sqlite(other)),
        }
    }

    pub fn record_approval(
        &self,
        project_id: i64,
        tool: &str,
        command: &str,
        time: u64,
    ) -> Result<(), DbError> {
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO permission_approvals(project_id, tool, command, approved_count, last_approved_at) \
             VALUES (?1, ?2, ?3, 1, ?4) \
             ON CONFLICT(project_id, tool, command) \
             DO UPDATE SET approved_count = approved_count + 1, last_approved_at = ?4",
            params![project_id, tool, command, time as i64],
        )?;
        Ok(())
    }
}
