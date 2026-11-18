use rusqlite::params;

use super::connection::Db;
use super::error::DbError;

#[derive(Default)]
pub struct ApprovalHistory {
    pub approved: u32,
    pub last_denied_at: Option<u64>,
}

impl Db {
    pub fn approval_history(
        &self,
        project_id: i64,
        tool: &str,
        command: &str,
    ) -> Result<ApprovalHistory, DbError> {
        let conn = self.conn()?;
        let (approved, last_denied_at) = conn.query_row(
            "SELECT \
               COALESCE((SELECT approved_count FROM permission_approvals \
                         WHERE project_id = ?1 AND tool = ?2 AND command = ?3), 0), \
               (SELECT last_denied_at FROM permission_denials \
                WHERE project_id = ?1 AND tool = ?2 AND command = ?3)",
            params![project_id, tool, command],
            |row| Ok((row.get::<_, u32>(0)?, row.get::<_, Option<i64>>(1)?)),
        )?;
        Ok(ApprovalHistory {
            approved,
            last_denied_at: last_denied_at.map(|time| time as u64),
        })
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

    pub fn record_denial(
        &self,
        project_id: i64,
        tool: &str,
        command: &str,
        time: u64,
    ) -> Result<(), DbError> {
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO permission_denials(project_id, tool, command, denied_count, last_denied_at) \
             VALUES (?1, ?2, ?3, 1, ?4) \
             ON CONFLICT(project_id, tool, command) \
             DO UPDATE SET denied_count = denied_count + 1, last_denied_at = ?4",
            params![project_id, tool, command, time as i64],
        )?;
        Ok(())
    }
}
