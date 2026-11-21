use std::collections::HashSet;

use rusqlite::{params, Row};
use serde::Serialize;

use crate::claude::TurnActivity;

use crate::agents::replica_of;

use super::connection::Db;
use super::error::DbError;
use super::orchestration::DEFAULT_LEADER_AGENT;

const STATE_RUNNING: &str = "em_andamento";
const STATE_INTERRUPTED: &str = "interrompido";
const STATE_DONE: &str = "concluido";
// Sem limite superior de janela: o último pedido do projeto não tem próximo para fechar a conta.
const OPEN_ENDED: i64 = i64::MAX;
const PROGRESS_MESSAGE_LIMIT: usize = 30;
// O pedido deixa de estar em curso quando o mesmo agente começa outro; delegações abrem pedidos de outros agentes e não cortam a janela.
const RUN_WINDOW_END: &str = "COALESCE((SELECT MIN(started_at) FROM orch_runs \
     WHERE project_id = ?1 AND agent = ?3 AND started_at > ?2), ?4)";

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InterruptedRun {
    pub id: i64,
    pub project_id: i64,
    pub agent: String,
    pub request: String,
    pub run_id: Option<i64>,
    pub item_id: Option<String>,
    pub started_at: i64,
    pub leader: bool,
}

pub struct RunHandoff {
    pub from_agent: String,
    pub to_agent: String,
    pub kind: String,
    pub item_id: Option<String>,
}

pub struct RunMessage {
    pub from_agent: String,
    pub to_agent: String,
    pub text: String,
    pub activity: Option<TurnActivity>,
}

pub struct RunProgress {
    pub handoffs: Vec<RunHandoff>,
    pub messages: Vec<RunMessage>,
}

impl RunProgress {
    pub fn is_empty(&self) -> bool {
        self.handoffs.is_empty() && self.messages.is_empty()
    }
}

fn run_select() -> String {
    format!(
        "SELECT r.id, r.project_id, r.agent, r.request, r.run_id, r.item_id, r.started_at, \
         r.agent = COALESCE(s.leader_agent, '{DEFAULT_LEADER_AGENT}') \
         FROM orch_runs r LEFT JOIN orch_settings s ON 1 = 1"
    )
}

fn run_from_row(row: &Row<'_>) -> rusqlite::Result<InterruptedRun> {
    Ok(InterruptedRun {
        id: row.get(0)?,
        project_id: row.get(1)?,
        agent: row.get(2)?,
        request: row.get(3)?,
        run_id: row.get(4)?,
        item_id: row.get(5)?,
        started_at: row.get(6)?,
        leader: row.get(7)?,
    })
}

impl Db {
    pub fn start_run(
        &self,
        project_id: i64,
        agent: &str,
        request: &str,
        run_id: Option<i64>,
        item_id: Option<&str>,
        started_at: i64,
    ) -> Result<i64, DbError> {
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO orch_runs(project_id, agent, request, run_id, item_id, state, started_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![project_id, agent, request, run_id, item_id, STATE_RUNNING, started_at],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn finish_run(&self, id: i64, ended_at: i64) -> Result<(), DbError> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE orch_runs SET state = ?2, ended_at = ?3 WHERE id = ?1",
            params![id, STATE_DONE, ended_at],
        )?;
        Ok(())
    }

    // Pausa por limite de uso: o pedido vira interrompido e a Retomada usa o caminho de sempre.
    pub fn interrupt_run(&self, id: i64) -> Result<(), DbError> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE orch_runs SET state = ?2 WHERE id = ?1 AND state = ?3",
            params![id, STATE_INTERRUPTED, STATE_RUNNING],
        )?;
        Ok(())
    }

    pub fn interrupted_runs(&self) -> Result<Vec<InterruptedRun>, DbError> {
        let conn = self.conn()?;
        let mut statement = conn.prepare(&format!(
            "{} WHERE r.state = ?1 ORDER BY r.started_at, r.id",
            run_select()
        ))?;
        let rows = statement.query_map([STATE_INTERRUPTED], run_from_row)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    // Reivindica o pedido de volta para em andamento: o UPDATE condicional barra o segundo clique e, se o app fechar de novo, a abertura o devolve aos interrompidos.
    pub fn claim_interrupted_run(&self, id: i64) -> Result<InterruptedRun, DbError> {
        let conn = self.conn()?;
        let claimed = conn.execute(
            "UPDATE orch_runs SET state = ?2 WHERE id = ?1 AND state = ?3",
            params![id, STATE_RUNNING, STATE_INTERRUPTED],
        )?;
        if claimed == 0 {
            return Err(DbError::RunNotInterrupted(id));
        }
        Ok(conn.query_row(&format!("{} WHERE r.id = ?1", run_select()), [id], run_from_row)?)
    }

    pub fn release_run(&self, id: i64) -> Result<(), DbError> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE orch_runs SET state = ?2 WHERE id = ?1 AND state = ?3",
            params![id, STATE_INTERRUPTED, STATE_RUNNING],
        )?;
        Ok(())
    }

    pub fn discard_interrupted_run(&self, id: i64) -> Result<(), DbError> {
        let conn = self.conn()?;
        let deleted = conn.execute(
            "DELETE FROM orch_runs WHERE id = ?1 AND state = ?2",
            params![id, STATE_INTERRUPTED],
        )?;
        if deleted == 0 {
            return Err(DbError::RunNotInterrupted(id));
        }
        Ok(())
    }

    // Delegações e mensagens do projeto desde o início do pedido até o mesmo agente começar outro.
    pub fn run_progress(&self, run: &InterruptedRun) -> Result<RunProgress, DbError> {
        let conn = self.conn()?;
        let mut statement = conn.prepare(&format!(
            "SELECT from_agent, to_agent, kind, item_id FROM orch_handoffs \
             WHERE project_id = ?1 AND created_at >= ?2 AND created_at < {RUN_WINDOW_END} \
             ORDER BY id"
        ))?;
        let handoffs = statement
            .query_map(
                params![run.project_id, run.started_at, run.agent, OPEN_ENDED],
                |row| {
                    Ok(RunHandoff {
                        from_agent: row.get(0)?,
                        to_agent: row.get(1)?,
                        kind: row.get(2)?,
                        item_id: row.get(3)?,
                    })
                },
            )?
            .collect::<Result<Vec<_>, _>>()?;
        let mut statement = conn.prepare(&format!(
            "SELECT from_agent, to_agent, text, activity FROM messages \
             WHERE project_id = ?1 AND created_at >= ?2 AND created_at < {RUN_WINDOW_END} \
             AND task_result IS NULL ORDER BY id"
        ))?;
        let messages = statement
            .query_map(
                params![run.project_id, run.started_at, run.agent, OPEN_ENDED],
                |row| {
                    let activity: Option<String> = row.get(3)?;
                    Ok(RunMessage {
                        from_agent: row.get(0)?,
                        to_agent: row.get(1)?,
                        text: row.get(2)?,
                        // Atividade ilegível não pode esconder a mensagem, como no histórico.
                        activity: activity.and_then(|json| serde_json::from_str(&json).ok()),
                    })
                },
            )?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(follow_chain(run, handoffs, messages))
    }
}

#[cfg(test)]
#[path = "../../tests/runs_tests.rs"]
mod tests;

fn base_of(agent: &str) -> &str {
    replica_of(agent).map_or(agent, |(base, _)| base)
}

// Só entra o que parte do agente do pedido: delegações em cadeia a partir dele e as mensagens de quem está na cadeia.
// O tempo sozinho misturaria fluxos paralelos de outros agentes no mesmo projeto.
fn follow_chain(run: &InterruptedRun, handoffs: Vec<RunHandoff>, messages: Vec<RunMessage>) -> RunProgress {
    let mut chain: HashSet<String> = HashSet::from([base_of(&run.agent).to_string()]);
    let handoffs: Vec<RunHandoff> = handoffs
        .into_iter()
        .filter(|handoff| {
            let reached = chain.contains(base_of(&handoff.from_agent));
            if reached {
                chain.insert(base_of(&handoff.to_agent).to_string());
            }
            reached
        })
        .collect();
    let mut messages: Vec<RunMessage> = messages
        .into_iter()
        .filter(|message| {
            chain.contains(base_of(&message.from_agent)) || chain.contains(base_of(&message.to_agent))
        })
        .collect();
    messages.drain(..messages.len().saturating_sub(PROGRESS_MESSAGE_LIMIT));
    RunProgress { handoffs, messages }
}
