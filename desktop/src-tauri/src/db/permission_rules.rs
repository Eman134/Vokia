use rusqlite::params;
use serde::Serialize;

use super::connection::Db;
use super::error::DbError;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionRule {
    pub tool: String,
    pub command: String,
    pub created_at: i64,
}

impl Db {
    pub fn has_permission_rule(
        &self,
        project_id: i64,
        tool: &str,
        command: &str,
    ) -> Result<bool, DbError> {
        let conn = self.conn()?;
        let matching: i64 = conn.query_row(
            "SELECT COUNT(*) FROM permission_rules \
             WHERE project_id = ?1 AND tool = ?2 AND command = ?3",
            params![project_id, tool, command],
            |row| row.get(0),
        )?;
        Ok(matching > 0)
    }

    pub fn save_permission_rule(
        &self,
        project_id: i64,
        tool: &str,
        command: &str,
        time: u64,
    ) -> Result<(), DbError> {
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO permission_rules(project_id, tool, command, created_at) \
             VALUES (?1, ?2, ?3, ?4) \
             ON CONFLICT(project_id, tool, command) DO NOTHING",
            params![project_id, tool, command, time as i64],
        )?;
        Ok(())
    }

    pub fn list_permission_rules(&self, project_id: i64) -> Result<Vec<PermissionRule>, DbError> {
        let conn = self.conn()?;
        let mut statement = conn.prepare(
            "SELECT tool, command, created_at FROM permission_rules \
             WHERE project_id = ?1 ORDER BY created_at DESC, tool, command",
        )?;
        let rules = statement
            .query_map([project_id], |row| {
                Ok(PermissionRule {
                    tool: row.get(0)?,
                    command: row.get(1)?,
                    created_at: row.get(2)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rules)
    }

    pub fn delete_permission_rule(
        &self,
        project_id: i64,
        tool: &str,
        command: &str,
    ) -> Result<(), DbError> {
        let conn = self.conn()?;
        conn.execute(
            "DELETE FROM permission_rules WHERE project_id = ?1 AND tool = ?2 AND command = ?3",
            params![project_id, tool, command],
        )?;
        Ok(())
    }
}
