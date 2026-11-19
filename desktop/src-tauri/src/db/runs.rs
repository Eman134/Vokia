use rusqlite::{params, Row};
use serde::Serialize;

use super::connection::Db;
use super::error::DbError;
use super::orchestration::DEFAULT_LEADER_AGENT;

const STATE_RUNNING: &str = "em_andamento";
const STATE_INTERRUPTED: &str = "interrompido";
const STATE_RESUMED: &str = "retomado";
const STATE_DONE: &str = "concluido";
// Sem limite superior de janela: o último pedido do projeto não tem próximo para fechar a conta.
const OPEN_ENDED: i64 = i64::MAX;

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

    // Reivindica o pedido antes de reenviá-lo: o UPDATE condicional barra o segundo clique.
    pub fn claim_interrupted_run(&self, id: i64) -> Result<InterruptedRun, DbError> {
        let conn = self.conn()?;
        let claimed = conn.execute(
            "UPDATE orch_runs SET state = ?2 WHERE id = ?1 AND state = ?3",
            params![id, STATE_RESUMED, STATE_INTERRUPTED],
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
            params![id, STATE_INTERRUPTED, STATE_RESUMED],
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

    // Delegações do projeto entre o início do pedido e o próximo pedido que começou depois dele.
    pub fn run_handoffs(&self, run: &InterruptedRun) -> Result<Vec<RunHandoff>, DbError> {
        let conn = self.conn()?;
        let mut statement = conn.prepare(
            "SELECT from_agent, to_agent, kind, item_id FROM orch_handoffs \
             WHERE project_id = ?1 AND created_at >= ?2 \
             AND created_at < COALESCE((SELECT MIN(started_at) FROM orch_runs \
                 WHERE project_id = ?1 AND started_at > ?2), ?3) \
             ORDER BY id",
        )?;
        let rows = statement.query_map(
            params![run.project_id, run.started_at, OPEN_ENDED],
            |row| {
                Ok(RunHandoff {
                    from_agent: row.get(0)?,
                    to_agent: row.get(1)?,
                    kind: row.get(2)?,
                    item_id: row.get(3)?,
                })
            },
        )?;
        Ok(rows.collect::<Result<_, _>>()?)
    }
}
