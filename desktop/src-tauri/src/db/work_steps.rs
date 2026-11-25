use rusqlite::params;
use serde::{Deserialize, Serialize};

use super::connection::Db;
use super::error::DbError;

const WORK_STEPS_OPEN: &str = "[work steps]";
const WORK_STEPS_CLOSE: &str = "[end work steps]";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnWorkStepsRecord {
    pub id: i64,
    pub project_id: i64,
    pub agent: String,
    pub session_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub card_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    pub block: String,
    pub step_count: usize,
    pub omitted_count: usize,
    pub created_at: i64,
}

#[derive(Clone, Debug)]
pub struct NewTurnWorkSteps<'a> {
    pub project_id: i64,
    pub agent: &'a str,
    pub session_key: &'a str,
    pub card_id: Option<&'a str>,
    pub provider: Option<&'a str>,
    pub block: &'a str,
    pub step_count: usize,
    pub omitted_count: usize,
    pub created_at: i64,
}

// Chave vinculada ao projeto, agente e card no banco do Vokia — nunca só ao ID nativo da CLI,
// evitando colisão entre provedores que usam formatos parecidos.
pub fn work_steps_session_key(project_id: i64, agent: &str, card_id: Option<&str>) -> String {
    match card_id.map(str::trim).filter(|id| !id.is_empty()) {
        Some(card) => format!("vokia:{project_id}:{agent}:card:{card}"),
        None => format!("vokia:{project_id}:{agent}"),
    }
}

impl Db {
    pub fn insert_turn_work_steps(&self, record: &NewTurnWorkSteps<'_>) -> Result<i64, DbError> {
        let conn = self.conn()?;
        let inserted = conn.execute(
            "INSERT INTO turn_work_steps \
                (project_id, agent, session_key, card_id, provider, block, step_count, omitted_count, created_at) \
             SELECT id, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9 FROM projects WHERE id = ?1",
            params![
                record.project_id,
                record.agent,
                record.session_key,
                record.card_id,
                record.provider,
                record.block,
                record.step_count as i64,
                record.omitted_count as i64,
                record.created_at,
            ],
        )?;
        if inserted == 0 {
            return Err(DbError::ProjectNotFound(record.project_id));
        }
        Ok(conn.last_insert_rowid())
    }

    // Devolve do mais recente para o mais antigo para suportar paginação (offset/limit) no history_read.
    #[allow(dead_code)] // pronto para integração sob demanda em history_read (096)
    pub fn list_turn_work_steps(
        &self,
        project_id: i64,
        agent: &str,
        session_key: &str,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<TurnWorkStepsRecord>, DbError> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let conn = self.conn()?;
        let mut statement = conn.prepare(
            "SELECT id, project_id, agent, session_key, card_id, provider, block, step_count, omitted_count, created_at \
             FROM turn_work_steps \
             WHERE project_id = ?1 AND agent = ?2 AND session_key = ?3 \
             ORDER BY created_at DESC, id DESC \
             LIMIT ?4 OFFSET ?5",
        )?;
        let rows = statement.query_map(
            params![project_id, agent, session_key, limit as i64, offset as i64],
            decode_row,
        )?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(DbError::from)
    }

    // Leitura tolerante a corrupção/ausência: pula linhas inválidas e nunca derruba o handoff nem a conversa.
    // Retorna os registros válidos em ordem cronológica (mais antigo -> mais recente).
    pub fn load_recent_work_steps_tolerant(
        &self,
        project_id: i64,
        agent: &str,
        session_key: &str,
        limit: usize,
    ) -> Vec<TurnWorkStepsRecord> {
        if limit == 0 {
            return Vec::new();
        }
        let Ok(conn) = self.conn() else {
            return Vec::new();
        };
        let Ok(mut statement) = conn.prepare(
            "SELECT id, project_id, agent, session_key, card_id, provider, block, step_count, omitted_count, created_at \
             FROM turn_work_steps \
             WHERE project_id = ?1 AND agent = ?2 AND session_key = ?3 \
             ORDER BY created_at DESC, id DESC \
             LIMIT ?4",
        ) else {
            return Vec::new();
        };
        let Ok(rows) = statement.query_map(
            params![project_id, agent, session_key, limit as i64],
            decode_row,
        ) else {
            return Vec::new();
        };
        let mut records = Vec::new();
        for row in rows {
            match row {
                Ok(record) if is_valid_work_steps_block(&record.block) => {
                    records.push(record);
                }
                Ok(_) | Err(_) => {
                    // Linha corrompida ou malformada é descartada com segurança.
                }
            }
        }
        records.reverse();
        records
    }
}

pub fn load_recent_work_steps_safe(
    db: Option<&Db>,
    project_id: i64,
    agent: &str,
    session_key: &str,
    limit: usize,
) -> Vec<TurnWorkStepsRecord> {
    db.map(|db| db.load_recent_work_steps_tolerant(project_id, agent, session_key, limit))
        .unwrap_or_default()
}

fn decode_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TurnWorkStepsRecord> {
    let step_count: i64 = row.get(7)?;
    let omitted_count: i64 = row.get(8)?;
    Ok(TurnWorkStepsRecord {
        id: row.get(0)?,
        project_id: row.get(1)?,
        agent: row.get(2)?,
        session_key: row.get(3)?,
        card_id: row.get(4)?,
        provider: row.get(5)?,
        block: row.get(6)?,
        step_count: usize::try_from(step_count).unwrap_or(0),
        omitted_count: usize::try_from(omitted_count).unwrap_or(0),
        created_at: row.get(9)?,
    })
}

fn is_valid_work_steps_block(block: &str) -> bool {
    let trimmed = block.trim();
    !trimmed.is_empty() && trimmed.contains(WORK_STEPS_OPEN) && trimmed.contains(WORK_STEPS_CLOSE)
}
