use std::collections::HashSet;

use chrono::Local;
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::agents::Team;
use crate::events::Agent;
use crate::plan_proposal::{parse_proposal, PlanDraft};

use super::artifacts::{
    artifact_exists, artifact_id, artifact_updated_at, delete_artifact, read_plan_artifact, replace_artifact,
    upsert_artifact_row, upsert_plan_criteria, Artifact,
};
use super::connection::Db;
use super::error::DbError;

pub const BACKLOG_LABEL_MAX_CHARS: usize = 32;
const DEFAULT_COLUMN: &str = "Backlog";
const IN_PROGRESS_COLUMN: &str = "In progress";
const IN_REVIEW_COLUMN: &str = "In review";
pub const BOARD_COLUMNS: &[&str] = &["Backlog", "Ready", IN_PROGRESS_COLUMN, "In review", "Done"];
const MOVABLE_COLUMNS: &[&str] = &["Backlog", "Ready", "Done"];
pub const BACKLOG_PRIORITIES: &[&str] = &["Baixa", "Média", "Alta", "Urgente"];
const ITEM_TYPES: &[&str] = &["task", "feature"];
const ITEM_STATES: &[&str] = &["vazio", "triagem", "gerando", "proposto", "vinculado"];
const MICRO_TASK_STATUSES: &[&str] = &["proposta", "ready", "em andamento", "em revisão", "feito"];
const EMPTY_PLAN: &str = "vazio";
const PROPOSED_PLAN: &str = "proposto";
const LINKED_PLAN: &str = "vinculado";
const PROPOSAL_REQUIRED: &str = "A proposta do plano não pode ser vazia.";
const LINKED_PLAN_REQUIRED: &str = "O plano vinculado precisa de id, nome e resumo.";
const LINKED_PLAN_INCOMPLETE: &str = "os dados do plano vinculado estão incompletos.";
const CHILDREN_INCOMPLETE: &str = "o total e os feitos dos filhos precisam vir juntos.";
const ITEM_WITH_ERROR: &str = "o item tem erro de leitura e não pode ser gravado de novo.";
const FALLBACK_ITEM_TYPE: &str = "task";
const FALLBACK_ITEM_STATE: &str = "vazio";
const ORIGIN_USER: &str = "usuario";
const ORIGIN_CONVERSATION: &str = "conversa";
const ORIGIN_AGENT: &str = "agente";
const ORIGINS: &[&str] = &[ORIGIN_USER, ORIGIN_CONVERSATION, ORIGIN_AGENT];
const DONE_COLUMN: &str = "Done";
const READY_COLUMN: &str = "Ready";
// Done é do usuário ou da validação final: o agente não move o card para lá nem para Ready.
pub const AGENT_FORBIDDEN_COLUMNS: &[&str] = &[READY_COLUMN, DONE_COLUMN];
const CLOSED_BY_USER: &str = "usuario";
const CLOSED_BY_AUTOMATIC: &str = "automatico";
const CLOSERS: &[&str] = &[CLOSED_BY_USER, CLOSED_BY_AUTOMATIC];
const USER_OWNER: &str = "voce";
const USER_ACTOR_NAME: &str = "Você";
const USER_CARD_TYPE: &str = "feature";
const AGENT_CARD_TYPE: &str = "task";
const NEW_CARD_STATE: &str = "vazio";
const TRIAGE_CARD_STATE: &str = "triagem";
const CARD_ID_PREFIX: &str = "item-";
const USER_CREATED_TEXT: &str = "Pedido criado por você.";
const CONVERSATION_CREATED_TEXT: &str = "Pedido criado a partir de uma mensagem da conversa.";
const ACTIVITY_TIME_FORMAT: &str = "%d/%m/%Y, %H:%M:%S";
const PLAN_CARD_CREATED_TEXT: &str = "Etapa criada pelo plano aceito do item";
const PLAN_IMPORTED_TEXT: &str = "Plano importado";

const ITEM_COLUMNS: &str = "id, title, \"type\", state, stage, triage_step, parent, feature, \
    description, plan, who, \"by\", is_new, \"column\", priority, estimate, tags, tasks, criteria, \
    questions, links, activity, children_total, children_done, origin, branch, worktree_path, \
    closed_by, auto_attempts, plan_note";

const UPSERT_ITEM_SQL: &str = "INSERT INTO backlog_items (project_id, id, title, \"type\", state, \
    stage, triage_step, parent, feature, description, plan, who, \"by\", is_new, \"column\", \
    priority, estimate, tags, tasks, criteria, questions, links, activity, children_total, \
    children_done, origin, plan_note, position) \
    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, \
    ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, \
    (SELECT COALESCE(MAX(position), 0) + 1 FROM backlog_items WHERE project_id = ?1)) \
    ON CONFLICT(project_id, id) DO UPDATE SET \
    title = excluded.title, \"type\" = excluded.\"type\", state = excluded.state, \
    stage = excluded.stage, triage_step = excluded.triage_step, parent = excluded.parent, \
    feature = excluded.feature, description = excluded.description, plan = excluded.plan, \
    who = excluded.who, \"by\" = excluded.\"by\", is_new = excluded.is_new, \
    \"column\" = excluded.\"column\", priority = excluded.priority, estimate = excluded.estimate, \
    tags = excluded.tags, tasks = excluded.tasks, criteria = excluded.criteria, \
    questions = excluded.questions, links = excluded.links, activity = excluded.activity, \
    children_total = excluded.children_total, children_done = excluded.children_done, \
    plan_note = excluded.plan_note";

const INSERT_CARD_SQL: &str = "INSERT INTO backlog_items (project_id, id, title, \"type\", state, \
    is_new, \"column\", who, \"by\", description, origin, activity, links, position) \
    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)";

const INSERT_PLAN_CARD_SQL: &str = "INSERT INTO backlog_items (project_id, id, title, \"type\", state, \
    parent, is_new, \"column\", who, \"by\", description, origin, activity, position) \
    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, \
    (SELECT COALESCE(MAX(position), 0) + 1 FROM backlog_items WHERE project_id = ?1)) \
    ON CONFLICT(project_id, id) DO NOTHING";

// Sem filho ativo, total e feitos ficam nulos juntos: item_from_row recusa um sem o outro.
const REFRESH_CHILDREN_SQL: &str = "UPDATE backlog_items SET \
    children_total = (SELECT NULLIF(COUNT(*), 0) FROM backlog_items AS child \
        WHERE child.project_id = ?1 AND child.parent = ?2 AND child.archived_at IS NULL), \
    children_done = (SELECT CASE WHEN COUNT(*) = 0 THEN NULL ELSE SUM(child.\"column\" = ?3) END \
        FROM backlog_items AS child \
        WHERE child.project_id = ?1 AND child.parent = ?2 AND child.archived_at IS NULL) \
    WHERE project_id = ?1 AND id = ?2";

const UPSERT_PLAN_SQL: &str = "INSERT INTO item_plans (project_id, item_id, state, text, plan_id, \
    plan_name, plan_meta, agent, linked_at) \
    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, CAST(unixepoch('subsec') * 1000 AS INTEGER)) \
    ON CONFLICT(project_id, item_id) DO UPDATE SET state = excluded.state, text = excluded.text, \
    plan_id = excluded.plan_id, plan_name = excluded.plan_name, plan_meta = excluded.plan_meta, \
    agent = excluded.agent, linked_at = excluded.linked_at";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Children {
    pub total: u32,
    pub done: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TriageQuestion {
    pub id: String,
    pub text: String,
    pub options: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptBlock {
    pub context: String,
    pub files: Vec<String>,
    pub steps: Vec<String>,
    pub done: Vec<String>,
    pub limits: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MicroTask {
    pub id: String,
    pub title: String,
    pub who: String,
    pub model: String,
    pub status: String,
    pub script: ScriptBlock,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub returned: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub learned: Option<Vec<String>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Criterion {
    pub id: String,
    pub text: String,
    pub done: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Activity {
    pub id: String,
    pub text: String,
    pub at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Link {
    pub id: String,
    pub label: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BacklogItem {
    pub id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub triage_step: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feature: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan: Option<String>,
    // Instrução do usuário para quem planeja: entra no prompt do pedido e dos ajustes do plano.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_note: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Children>,
    pub who: String,
    pub by: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_new: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimate: Option<String>,
    #[serde(default)]
    pub questions: Vec<TriageQuestion>,
    #[serde(default)]
    pub micro: Vec<MicroTask>,
    #[serde(default)]
    pub criteria: Vec<Criterion>,
    #[serde(default)]
    pub activity: Vec<Activity>,
    #[serde(default)]
    pub links: Vec<Link>,
    #[serde(default = "default_origin")]
    pub origin: String,
    pub branch: Option<String>,
    pub worktree_path: Option<String>,
    // Quem concluiu o card (usuario ou automatico) e quantas devoluções da validação automática ele já teve.
    #[serde(default)]
    pub closed_by: Option<String>,
    #[serde(default)]
    pub auto_attempts: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ItemError>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemError {
    pub reason: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkedPlan {
    pub id: String,
    pub name: String,
    pub meta: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemPlanInput {
    pub state: String,
    pub proposal: Option<String>,
    pub plan: Option<LinkedPlan>,
    pub agent: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemPlanEntry {
    pub item_id: String,
    pub state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proposal: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan: Option<LinkedPlan>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    pub linked_at: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BacklogSnapshot {
    pub items: Vec<BacklogItem>,
    pub plans: Vec<ItemPlanEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewCard {
    pub title: String,
    pub description: Option<String>,
    pub origin: String,
    #[serde(default)]
    pub links: Vec<Link>,
}

pub struct AgentActor {
    pub id: Agent,
    pub name: String,
    pub current_card: Option<String>,
}

// Líder que assume o card criado pelo orquestrador; o autor continua sendo quem criou.
pub struct CardResponsible {
    pub leader: Agent,
    pub team_name: String,
    pub leader_name: String,
}

pub enum CardOperation {
    Create {
        title: String,
        description: Option<String>,
        responsible: Option<CardResponsible>,
    },
    Move {
        id: String,
        column: String,
    },
    Update {
        id: String,
        title: Option<String>,
        description: Option<String>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum CardOutcome {
    #[serde(rename = "criado")]
    Created,
    #[serde(rename = "movido")]
    Moved,
    #[serde(rename = "atualizado")]
    Updated,
    #[serde(rename = "sem mudança")]
    Unchanged,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardResult {
    pub id: String,
    pub title: String,
    pub column: String,
    pub outcome: CardOutcome,
}

struct CardDraft<'a> {
    title: String,
    kind: &'a str,
    owner: &'a str,
    description: Option<String>,
    origin: &'a str,
    activity: Vec<Activity>,
    links: Vec<Link>,
}

struct PlanCard<'a> {
    id: String,
    parent: &'a str,
    title: &'a str,
    description: Option<&'a str>,
}

impl Db {
    pub fn list_backlog(&self, project_id: i64) -> Result<BacklogSnapshot, DbError> {
        let conn = self.conn()?;
        ensure_project(&conn, project_id)?;
        let items = read_items(&conn, project_id)?;
        let plans = read_plans(&conn, project_id)?;
        Ok(BacklogSnapshot { items, plans })
    }

    pub fn read_card(&self, project_id: i64, id: &str) -> Result<BacklogItem, DbError> {
        let conn = self.conn()?;
        ensure_project(&conn, project_id)?;
        find_card(&conn, project_id, id)
    }

    pub fn save_backlog_item(&self, project_id: i64, item: &BacklogItem) -> Result<(), DbError> {
        validate_item(item)?;
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        ensure_project(&tx, project_id)?;
        upsert_item(&tx, project_id, item)?;
        refresh_children(&tx, project_id, &item.id)?;
        refresh_parent_of(&tx, project_id, &item.id)?;
        tx.commit()?;
        Ok(())
    }

    pub fn move_backlog_item(
        &self,
        project_id: i64,
        id: &str,
        column: &str,
    ) -> Result<(), DbError> {
        if !BOARD_COLUMNS.contains(&column) {
            return Err(DbError::InvalidBacklogColumn(column.to_string()));
        }
        if !MOVABLE_COLUMNS.contains(&column) {
            return Err(DbError::ColumnNotMovable(column.to_string()));
        }
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        ensure_project(&tx, project_id)?;
        let changed = tx.execute(
            "UPDATE backlog_items SET \"column\" = ?3, \
                closed_by = CASE WHEN ?3 = ?4 THEN ?5 ELSE NULL END \
             WHERE project_id = ?1 AND id = ?2",
            params![project_id, id, column, DONE_COLUMN, CLOSED_BY_USER],
        )?;
        if changed == 0 {
            return Err(DbError::BacklogItemNotFound(id.to_string()));
        }
        refresh_parent_of(&tx, project_id, id)?;
        tx.commit()?;
        Ok(())
    }

    pub fn reorder_column_items(
        &self,
        project_id: i64,
        column: &str,
        item_ids: &[String],
    ) -> Result<(), DbError> {
        let column = board_column(column)?;
        let mut seen = HashSet::with_capacity(item_ids.len());
        for id in item_ids {
            if !seen.insert(id.as_str()) {
                return Err(DbError::InvalidBacklogReorder(format!(
                    "O card {id} está duplicado na reordenação."
                )));
            }
        }
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        ensure_project(&tx, project_id)?;
        if item_ids.is_empty() {
            tx.commit()?;
            return Ok(());
        }
        let mut positions = Vec::with_capacity(item_ids.len());
        for id in item_ids {
            let row = tx
                .query_row(
                    "SELECT \"column\", archived_at, position FROM backlog_items \
                     WHERE project_id = ?1 AND id = ?2",
                    params![project_id, id],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, Option<String>>(1)?,
                            row.get::<_, i64>(2)?,
                        ))
                    },
                )
                .optional()?
                .ok_or_else(|| DbError::BacklogItemNotFound(id.clone()))?;
            if row.1.is_some() || row.0 != column {
                return Err(DbError::InvalidBacklogReorder(format!(
                    "O card {id} não está ativo na coluna {column}."
                )));
            }
            positions.push(row.2);
        }
        // Preserva as posições existentes da coluna em ordem crescente para não reduzir MAX(position).
        positions.sort_unstable();
        let mut prev_pos = i64::MIN;
        for (id, pos) in item_ids.iter().zip(positions) {
            let target_pos = if pos <= prev_pos { prev_pos + 1 } else { pos };
            prev_pos = target_pos;
            tx.execute(
                "UPDATE backlog_items SET position = ?3 WHERE project_id = ?1 AND id = ?2",
                params![project_id, id, target_pos],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn set_item_priority(
        &self,
        project_id: i64,
        item_id: &str,
        priority: &str,
    ) -> Result<(), DbError> {
        let normalized = parse_priority(priority)?;
        let conn = self.conn()?;
        ensure_project(&conn, project_id)?;
        let changed = conn.execute(
            "UPDATE backlog_items SET priority = ?3 WHERE project_id = ?1 AND id = ?2",
            params![project_id, item_id, normalized],
        )?;
        if changed == 0 {
            return Err(DbError::BacklogItemNotFound(item_id.to_string()));
        }
        Ok(())
    }

    // Worktrees em uso por um líder, em todos os projetos: cards dele em In progress que já têm worktree.
    pub fn count_team_worktrees(&self, leader: &str) -> Result<i64, DbError> {
        let conn = self.conn()?;
        Ok(conn.query_row(
            "SELECT COUNT(*) FROM backlog_items \
             WHERE who = ?1 AND \"column\" = ?2 AND worktree_path IS NOT NULL AND archived_at IS NULL",
            params![leader, IN_PROGRESS_COLUMN],
            |row| row.get(0),
        )?)
    }

    // Card passa para In progress ou In review por ação do app (fim do fluxo, devolução); Done só por close_card.
    pub fn advance_card(&self, project_id: i64, id: &str, column: &str) -> Result<(), DbError> {
        if column != IN_PROGRESS_COLUMN && column != IN_REVIEW_COLUMN {
            return Err(DbError::ColumnNotMovable(column.to_string()));
        }
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        ensure_project(&tx, project_id)?;
        let changed = tx.execute(
            "UPDATE backlog_items SET \"column\" = ?3, \
                cycle_started_at = CASE WHEN ?3 = ?4 THEN CAST(unixepoch('subsec') * 1000 AS INTEGER) \
                    ELSE cycle_started_at END \
             WHERE project_id = ?1 AND id = ?2",
            params![project_id, id, column, IN_PROGRESS_COLUMN],
        )?;
        if changed == 0 {
            return Err(DbError::BacklogItemNotFound(id.to_string()));
        }
        refresh_parent_of(&tx, project_id, id)?;
        tx.commit()?;
        Ok(())
    }

    // Conclui o card: Done com quem concluiu (o usuário, ou a validação automática).
    pub fn close_card(&self, project_id: i64, id: &str, closed_by: &str) -> Result<(), DbError> {
        if !CLOSERS.contains(&closed_by) {
            return Err(DbError::InvalidClosedBy(closed_by.to_string()));
        }
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        ensure_project(&tx, project_id)?;
        let changed = tx.execute(
            "UPDATE backlog_items SET \"column\" = ?3, closed_by = ?4 WHERE project_id = ?1 AND id = ?2",
            params![project_id, id, DONE_COLUMN, closed_by],
        )?;
        if changed == 0 {
            return Err(DbError::BacklogItemNotFound(id.to_string()));
        }
        refresh_parent_of(&tx, project_id, id)?;
        tx.commit()?;
        Ok(())
    }

    // Reabre o card devolvido pelo usuário: volta a In progress, sem quem concluiu e com as tentativas zeradas.
    pub fn reopen_card(&self, project_id: i64, id: &str) -> Result<(), DbError> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        ensure_project(&tx, project_id)?;
        let changed = tx.execute(
            "UPDATE backlog_items SET \"column\" = ?3, closed_by = NULL, auto_attempts = 0, \
                cycle_started_at = CAST(unixepoch('subsec') * 1000 AS INTEGER) \
             WHERE project_id = ?1 AND id = ?2",
            params![project_id, id, IN_PROGRESS_COLUMN],
        )?;
        if changed == 0 {
            return Err(DbError::BacklogItemNotFound(id.to_string()));
        }
        refresh_parent_of(&tx, project_id, id)?;
        tx.commit()?;
        Ok(())
    }

    pub fn bump_auto_attempts(&self, project_id: i64, id: &str) -> Result<i64, DbError> {
        let conn = self.conn()?;
        ensure_project(&conn, project_id)?;
        conn.query_row(
            "UPDATE backlog_items SET auto_attempts = auto_attempts + 1 \
             WHERE project_id = ?1 AND id = ?2 RETURNING auto_attempts",
            params![project_id, id],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| DbError::BacklogItemNotFound(id.to_string()))
    }

    pub fn add_card_activity(&self, project_id: i64, id: &str, text: &str) -> Result<(), DbError> {
        let conn = self.conn()?;
        ensure_project(&conn, project_id)?;
        let mut card = find_card(&conn, project_id, id)?;
        card.activity.push(activity_entry(
            &card.id,
            card.activity.len() + 1,
            text.to_string(),
        ));
        conn.execute(
            "UPDATE backlog_items SET activity = ?3 WHERE project_id = ?1 AND id = ?2",
            params![project_id, id, to_json(id, "activity", &card.activity)?],
        )?;
        Ok(())
    }

    pub fn add_backlog_label(&self, project_id: i64, item_id: &str, label: &str) -> Result<(), DbError> {
        let label = normalized_label(label)?;
        let conn = self.conn()?;
        ensure_project(&conn, project_id)?;
        let item = find_card(&conn, project_id, item_id)?;
        validate_item(&item)?;
        if item.tags.iter().any(|tag| label_key(tag) == label) {
            return Err(DbError::DuplicateBacklogLabel(label));
        }
        let mut tags = item.tags;
        tags.push(label);
        write_tags(&conn, project_id, item_id, &tags)
    }

    pub fn remove_backlog_label(&self, project_id: i64, item_id: &str, label: &str) -> Result<(), DbError> {
        let label = label_key(label);
        let conn = self.conn()?;
        ensure_project(&conn, project_id)?;
        let item = find_card(&conn, project_id, item_id)?;
        validate_item(&item)?;
        if !item.tags.iter().any(|tag| label_key(tag) == label) {
            return Err(DbError::BacklogLabelNotFound(label));
        }
        let tags: Vec<String> = item
            .tags
            .into_iter()
            .filter(|tag| label_key(tag) != label)
            .collect();
        write_tags(&conn, project_id, item_id, &tags)
    }

    // Requester None é a interface do usuário, que tem permissão global como o Ember.
    pub fn set_backlog_responsible(
        &self,
        project_id: i64,
        item_id: &str,
        agent: &Agent,
        team: &Team,
        requester: Option<&Agent>,
    ) -> Result<CardOutcome, DbError> {
        if team.profile(agent).is_none() {
            return Err(DbError::ResponsibleOutsideTeam(agent.to_string()));
        }
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        ensure_project(&tx, project_id)?;
        let mut item = find_card(&tx, project_id, item_id)?;
        validate_item(&item)?;
        if item.who == agent.as_str() {
            return Ok(CardOutcome::Unchanged);
        }
        let unassigned = item.who == USER_OWNER || item.who == Agent::ember().as_str();
        if !unassigned && !may_reassign(requester, &item.who) {
            return Err(DbError::ResponsibleNotAllowed(item.id));
        }
        let text = responsible_change_text(team, requester, &item.who, agent, unassigned);
        item.activity
            .push(activity_entry(&item.id, item.activity.len() + 1, text));
        tx.execute(
            "UPDATE backlog_items SET who = ?3, activity = ?4 WHERE project_id = ?1 AND id = ?2",
            params![
                project_id,
                item_id,
                agent.as_str(),
                to_json(item_id, "activity", &item.activity)?,
            ],
        )?;
        tx.commit()?;
        Ok(CardOutcome::Updated)
    }

    pub fn archive_backlog_item(&self, project_id: i64, item_id: &str) -> Result<(), DbError> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        ensure_project(&tx, project_id)?;
        let changed = tx.execute(
            "UPDATE backlog_items SET archived_at = ?3 WHERE project_id = ?1 AND id = ?2 AND archived_at IS NULL",
            params![project_id, item_id, Local::now().to_rfc3339()],
        )?;
        if changed == 1 {
            refresh_parent_of(&tx, project_id, item_id)?;
            tx.commit()?;
            return Ok(());
        }
        ensure_item(&tx, project_id, item_id)?;
        Err(DbError::BacklogItemArchived)
    }

    pub fn restore_backlog_item(&self, project_id: i64, item_id: &str) -> Result<(), DbError> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        ensure_project(&tx, project_id)?;
        let changed = tx.execute(
            "UPDATE backlog_items SET archived_at = NULL WHERE project_id = ?1 AND id = ?2 AND archived_at IS NOT NULL",
            params![project_id, item_id],
        )?;
        if changed == 1 {
            refresh_parent_of(&tx, project_id, item_id)?;
            tx.commit()?;
            return Ok(());
        }
        ensure_item(&tx, project_id, item_id)?;
        Err(DbError::BacklogItemNotArchived)
    }

    pub fn delete_backlog_item(&self, project_id: i64, item_id: &str) -> Result<(), DbError> {
        self.delete_backlog_item_with_cascade(project_id, item_id, false)
    }

    pub fn delete_backlog_item_with_cascade(
        &self,
        project_id: i64,
        item_id: &str,
        cascade: bool,
    ) -> Result<(), DbError> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        ensure_project(&tx, project_id)?;
        let column: String = tx
            .query_row(
                "SELECT \"column\" FROM backlog_items WHERE project_id = ?1 AND id = ?2",
                params![project_id, item_id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| DbError::BacklogItemNotFound(item_id.to_string()))?;
        if column == IN_PROGRESS_COLUMN {
            return Err(DbError::BacklogDeleteInProgress);
        }
        let plan_id: Option<String> = tx
            .query_row(
                "SELECT plan_id FROM item_plans WHERE project_id = ?1 AND item_id = ?2",
                params![project_id, item_id],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(plan_id) = plan_id {
            if !cascade {
                return Err(DbError::BacklogDeleteWithPlan);
            }
            if !plan_id.is_empty() {
                tx.execute(
                    "DELETE FROM backlog_items WHERE project_id = ?1 AND (parent = ?2 OR id LIKE ?3)",
                    params![project_id, item_id, format!("{plan_id}.%")],
                )?;
            } else {
                tx.execute(
                    "DELETE FROM backlog_items WHERE project_id = ?1 AND parent = ?2",
                    params![project_id, item_id],
                )?;
            }
            tx.execute(
                "DELETE FROM item_plans WHERE project_id = ?1 AND item_id = ?2",
                params![project_id, item_id],
            )?;
            delete_artifact(&tx, project_id, item_id)?;
        }
        let parent = parent_of(&tx, project_id, item_id)?;
        tx.execute(
            "DELETE FROM backlog_items WHERE project_id = ?1 AND id = ?2",
            params![project_id, item_id],
        )?;
        if let Some(parent) = parent {
            refresh_children(&tx, project_id, &parent)?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn delete_plan_cascade(&self, project_id: i64, plan_id: &str) -> Result<(), DbError> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        ensure_project(&tx, project_id)?;
        let item_id: String = tx
            .query_row(
                "SELECT item_id FROM artifacts WHERE project_id = ?1 AND id = ?2",
                params![project_id, plan_id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| DbError::ArtifactNotFound(plan_id.to_string()))?;
        tx.execute(
            "DELETE FROM backlog_items WHERE project_id = ?1 AND id LIKE ?2",
            params![project_id, format!("{plan_id}.%")],
        )?;
        tx.execute(
            "DELETE FROM item_plans WHERE project_id = ?1 AND (plan_id = ?2 OR item_id = ?3)",
            params![project_id, plan_id, item_id],
        )?;
        delete_artifact(&tx, project_id, &item_id)?;
        refresh_children(&tx, project_id, &item_id)?;
        tx.commit()?;
        Ok(())
    }

    pub fn create_card(&self, project_id: i64, card: &NewCard) -> Result<BacklogItem, DbError> {
        let title = normalized_title(&card.title)?;
        let origin = user_origin(&card.origin)?;
        let text = if origin == ORIGIN_CONVERSATION {
            CONVERSATION_CREATED_TEXT
        } else {
            USER_CREATED_TEXT
        };
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        ensure_project(&tx, project_id)?;
        let (id, position) = next_card_id(&tx, project_id)?;
        let draft = CardDraft {
            title,
            kind: USER_CARD_TYPE,
            owner: USER_OWNER,
            description: normalized_description(card.description.as_deref()),
            origin,
            activity: vec![activity_entry(&id, 1, text.to_string())],
            links: card.links.clone(),
        };
        insert_card(&tx, project_id, &fresh_card(&id, draft), position)?;
        let stored = find_card(&tx, project_id, &id)?;
        tx.commit()?;
        Ok(stored)
    }

    pub fn apply_card_operations(
        &self,
        project_id: i64,
        actor: &AgentActor,
        operations: &[CardOperation],
    ) -> Result<Vec<CardResult>, DbError> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        ensure_project(&tx, project_id)?;
        let mut results = Vec::with_capacity(operations.len());
        for operation in operations {
            let result = match operation {
                CardOperation::Create {
                    title,
                    description,
                    responsible,
                } => create_agent_card(
                    &tx,
                    project_id,
                    actor,
                    title,
                    description.as_deref(),
                    responsible.as_ref(),
                )?,
                CardOperation::Move { id, column } => {
                    move_agent_card(&tx, project_id, actor, id, column)?
                }
                CardOperation::Update {
                    id,
                    title,
                    description,
                } => update_agent_card(
                    &tx,
                    project_id,
                    actor,
                    id,
                    title.as_deref(),
                    description.as_deref(),
                )?,
            };
            results.push(result);
        }
        tx.commit()?;
        Ok(results)
    }

    pub fn save_item_plan(
        &self,
        project_id: i64,
        item_id: &str,
        plan: &ItemPlanInput,
    ) -> Result<(), DbError> {
        validate_plan(plan)?;
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        ensure_project(&tx, project_id)?;
        write_item_plan(&tx, project_id, item_id, plan)?;
        tx.commit()?;
        Ok(())
    }

    // Edita o texto de um plano proposto ou aceito sem perder os critérios já marcados.
    // Plano aceito também sincroniza as etapas: existentes são atualizadas, novas inseridas e removidas arquivadas.
    pub fn update_item_plan(
        &self,
        project_id: i64,
        item_id: &str,
        proposal: &str,
        expected_updated_at: Option<i64>,
    ) -> Result<Artifact, DbError> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        ensure_project(&tx, project_id)?;
        let item = find_card(&tx, project_id, item_id)?;
        let saved = tx
            .query_row(
                "SELECT state, plan_id, agent FROM item_plans WHERE project_id = ?1 AND item_id = ?2",
                params![project_id, item_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                },
            )
            .optional()?;
        let Some((state, linked_id, agent)) = saved else {
            return Err(DbError::PlanNotFound);
        };
        let own_plan = linked_id.as_deref().is_none_or(|linked| linked == item_id);
        if !own_plan || (state != PROPOSED_PLAN && state != LINKED_PLAN) {
            return Err(DbError::PlanNotFound);
        }
        if is_blank(proposal) {
            return Err(DbError::InvalidBacklogPlan(PROPOSAL_REQUIRED));
        }
        let draft = parse_proposal(proposal);
        if draft.lots.is_empty() {
            return Err(DbError::PlanWithoutStages);
        }
        let plan_id = artifact_id(item_id);
        if let (Some(expected), Some(current)) =
            (expected_updated_at, artifact_updated_at(&tx, project_id, &plan_id)?)
        {
            if expected != current {
                return Err(DbError::ArtifactConflict(plan_id));
            }
        }
        tx.execute(
            "UPDATE item_plans SET text = ?3 WHERE project_id = ?1 AND item_id = ?2",
            params![project_id, item_id, proposal],
        )?;
        upsert_artifact_row(&tx, project_id, item_id, &item.title, proposal, agent.as_deref(), &draft)?;
        upsert_plan_criteria(&tx, project_id, &plan_id, &draft, true)?;
        if state == LINKED_PLAN {
            validate_item(&item)?;
            sync_plan_cards(&tx, project_id, &item, agent.as_deref(), &draft)?;
        }
        let artifact = read_plan_artifact(&tx, project_id, &plan_id)?.ok_or(DbError::PlanNotFound)?;
        tx.commit()?;
        Ok(artifact)
    }

    // Card e plano proposto nascem na mesma transação: plano recusado não deixa card órfão.
    pub fn import_plan(
        &self,
        project_id: i64,
        title: &str,
        proposal: &str,
        author: Option<&str>,
    ) -> Result<BacklogItem, DbError> {
        let title = normalized_title(title)?;
        if is_blank(proposal) {
            return Err(DbError::InvalidBacklogPlan(PROPOSAL_REQUIRED));
        }
        if parse_proposal(proposal).lots.is_empty() {
            return Err(DbError::PlanWithoutStages);
        }
        let plan = ItemPlanInput {
            state: PROPOSED_PLAN.to_string(),
            proposal: Some(proposal.to_string()),
            plan: None,
            agent: author.map(str::to_string),
        };
        validate_plan(&plan)?;
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        ensure_project(&tx, project_id)?;
        let (id, position) = next_card_id(&tx, project_id)?;
        let draft = CardDraft {
            title,
            kind: USER_CARD_TYPE,
            owner: USER_OWNER,
            description: None,
            origin: ORIGIN_USER,
            activity: vec![activity_entry(&id, 1, PLAN_IMPORTED_TEXT.to_string())],
            links: Vec::new(),
        };
        insert_card(&tx, project_id, &fresh_card(&id, draft), position)?;
        write_item_plan(&tx, project_id, &id, &plan)?;
        let stored = find_card(&tx, project_id, &id)?;
        tx.commit()?;
        Ok(stored)
    }
}

fn write_item_plan(
    conn: &Connection,
    project_id: i64,
    item_id: &str,
    plan: &ItemPlanInput,
) -> Result<(), DbError> {
    let item = find_card(conn, project_id, item_id)?;
    if plan.state == EMPTY_PLAN {
        let previous: Option<String> = conn
            .query_row(
                "SELECT state FROM item_plans WHERE project_id = ?1 AND item_id = ?2",
                params![project_id, item_id],
                |row| row.get(0),
            )
            .optional()?;
        conn.execute(
            "DELETE FROM item_plans WHERE project_id = ?1 AND item_id = ?2",
            params![project_id, item_id],
        )?;
        // Descartar a proposta apaga o plano; desvincular um plano aceito o mantém (D3).
        if previous.as_deref() == Some(PROPOSED_PLAN) {
            delete_artifact(conn, project_id, item_id)?;
        }
        return Ok(());
    }
    let linked = plan.plan.as_ref();
    conn.execute(
        UPSERT_PLAN_SQL,
        params![
            project_id,
            item_id,
            plan.state,
            plan.proposal,
            linked.map(|linked| linked.id.as_str()),
            linked.map(|linked| linked.name.as_str()),
            linked.map(|linked| linked.meta.as_str()),
            plan.agent,
        ],
    )?;
    let proposal = plan.proposal.as_deref().filter(|text| !is_blank(text));
    let own_plan = linked.is_none_or(|linked| linked.id == item_id);
    if let (Some(text), true) = (proposal, own_plan) {
        let draft = parse_proposal(text);
        let author = plan.agent.as_deref();
        if plan.state == PROPOSED_PLAN {
            replace_artifact(conn, project_id, item_id, &item.title, text, author, &draft)?;
        } else {
            validate_item(&item)?;
            if !artifact_exists(conn, project_id, item_id)? {
                replace_artifact(conn, project_id, item_id, &item.title, text, author, &draft)?;
            }
            create_plan_cards(conn, project_id, &item, author, &draft)?;
        }
    }
    Ok(())
}

fn is_blank(text: &str) -> bool {
    text.trim().is_empty()
}

fn is_complete_plan(plan: &LinkedPlan) -> bool {
    !is_blank(&plan.id) && !is_blank(&plan.name) && !is_blank(&plan.meta)
}

fn validate_item(item: &BacklogItem) -> Result<(), DbError> {
    if item.error.is_some() {
        return Err(DbError::BacklogItemData {
            item_id: item.id.clone(),
            reason: ITEM_WITH_ERROR.to_string(),
        });
    }
    if !ORIGINS.contains(&item.origin.as_str()) {
        return Err(DbError::InvalidBacklogOrigin(item.origin.clone()));
    }
    if !ITEM_TYPES.contains(&item.kind.as_str()) {
        return Err(DbError::InvalidBacklogType(item.kind.clone()));
    }
    if !ITEM_STATES.contains(&item.state.as_str()) {
        return Err(DbError::InvalidBacklogState(item.state.clone()));
    }
    if let Some(column) = item.column.as_deref() {
        if !BOARD_COLUMNS.contains(&column) {
            return Err(DbError::InvalidBacklogColumn(column.to_string()));
        }
    }
    if let Some(task) = item
        .micro
        .iter()
        .find(|task| !MICRO_TASK_STATUSES.contains(&task.status.as_str()))
    {
        return Err(DbError::InvalidMicroTaskStatus(task.status.clone()));
    }
    Ok(())
}

fn validate_plan(plan: &ItemPlanInput) -> Result<(), DbError> {
    if plan.state == EMPTY_PLAN {
        return Ok(());
    }
    if let Some(agent) = plan.agent.as_deref() {
        if Agent::from_stored(agent).is_none() {
            return Err(DbError::InvalidPlanAgent(agent.to_string()));
        }
    }
    match plan.state.as_str() {
        PROPOSED_PLAN if plan.proposal.as_deref().is_none_or(is_blank) => {
            Err(DbError::InvalidBacklogPlan(PROPOSAL_REQUIRED))
        }
        PROPOSED_PLAN => Ok(()),
        LINKED_PLAN if !plan.plan.as_ref().is_some_and(is_complete_plan) => {
            Err(DbError::InvalidBacklogPlan(LINKED_PLAN_REQUIRED))
        }
        LINKED_PLAN => Ok(()),
        _ => Err(DbError::InvalidPlanState(plan.state.clone())),
    }
}

pub(super) fn ensure_project(conn: &Connection, project_id: i64) -> Result<(), DbError> {
    conn.query_row("SELECT 1 FROM projects WHERE id = ?1", [project_id], |_| {
        Ok(())
    })
    .optional()?
    .ok_or(DbError::ProjectNotFound(project_id))
}

fn ensure_item(conn: &Connection, project_id: i64, item_id: &str) -> Result<(), DbError> {
    conn.query_row(
        "SELECT 1 FROM backlog_items WHERE project_id = ?1 AND id = ?2",
        params![project_id, item_id],
        |_| Ok(()),
    )
    .optional()?
    .ok_or_else(|| DbError::BacklogItemNotFound(item_id.to_string()))
}

// Mesma regra do normalizeTag do front: espaços colapsados e minúsculas, para comparar etiquetas.
fn label_key(raw: &str) -> String {
    raw.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn normalized_label(raw: &str) -> Result<String, DbError> {
    let label = label_key(raw);
    if (1..=BACKLOG_LABEL_MAX_CHARS).contains(&label.chars().count()) {
        Ok(label)
    } else {
        Err(DbError::BacklogLabelLength)
    }
}

fn write_tags(conn: &Connection, project_id: i64, item_id: &str, tags: &[String]) -> Result<(), DbError> {
    let json = to_json(item_id, "tags", &tags)?;
    conn.execute(
        "UPDATE backlog_items SET tags = ?3 WHERE project_id = ?1 AND id = ?2",
        params![project_id, item_id, json],
    )?;
    Ok(())
}

fn default_origin() -> String {
    ORIGIN_USER.to_string()
}

fn origin_from_row(row: &Row<'_>) -> rusqlite::Result<String> {
    let origin: String = row.get("origin")?;
    Ok(if ORIGINS.contains(&origin.as_str()) {
        origin
    } else {
        ORIGIN_USER.to_string()
    })
}

fn normalized_title(raw: &str) -> Result<String, DbError> {
    let title = raw.trim();
    if title.is_empty() {
        return Err(DbError::EmptyCardTitle);
    }
    Ok(title.to_string())
}

fn normalized_description(description: Option<&str>) -> Option<String> {
    description
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
}

fn user_origin(origin: &str) -> Result<&'static str, DbError> {
    match origin {
        ORIGIN_USER => Ok(ORIGIN_USER),
        ORIGIN_CONVERSATION => Ok(ORIGIN_CONVERSATION),
        ORIGIN_AGENT => Err(DbError::AgentOriginNotAllowed),
        other => Err(DbError::InvalidBacklogOrigin(other.to_string())),
    }
}

pub fn board_column(requested: &str) -> Result<&'static str, DbError> {
    BOARD_COLUMNS
        .iter()
        .copied()
        .find(|column| column.eq_ignore_ascii_case(requested))
        .ok_or_else(|| DbError::InvalidBacklogColumn(requested.to_string()))
}

fn parse_priority(raw: &str) -> Result<Option<&'static str>, DbError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let lowered = trimmed.to_lowercase();
    BACKLOG_PRIORITIES
        .iter()
        .copied()
        .find(|allowed| allowed.to_lowercase() == lowered)
        .map(Some)
        .ok_or_else(|| DbError::InvalidBacklogPriority(raw.to_string()))
}

fn next_card_id(conn: &Connection, project_id: i64) -> Result<(String, i64), DbError> {
    let position: i64 = conn.query_row(
        "SELECT COALESCE(MAX(position), 0) + 1 FROM backlog_items WHERE project_id = ?1",
        [project_id],
        |row| row.get(0),
    )?;
    Ok((format!("{CARD_ID_PREFIX}{position}"), position))
}

fn ensure_title_free(conn: &Connection, project_id: i64, title: &str) -> Result<(), DbError> {
    let wanted = title.trim().to_lowercase();
    let mut statement = conn.prepare(
        "SELECT id, title FROM backlog_items WHERE project_id = ?1 AND \"column\" <> ?2",
    )?;
    let rows = statement.query_map(params![project_id, DONE_COLUMN], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    for row in rows {
        let (id, existing) = row?;
        if existing.trim().to_lowercase() == wanted {
            return Err(DbError::DuplicateCardTitle(id));
        }
    }
    Ok(())
}

fn activity_entry(card_id: &str, number: usize, text: String) -> Activity {
    Activity {
        id: format!("{card_id}-{number}"),
        text,
        at: Local::now().format(ACTIVITY_TIME_FORMAT).to_string(),
    }
}

fn fresh_card(id: &str, draft: CardDraft<'_>) -> BacklogItem {
    BacklogItem {
        id: id.to_string(),
        title: draft.title,
        kind: draft.kind.to_string(),
        state: NEW_CARD_STATE.to_string(),
        stage: None,
        triage_step: None,
        parent: None,
        feature: None,
        tags: Vec::new(),
        description: draft.description,
        plan: None,
        plan_note: None,
        children: None,
        who: draft.owner.to_string(),
        by: draft.owner.to_string(),
        is_new: Some(true),
        column: Some(DEFAULT_COLUMN.to_string()),
        priority: None,
        estimate: None,
        questions: Vec::new(),
        micro: Vec::new(),
        criteria: Vec::new(),
        activity: draft.activity,
        links: draft.links,
        error: None,
        origin: draft.origin.to_string(),
        branch: None,
        worktree_path: None,
        closed_by: None,
        auto_attempts: 0,
    }
}

fn insert_card(
    conn: &Connection,
    project_id: i64,
    item: &BacklogItem,
    position: i64,
) -> Result<(), DbError> {
    conn.execute(
        INSERT_CARD_SQL,
        params![
            project_id,
            item.id,
            item.title,
            item.kind,
            item.state,
            item.is_new,
            item.column.as_deref().unwrap_or(DEFAULT_COLUMN),
            item.who,
            item.by,
            item.description,
            item.origin,
            to_json(&item.id, "activity", &item.activity)?,
            to_json(&item.id, "links", &item.links)?,
            position,
        ],
    )?;
    Ok(())
}

// Ids derivados do plano tornam a criação idempotente: aceitar de novo não duplica nem altera etapas.
fn create_plan_cards(
    conn: &Connection,
    project_id: i64,
    item: &BacklogItem,
    author: Option<&str>,
    draft: &PlanDraft,
) -> Result<(), DbError> {
    let plan_id = artifact_id(&item.id);
    let author = author.unwrap_or(item.who.as_str());
    for lot in &draft.lots {
        let stage = PlanCard {
            id: format!("{plan_id}.{}", lot.n),
            parent: &item.id,
            title: &lot.title,
            description: Some(lot.meta.as_str()).filter(|meta| !meta.is_empty()),
        };
        insert_plan_card(conn, project_id, item, author, &stage)?;
        for (index, task) in lot.tasks.iter().enumerate() {
            let subtask = PlanCard {
                id: format!("{}.{}", stage.id, index + 1),
                parent: &stage.id,
                title: task,
                description: None,
            };
            insert_plan_card(conn, project_id, item, author, &subtask)?;
        }
        refresh_children(conn, project_id, &stage.id)?;
    }
    refresh_children(conn, project_id, &item.id)?;
    Ok(())
}

struct StoredPlanCard {
    id: String,
    title: String,
    column: String,
    archived: bool,
}

fn plan_children(conn: &Connection, project_id: i64, parent: &str) -> Result<Vec<StoredPlanCard>, DbError> {
    let mut statement = conn.prepare(
        "SELECT id, title, \"column\", archived_at IS NOT NULL FROM backlog_items WHERE project_id = ?1 AND parent = ?2",
    )?;
    let rows = statement.query_map(params![project_id, parent], |row| {
        Ok(StoredPlanCard {
            id: row.get(0)?,
            title: row.get(1)?,
            column: row.get(2)?,
            archived: row.get(3)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

// Só Backlog e Ready ainda não começaram; daí em diante a etapa tem trabalho feito e não sai do plano.
fn is_unstarted(column: &str) -> bool {
    column == DEFAULT_COLUMN || column == READY_COLUMN
}

fn is_numbered_child(id: &str, prefix: &str) -> bool {
    id.strip_prefix(prefix)
        .is_some_and(|rest| !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()))
}

fn archive_plan_card(conn: &Connection, project_id: i64, card: &StoredPlanCard) -> Result<(), DbError> {
    let children = plan_children(conn, project_id, &card.id)?;
    let started = std::iter::once(card)
        .chain(children.iter().filter(|child| !child.archived))
        .find(|candidate| !is_unstarted(&candidate.column));
    if let Some(started) = started {
        return Err(DbError::PlanStageStarted(started.id.clone()));
    }
    let now = Local::now().to_rfc3339();
    for id in std::iter::once(&card.id).chain(children.iter().map(|child| &child.id)) {
        conn.execute(
            "UPDATE backlog_items SET archived_at = ?3 WHERE project_id = ?1 AND id = ?2 AND archived_at IS NULL",
            params![project_id, id, now],
        )?;
    }
    Ok(())
}

// Mesmo id, mesma posição: título e descrição seguem o plano; o que sobra no banco é arquivado ou recusado.
fn sync_plan_level(
    conn: &Connection,
    project_id: i64,
    item: &BacklogItem,
    author: &str,
    parent: &str,
    prefix: &str,
    wanted: &[PlanCard<'_>],
) -> Result<(), DbError> {
    let stored = plan_children(conn, project_id, parent)?;
    for card in wanted {
        if stored.iter().any(|existing| existing.id == card.id) {
            conn.execute(
                "UPDATE backlog_items SET title = ?3, description = ?4, archived_at = NULL \
                 WHERE project_id = ?1 AND id = ?2",
                params![project_id, card.id, card.title, card.description],
            )?;
        } else {
            insert_plan_card(conn, project_id, item, author, card)?;
        }
    }
    for existing in stored
        .iter()
        .filter(|existing| !existing.archived && is_numbered_child(&existing.id, prefix))
        .filter(|existing| !wanted.iter().any(|card| card.id == existing.id))
    {
        archive_plan_card(conn, project_id, existing)?;
    }
    refresh_children(conn, project_id, parent)?;
    Ok(())
}

// Cada item novo (lote ou tarefa) ganha o id de um card existente sob o mesmo prefixo: primeiro o de mesmo título; depois,
// para o que mudou de título, o card da mesma posição (só se nada saiu, ou se esse card ainda não começou); o resto recebe
// um id novo. O card que sobra sem item é o removido, e quem decide entre arquivar e recusar é sync_plan_level.
fn assign_child_ids(prefix: &str, stored: &[StoredPlanCard], titles: &[&str]) -> Vec<String> {
    let owned: Vec<&StoredPlanCard> = stored.iter().filter(|card| is_numbered_child(&card.id, prefix)).collect();
    let mut assigned: Vec<Option<&str>> = vec![None; titles.len()];
    let is_free = |assigned: &[Option<&str>], id: &str| !assigned.contains(&Some(id));
    for (index, title) in titles.iter().enumerate() {
        let same_title = owned
            .iter()
            .filter(|card| is_free(&assigned, &card.id) && card.title.trim().eq_ignore_ascii_case(title.trim()))
            .min_by_key(|card| card.archived);
        assigned[index] = same_title.map(|card| card.id.as_str());
    }
    let removed = titles.len() < owned.iter().filter(|card| !card.archived).count();
    for index in 0..titles.len() {
        if assigned[index].is_some() {
            continue;
        }
        let position = format!("{prefix}{}", index + 1);
        let same_position = owned
            .iter()
            .find(|card| card.id == position && is_free(&assigned, &card.id))
            .filter(|card| !removed || is_unstarted(&card.column));
        assigned[index] = same_position.map(|card| card.id.as_str());
    }
    let mut last = owned
        .iter()
        .filter_map(|card| card.id.strip_prefix(prefix)?.parse::<u32>().ok())
        .max()
        .unwrap_or(0);
    assigned
        .into_iter()
        .map(|id| match id {
            Some(id) => id.to_string(),
            None => {
                last += 1;
                format!("{prefix}{last}")
            }
        })
        .collect()
}

fn sync_plan_cards(
    conn: &Connection,
    project_id: i64,
    item: &BacklogItem,
    author: Option<&str>,
    draft: &PlanDraft,
) -> Result<(), DbError> {
    let plan_id = artifact_id(&item.id);
    let author = author.unwrap_or(item.who.as_str());
    let stored = plan_children(conn, project_id, &item.id)?;
    let stage_prefix = format!("{plan_id}.");
    let stage_titles: Vec<&str> = draft.lots.iter().map(|lot| lot.title.as_str()).collect();
    let stage_ids = assign_child_ids(&stage_prefix, &stored, &stage_titles);
    let stages: Vec<PlanCard<'_>> = draft
        .lots
        .iter()
        .zip(stage_ids)
        .map(|(lot, id)| PlanCard {
            id,
            parent: &item.id,
            title: &lot.title,
            description: Some(lot.meta.as_str()).filter(|meta| !meta.is_empty()),
        })
        .collect();
    sync_plan_level(conn, project_id, item, author, &item.id, &stage_prefix, &stages)?;
    for (stage, lot) in stages.iter().zip(&draft.lots) {
        let task_prefix = format!("{}.", stage.id);
        let task_titles: Vec<&str> = lot.tasks.iter().map(String::as_str).collect();
        let stored_tasks = plan_children(conn, project_id, &stage.id)?;
        let task_ids = assign_child_ids(&task_prefix, &stored_tasks, &task_titles);
        let subtasks: Vec<PlanCard<'_>> = lot
            .tasks
            .iter()
            .zip(task_ids)
            .map(|(task, id)| PlanCard {
                id,
                parent: &stage.id,
                title: task,
                description: None,
            })
            .collect();
        sync_plan_level(conn, project_id, item, author, &stage.id, &task_prefix, &subtasks)?;
    }
    Ok(())
}

fn insert_plan_card(
    conn: &Connection,
    project_id: i64,
    item: &BacklogItem,
    author: &str,
    card: &PlanCard<'_>,
) -> Result<(), DbError> {
    let activity = vec![activity_entry(
        &card.id,
        1,
        format!("{PLAN_CARD_CREATED_TEXT} {}.", item.id),
    )];
    conn.execute(
        INSERT_PLAN_CARD_SQL,
        params![
            project_id,
            card.id,
            card.title,
            item.kind,
            NEW_CARD_STATE,
            card.parent,
            true,
            DEFAULT_COLUMN,
            item.who,
            author,
            card.description,
            ORIGIN_AGENT,
            to_json(&card.id, "activity", &activity)?,
        ],
    )?;
    Ok(())
}

pub(super) fn refresh_children(conn: &Connection, project_id: i64, parent_id: &str) -> rusqlite::Result<()> {
    conn.execute(REFRESH_CHILDREN_SQL, params![project_id, parent_id, DONE_COLUMN])?;
    Ok(())
}

fn parent_of(conn: &Connection, project_id: i64, id: &str) -> Result<Option<String>, DbError> {
    Ok(conn
        .query_row(
            "SELECT parent FROM backlog_items WHERE project_id = ?1 AND id = ?2",
            params![project_id, id],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()?
        .flatten())
}

fn refresh_parent_of(conn: &Connection, project_id: i64, id: &str) -> Result<(), DbError> {
    if let Some(parent) = parent_of(conn, project_id, id)? {
        refresh_children(conn, project_id, &parent)?;
    }
    Ok(())
}

fn find_card(conn: &Connection, project_id: i64, id: &str) -> Result<BacklogItem, DbError> {
    conn.query_row(
        &format!("SELECT {ITEM_COLUMNS} FROM backlog_items WHERE project_id = ?1 AND id = ?2"),
        params![project_id, id],
        read_item,
    )
    .optional()?
    .ok_or_else(|| DbError::BacklogItemNotFound(id.to_string()))
}

fn create_agent_card(
    conn: &Connection,
    project_id: i64,
    actor: &AgentActor,
    title: &str,
    description: Option<&str>,
    responsible: Option<&CardResponsible>,
) -> Result<CardResult, DbError> {
    let title = normalized_title(title)?;
    ensure_title_free(conn, project_id, &title)?;
    let (id, position) = next_card_id(conn, project_id)?;
    let (owner, created) = match responsible {
        Some(responsible) => (
            responsible.leader.as_str(),
            format!(
                "{} criou o card para o time {} (líder {}).",
                actor.name, responsible.team_name, responsible.leader_name
            ),
        ),
        None => (
            actor.id.as_str(),
            format!("{} criou o card na conversa.", actor.name),
        ),
    };
    let draft = CardDraft {
        title: title.clone(),
        kind: AGENT_CARD_TYPE,
        owner,
        description: normalized_description(description),
        origin: ORIGIN_AGENT,
        activity: vec![activity_entry(&id, 1, created)],
        links: Vec::new(),
    };
    let mut item = fresh_card(&id, draft);
    item.state = TRIAGE_CARD_STATE.to_string();
    insert_card(conn, project_id, &item, position)?;
    Ok(CardResult {
        id,
        title,
        column: DEFAULT_COLUMN.to_string(),
        outcome: CardOutcome::Created,
    })
}

fn move_agent_card(
    conn: &Connection,
    project_id: i64,
    actor: &AgentActor,
    id: &str,
    requested_column: &str,
) -> Result<CardResult, DbError> {
    let column = board_column(requested_column)?;
    if AGENT_FORBIDDEN_COLUMNS.contains(&column) {
        return Err(DbError::AgentForbiddenColumn(column.to_string()));
    }
    let mut card = find_card(conn, project_id, id)?;
    validate_item(&card)?;
    let current = card
        .column
        .clone()
        .unwrap_or_else(|| DEFAULT_COLUMN.to_string());
    ensure_agent_can_change(&card, &current, actor)?;
    if current == column {
        return Ok(CardResult {
            id: card.id,
            title: card.title,
            column: current,
            outcome: CardOutcome::Unchanged,
        });
    }
    let text = format!("{} moveu de {current} para {column}.", actor.name);
    card.activity
        .push(activity_entry(&card.id, card.activity.len() + 1, text));
    conn.execute(
        "UPDATE backlog_items SET \"column\" = ?3, activity = ?4, \
            cycle_started_at = CASE WHEN ?3 = ?5 THEN CAST(unixepoch('subsec') * 1000 AS INTEGER) \
                ELSE cycle_started_at END \
         WHERE project_id = ?1 AND id = ?2",
        params![
            project_id,
            card.id,
            column,
            to_json(&card.id, "activity", &card.activity)?,
            IN_PROGRESS_COLUMN,
        ],
    )?;
    Ok(CardResult {
        id: card.id,
        title: card.title,
        column: column.to_string(),
        outcome: CardOutcome::Moved,
    })
}

// Card fechado ou de outro agente não muda por ferramenta; vale para mover e para editar.
fn ensure_agent_can_change(
    card: &BacklogItem,
    current_column: &str,
    actor: &AgentActor,
) -> Result<(), DbError> {
    if current_column == DONE_COLUMN {
        return Err(DbError::CardClosed(card.id.clone()));
    }
    // O Ember orquestra o projeto: move e edita cards de qualquer agente. Continua nas colunas que a ferramenta
    // já libera e fora de Done; a atividade do card guarda quem fez a ação.
    let owned =
        actor.id == Agent::ember()
            || card.who == actor.id.as_str()
            || actor.current_card.as_deref() == Some(card.id.as_str());
    if !owned {
        return Err(DbError::CardNotOwned(card.id.clone()));
    }
    Ok(())
}

// Trocar um responsável já definido cabe ao Ember ou ao próprio responsável atual.
fn may_reassign(requester: Option<&Agent>, current: &str) -> bool {
    match requester {
        None => true,
        Some(requester) => *requester == Agent::ember() || requester.as_str() == current,
    }
}

fn responsible_change_text(
    team: &Team,
    requester: Option<&Agent>,
    previous: &str,
    next: &Agent,
    unassigned: bool,
) -> String {
    let actor = match requester {
        Some(requester) => team.name_of(requester),
        None => USER_ACTOR_NAME,
    };
    let next = team.name_of(next);
    if unassigned {
        return format!("{actor} definiu o responsável como {next}.");
    }
    let previous = match Agent::from_stored(previous) {
        Some(agent) => team.name_of(&agent).to_string(),
        None => previous.to_string(),
    };
    format!("{actor} trocou o responsável de {previous} para {next}.")
}

// Descrição vazia não apaga a atual; título só muda para um que nenhum card aberto use.
fn update_agent_card(
    conn: &Connection,
    project_id: i64,
    actor: &AgentActor,
    id: &str,
    title: Option<&str>,
    description: Option<&str>,
) -> Result<CardResult, DbError> {
    let mut card = find_card(conn, project_id, id)?;
    validate_item(&card)?;
    let current = card
        .column
        .clone()
        .unwrap_or_else(|| DEFAULT_COLUMN.to_string());
    if current == READY_COLUMN {
        return Err(DbError::CardInReady(card.id));
    }
    ensure_agent_can_change(&card, &current, actor)?;
    let new_title = title.map(normalized_title).transpose()?;
    let new_description = normalized_description(description);
    let title_changes = new_title.as_ref().is_some_and(|new| *new != card.title);
    let description_changes =
        new_description.is_some() && new_description.as_deref() != card.description.as_deref();
    if title_changes {
        let new = new_title.as_deref().unwrap_or_default();
        if new.to_lowercase() != card.title.trim().to_lowercase() {
            ensure_title_free(conn, project_id, new)?;
        }
    }
    if !title_changes && !description_changes {
        return Ok(CardResult {
            id: card.id,
            title: card.title,
            column: current,
            outcome: CardOutcome::Unchanged,
        });
    }
    let mut edited = Vec::new();
    if title_changes {
        card.title = new_title.unwrap_or_default();
        edited.push("o título");
    }
    if description_changes {
        card.description = new_description;
        edited.push("a descrição");
    }
    let text = format!("{} editou {}.", actor.name, edited.join(" e "));
    card.activity
        .push(activity_entry(&card.id, card.activity.len() + 1, text));
    conn.execute(
        "UPDATE backlog_items SET title = ?3, description = ?4, activity = ?5 WHERE project_id = ?1 AND id = ?2",
        params![
            project_id,
            card.id,
            card.title,
            card.description,
            to_json(&card.id, "activity", &card.activity)?,
        ],
    )?;
    Ok(CardResult {
        id: card.id,
        title: card.title,
        column: current,
        outcome: CardOutcome::Updated,
    })
}

fn to_json<T: Serialize>(item_id: &str, field: &str, value: &T) -> Result<String, DbError> {
    serde_json::to_string(value).map_err(|error| DbError::BacklogItemData {
        item_id: item_id.to_string(),
        reason: format!("{field}: {error}"),
    })
}

fn json_column<T: DeserializeOwned>(
    row: &Row<'_>,
    column: &str,
    item_id: &str,
) -> Result<T, DbError> {
    let raw: String = row.get(column)?;
    serde_json::from_str(&raw).map_err(|error| DbError::BacklogItemData {
        item_id: item_id.to_string(),
        reason: format!("{column}: {error}"),
    })
}

fn upsert_item(conn: &Connection, project_id: i64, item: &BacklogItem) -> Result<(), DbError> {
    let children = item.children.as_ref();
    conn.execute(
        UPSERT_ITEM_SQL,
        params![
            project_id,
            item.id,
            item.title,
            item.kind,
            item.state,
            item.stage,
            item.triage_step,
            item.parent,
            item.feature,
            item.description,
            item.plan,
            item.who,
            item.by,
            item.is_new,
            item.column.as_deref().unwrap_or(DEFAULT_COLUMN),
            item.priority,
            item.estimate,
            to_json(&item.id, "tags", &item.tags)?,
            to_json(&item.id, "tasks", &item.micro)?,
            to_json(&item.id, "criteria", &item.criteria)?,
            to_json(&item.id, "questions", &item.questions)?,
            to_json(&item.id, "links", &item.links)?,
            to_json(&item.id, "activity", &item.activity)?,
            children.map(|children| children.total),
            children.map(|children| children.done),
            item.origin,
            normalized_description(item.plan_note.as_deref()),
        ],
    )?;
    Ok(())
}

fn read_items(conn: &Connection, project_id: i64) -> Result<Vec<BacklogItem>, DbError> {
    let mut statement = conn.prepare(&format!(
        "SELECT {ITEM_COLUMNS} FROM backlog_items WHERE project_id = ?1 AND archived_at IS NULL ORDER BY position"
    ))?;
    let rows = statement.query_map([project_id], read_item)?;
    rows.collect::<rusqlite::Result<Vec<BacklogItem>>>()
        .map_err(DbError::from)
}

// Um item ilegível não derruba a lista: volta marcado com a causa, sem os campos que não deu para ler.
fn read_item(row: &Row<'_>) -> rusqlite::Result<BacklogItem> {
    match item_from_row(row) {
        Ok(item) => Ok(item),
        Err(DbError::Sqlite(error)) => Err(error),
        Err(error) => broken_item_from_row(row, error),
    }
}

fn broken_item_from_row(row: &Row<'_>, error: DbError) -> rusqlite::Result<BacklogItem> {
    let kind: String = row.get("type")?;
    let state: String = row.get("state")?;
    let column: String = row.get("column")?;
    Ok(BacklogItem {
        id: row.get("id")?,
        title: row.get("title")?,
        kind: if ITEM_TYPES.contains(&kind.as_str()) {
            kind
        } else {
            FALLBACK_ITEM_TYPE.to_string()
        },
        state: if ITEM_STATES.contains(&state.as_str()) {
            state
        } else {
            FALLBACK_ITEM_STATE.to_string()
        },
        stage: None,
        triage_step: None,
        parent: None,
        feature: None,
        tags: Vec::new(),
        description: None,
        plan: None,
        plan_note: row.get("plan_note")?,
        children: None,
        who: row.get("who")?,
        by: row.get("by")?,
        origin: origin_from_row(row)?,
        branch: row.get("branch")?,
        worktree_path: row.get("worktree_path")?,
        closed_by: row.get("closed_by")?,
        auto_attempts: row.get("auto_attempts")?,
        is_new: None,
        column: BOARD_COLUMNS.contains(&column.as_str()).then_some(column),
        priority: None,
        estimate: None,
        questions: Vec::new(),
        micro: Vec::new(),
        criteria: Vec::new(),
        activity: Vec::new(),
        links: Vec::new(),
        error: Some(ItemError {
            reason: error.to_string(),
        }),
    })
}

fn item_from_row(row: &Row<'_>) -> Result<BacklogItem, DbError> {
    let id: String = row.get("id")?;
    let children = match (
        row.get::<_, Option<u32>>("children_total")?,
        row.get::<_, Option<u32>>("children_done")?,
    ) {
        (Some(total), Some(done)) => Some(Children { total, done }),
        (None, None) => None,
        _ => {
            return Err(DbError::BacklogItemData {
                item_id: id.clone(),
                reason: CHILDREN_INCOMPLETE.to_string(),
            })
        }
    };
    let item = BacklogItem {
        title: row.get("title")?,
        kind: row.get("type")?,
        state: row.get("state")?,
        stage: row.get("stage")?,
        triage_step: row.get("triage_step")?,
        parent: row.get("parent")?,
        feature: row.get("feature")?,
        tags: json_column(row, "tags", &id)?,
        description: row.get("description")?,
        plan: row.get("plan")?,
        plan_note: row.get("plan_note")?,
        children,
        who: row.get("who")?,
        by: row.get("by")?,
        is_new: row.get("is_new")?,
        column: Some(row.get("column")?),
        priority: row.get("priority")?,
        estimate: row.get("estimate")?,
        questions: json_column(row, "questions", &id)?,
        micro: json_column(row, "tasks", &id)?,
        criteria: json_column(row, "criteria", &id)?,
        activity: json_column(row, "activity", &id)?,
        links: json_column(row, "links", &id)?,
        origin: row.get("origin")?,
        branch: row.get("branch")?,
        worktree_path: row.get("worktree_path")?,
        closed_by: row.get("closed_by")?,
        auto_attempts: row.get("auto_attempts")?,
        error: None,
        id,
    };
    validate_item(&item)?;
    Ok(item)
}

fn read_plans(conn: &Connection, project_id: i64) -> Result<Vec<ItemPlanEntry>, DbError> {
    let mut statement = conn.prepare(
        "SELECT item_id, state, text, plan_id, plan_name, plan_meta, agent, linked_at \
         FROM item_plans WHERE project_id = ?1 ORDER BY linked_at",
    )?;
    let rows = statement.query_map([project_id], |row| Ok(plan_from_row(row)))?;
    rows.map(|row| -> Result<ItemPlanEntry, DbError> { row? })
        .collect()
}

fn plan_from_row(row: &Row<'_>) -> Result<ItemPlanEntry, DbError> {
    let item_id: String = row.get("item_id")?;
    let state: String = row.get("state")?;
    let agent: Option<String> = row.get("agent")?;
    let plan_id: Option<String> = row.get("plan_id")?;
    let plan_name: Option<String> = row.get("plan_name")?;
    let plan_meta: Option<String> = row.get("plan_meta")?;
    if state != PROPOSED_PLAN && state != LINKED_PLAN {
        return Err(DbError::InvalidPlanState(state));
    }
    if let Some(agent) = agent.as_deref() {
        if Agent::from_stored(agent).is_none() {
            return Err(DbError::InvalidPlanAgent(agent.to_string()));
        }
    }
    let plan = match (plan_id, plan_name, plan_meta) {
        (Some(id), Some(name), Some(meta)) => Some(LinkedPlan { id, name, meta }),
        (None, None, None) => None,
        _ => {
            return Err(DbError::BacklogItemData {
                item_id,
                reason: LINKED_PLAN_INCOMPLETE.to_string(),
            })
        }
    };
    Ok(ItemPlanEntry {
        proposal: row.get("text")?,
        linked_at: row.get("linked_at")?,
        item_id,
        state,
        plan,
        agent,
    })
}

#[cfg(test)]
#[path = "../../tests/plan_note_tests.rs"]
mod plan_note_tests;
