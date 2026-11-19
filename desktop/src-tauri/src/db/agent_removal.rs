use rusqlite::{params, Connection, OptionalExtension};

use crate::agents::{join_items, Agent};
use crate::events::Speaker;

use super::connection::Db;
use super::error::DbError;

pub struct FeedNote {
    pub project_id: i64,
    pub text: String,
}

impl Db {
    // Tudo numa transação: recusa com posição, passa os cards ao sucessor, grava o aviso no feed e
    // só então tira o agente do registro. Se o registro falhar, a transação volta inteira.
    pub fn remove_agent<E>(
        &self,
        agent: &Agent,
        agent_name: &str,
        successor: &Agent,
        successor_name: &str,
        time: u64,
        remove_from_registry: impl FnOnce() -> Result<(), E>,
    ) -> Result<Vec<FeedNote>, E>
    where
        E: From<DbError>,
    {
        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(DbError::from)?;
        refuse_if_positioned(&tx, agent, agent_name)?;
        let notes = hand_over_cards(&tx, None, agent, agent_name, successor, successor_name, time)?;
        remove_from_registry()?;
        tx.commit().map_err(DbError::from)?;
        Ok(notes)
    }
}

// `project_id` limita a troca a um projeto; sem ele vale para todos.
pub(super) fn hand_over_cards(
    tx: &Connection,
    project_id: Option<i64>,
    agent: &Agent,
    agent_name: &str,
    successor: &Agent,
    successor_name: &str,
    time: u64,
) -> Result<Vec<FeedNote>, DbError> {
    let cards = reassign_cards(tx, project_id, agent, successor)?;
    let mut notes = Vec::with_capacity(cards.len());
    for (project_id, card_id) in cards {
        let text = format!("{successor_name} assumiu o card {card_id} de {agent_name}");
        insert_feed_note(tx, project_id, successor, &text, time)?;
        notes.push(FeedNote { project_id, text });
    }
    Ok(notes)
}

fn refuse_if_positioned(tx: &Connection, agent: &Agent, agent_name: &str) -> Result<(), DbError> {
    let mut statement = tx.prepare(
        "SELECT t.name, l.name FROM orch_placements p \
         JOIN orch_teams t ON t.id = p.team_id \
         JOIN orch_layers l ON l.id = p.layer_id \
         WHERE p.agent_id = ?1 ORDER BY t.position, l.position",
    )?;
    let positions: Vec<String> = statement
        .query_map([agent.as_str()], |row| {
            let team: String = row.get(0)?;
            let layer: String = row.get(1)?;
            Ok(format!("time {team}, camada {layer}"))
        })?
        .collect::<rusqlite::Result<_>>()?;
    if !positions.is_empty() {
        return Err(DbError::AgentHasPosition(format!(
            "{agent_name} está posicionado em {}; tire-o dessas posições antes de excluir.",
            join_items(&positions)
        )));
    }
    let led_team: Option<String> = tx
        .query_row(
            "SELECT name FROM orch_teams WHERE leader_agent = ?1 LIMIT 1",
            [agent.as_str()],
            |row| row.get(0),
        )
        .optional()?;
    if let Some(team) = led_team {
        return Err(DbError::AgentHasPosition(format!(
            "{agent_name} é líder do time {team}; troque o líder antes de excluir."
        )));
    }
    let leads_project: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM orch_settings WHERE leader_agent = ?1)",
        [agent.as_str()],
        |row| row.get(0),
    )?;
    if leads_project {
        return Err(DbError::AgentHasPosition(format!(
            "{agent_name} é o líder do projeto; troque o líder antes de excluir."
        )));
    }
    Ok(())
}

// Devolve (projeto, card) de cada card que mudou de dono, inclusive os em andamento.
fn reassign_cards(
    tx: &Connection,
    project_id: Option<i64>,
    agent: &Agent,
    successor: &Agent,
) -> Result<Vec<(i64, String)>, DbError> {
    let mut statement = tx.prepare(
        "SELECT project_id, id FROM backlog_items WHERE who = ?1 AND (?2 IS NULL OR project_id = ?2)          ORDER BY project_id, position",
    )?;
    let cards: Vec<(i64, String)> = statement
        .query_map(params![agent.as_str(), project_id], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?
        .collect::<rusqlite::Result<_>>()?;
    tx.execute(
        "UPDATE backlog_items SET who = ?3 WHERE who = ?1 AND (?2 IS NULL OR project_id = ?2)",
        params![agent.as_str(), project_id, successor.as_str()],
    )?;
    Ok(cards)
}

fn insert_feed_note(
    tx: &Connection,
    project_id: i64,
    from: &Agent,
    text: &str,
    time: u64,
) -> Result<(), DbError> {
    tx.execute(
        "INSERT INTO messages(project_id, from_agent, to_agent, text, created_at, channel_id, activity, request_proposal) \
         VALUES (?1, ?2, ?3, ?4, ?5, NULL, NULL, NULL)",
        params![
            project_id,
            from.as_str(),
            Speaker::User.as_str(),
            text,
            time as i64
        ],
    )?;
    Ok(())
}
