use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use crate::agents::Agent;
use crate::orchestration::flow_validation::validate_flow;

use super::agent_removal::{hand_over_cards, FeedNote};
use super::connection::Db;
use super::error::DbError;

// Nó virtual "Você": só recebe pela regra escalar e nunca vira camada.
const VOCE_NODE: &str = "voce";
const INSTRUCTIONS_MAX_CHARS: usize = 4000;
const MOVEMENTS_MIN_LIMIT: i64 = 1;

// Pontas do fluxo de um time: não são agentes, só o início e o fim da linha.
pub(crate) const FLOW_START: &str = "__inicio";
pub(crate) const FLOW_END: &str = "__fim";
pub(crate) const LINK_FLOW: &str = "fluxo";
pub(crate) const LINK_COMMUNICATION: &str = "comunicacao";
pub(crate) const GATEWAY_PARALLEL: &str = "paralelo";
pub(crate) const GATEWAY_EXCLUSIVE: &str = "exclusivo";
const GATEWAY_ID_PREFIX: &str = "gw-";
const ACTION_CONSULT: &str = "consultar";
const START_EDGE_EVENT: &str = "plano_aprovado";
const START_EDGE_KIND: &str = "planejar";
// Mesmas medidas da grade da tela (CANVAS_AREA e posição inicial de cada camada); a migração repete os números em SQL.
const FLOW_CANVAS_SIZE: f64 = 1600.0;
const FLOW_GRID_ORIGIN_Y: f64 = 120.0;
const FLOW_GRID_ROW_STEP: f64 = 110.0;

pub(crate) const DEFAULT_LEADER_AGENT: &str = "axiom";

// Ember orquestra o programa inteiro: não conduz time nem ocupa posição de camada.
fn is_orchestrator(agent_id: &str) -> bool {
    agent_id == Agent::ember().as_str()
}
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
const LINK_VALUES: &[&str] = &[LINK_FLOW, LINK_COMMUNICATION];
const SYNC_MODE_VALUES: &[&str] = &["sincrona", "assincrona"];
const GATEWAY_KIND_VALUES: &[&str] = &[GATEWAY_PARALLEL, GATEWAY_EXCLUSIVE];
const EDGE_ACTION_VALUES: &[&str] = &[
    ACTION_DELEGATE,
    ACTION_RETURN,
    ACTION_ESCALATE,
    ACTION_CONSULT,
];
const FINAL_VALIDATION_VALUES: &[&str] = &["manual", "automatica"];

pub(super) const INSERT_EDGE_SQL: &str =
    "INSERT INTO orch_flow_edges (team_id, from_node, to_node, link, \
    sync_mode, when_event, is_default, action, kind, turn_limit, curve, instructions, position) \
    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, \
        (SELECT COALESCE(MAX(position) + 1, 0) FROM orch_flow_edges WHERE team_id = ?1))";

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

const MAX_INSTANCES: Bounds = Bounds {
    field: "max_instances",
    min: 1,
    max: 4,
};
const MAX_WORKTREES: Bounds = Bounds {
    field: "max_worktrees",
    min: 1,
    max: 8,
};
const TURN_LIMIT: Bounds = Bounds {
    field: "turn_limit",
    min: 1,
    max: 50,
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
    pub leader_agent: Option<String>,
    pub instructions: String,
    pub final_validation: String,
    pub start_x: Option<f64>,
    pub start_y: Option<f64>,
    pub end_x: Option<f64>,
    pub end_y: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Placement {
    pub agent_id: String,
    pub team_id: String,
    pub layer_id: String,
    pub instructions: String,
    pub max_instances: i64,
    pub pos_x: f64,
    pub pos_y: f64,
    pub criteria: String,
    pub max_worktrees: Option<i64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowEdge {
    pub id: i64,
    pub team_id: String,
    pub from_node: String,
    pub to_node: String,
    pub link: String,
    pub sync_mode: String,
    pub when_event: String,
    pub is_default: bool,
    pub action: String,
    pub kind: String,
    pub turn_limit: Option<i64>,
    pub curve: f64,
    // Instrução da ligação (como o agente chega até aqui); vai no prompt de quem envia por ela.
    pub instructions: String,
    pub position: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowGateway {
    pub id: String,
    pub team_id: String,
    pub kind: String,
    pub pos_x: f64,
    pub pos_y: f64,
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
    pub event: Option<String>,
    pub edge_id: Option<i64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrchestrationSnapshot {
    pub settings: OrchestrationSettings,
    pub layers: Vec<Layer>,
    pub teams: Vec<Team>,
    pub placements: Vec<Placement>,
    pub edges: Vec<FlowEdge>,
    pub gateways: Vec<FlowGateway>,
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
    pub instructions: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlacementDraft {
    pub agent_id: String,
    pub team_id: String,
    pub layer_id: String,
    pub instructions: String,
    pub max_instances: i64,
    pub pos_x: Option<f64>,
    pub pos_y: Option<f64>,
    #[serde(default)]
    pub criteria: String,
    pub max_worktrees: Option<i64>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowNodeDraft {
    pub agent_id: String,
    pub layer_id: String,
    pub pos_x: f64,
    pub pos_y: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowGatewayDraft {
    pub id: String,
    pub kind: String,
    pub pos_x: f64,
    pub pos_y: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowEdgeDraft {
    pub from_node: String,
    pub to_node: String,
    pub link: String,
    pub sync_mode: String,
    pub when_event: String,
    #[serde(default)]
    pub is_default: bool,
    pub action: String,
    pub kind: String,
    pub turn_limit: Option<i64>,
    #[serde(default)]
    pub curve: f64,
    #[serde(default)]
    pub instructions: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct FlowPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamFlowDraft {
    pub nodes: Vec<FlowNodeDraft>,
    pub gateways: Vec<FlowGatewayDraft>,
    pub edges: Vec<FlowEdgeDraft>,
    pub start: Option<FlowPoint>,
    pub end: Option<FlowPoint>,
}

#[derive(Clone, Debug, Serialize)]
pub struct FlowProblem {
    pub code: &'static str,
    pub node: Option<String>,
    pub message: String,
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
    pub event: Option<String>,
    pub edge_id: Option<i64>,
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
const TEAM_COLUMNS: &str = "id, name, position, leader_agent, instructions, final_validation, \
    start_x, start_y, end_x, end_y";
const GATEWAY_COLUMNS: &str = "id, team_id, kind, pos_x, pos_y";
const PLACEMENT_COLUMNS: &str = "agent_id, team_id, layer_id, instructions, max_instances, \
    pos_x, pos_y, criteria, max_worktrees";
const EDGE_COLUMNS: &str =
    "id, team_id, from_node, to_node, link, sync_mode, when_event, is_default, \
    action, kind, turn_limit, curve, position, instructions";
const HANDOFF_COLUMNS: &str = "id, run_id, from_agent, to_agent, from_layer, to_layer, action, \
    kind, item_id, return_count, message_id, created_at, event, edge_id";

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
        leader_agent: row.get(3)?,
        instructions: row.get(4)?,
        final_validation: row.get(5)?,
        start_x: row.get(6)?,
        start_y: row.get(7)?,
        end_x: row.get(8)?,
        end_y: row.get(9)?,
    })
}

fn gateway_from_row(row: &Row<'_>) -> rusqlite::Result<FlowGateway> {
    Ok(FlowGateway {
        id: row.get(0)?,
        team_id: row.get(1)?,
        kind: row.get(2)?,
        pos_x: row.get(3)?,
        pos_y: row.get(4)?,
    })
}

fn placement_from_row(row: &Row<'_>) -> rusqlite::Result<Placement> {
    Ok(Placement {
        agent_id: row.get(0)?,
        team_id: row.get(1)?,
        layer_id: row.get(2)?,
        instructions: row.get(3)?,
        max_instances: row.get(4)?,
        pos_x: row.get(5)?,
        pos_y: row.get(6)?,
        criteria: row.get(7)?,
        max_worktrees: row.get(8)?,
    })
}

fn edge_from_row(row: &Row<'_>) -> rusqlite::Result<FlowEdge> {
    Ok(FlowEdge {
        id: row.get(0)?,
        team_id: row.get(1)?,
        from_node: row.get(2)?,
        to_node: row.get(3)?,
        link: row.get(4)?,
        sync_mode: row.get(5)?,
        when_event: row.get(6)?,
        is_default: row.get(7)?,
        action: row.get(8)?,
        kind: row.get(9)?,
        turn_limit: row.get(10)?,
        curve: row.get(11)?,
        position: row.get(12)?,
        instructions: row.get(13)?,
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
        event: row.get(12)?,
        edge_id: row.get(13)?,
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

fn layer_exists(conn: &Connection, id: &str) -> Result<bool, DbError> {
    row_exists(conn, "SELECT 1 FROM orch_layers WHERE id = ?1", params![id])
}

fn team_exists(conn: &Connection, id: &str) -> Result<bool, DbError> {
    row_exists(conn, "SELECT 1 FROM orch_teams WHERE id = ?1", params![id])
}

fn fetch_settings(conn: &Connection) -> Result<Option<OrchestrationSettings>, DbError> {
    Ok(conn
        .query_row(
            &format!("SELECT {SETTINGS_COLUMNS} FROM orch_settings"),
            [],
            settings_from_row,
        )
        .optional()?)
}

fn fetch_layer(conn: &Connection, id: &str) -> Result<Layer, DbError> {
    conn.query_row(
        &format!("SELECT {LAYER_COLUMNS} FROM orch_layers WHERE id = ?1"),
        params![id],
        layer_from_row,
    )
    .optional()?
    .ok_or_else(|| DbError::LayerNotFound(id.to_string()))
}

fn fetch_team(conn: &Connection, id: &str) -> Result<Team, DbError> {
    conn.query_row(
        &format!("SELECT {TEAM_COLUMNS} FROM orch_teams WHERE id = ?1"),
        params![id],
        team_from_row,
    )
    .optional()?
    .ok_or_else(|| DbError::TeamNotFound(id.to_string()))
}

fn fetch_placement(conn: &Connection, agent_id: &str) -> Result<Placement, DbError> {
    Ok(conn.query_row(
        &format!("SELECT {PLACEMENT_COLUMNS} FROM orch_placements WHERE agent_id = ?1"),
        params![agent_id],
        placement_from_row,
    )?)
}

fn list_layers(conn: &Connection) -> Result<Vec<Layer>, DbError> {
    let mut statement = conn.prepare(&format!(
        "SELECT {LAYER_COLUMNS} FROM orch_layers ORDER BY position, id"
    ))?;
    let rows = statement.query_map([], layer_from_row)?;
    Ok(rows.collect::<Result<_, _>>()?)
}

fn list_teams(conn: &Connection) -> Result<Vec<Team>, DbError> {
    let mut statement = conn.prepare(&format!(
        "SELECT {TEAM_COLUMNS} FROM orch_teams ORDER BY position, id"
    ))?;
    let rows = statement.query_map([], team_from_row)?;
    Ok(rows.collect::<Result<_, _>>()?)
}

fn list_placements(conn: &Connection) -> Result<Vec<Placement>, DbError> {
    let mut statement = conn.prepare(&format!(
        "SELECT {PLACEMENT_COLUMNS} FROM orch_placements ORDER BY agent_id"
    ))?;
    let rows = statement.query_map([], placement_from_row)?;
    Ok(rows.collect::<Result<_, _>>()?)
}

fn list_edges(conn: &Connection) -> Result<Vec<FlowEdge>, DbError> {
    let mut statement = conn.prepare(&format!(
        "SELECT {EDGE_COLUMNS} FROM orch_flow_edges ORDER BY team_id, position, id"
    ))?;
    let rows = statement.query_map([], edge_from_row)?;
    Ok(rows.collect::<Result<_, _>>()?)
}

fn list_gateways(conn: &Connection) -> Result<Vec<FlowGateway>, DbError> {
    let mut statement = conn.prepare(&format!(
        "SELECT {GATEWAY_COLUMNS} FROM orch_flow_gateways ORDER BY team_id, id"
    ))?;
    let rows = statement.query_map([], gateway_from_row)?;
    Ok(rows.collect::<Result<_, _>>()?)
}

fn read_snapshot(conn: &Connection) -> Result<OrchestrationSnapshot, DbError> {
    Ok(OrchestrationSnapshot {
        settings: fetch_settings(conn)?.unwrap_or_default(),
        layers: list_layers(conn)?,
        teams: list_teams(conn)?,
        placements: list_placements(conn)?,
        edges: list_edges(conn)?,
        gateways: list_gateways(conn)?,
    })
}

fn flow_field_error(field: &'static str, value: &str) -> DbError {
    DbError::InvalidOrchestrationField {
        field,
        value: value.to_string(),
    }
}

// Forma do rascunho que o banco não aceitaria de qualquer jeito; as regras de fluxo (V1 a V7) ficam em flow_validation.
fn check_flow_shape(draft: &TeamFlowDraft) -> Result<(), DbError> {
    let mut agents = std::collections::HashSet::new();
    for node in &draft.nodes {
        if is_orchestrator(&node.agent_id) {
            return Err(DbError::OrchestratorNotPlaceable);
        }
        if !agents.insert(node.agent_id.as_str()) {
            return Err(flow_field_error("nodes", &node.agent_id));
        }
        check_canvas("pos_x", node.pos_x)?;
        check_canvas("pos_y", node.pos_y)?;
    }
    let mut gateways = std::collections::HashSet::new();
    for gateway in &draft.gateways {
        let valid_id = gateway.id.starts_with(GATEWAY_ID_PREFIX)
            && !agents.contains(gateway.id.as_str())
            && gateways.insert(gateway.id.as_str());
        if !valid_id {
            return Err(flow_field_error("gateways", &gateway.id));
        }
        check_one_of("gateway_kind", &gateway.kind, GATEWAY_KIND_VALUES)?;
        check_canvas("pos_x", gateway.pos_x)?;
        check_canvas("pos_y", gateway.pos_y)?;
    }
    let mut pairs = std::collections::HashSet::new();
    for edge in &draft.edges {
        check_one_of("link", &edge.link, LINK_VALUES)?;
        check_one_of("sync_mode", &edge.sync_mode, SYNC_MODE_VALUES)?;
        check_one_of("when_event", &edge.when_event, WHEN_EVENT_VALUES)?;
        check_one_of("action", &edge.action, EDGE_ACTION_VALUES)?;
        check_one_of("kind", &edge.kind, REQUEST_KIND_VALUES)?;
        if let Some(turn_limit) = edge.turn_limit {
            check_bounds(&TURN_LIMIT, turn_limit)?;
        }
        check_instructions(&edge.instructions)?;
        let consults = edge.action == ACTION_CONSULT;
        let shape_ok = edge.from_node != edge.to_node
            && edge.to_node != FLOW_START
            && edge.from_node != FLOW_END
            && (edge.action == ACTION_ESCALATE) == (edge.to_node == FLOW_END)
            && consults == (edge.link == LINK_COMMUNICATION)
            && pairs.insert((edge.from_node.as_str(), edge.to_node.as_str()));
        if !shape_ok {
            return Err(flow_field_error("edges", &edge.from_node));
        }
    }
    for point in [&draft.start, &draft.end].into_iter().flatten() {
        check_canvas("x", point.x)?;
        check_canvas("y", point.y)?;
    }
    Ok(())
}

fn is_placed_in(conn: &Connection, team_id: &str, agent_id: &str) -> Result<bool, DbError> {
    row_exists(
        conn,
        "SELECT 1 FROM orch_placements WHERE team_id = ?1 AND agent_id = ?2",
        params![team_id, agent_id],
    )
}

// Líder e início andam juntos: o líder do time é o destino da única seta de início, e só um agente do time pode ser.
fn set_start_edge(conn: &Connection, team_id: &str, leader: &str) -> Result<(), DbError> {
    if !is_placed_in(conn, team_id, leader)? {
        return Err(DbError::FlowNodeOutsideTeam(leader.to_string()));
    }
    clear_start_edge(conn, team_id)?;
    conn.execute(
        "INSERT INTO orch_flow_edges (team_id, from_node, to_node, when_event, action, kind, position) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, \
            (SELECT COALESCE(MAX(position) + 1, 0) FROM orch_flow_edges WHERE team_id = ?1))",
        params![
            team_id,
            FLOW_START,
            leader,
            START_EDGE_EVENT,
            ACTION_DELEGATE,
            START_EDGE_KIND
        ],
    )?;
    Ok(())
}

fn clear_start_edge(conn: &Connection, team_id: &str) -> Result<(), DbError> {
    conn.execute(
        "DELETE FROM orch_flow_edges WHERE team_id = ?1 AND from_node = ?2",
        params![team_id, FLOW_START],
    )?;
    Ok(())
}

// Saiu do time (ou da orquestração): as setas dele somem e, se era o líder do time, o time fica sem líder.
fn detach_agent_flow(conn: &Connection, agent_id: &str) -> Result<(), DbError> {
    conn.execute(
        "UPDATE orch_teams SET leader_agent = NULL WHERE leader_agent = ?1 \
         AND id IN (SELECT team_id FROM orch_placements WHERE agent_id = ?1)",
        params![agent_id],
    )?;
    conn.execute(
        "DELETE FROM orch_flow_edges WHERE from_node = ?1 OR to_node = ?1",
        params![agent_id],
    )?;
    Ok(())
}

fn check_canvas(field: &'static str, value: f64) -> Result<(), DbError> {
    if (0.0..=FLOW_CANVAS_SIZE).contains(&value) {
        Ok(())
    } else {
        Err(DbError::OrchestrationOutOfRange {
            field,
            min: 0,
            max: FLOW_CANVAS_SIZE as i64,
        })
    }
}

// Posição da grade para quem entra sem posição: y pela ordem da camada, x espalhado entre os agentes do time na camada.
fn grid_position(conn: &Connection, draft: &PlacementDraft) -> Result<(f64, f64), DbError> {
    let layers_before: i64 = conn.query_row(
        "SELECT COUNT(*) FROM orch_layers own, orch_layers other WHERE own.id = ?1 \
         AND (other.position < own.position OR (other.position = own.position AND other.id < own.id))",
        params![draft.layer_id],
        |row| row.get(0),
    )?;
    let peers: i64 = conn.query_row(
        "SELECT COUNT(*) FROM orch_placements WHERE team_id = ?1 AND layer_id = ?2 AND agent_id <> ?3",
        params![draft.team_id, draft.layer_id, draft.agent_id],
        |row| row.get(0),
    )?;
    let in_layer = (peers + 1) as f64;
    Ok((
        FLOW_CANVAS_SIZE * in_layer / (in_layer + 1.0),
        FLOW_GRID_ORIGIN_Y + FLOW_GRID_ROW_STEP * layers_before as f64,
    ))
}

fn layer_ids(conn: &Connection) -> Result<Vec<String>, DbError> {
    let mut statement = conn.prepare("SELECT id FROM orch_layers")?;
    let rows = statement.query_map([], |row| row.get(0))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

fn upsert_settings(conn: &Connection, settings: &OrchestrationSettings) -> Result<(), DbError> {
    conn.execute(
        "INSERT INTO orch_settings (id, enforcement, leader_agent, step_mode, human_gate, \
            max_parallel, pair_turn_limit, task_turn_limit, worktree_policy) \
         VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) \
         ON CONFLICT (id) DO UPDATE SET \
            enforcement = excluded.enforcement, leader_agent = excluded.leader_agent, \
            step_mode = excluded.step_mode, human_gate = excluded.human_gate, \
            max_parallel = excluded.max_parallel, pair_turn_limit = excluded.pair_turn_limit, \
            task_turn_limit = excluded.task_turn_limit, worktree_policy = excluded.worktree_policy",
        params![
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

fn insert_layer(conn: &Connection, draft: &LayerDraft) -> Result<String, DbError> {
    let id = slug_of(&draft.name)?;
    if id == VOCE_NODE {
        return Err(DbError::ReservedLayerName(draft.name.clone()));
    }
    if layer_exists(conn, &id)? {
        return Err(DbError::LayerNameTaken(id));
    }
    conn.execute(
        "INSERT INTO orch_layers (id, name, description, position, kind, instructions) \
         VALUES (?1, ?2, ?3, (SELECT COALESCE(MAX(position) + 1, 0) FROM orch_layers), ?4, ?5)",
        params![
            id,
            draft.name,
            draft.description,
            draft.kind,
            draft.instructions
        ],
    )?;
    Ok(id)
}

fn insert_team(conn: &Connection, draft: &TeamDraft) -> Result<String, DbError> {
    let id = slug_of(&draft.name)?;
    if team_exists(conn, &id)? {
        return Err(DbError::TeamNameTaken(id));
    }
    conn.execute(
        "INSERT INTO orch_teams (id, name, position, instructions) \
         VALUES (?1, ?2, (SELECT COALESCE(MAX(position) + 1, 0) FROM orch_teams), ?3)",
        params![
            id,
            draft.name,
            draft.instructions.as_deref().unwrap_or_default()
        ],
    )?;
    Ok(id)
}

fn upsert_placement(conn: &Connection, draft: &PlacementDraft) -> Result<(), DbError> {
    if !team_exists(conn, &draft.team_id)? {
        return Err(DbError::TeamNotFound(draft.team_id.clone()));
    }
    if !layer_exists(conn, &draft.layer_id)? {
        return Err(DbError::LayerNotFound(draft.layer_id.clone()));
    }
    let current = conn
        .query_row(
            &format!("SELECT {PLACEMENT_COLUMNS} FROM orch_placements WHERE agent_id = ?1"),
            params![draft.agent_id],
            placement_from_row,
        )
        .optional()?;
    if current
        .as_ref()
        .is_some_and(|placement| placement.team_id != draft.team_id)
    {
        detach_agent_flow(conn, &draft.agent_id)?;
    }
    let grid = grid_position(conn, draft)?;
    let pos_x = draft
        .pos_x
        .or(current.as_ref().map(|placement| placement.pos_x))
        .unwrap_or(grid.0);
    let pos_y = draft
        .pos_y
        .or(current.as_ref().map(|placement| placement.pos_y))
        .unwrap_or(grid.1);
    conn.execute(
        "INSERT INTO orch_placements (agent_id, team_id, layer_id, instructions, max_instances, \
            pos_x, pos_y, criteria, max_worktrees) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9) \
         ON CONFLICT (agent_id) DO UPDATE SET \
            team_id = excluded.team_id, layer_id = excluded.layer_id, \
            instructions = excluded.instructions, max_instances = excluded.max_instances, \
            pos_x = excluded.pos_x, pos_y = excluded.pos_y, \
            criteria = excluded.criteria, max_worktrees = excluded.max_worktrees",
        params![
            draft.agent_id,
            draft.team_id,
            draft.layer_id,
            draft.instructions,
            draft.max_instances,
            pos_x,
            pos_y,
            draft.criteria,
            draft.max_worktrees
        ],
    )?;
    Ok(())
}

impl Db {
    pub fn orchestration(&self) -> Result<OrchestrationSnapshot, DbError> {
        let conn = self.conn()?;
        read_snapshot(&conn)
    }

    pub fn set_final_validation(&self, team_id: &str, mode: &str) -> Result<(), DbError> {
        check_one_of("final_validation", mode, FINAL_VALIDATION_VALUES)?;
        let conn = self.conn()?;
        let changed = conn.execute(
            "UPDATE orch_teams SET final_validation = ?2 WHERE id = ?1",
            params![team_id, mode],
        )?;
        if changed == 0 {
            return Err(DbError::TeamNotFound(team_id.to_string()));
        }
        Ok(())
    }

    // A estrutura do fluxo só muda aqui, de uma vez: fluxo inválido não grava nada. Os agentes do time que não vêm
    // no rascunho saem do time; os que vêm mantêm as instruções, os critérios, as instâncias e o máximo de worktrees.
    pub fn save_team_flow(
        &self,
        team_id: &str,
        draft: &TeamFlowDraft,
        name_of: &dyn Fn(&str) -> String,
    ) -> Result<OrchestrationSnapshot, DbError> {
        check_flow_shape(draft)?;
        let problems = validate_flow(draft, name_of);
        if !problems.is_empty() {
            return Err(DbError::FlowInvalid(problems));
        }
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        if !team_exists(&tx, team_id)? {
            return Err(DbError::TeamNotFound(team_id.to_string()));
        }
        for gateway in &draft.gateways {
            let taken = row_exists(
                &tx,
                "SELECT 1 FROM orch_flow_gateways WHERE id = ?1 AND team_id <> ?2",
                params![gateway.id, team_id],
            )?;
            if taken {
                return Err(flow_field_error("gateways", &gateway.id));
            }
        }
        let placed: Vec<String> = {
            let mut statement =
                tx.prepare("SELECT agent_id FROM orch_placements WHERE team_id = ?1")?;
            let rows = statement.query_map(params![team_id], |row| row.get(0))?;
            rows.collect::<rusqlite::Result<_>>()?
        };
        for agent in placed
            .iter()
            .filter(|agent| !draft.nodes.iter().any(|node| node.agent_id == **agent))
        {
            detach_agent_flow(&tx, agent)?;
            tx.execute(
                "DELETE FROM orch_placements WHERE agent_id = ?1",
                params![agent],
            )?;
        }
        for node in &draft.nodes {
            let kept = tx
                .query_row(
                    &format!("SELECT {PLACEMENT_COLUMNS} FROM orch_placements WHERE agent_id = ?1"),
                    params![node.agent_id],
                    placement_from_row,
                )
                .optional()?;
            upsert_placement(
                &tx,
                &PlacementDraft {
                    agent_id: node.agent_id.clone(),
                    team_id: team_id.to_string(),
                    layer_id: node.layer_id.clone(),
                    instructions: kept
                        .as_ref()
                        .map(|placement| placement.instructions.clone())
                        .unwrap_or_default(),
                    max_instances: kept.as_ref().map_or(1, |placement| placement.max_instances),
                    pos_x: Some(node.pos_x),
                    pos_y: Some(node.pos_y),
                    criteria: kept
                        .as_ref()
                        .map(|placement| placement.criteria.clone())
                        .unwrap_or_default(),
                    max_worktrees: kept.and_then(|placement| placement.max_worktrees),
                },
            )?;
        }
        tx.execute(
            "DELETE FROM orch_flow_edges WHERE team_id = ?1",
            params![team_id],
        )?;
        tx.execute(
            "DELETE FROM orch_flow_gateways WHERE team_id = ?1",
            params![team_id],
        )?;
        for gateway in &draft.gateways {
            tx.execute(
                "INSERT INTO orch_flow_gateways (id, team_id, kind, pos_x, pos_y) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![gateway.id, team_id, gateway.kind, gateway.pos_x, gateway.pos_y],
            )?;
        }
        for edge in &draft.edges {
            tx.execute(
                INSERT_EDGE_SQL,
                params![
                    team_id,
                    edge.from_node,
                    edge.to_node,
                    edge.link,
                    edge.sync_mode,
                    edge.when_event,
                    edge.is_default,
                    edge.action,
                    edge.kind,
                    edge.turn_limit,
                    edge.curve,
                    edge.instructions
                ],
            )?;
        }
        let leader = draft
            .edges
            .iter()
            .find(|edge| edge.from_node == FLOW_START)
            .map(|edge| edge.to_node.as_str())
            .filter(|node| {
                draft
                    .nodes
                    .iter()
                    .any(|candidate| candidate.agent_id == *node)
            });
        tx.execute(
            "UPDATE orch_teams SET leader_agent = ?2, start_x = ?3, start_y = ?4, end_x = ?5, end_y = ?6 \
             WHERE id = ?1",
            params![
                team_id,
                leader,
                draft.start.as_ref().map(|point| point.x),
                draft.start.as_ref().map(|point| point.y),
                draft.end.as_ref().map(|point| point.x),
                draft.end.as_ref().map(|point| point.y)
            ],
        )?;
        let snapshot = read_snapshot(&tx)?;
        tx.commit()?;
        Ok(snapshot)
    }

    pub fn save_settings(&self, settings: &OrchestrationSettings) -> Result<(), DbError> {
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
        if is_orchestrator(&settings.leader_agent) {
            return Err(DbError::OrchestratorNotLeader);
        }
        let conn = self.conn()?;
        upsert_settings(&conn, settings)
    }

    pub fn save_layer(&self, draft: &LayerDraft) -> Result<Layer, DbError> {
        check_name(&draft.name)?;
        check_one_of("kind", &draft.kind, LAYER_KIND_VALUES)?;
        check_instructions(&draft.instructions)?;
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let id = match &draft.id {
            Some(id) => {
                let changed = tx.execute(
                    "UPDATE orch_layers SET name = ?2, description = ?3, kind = ?4, instructions = ?5 \
                     WHERE id = ?1",
                    params![id, draft.name, draft.description, draft.kind, draft.instructions],
                )?;
                if changed == 0 {
                    return Err(DbError::LayerNotFound(id.clone()));
                }
                id.clone()
            }
            None => insert_layer(&tx, draft)?,
        };
        let layer = fetch_layer(&tx, &id)?;
        tx.commit()?;
        Ok(layer)
    }

    pub fn delete_layer(&self, id: &str) -> Result<(), DbError> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        if !layer_exists(&tx, id)? {
            return Err(DbError::LayerNotFound(id.to_string()));
        }
        if row_exists(
            &tx,
            "SELECT 1 FROM orch_placements WHERE layer_id = ?1",
            params![id],
        )? {
            return Err(DbError::LayerHasAgents(id.to_string()));
        }
        tx.execute("DELETE FROM orch_layers WHERE id = ?1", params![id])?;
        tx.commit()?;
        Ok(())
    }

    pub fn reorder_layers(&self, ids: &[String]) -> Result<(), DbError> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let mut current = layer_ids(&tx)?;
        let mut requested = ids.to_vec();
        current.sort();
        requested.sort();
        if current != requested {
            return Err(DbError::LayerOrderInvalid);
        }
        for (position, id) in ids.iter().enumerate() {
            tx.execute(
                "UPDATE orch_layers SET position = ?2 WHERE id = ?1",
                params![id, position as i64],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn set_team_leader(&self, team_id: &str, leader: &str) -> Result<Team, DbError> {
        if is_orchestrator(leader) {
            return Err(DbError::OrchestratorNotLeader);
        }
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let changed = tx.execute(
            "UPDATE orch_teams SET leader_agent = ?2 WHERE id = ?1",
            params![team_id, leader],
        )?;
        if changed == 0 {
            return Err(DbError::TeamNotFound(team_id.to_string()));
        }
        // Líder fora do time não tem como receber a seta de início.
        if is_placed_in(&tx, team_id, leader)? {
            set_start_edge(&tx, team_id, leader)?;
        } else {
            clear_start_edge(&tx, team_id)?;
        }
        let team = fetch_team(&tx, team_id)?;
        tx.commit()?;
        Ok(team)
    }

    // "Automático": o time deixa de ter líder gravado e a ligação de início sai; o líder passa a vir da composição.
    pub fn clear_team_leader(&self, team_id: &str) -> Result<Team, DbError> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let changed = tx.execute(
            "UPDATE orch_teams SET leader_agent = NULL WHERE id = ?1",
            params![team_id],
        )?;
        if changed == 0 {
            return Err(DbError::TeamNotFound(team_id.to_string()));
        }
        clear_start_edge(&tx, team_id)?;
        let team = fetch_team(&tx, team_id)?;
        tx.commit()?;
        Ok(team)
    }

    // Só preenche time sem líder: o líder já gravado nunca muda por aqui. O menor índice no elenco vence,
    // a mesma ordem que o interpretador usava para escolher o líder pela camada planejar.
    pub fn fill_missing_team_leaders(&self, roster: &[String]) -> Result<(), DbError> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let candidates: Vec<(String, String)> = {
            let mut statement = tx.prepare(
                "SELECT p.team_id, p.agent_id FROM orch_placements p \
                 JOIN orch_layers l ON l.id = p.layer_id \
                 JOIN orch_teams t ON t.id = p.team_id \
                 WHERE t.leader_agent IS NULL AND l.kind = 'planejar' \
                   AND NOT EXISTS (SELECT 1 FROM orch_flow_edges e \
                                   WHERE e.team_id = t.id AND e.from_node = ?1)",
            )?;
            let rows =
                statement.query_map(params![FLOW_START], |row| Ok((row.get(0)?, row.get(1)?)))?;
            rows.collect::<Result<_, _>>()?
        };
        let mut chosen: Vec<(String, usize, String)> = Vec::new();
        for (team_id, agent_id) in candidates {
            let Some(rank) = roster.iter().position(|id| *id == agent_id) else {
                continue;
            };
            match chosen.iter_mut().find(|(team, _, _)| *team == team_id) {
                Some(entry) => {
                    if rank < entry.1 {
                        entry.1 = rank;
                        entry.2 = agent_id;
                    }
                }
                None => chosen.push((team_id, rank, agent_id)),
            }
        }
        for (team_id, _, agent_id) in chosen {
            tx.execute(
                "UPDATE orch_teams SET leader_agent = ?2 WHERE id = ?1 AND leader_agent IS NULL",
                params![team_id, agent_id],
            )?;
            set_start_edge(&tx, &team_id, &agent_id)?;
        }
        tx.commit()?;
        Ok(())
    }

    // Sem instructions no rascunho, o texto gravado fica como está.
    pub fn save_team(&self, draft: &TeamDraft) -> Result<Team, DbError> {
        check_name(&draft.name)?;
        check_instructions(draft.instructions.as_deref().unwrap_or_default())?;
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let id = match &draft.id {
            Some(id) => {
                let changed = tx.execute(
                    "UPDATE orch_teams SET name = ?2, instructions = COALESCE(?3, instructions) WHERE id = ?1",
                    params![id, draft.name, draft.instructions],
                )?;
                if changed == 0 {
                    return Err(DbError::TeamNotFound(id.clone()));
                }
                id.clone()
            }
            None => insert_team(&tx, draft)?,
        };
        let team = fetch_team(&tx, &id)?;
        tx.commit()?;
        Ok(team)
    }

    // Os agentes do time ficam sem posição (continuam no elenco) e seus cards, de todos os projetos,
    // passam ao sucessor, tudo na mesma transação que remove o time e, com ele, o líder do time.
    pub fn delete_team(
        &self,
        id: &str,
        successor: &Agent,
        successor_name: &str,
        agent_name: impl Fn(&Agent) -> String,
        time: u64,
    ) -> Result<Vec<FeedNote>, DbError> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        if !team_exists(&tx, id)? {
            return Err(DbError::TeamNotFound(id.to_string()));
        }
        let mut statement = tx
            .prepare("SELECT agent_id FROM orch_placements WHERE team_id = ?1 ORDER BY agent_id")?;
        let placed: Vec<String> = statement
            .query_map(params![id], |row| row.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        drop(statement);
        let mut notes = Vec::new();
        for stored in placed {
            let Some(agent) = Agent::from_stored(&stored) else {
                continue;
            };
            notes.extend(hand_over_cards(
                &tx,
                None,
                &agent,
                &agent_name(&agent),
                successor,
                successor_name,
                time,
            )?);
        }
        tx.execute(
            "DELETE FROM orch_placements WHERE team_id = ?1",
            params![id],
        )?;
        tx.execute("DELETE FROM orch_teams WHERE id = ?1", params![id])?;
        tx.commit()?;
        Ok(notes)
    }

    // Uma posição por agente: posicionar de novo move o agente em vez de criar outra.
    pub fn place_agent(&self, draft: &PlacementDraft) -> Result<Placement, DbError> {
        if is_orchestrator(&draft.agent_id) {
            return Err(DbError::OrchestratorNotPlaceable);
        }
        check_bounds(&MAX_INSTANCES, draft.max_instances)?;
        check_instructions(&draft.instructions)?;
        check_instructions(&draft.criteria)?;
        if let Some(max_worktrees) = draft.max_worktrees {
            check_bounds(&MAX_WORKTREES, max_worktrees)?;
        }
        if let Some(pos_x) = draft.pos_x {
            check_canvas("pos_x", pos_x)?;
        }
        if let Some(pos_y) = draft.pos_y {
            check_canvas("pos_y", pos_y)?;
        }
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        upsert_placement(&tx, draft)?;
        let placement = fetch_placement(&tx, &draft.agent_id)?;
        tx.commit()?;
        Ok(placement)
    }

    // O agente continua no elenco e volta para "Sem posição"; posição inexistente já é o resultado pedido.
    pub fn unplace_agent(&self, agent_id: &str) -> Result<(), DbError> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        detach_agent_flow(&tx, agent_id)?;
        tx.execute(
            "DELETE FROM orch_placements WHERE agent_id = ?1",
            params![agent_id],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn record_handoff(&self, project_id: i64, draft: &HandoffDraft) -> Result<(), DbError> {
        check_one_of("action", &draft.action, EDGE_ACTION_VALUES)?;
        check_one_of("kind", &draft.kind, REQUEST_KIND_VALUES)?;
        check_bounds(&RETURN_COUNT, draft.return_count)?;
        let conn = self.conn()?;
        ensure_project(&conn, project_id)?;
        conn.execute(
            "INSERT INTO orch_handoffs (project_id, run_id, from_agent, to_agent, from_layer, to_layer, \
                action, kind, item_id, return_count, message_id, event, edge_id) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
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
                draft.message_id,
                draft.event,
                draft.edge_id
            ],
        )?;
        Ok(())
    }

    // Checagem da semente e inserções na mesma transação, com o lock da conexão do início ao fim.
    pub fn seed_orchestration(&self, seed: &OrchestrationSeed) -> Result<bool, DbError> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        if fetch_settings(&tx)?.is_some() {
            return Ok(false);
        }
        upsert_settings(&tx, &seed.settings)?;
        for layer in &seed.layers {
            insert_layer(&tx, layer)?;
        }
        check_instructions(seed.team.instructions.as_deref().unwrap_or_default())?;
        insert_team(&tx, &seed.team)?;
        for placement in &seed.placements {
            upsert_placement(&tx, placement)?;
        }
        tx.commit()?;
        Ok(true)
    }

    pub fn delete_placements_outside(&self, agent_ids: &[String]) -> Result<(), DbError> {
        let ids_json = serde_json::to_string(agent_ids)
            .expect("uma lista de textos sempre serializa para JSON");
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        tx.execute(
            "DELETE FROM orch_flow_edges \
             WHERE (from_node <> ?2 AND from_node NOT IN (SELECT value FROM json_each(?1))) \
                OR (to_node <> ?3 AND to_node NOT IN (SELECT value FROM json_each(?1)))",
            params![ids_json, FLOW_START, FLOW_END],
        )?;
        tx.execute(
            "DELETE FROM orch_placements WHERE agent_id NOT IN (SELECT value FROM json_each(?1))",
            params![ids_json],
        )?;
        tx.commit()?;
        Ok(())
    }

    // Passagens da ligação neste card desde que ele entrou em In progress pela última vez (devolver zera a contagem).
    pub fn count_edge_passages(
        &self,
        project_id: i64,
        item_id: &str,
        edge_id: i64,
    ) -> Result<i64, DbError> {
        let conn = self.conn()?;
        Ok(conn.query_row(
            "SELECT COUNT(*) FROM orch_handoffs \
             WHERE project_id = ?1 AND item_id = ?2 AND edge_id = ?3 \
               AND created_at >= COALESCE((SELECT cycle_started_at FROM backlog_items \
                                           WHERE project_id = ?1 AND id = ?2), 0)",
            params![project_id, item_id, edge_id],
            |row| row.get(0),
        )?)
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
