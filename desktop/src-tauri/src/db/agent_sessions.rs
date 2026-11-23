use rusqlite::{params, OptionalExtension};
use serde::Serialize;

use crate::events::Agent;
use crate::session_key::SessionKey;

use super::connection::Db;
use super::error::DbError;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentPause {
    pub project_id: i64,
    pub agent: Agent,
}

impl Db {
    pub fn save_agent_session(&self, key: &SessionKey, session_id: &str) -> Result<(), DbError> {
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO agent_sessions(project_id, agent, session_id, updated_at) \
             VALUES (?1, ?2, ?3, CAST(unixepoch('subsec') * 1000 AS INTEGER)) \
             ON CONFLICT(project_id, agent) DO UPDATE SET \
             session_id = excluded.session_id, updated_at = excluded.updated_at",
            params![key.project_id, key.agent.as_str(), session_id],
        )?;
        Ok(())
    }

    pub fn agent_session(&self, key: &SessionKey) -> Result<Option<String>, DbError> {
        let conn = self.conn()?;
        let session_id = conn
            .query_row(
                "SELECT session_id FROM agent_sessions WHERE project_id = ?1 AND agent = ?2",
                params![key.project_id, key.agent.as_str()],
                |row| row.get(0),
            )
            .optional()?;
        Ok(session_id)
    }

    pub fn clear_agent_session(&self, key: &SessionKey) -> Result<(), DbError> {
        let conn = self.conn()?;
        conn.execute(
            "DELETE FROM agent_sessions WHERE project_id = ?1 AND agent = ?2",
            params![key.project_id, key.agent.as_str()],
        )?;
        Ok(())
    }

    pub fn clear_agent_sessions(&self, agent: &Agent) -> Result<(), DbError> {
        let conn = self.conn()?;
        conn.execute(
            "DELETE FROM agent_sessions WHERE agent = ?1",
            params![agent.as_str()],
        )?;
        Ok(())
    }

    pub fn agent_pauses(&self) -> Result<Vec<AgentPause>, DbError> {
        let conn = self.conn()?;
        let mut statement = conn.prepare(
            "SELECT agent_pauses.project_id, agent_pauses.agent FROM agent_pauses \
             JOIN projects ON projects.id = agent_pauses.project_id \
             ORDER BY agent_pauses.paused_at",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?;
        rows.map(|row| -> Result<AgentPause, DbError> {
            let (project_id, agent) = row?;
            Ok(AgentPause {
                project_id,
                agent: parse_agent(&agent)?,
            })
        })
        .collect()
    }

    pub fn project_paused_agents(&self, project_id: i64) -> Result<Vec<Agent>, DbError> {
        Ok(self
            .agent_pauses()?
            .into_iter()
            .filter(|pause| pause.project_id == project_id)
            .map(|pause| pause.agent)
            .collect())
    }

    pub fn set_agent_paused(
        &self,
        project_id: i64,
        agent: Agent,
        paused: bool,
    ) -> Result<(), DbError> {
        let conn = self.conn()?;
        let key = agent_key(&agent);
        if !paused {
            conn.execute(
                "DELETE FROM agent_pauses WHERE project_id = ?1 AND agent = ?2",
                params![project_id, key],
            )?;
            return Ok(());
        }
        let inserted = conn.execute(
            "INSERT OR REPLACE INTO agent_pauses(project_id, agent) \
             SELECT id, ?2 FROM projects WHERE id = ?1",
            params![project_id, key],
        )?;
        if inserted == 0 {
            return Err(DbError::ProjectNotFound(project_id));
        }
        Ok(())
    }

    pub fn is_agent_paused(&self, project_id: i64, agent: &Agent) -> Result<bool, DbError> {
        let conn = self.conn()?;
        let base = crate::agents::replica_of(agent.as_str()).map(|(base, _)| base);
        let found: Option<i64> = match base {
            Some(base_id) => conn
                .query_row(
                    "SELECT 1 FROM agent_pauses WHERE project_id = ?1 AND (agent = ?2 OR agent = ?3) LIMIT 1",
                    params![project_id, agent.as_str(), base_id],
                    |row| row.get(0),
                )
                .optional()?,
            None => conn
                .query_row(
                    "SELECT 1 FROM agent_pauses WHERE project_id = ?1 AND agent = ?2 LIMIT 1",
                    params![project_id, agent.as_str()],
                    |row| row.get(0),
                )
                .optional()?,
        };
        Ok(found.is_some())
    }

    pub fn pause_project_agents(&self, project_id: i64, agents: &[Agent]) -> Result<(), DbError> {
        for agent in agents {
            self.set_agent_paused(project_id, agent.clone(), true)?;
        }
        Ok(())
    }

    pub fn resume_project_agents(&self, project_id: i64) -> Result<(), DbError> {
        let conn = self.conn()?;
        conn.execute(
            "DELETE FROM agent_pauses WHERE project_id = ?1",
            params![project_id],
        )?;
        Ok(())
    }
}

fn agent_key(agent: &Agent) -> String {
    agent.as_str().to_string()
}

fn parse_agent(value: &str) -> Result<Agent, DbError> {
    Agent::from_stored(value).ok_or_else(|| DbError::InvalidAgent(value.to_string()))
}
