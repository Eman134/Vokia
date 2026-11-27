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
    pub last_used_at: Option<i64>,
}

const COMPOUND_OPERATORS: [&str; 11] = ["&&", "||", ";", "|", "&", "$(", "`", ">", "<", "\n", "\r"];
const ASSIGNMENT_MARK: char = '=';
const ROOT_RULE_SUFFIX: &str = " *";

/// Primeiro token de um comando simples; `None` quando há operador composto ou prefixo `VAR=val`,
/// porque liberar a raiz de uma composição liberaria o que vem depois dela.
pub fn root_command(target: &str) -> Option<&str> {
    if COMPOUND_OPERATORS.iter().any(|operator| target.contains(operator)) {
        return None;
    }
    let token = target.split_whitespace().next()?;
    (!token.contains(ASSIGNMENT_MARK)).then_some(token)
}

impl Db {
    pub fn has_permission_rule(
        &self,
        project_id: i64,
        tool: &str,
        command: &str,
    ) -> Result<bool, DbError> {
        let conn = self.conn()?;
        let root_pattern = root_command(command).map(|root| format!("{root}{ROOT_RULE_SUFFIX}"));
        let matching: i64 = conn.query_row(
            "SELECT COUNT(*) FROM permission_rules \
             WHERE project_id = ?1 AND tool = ?2 AND (command = ?3 OR (?4 IS NOT NULL AND command = ?4))",
            params![project_id, tool, command, root_pattern],
            |row| row.get(0),
        )?;
        if matching > 0 {
            let _ = conn.execute(
                "UPDATE permission_rules SET last_used_at = CAST(unixepoch('subsec') * 1000 AS INTEGER) \
                 WHERE project_id = ?1 AND tool = ?2 AND (command = ?3 OR (?4 IS NOT NULL AND command = ?4))",
                params![project_id, tool, command, root_pattern],
            );
        }
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
            "INSERT INTO permission_rules(project_id, tool, command, created_at, last_used_at) \
             VALUES (?1, ?2, ?3, ?4, ?4) \
             ON CONFLICT(project_id, tool, command) DO UPDATE SET last_used_at = excluded.last_used_at",
            params![project_id, tool, command, time as i64],
        )?;
        Ok(())
    }

    pub fn list_permission_rules(&self, project_id: i64) -> Result<Vec<PermissionRule>, DbError> {
        let conn = self.conn()?;
        let mut statement = conn.prepare(
            "SELECT tool, command, created_at, last_used_at FROM permission_rules \
             WHERE project_id = ?1 ORDER BY COALESCE(last_used_at, created_at) DESC, tool, command",
        )?;
        let rules = statement
            .query_map([project_id], |row| {
                Ok(PermissionRule {
                    tool: row.get(0)?,
                    command: row.get(1)?,
                    created_at: row.get(2)?,
                    last_used_at: row.get(3)?,
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

#[cfg(test)]
#[path = "../../tests/permission_root_db_tests.rs"]
mod tests;
