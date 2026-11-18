use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use super::connection::Db;
use super::error::DbError;

// Nó virtual "Você": só recebe pela regra escalar e nunca vira camada.
const VOCE_NODE: &str = "voce";
const INSTRUCTIONS_MAX_CHARS: usize = 4000;
const MOVEMENTS_MIN_LIMIT: i64 = 1;

const DEFAULT_LEADER_AGENT: &str = "ember";
const DEFAULT_ENFORCEMENT: &str = "off";
const DEFAULT_STEP_MODE: &str = "lotes";
const DEFAULT_HUMAN_GATE: &str = "lote";
const DEFAULT_WORKTREE_POLICY: &str = "paralelo";
const DEFAULT_MAX_PARALLEL: i64 = 4;
const DEFAULT_PAIR_TURN_LIMIT: i64 = 8;
const DEFAULT_TASK_TURN_LIMIT: i64 = 60;

const ENFORCEMENT_VALUES: &[&str] = &["off", "on"];
const STEP_MODE_VALUES: &[&str] = &["lotes", "livre"];
const HUMAN_GATE_VALUES: &[&str] = &["lote", "fim", "nunca"];
const WORKTREE_POLICY_VALUES: &[&str] = &["sempre", "paralelo", "nunca"];
const LAYER_KIND_VALUES: &[&str] = &["planejar", "implementar", "revisar", "pesquisar", "livre"];
pub(crate) const REQUEST_KIND_VALUES: &[&str] =
    &["planejar", "implementar", "testar", "revisar", "pesquisar"];
pub(crate) const WHEN_EVENT_VALUES: &[&str] = &[
    "plano_aprovado",
    "tarefa_concluida",
    "revisao_reprovou",
    "testes_falharam",
    "revisao_aprovou",
    "pedido_de_ajuda",
];
const ACTION_DELEGATE: &str = "delegar";
const ACTION_RETURN: &str = "devolver";
const ACTION_ESCALATE: &str = "escalar";
const ACTION_VALUES: &[&str] = &[ACTION_DELEGATE, ACTION_RETURN, ACTION_ESCALATE];
const EXEC_MODE_VALUES: &[&str] = &["sequencia", "paralelo"];

struct Bounds {
    field: &'static str,
    min: i64,
    max: i64,
}

const MAX_PARALLEL: Bounds = Bounds {
    field: "max_parallel",
    min: 1,
    max: 8,
};
const PAIR_TURN_LIMIT: Bounds = Bounds {
    field: "pair_turn_limit",
    min: 1,
    max: 50,
};
const TASK_TURN_LIMIT: Bounds = Bounds {
    field: "task_turn_limit",
    min: 5,
    max: 200,
};
const RETURN_LIMIT: Bounds = Bounds {
    field: "return_limit",
    min: 1,
    max: 5,
};
const MAX_INSTANCES: Bounds = Bounds {
    field: "max_instances",
    min: 1,
    max: 4,
};
const RETURN_COUNT: Bounds = Bounds {
    field: "return_count",
    min: 0,
    max: i64::MAX,
};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrchestrationSettings {
    pub enforcement: String,
    pub leader_agent: String,
    pub step_mode: String,
    pub human_gate: String,
    pub max_parallel: i64,
    pub pair_turn_limit: i64,
    pub task_turn_limit: i64,
    pub worktree_policy: String,
}

impl Default for OrchestrationSettings {
    fn default() -> Self {
        Self {
            enforcement: DEFAULT_ENFORCEMENT.to_string(),
            leader_agent: DEFAULT_LEADER_AGENT.to_string(),
            step_mode: DEFAULT_STEP_MODE.to_string(),
            human_gate: DEFAULT_HUMAN_GATE.to_string(),
            max_parallel: DEFAULT_MAX_PARALLEL,
            pair_turn_limit: DEFAULT_PAIR_TURN_LIMIT,
            task_turn_limit: DEFAULT_TASK_TURN_LIMIT,
            worktree_policy: DEFAULT_WORKTREE_POLICY.to_string(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Layer {
    pub id: String,
    pub name: String,
    pub description: String,
    pub position: i64,
    pub kind: String,
    pub instructions: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Team {
    pub id: String,
    pub name: String,
    pub position: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Placement {
    pub agent_id: String,
    pub team_id: String,
    pub layer_id: String,
    pub instructions: String,
    pub max_instances: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    pub id: i64,
    pub from_layer: String,
    pub to_layer: String,
    pub when_event: String,
    pub action: String,
    pub kind: String,
    pub exec_mode: String,
    pub return_limit: i64,
    pub position: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Handoff {
    pub id: i64,
    pub run_id: Option<i64>,
    pub from_agent: String,
    pub to_agent: String,
    pub from_layer: Option<String>,
    pub to_layer: Option<String>,
    pub action: String,
    pub kind: String,
    pub item_id: Option<String>,
    pub return_count: i64,
    pub message_id: Option<i64>,
    pub created_at: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrchestrationSnapshot {
    pub settings: OrchestrationSettings,
    pub layers: Vec<Layer>,
    pub teams: Vec<Team>,
    pub placements: Vec<Placement>,
    pub rules: Vec<Rule>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayerDraft {
    pub id: Option<String>,
    pub name: String,
    pub description: String,
    pub kind: String,
    pub instructions: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamDraft {
    pub id: Option<String>,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlacementDraft {
    pub agent_id: String,
    pub team_id: String,
    pub layer_id: String,
    pub instructions: String,
    pub max_instances: i64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleDraft {
    pub id: Option<i64>,
    pub from_layer: String,
    pub to_layer: String,
    pub when_event: String,
    pub action: String,
    pub kind: String,
    pub exec_mode: String,
    pub return_limit: i64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HandoffDraft {
    pub run_id: Option<i64>,
    pub from_agent: String,
    pub to_agent: String,
    pub from_layer: Option<String>,
    pub to_layer: Option<String>,
    pub action: String,
    pub kind: String,
    pub item_id: Option<String>,
    pub return_count: i64,
    pub message_id: Option<i64>,
}

#[derive(Clone, Debug)]
pub struct OrchestrationSeed {
    pub settings: OrchestrationSettings,
    pub layers: Vec<LayerDraft>,
    pub team: TeamDraft,
    pub placements: Vec<PlacementDraft>,
}

const SETTINGS_COLUMNS: &str = "enforcement, leader_agent, step_mode, human_gate, max_parallel, \
    pair_turn_limit, task_turn_limit, worktree_policy";
const LAYER_COLUMNS: &str = "id, name, description, position, kind, instructions";
const TEAM_COLUMNS: &str = "id, name, position";
const PLACEMENT_COLUMNS: &str = "agent_id, team_id, layer_id, instructions, max_instances";
const RULE_COLUMNS: &str =
    "id, from_layer, to_layer, when_event, action, kind, exec_mode, return_limit, position";
const HANDOFF_COLUMNS: &str = "id, run_id, from_agent, to_agent, from_layer, to_layer, action, \
    kind, item_id, return_count, message_id, created_at";

fn settings_from_row(row: &Row<'_>) -> rusqlite::Result<OrchestrationSettings> {
    Ok(OrchestrationSettings {
        enforcement: row.get(0)?,
        leader_agent: row.get(1)?,
        step_mode: row.get(2)?,
        human_gate: row.get(3)?,
        max_parallel: row.get(4)?,
        pair_turn_limit: row.get(5)?,
        task_turn_limit: row.get(6)?,
        worktree_policy: row.get(7)?,
    })
}

fn layer_from_row(row: &Row<'_>) -> rusqlite::Result<Layer> {
    Ok(Layer {
        id: row.get(0)?,
        name: row.get(1)?,
        description: row.get(2)?,
        position: row.get(3)?,
        kind: row.get(4)?,
        instructions: row.get(5)?,
    })
}

fn team_from_row(row: &Row<'_>) -> rusqlite::Result<Team> {
    Ok(Team {
        id: row.get(0)?,
        name: row.get(1)?,
        position: row.get(2)?,
    })
}

fn placement_from_row(row: &Row<'_>) -> rusqlite::Result<Placement> {
    Ok(Placement {
        agent_id: row.get(0)?,
        team_id: row.get(1)?,
        layer_id: row.get(2)?,
        instructions: row.get(3)?,
        max_instances: row.get(4)?,
    })
}

fn rule_from_row(row: &Row<'_>) -> rusqlite::Result<Rule> {
    Ok(Rule {
        id: row.get(0)?,
        from_layer: row.get(1)?,
        to_layer: row.get(2)?,
        when_event: row.get(3)?,
        action: row.get(4)?,
        kind: row.get(5)?,
        exec_mode: row.get(6)?,
        return_limit: row.get(7)?,
        position: row.get(8)?,
    })
}

fn handoff_from_row(row: &Row<'_>) -> rusqlite::Result<Handoff> {
    Ok(Handoff {
        id: row.get(0)?,
        run_id: row.get(1)?,
        from_agent: row.get(2)?,
        to_agent: row.get(3)?,
        from_layer: row.get(4)?,
        to_layer: row.get(5)?,
        action: row.get(6)?,
        kind: row.get(7)?,
        item_id: row.get(8)?,
        return_count: row.get(9)?,
        message_id: row.get(10)?,
        created_at: row.get(11)?,
    })
}

fn check_one_of(field: &'static str, value: &str, allowed: &[&str]) -> Result<(), DbError> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(DbError::InvalidOrchestrationField {
            field,
            value: value.to_string(),
        })
    }
}

fn check_bounds(bounds: &Bounds, value: i64) -> Result<(), DbError> {
    if (bounds.min..=bounds.max).contains(&value) {
        Ok(())
    } else {
        Err(DbError::OrchestrationOutOfRange {
            field: bounds.field,
            min: bounds.min,
            max: bounds.max,
        })
    }
}

fn check_instructions(text: &str) -> Result<(), DbError> {
    if text.chars().count() > INSTRUCTIONS_MAX_CHARS {
        Err(DbError::OrchestrationInstructionsTooLong {
            max: INSTRUCTIONS_MAX_CHARS,
        })
    } else {
        Ok(())
    }
}

fn check_name(name: &str) -> Result<(), DbError> {
    if name.trim().is_empty() {
        Err(DbError::OrchestrationNameEmpty)
    } else {
        Ok(())
    }
}

// Mesma normalização de nome do elenco de agentes (acentos e caixa), separada por hífen.
fn slug_of(name: &str) -> Result<String, DbError> {
    let slug = crate::agents::name_key(name)
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if slug.is_empty() {
        Err(DbError::OrchestrationNameEmpty)
    } else {
        Ok(slug)
    }
}

fn row_exists(
    conn: &Connection,
    sql: &str,
    params: impl rusqlite::Params,
) -> Result<bool, DbError> {
    Ok(conn.query_row(&format!("SELECT EXISTS({sql})"), params, |row| row.get(0))?)
}

fn ensure_project(conn: &Connection, project_id: i64) -> Result<(), DbError> {
    if row_exists(
        conn,
        "SELECT 1 FROM projects WHERE id = ?1",
        params![project_id],
    )? {
        Ok(())
    } else {
        Err(DbError::ProjectNotFound(project_id))
    }
}

fn layer_exists(conn: &Connection, project_id: i64, id: &str) -> Result<bool, DbError> {
    row_exists(
        conn,
        "SELECT 1 FROM orch_layers WHERE project_id = ?1 AND id = ?2",
        params![project_id, id],
    )
}

fn team_exists(conn: &Connection, project_id: i64, id: &str) -> Result<bool, DbError> {
    row_exists(
        conn,
        "SELECT 1 FROM orch_teams WHERE project_id = ?1 AND id = ?2",
        params![project_id, id],
    )
}

fn fetch_settings(
    conn: &Connection,
    project_id: i64,
) -> Result<Option<OrchestrationSettings>, DbError> {
    Ok(conn
        .query_row(
            &format!("SELECT {SETTINGS_COLUMNS} FROM orch_settings WHERE project_id = ?1"),
            params![project_id],
            settings_from_row,
        )
        .optional()?)
}

fn fetch_layer(conn: &Connection, project_id: i64, id: &str) -> Result<Layer, DbError> {
    conn.query_row(
        &format!("SELECT {LAYER_COLUMNS} FROM orch_layers WHERE project_id = ?1 AND id = ?2"),
        params![project_id, id],
        layer_from_row,
    )
    .optional()?
    .ok_or_else(|| DbError::LayerNotFound(id.to_string()))
}

fn fetch_team(conn: &Connection, project_id: i64, id: &str) -> Result<Team, DbError> {
    conn.query_row(
        &format!("SELECT {TEAM_COLUMNS} FROM orch_teams WHERE project_id = ?1 AND id = ?2"),
        params![project_id, id],
        team_from_row,
    )
    .optional()?
    .ok_or_else(|| DbError::TeamNotFound(id.to_string()))
}

fn fetch_placement(
    conn: &Connection,
    project_id: i64,
    agent_id: &str,
) -> Result<Placement, DbError> {
    Ok(conn.query_row(
        &format!(
            "SELECT {PLACEMENT_COLUMNS} FROM orch_placements WHERE project_id = ?1 AND agent_id = ?2"
        ),
        params![project_id, agent_id],
        placement_from_row,
    )?)
}

fn fetch_rule(conn: &Connection, project_id: i64, id: i64) -> Result<Rule, DbError> {
    conn.query_row(
        &format!("SELECT {RULE_COLUMNS} FROM orch_rules WHERE project_id = ?1 AND id = ?2"),
        params![project_id, id],
        rule_from_row,
    )
    .optional()?
    .ok_or(DbError::RuleNotFound(id))
}

fn list_layers(conn: &Connection, project_id: i64) -> Result<Vec<Layer>, DbError> {
    let mut statement = conn.prepare(&format!(
        "SELECT {LAYER_COLUMNS} FROM orch_layers WHERE project_id = ?1 ORDER BY position, id"
    ))?;
    let rows = statement.query_map(params![project_id], layer_from_row)?;
    Ok(rows.collect::<Result<_, _>>()?)
}

fn list_teams(conn: &Connection, project_id: i64) -> Result<Vec<Team>, DbError> {
    let mut statement = conn.prepare(&format!(
        "SELECT {TEAM_COLUMNS} FROM orch_teams WHERE project_id = ?1 ORDER BY position, id"
    ))?;
    let rows = statement.query_map(params![project_id], team_from_row)?;
    Ok(rows.collect::<Result<_, _>>()?)
}

fn list_placements(conn: &Connection, project_id: i64) -> Result<Vec<Placement>, DbError> {
    let mut statement = conn.prepare(&format!(
        "SELECT {PLACEMENT_COLUMNS} FROM orch_placements WHERE project_id = ?1 ORDER BY agent_id"
    ))?;
    let rows = statement.query_map(params![project_id], placement_from_row)?;
    Ok(rows.collect::<Result<_, _>>()?)
}

fn list_rules(conn: &Connection, project_id: i64) -> Result<Vec<Rule>, DbError> {
    let mut statement = conn.prepare(&format!(
        "SELECT {RULE_COLUMNS} FROM orch_rules WHERE project_id = ?1 ORDER BY position, id"
    ))?;
    let rows = statement.query_map(params![project_id], rule_from_row)?;
    Ok(rows.collect::<Result<_, _>>()?)
}

fn layer_ids(conn: &Connection, project_id: i64) -> Result<Vec<String>, DbError> {
    let mut statement = conn.prepare("SELECT id FROM orch_layers WHERE project_id = ?1")?;
    let rows = statement.query_map(params![project_id], |row| row.get(0))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

// Devolver só vale para camada anterior; reordenar pode quebrar isso, então é checado de novo.
fn ensure_return_rules_backward(conn: &Connection, project_id: i64) -> Result<(), DbError> {
    let violated = row_exists(
        conn,
        "SELECT 1 FROM orch_rules r \
         JOIN orch_layers f ON f.project_id = r.project_id AND f.id = r.from_layer \
         JOIN orch_layers t ON t.project_id = r.project_id AND t.id = r.to_layer \
         WHERE r.project_id = ?1 AND r.action = ?2 AND t.position >= f.position",
        params![project_id, ACTION_RETURN],
    )?;
    if violated {
        Err(DbError::RuleReturnNotBackward)
    } else {
        Ok(())
    }
}

fn upsert_settings(
    conn: &Connection,
    project_id: i64,
    settings: &OrchestrationSettings,
) -> Result<(), DbError> {
    conn.execute(
        "INSERT INTO orch_settings (project_id, enforcement, leader_agent, step_mode, human_gate, \
            max_parallel, pair_turn_limit, task_turn_limit, worktree_policy) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9) \
         ON CONFLICT (project_id) DO UPDATE SET \
            enforcement = excluded.enforcement, leader_agent = excluded.leader_agent, \
            step_mode = excluded.step_mode, human_gate = excluded.human_gate, \
            max_parallel = excluded.max_parallel, pair_turn_limit = excluded.pair_turn_limit, \
            task_turn_limit = excluded.task_turn_limit, worktree_policy = excluded.worktree_policy",
        params![
            project_id,
            settings.enforcement,
            settings.leader_agent,
            settings.step_mode,
            settings.human_gate,
            settings.max_parallel,
            settings.pair_turn_limit,
            settings.task_turn_limit,
            settings.worktree_policy,
        ],
    )?;
    Ok(())
}

fn insert_layer(conn: &Connection, project_id: i64, draft: &LayerDraft) -> Result<String, DbError> {
    let id = slug_of(&draft.name)?;
    if id == VOCE_NODE {
        return Err(DbError::ReservedLayerName(draft.name.clone()));
    }
    if layer_exists(conn, project_id, &id)? {
        return Err(DbError::LayerNameTaken(id));
    }
    conn.execute(
        "INSERT INTO orch_layers (project_id, id, name, description, position, kind, instructions) \
         VALUES (?1, ?2, ?3, ?4, \
            (SELECT COALESCE(MAX(position) + 1, 0) FROM orch_layers WHERE project_id = ?1), \
            ?5, ?6)",
        params![
            project_id,
            id,
            draft.name,
            draft.description,
            draft.kind,
            draft.instructions
        ],
    )?;
    Ok(id)
}

fn insert_team(conn: &Connection, project_id: i64, draft: &TeamDraft) -> Result<String, DbError> {
    let id = slug_of(&draft.name)?;
    if team_exists(conn, project_id, &id)? {
        return Err(DbError::TeamNameTaken(id));
    }
    conn.execute(
        "INSERT INTO orch_teams (project_id, id, name, position) \
         VALUES (?1, ?2, ?3, \
            (SELECT COALESCE(MAX(position) + 1, 0) FROM orch_teams WHERE project_id = ?1))",
        params![project_id, id, draft.name],
    )?;
    Ok(id)
}

fn upsert_placement(
    conn: &Connection,
    project_id: i64,
    draft: &PlacementDraft,
) -> Result<(), DbError> {
    if !team_exists(conn, project_id, &draft.team_id)? {
        return Err(DbError::TeamNotFound(draft.team_id.clone()));
    }
    if !layer_exists(conn, project_id, &draft.layer_id)? {
        return Err(DbError::LayerNotFound(draft.layer_id.clone()));
    }
    conn.execute(
        "INSERT INTO orch_placements (project_id, agent_id, team_id, layer_id, instructions, max_instances) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6) \
         ON CONFLICT (project_id, agent_id) DO UPDATE SET \
            team_id = excluded.team_id, layer_id = excluded.layer_id, \
            instructions = excluded.instructions, max_instances = excluded.max_instances",
        params![
            project_id,
            draft.agent_id,
            draft.team_id,
            draft.layer_id,
            draft.instructions,
            draft.max_instances
        ],
    )?;
    Ok(())
}

impl Db {
    pub fn orchestration(&self, project_id: i64) -> Result<OrchestrationSnapshot, DbError> {
        let conn = self.conn()?;
        ensure_project(&conn, project_id)?;
        Ok(OrchestrationSnapshot {
            settings: fetch_settings(&conn, project_id)?.unwrap_or_default(),
            layers: list_layers(&conn, project_id)?,
            teams: list_teams(&conn, project_id)?,
            placements: list_placements(&conn, project_id)?,
            rules: list_rules(&conn, project_id)?,
        })
    }

    pub fn save_settings(
        &self,
        project_id: i64,
        settings: &OrchestrationSettings,
    ) -> Result<(), DbError> {
        check_one_of("enforcement", &settings.enforcement, ENFORCEMENT_VALUES)?;
        check_one_of("step_mode", &settings.step_mode, STEP_MODE_VALUES)?;
        check_one_of("human_gate", &settings.human_gate, HUMAN_GATE_VALUES)?;
        check_one_of(
            "worktree_policy",
            &settings.worktree_policy,
            WORKTREE_POLICY_VALUES,
        )?;
        check_bounds(&MAX_PARALLEL, settings.max_parallel)?;
        check_bounds(&PAIR_TURN_LIMIT, settings.pair_turn_limit)?;
        check_bounds(&TASK_TURN_LIMIT, settings.task_turn_limit)?;
        let conn = self.conn()?;
        ensure_project(&conn, project_id)?;
        upsert_settings(&conn, project_id, settings)
    }

    pub fn save_layer(&self, project_id: i64, draft: &LayerDraft) -> Result<Layer, DbError> {
        check_name(&draft.name)?;
        check_one_of("kind", &draft.kind, LAYER_KIND_VALUES)?;
        check_instructions(&draft.instructions)?;
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        ensure_project(&tx, project_id)?;
        let id = match &draft.id {
            Some(id) => {
                let changed = tx.execute(
                    "UPDATE orch_layers SET name = ?3, description = ?4, kind = ?5, instructions = ?6 \
                     WHERE project_id = ?1 AND id = ?2",
                    params![
                        project_id,
                        id,
                        draft.name,
                        draft.description,
                        draft.kind,
                        draft.instructions
                    ],
                )?;
                if changed == 0 {
                    return Err(DbError::LayerNotFound(id.clone()));
                }
                id.clone()
            }
            None => insert_layer(&tx, project_id, draft)?,
        };
        let layer = fetch_layer(&tx, project_id, &id)?;
        tx.commit()?;
        Ok(layer)
    }

    pub fn delete_layer(&self, project_id: i64, id: &str) -> Result<(), DbError> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        ensure_project(&tx, project_id)?;
        if !layer_exists(&tx, project_id, id)? {
            return Err(DbError::LayerNotFound(id.to_string()));
        }
        if row_exists(
            &tx,
            "SELECT 1 FROM orch_placements WHERE project_id = ?1 AND layer_id = ?2",
            params![project_id, id],
        )? {
            return Err(DbError::LayerHasAgents(id.to_string()));
        }
        tx.execute(
            "DELETE FROM orch_rules WHERE project_id = ?1 AND (from_layer = ?2 OR to_layer = ?2)",
            params![project_id, id],
        )?;
        tx.execute(
            "DELETE FROM orch_layers WHERE project_id = ?1 AND id = ?2",
            params![project_id, id],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn reorder_layers(&self, project_id: i64, ids: &[String]) -> Result<(), DbError> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        ensure_project(&tx, project_id)?;
        let mut current = layer_ids(&tx, project_id)?;
        let mut requested = ids.to_vec();
        current.sort();
        requested.sort();
        if current != requested {
            return Err(DbError::LayerOrderInvalid);
        }
        for (position, id) in ids.iter().enumerate() {
            tx.execute(
                "UPDATE orch_layers SET position = ?3 WHERE project_id = ?1 AND id = ?2",
                params![project_id, id, position as i64],
            )?;
        }
        ensure_return_rules_backward(&tx, project_id)?;
        tx.commit()?;
        Ok(())
    }

    pub fn save_team(&self, project_id: i64, draft: &TeamDraft) -> Result<Team, DbError> {
        check_name(&draft.name)?;
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        ensure_project(&tx, project_id)?;
        let id = match &draft.id {
            Some(id) => {
                let changed = tx.execute(
                    "UPDATE orch_teams SET name = ?3 WHERE project_id = ?1 AND id = ?2",
                    params![project_id, id, draft.name],
                )?;
                if changed == 0 {
                    return Err(DbError::TeamNotFound(id.clone()));
                }
                id.clone()
            }
            None => insert_team(&tx, project_id, draft)?,
        };
        let team = fetch_team(&tx, project_id, &id)?;
        tx.commit()?;
        Ok(team)
    }

    pub fn delete_team(&self, project_id: i64, id: &str) -> Result<(), DbError> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        ensure_project(&tx, project_id)?;
        if !team_exists(&tx, project_id, id)? {
            return Err(DbError::TeamNotFound(id.to_string()));
        }
        if row_exists(
            &tx,
            "SELECT 1 FROM orch_placements WHERE project_id = ?1 AND team_id = ?2",
            params![project_id, id],
        )? {
            return Err(DbError::TeamHasAgents(id.to_string()));
        }
        tx.execute(
            "DELETE FROM orch_teams WHERE project_id = ?1 AND id = ?2",
            params![project_id, id],
        )?;
        tx.commit()?;
        Ok(())
    }

    // Uma posição por agente: posicionar de novo move o agente em vez de criar outra.
    pub fn place_agent(
        &self,
        project_id: i64,
        draft: &PlacementDraft,
    ) -> Result<Placement, DbError> {
        check_bounds(&MAX_INSTANCES, draft.max_instances)?;
        check_instructions(&draft.instructions)?;
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        ensure_project(&tx, project_id)?;
        upsert_placement(&tx, project_id, draft)?;
        let placement = fetch_placement(&tx, project_id, &draft.agent_id)?;
        tx.commit()?;
        Ok(placement)
    }

    pub fn save_rule(&self, project_id: i64, draft: &RuleDraft) -> Result<Rule, DbError> {
        check_one_of("when_event", &draft.when_event, WHEN_EVENT_VALUES)?;
        check_one_of("action", &draft.action, ACTION_VALUES)?;
        check_one_of("kind", &draft.kind, REQUEST_KIND_VALUES)?;
        check_one_of("exec_mode", &draft.exec_mode, EXEC_MODE_VALUES)?;
        check_bounds(&RETURN_LIMIT, draft.return_limit)?;
        if draft.from_layer == draft.to_layer {
            return Err(DbError::RuleSelfLoop);
        }
        let targets_voce = draft.to_layer == VOCE_NODE;
        let escalates = draft.action == ACTION_ESCALATE;
        if escalates && !targets_voce {
            return Err(DbError::RuleEscalateTarget);
        }
        if targets_voce && !escalates {
            return Err(DbError::RuleVoceNeedsEscalate);
        }
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        ensure_project(&tx, project_id)?;
        if !layer_exists(&tx, project_id, &draft.from_layer)? {
            return Err(DbError::LayerNotFound(draft.from_layer.clone()));
        }
        if !targets_voce && !layer_exists(&tx, project_id, &draft.to_layer)? {
            return Err(DbError::LayerNotFound(draft.to_layer.clone()));
        }
        if draft.action == ACTION_RETURN
            && !row_exists(
                &tx,
                "SELECT 1 FROM orch_layers t JOIN orch_layers f ON f.project_id = t.project_id \
                 WHERE t.project_id = ?1 AND t.id = ?2 AND f.id = ?3 AND t.position < f.position",
                params![project_id, draft.to_layer, draft.from_layer],
            )?
        {
            return Err(DbError::RuleReturnNotBackward);
        }
        if row_exists(
            &tx,
            "SELECT 1 FROM orch_rules WHERE project_id = ?1 AND from_layer = ?2 AND to_layer = ?3 \
             AND id IS NOT ?4",
            params![project_id, draft.from_layer, draft.to_layer, draft.id],
        )? {
            return Err(DbError::RuleDuplicated {
                from: draft.from_layer.clone(),
                to: draft.to_layer.clone(),
            });
        }
        let id = match draft.id {
            Some(id) => {
                let changed = tx.execute(
                    "UPDATE orch_rules SET from_layer = ?3, to_layer = ?4, when_event = ?5, action = ?6, \
                        kind = ?7, exec_mode = ?8, return_limit = ?9 \
                     WHERE project_id = ?1 AND id = ?2",
                    params![
                        project_id,
                        id,
                        draft.from_layer,
                        draft.to_layer,
                        draft.when_event,
                        draft.action,
                        draft.kind,
                        draft.exec_mode,
                        draft.return_limit
                    ],
                )?;
                if changed == 0 {
                    return Err(DbError::RuleNotFound(id));
                }
                id
            }
            None => {
                tx.execute(
                    "INSERT INTO orch_rules (project_id, from_layer, to_layer, when_event, action, kind, \
                        exec_mode, return_limit, position) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, \
                        (SELECT COALESCE(MAX(position) + 1, 0) FROM orch_rules WHERE project_id = ?1))",
                    params![
                        project_id,
                        draft.from_layer,
                        draft.to_layer,
                        draft.when_event,
                        draft.action,
                        draft.kind,
                        draft.exec_mode,
                        draft.return_limit
                    ],
                )?;
                tx.last_insert_rowid()
            }
        };
        let rule = fetch_rule(&tx, project_id, id)?;
        tx.commit()?;
        Ok(rule)
    }

    pub fn delete_rule(&self, project_id: i64, id: i64) -> Result<(), DbError> {
        let conn = self.conn()?;
        ensure_project(&conn, project_id)?;
        let deleted = conn.execute(
            "DELETE FROM orch_rules WHERE project_id = ?1 AND id = ?2",
            params![project_id, id],
        )?;
        if deleted == 0 {
            return Err(DbError::RuleNotFound(id));
        }
        Ok(())
    }

    pub fn record_handoff(&self, project_id: i64, draft: &HandoffDraft) -> Result<(), DbError> {
        check_one_of("action", &draft.action, ACTION_VALUES)?;
        check_one_of("kind", &draft.kind, REQUEST_KIND_VALUES)?;
        check_bounds(&RETURN_COUNT, draft.return_count)?;
        let conn = self.conn()?;
        ensure_project(&conn, project_id)?;
        conn.execute(
            "INSERT INTO orch_handoffs (project_id, run_id, from_agent, to_agent, from_layer, to_layer, \
                action, kind, item_id, return_count, message_id) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                project_id,
                draft.run_id,
                draft.from_agent,
                draft.to_agent,
                draft.from_layer,
                draft.to_layer,
                draft.action,
                draft.kind,
                draft.item_id,
                draft.return_count,
                draft.message_id
            ],
        )?;
        Ok(())
    }

    // Checagem da semente e inserções na mesma transação, com o lock da conexão do início ao fim.
    pub fn seed_orchestration(
        &self,
        project_id: i64,
        seed: &OrchestrationSeed,
    ) -> Result<bool, DbError> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        ensure_project(&tx, project_id)?;
        if fetch_settings(&tx, project_id)?.is_some() {
            return Ok(false);
        }
        upsert_settings(&tx, project_id, &seed.settings)?;
        for layer in &seed.layers {
            insert_layer(&tx, project_id, layer)?;
        }
        insert_team(&tx, project_id, &seed.team)?;
        for placement in &seed.placements {
            upsert_placement(&tx, project_id, placement)?;
        }
        tx.commit()?;
        Ok(true)
    }

    pub fn delete_placements_outside(
        &self,
        project_id: i64,
        agent_ids: &[String],
    ) -> Result<(), DbError> {
        let ids_json = serde_json::to_string(agent_ids)
            .expect("uma lista de textos sempre serializa para JSON");
        let conn = self.conn()?;
        ensure_project(&conn, project_id)?;
        conn.execute(
            "DELETE FROM orch_placements WHERE project_id = ?1 \
             AND agent_id NOT IN (SELECT value FROM json_each(?2))",
            params![project_id, ids_json],
        )?;
        Ok(())
    }

    pub fn list_movements(&self, project_id: i64, limit: i64) -> Result<Vec<Handoff>, DbError> {
        if limit < MOVEMENTS_MIN_LIMIT {
            return Err(DbError::InvalidOrchestrationField {
                field: "limit",
                value: limit.to_string(),
            });
        }
        let conn = self.conn()?;
        ensure_project(&conn, project_id)?;
        let mut statement = conn.prepare(&format!(
            "SELECT {HANDOFF_COLUMNS} FROM orch_handoffs WHERE project_id = ?1 ORDER BY id DESC LIMIT ?2"
        ))?;
        let rows = statement.query_map(params![project_id, limit], handoff_from_row)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }
}

#[cfg(test)]
#[path = "../../tests/orchestration_db_tests.rs"]
mod tests;
