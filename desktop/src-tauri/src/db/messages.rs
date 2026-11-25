use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::claude::{TurnActivity, TurnUsage};
use crate::events::{Agent, RequestProposal, Speaker, VokiaEvent};

use super::connection::Db;
use super::error::DbError;

const HISTORY_LIMIT: i64 = 500;

// O cartão de conclusão vai na própria tabela de mensagens: a ordem por id e o clear_messages valem para ele também.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredTaskResult {
    pub objective: String,
    pub concluded: bool,
    pub turns: i64,
    pub agents: Vec<Agent>,
}

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
        tx.execute(
            "DELETE FROM turn_work_steps WHERE project_id = ?1",
            [project_id],
        )?;
        tx.commit()?;
        Ok(())
    }

#[allow(clippy::too_many_arguments)] // cada argumento é uma coluna da mensagem
    pub fn insert_message(
        &self,
        project_id: i64,
        from: Speaker,
        to: Speaker,
        text: &str,
        time: u64,
        channel_id: Option<i64>,
        item_id: Option<&str>,
        activity: Option<&TurnActivity>,
        request_proposal: Option<&RequestProposal>,
        usage: Option<&TurnUsage>,
    ) -> Result<(), DbError> {
        let activity = activity
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| DbError::MessageActivityData(error.to_string()))?;
        let request_proposal = request_proposal
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| DbError::MessageProposalData(error.to_string()))?;
        let usage = usage
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| DbError::MessageActivityData(error.to_string()))?;
        let conn = self.conn()?;
        let inserted = conn.execute(
            "INSERT INTO messages(project_id, from_agent, to_agent, text, created_at, channel_id, item_id, activity, request_proposal, usage) \
             SELECT id, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10 FROM projects WHERE id = ?1",
            params![
                project_id,
                from.as_str(),
                to.as_str(),
                text,
                time as i64,
                channel_id,
                item_id,
                activity,
                request_proposal,
                usage
            ],
        )?;
        if inserted == 0 {
            return Err(DbError::ProjectNotFound(project_id));
        }
        Ok(())
    }

    pub fn insert_task_result(
        &self,
        project_id: i64,
        from: Speaker,
        time: u64,
        channel_id: Option<i64>,
        item_id: Option<&str>,
        result: &StoredTaskResult,
    ) -> Result<(), DbError> {
        let json = serde_json::to_string(result)
            .map_err(|error| DbError::MessageActivityData(error.to_string()))?;
        let conn = self.conn()?;
        let inserted = conn.execute(
            "INSERT INTO messages(project_id, from_agent, to_agent, text, created_at, channel_id, item_id, task_result) \
             SELECT id, ?2, ?3, '', ?4, ?5, ?6, ?7 FROM projects WHERE id = ?1",
            params![project_id, from.as_str(), Speaker::User.as_str(), time as i64, channel_id, item_id, json],
        )?;
        if inserted == 0 {
            return Err(DbError::ProjectNotFound(project_id));
        }
        Ok(())
    }

    pub fn load_history(&self, project_id: i64) -> Result<Vec<VokiaEvent>, DbError> {
        let conn = self.conn()?;
        let mut statement = conn.prepare(
            "SELECT from_agent, to_agent, text, created_at, channel_id, activity, request_proposal, usage, task_result, item_id FROM ( \
                 SELECT id, from_agent, to_agent, text, created_at, channel_id, activity, request_proposal, usage, task_result, item_id FROM messages \
                 WHERE project_id = ?1 ORDER BY id DESC LIMIT ?2 \
             ) ORDER BY id",
        )?;
        let rows = statement.query_map(params![project_id, HISTORY_LIMIT], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, Option<i64>>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, Option<String>>(7)?,
                row.get::<_, Option<String>>(8)?,
                row.get::<_, Option<String>>(9)?,
            ))
        })?;
        rows.map(|row| -> Result<VokiaEvent, DbError> {
            let (from, to, text, time, channel_id, activity, request_proposal, usage, task_result, item_id) = row?;
            // Instâncias excluídas deixam remetentes que já não existem: o id bruto segue como Speaker::Foreign e a mensagem é carregada, marcada.
            let foreign_instance = Speaker::parse(&from).is_none() || Speaker::parse(&to).is_none();
            let from = Speaker::parse(&from).unwrap_or_else(|| Speaker::Foreign(from.clone()));
            let to = Speaker::parse(&to).unwrap_or_else(|| Speaker::Foreign(to.clone()));
            // JSON ilegível não derruba o histórico: a linha cai como mensagem vazia, como qualquer outra sem texto útil.
            if let Some(result) = task_result.and_then(|json| serde_json::from_str::<StoredTaskResult>(&json).ok()) {
                return Ok(VokiaEvent::TaskResult {
                    project_id,
                    objective: result.objective,
                    concluded: result.concluded,
                    turns: result.turns,
                    agents: result.agents,
                    time: time as u64,
                    channel_id,
                    item_id,
                });
            }
            let is_error = from != Speaker::User
                && (text == "Limite esgotado"
                    || text.starts_with("A CLI ")
                    || text.starts_with("Não foi possível iniciar a CLI ")
                    || text.starts_with("Não foi possível executar a CLI ")
                    || text.starts_with("Modelo ")
                    || text.starts_with("A sessão da CLI ")
                    || text.starts_with("Não foi possível preparar o MCP "));
            Ok(VokiaEvent::Message {
                from,
                to,
                foreign_instance,
                is_error,
                text,
                time: time as u64,
                project_id: Some(project_id),
                channel_id,
                item_id,
                // Atividade ilegível não pode esconder a mensagem: o histórico carrega sem ela.
                activity: activity.and_then(|json| serde_json::from_str(&json).ok()),
                request_proposal: request_proposal
                    .and_then(|json| serde_json::from_str(&json).ok()),
                usage: usage.and_then(|json| serde_json::from_str(&json).ok()),
            })
        })
        .collect()
    }
}

#[cfg(test)]
#[path = "../../tests/messages_tests.rs"]
mod tests;
