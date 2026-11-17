use chrono::Local;
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::events::Agent;

use super::connection::Db;
use super::error::DbError;

const DEFAULT_COLUMN: &str = "Backlog";
pub const BOARD_COLUMNS: &[&str] = &["Backlog", "Ready", "In progress", "In review", "Done"];
const MOVABLE_COLUMNS: &[&str] = &["Backlog", "Ready", "Done"];
const ITEM_TYPES: &[&str] = &["task", "feature"];
const ITEM_STATES: &[&str] = &["vazio", "triagem", "gerando", "proposto", "vinculado"];
const MICRO_TASK_STATUSES: &[&str] = &["proposta", "ready", "em andamento", "em revisão", "feito"];
const EMPTY_PLAN: &str = "vazio";
const PROPOSED_PLAN: &str = "proposto";
const LINKED_PLAN: &str = "vinculado";
const PLAN_AGENTS: &[&str] = &["ember", "davi"];
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
pub const AGENT_FORBIDDEN_COLUMN: &str = "Ready";
const USER_OWNER: &str = "voce";
const USER_CARD_TYPE: &str = "feature";
const AGENT_CARD_TYPE: &str = "task";
const NEW_CARD_STATE: &str = "vazio";
const CARD_ID_PREFIX: &str = "item-";
const USER_CREATED_TEXT: &str = "Pedido criado por você.";
const CONVERSATION_CREATED_TEXT: &str = "Pedido criado a partir de uma mensagem da conversa.";
const ACTIVITY_TIME_FORMAT: &str = "%d/%m/%Y, %H:%M:%S";

const ITEM_COLUMNS: &str = "id, title, \"type\", state, stage, triage_step, parent, feature, \
    description, plan, who, \"by\", is_new, \"column\", priority, estimate, tags, tasks, criteria, \
    questions, links, activity, children_total, children_done, origin";

const UPSERT_ITEM_SQL: &str = "INSERT INTO backlog_items (project_id, id, title, \"type\", state, \
    stage, triage_step, parent, feature, description, plan, who, \"by\", is_new, \"column\", \
    priority, estimate, tags, tasks, criteria, questions, links, activity, children_total, \
    children_done, origin, position) \
    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, \
    ?20, ?21, ?22, ?23, ?24, ?25, ?26, \
    (SELECT COALESCE(MAX(position), 0) + 1 FROM backlog_items WHERE project_id = ?1)) \
    ON CONFLICT(project_id, id) DO UPDATE SET \
    title = excluded.title, \"type\" = excluded.\"type\", state = excluded.state, \
    stage = excluded.stage, triage_step = excluded.triage_step, parent = excluded.parent, \
    feature = excluded.feature, description = excluded.description, plan = excluded.plan, \
    who = excluded.who, \"by\" = excluded.\"by\", is_new = excluded.is_new, \
    \"column\" = excluded.\"column\", priority = excluded.priority, estimate = excluded.estimate, \
    tags = excluded.tags, tasks = excluded.tasks, criteria = excluded.criteria, \
    questions = excluded.questions, links = excluded.links, activity = excluded.activity, \
    children_total = excluded.children_total, children_done = excluded.children_done";

const INSERT_CARD_SQL: &str = "INSERT INTO backlog_items (project_id, id, title, \"type\", state, \
    is_new, \"column\", who, \"by\", description, origin, activity, links, position) \
    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)";

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

pub enum CardOperation {
    Create {
        title: String,
        description: Option<String>,
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
        let conn = self.conn()?;
        ensure_project(&conn, project_id)?;
        upsert_item(&conn, project_id, item)
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
        let conn = self.conn()?;
        ensure_project(&conn, project_id)?;
        let changed = conn.execute(
            "UPDATE backlog_items SET \"column\" = ?3 WHERE project_id = ?1 AND id = ?2",
            params![project_id, id, column],
        )?;
        if changed == 0 {
            return Err(DbError::BacklogItemNotFound(id.to_string()));
        }
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
                CardOperation::Create { title, description } => {
                    create_agent_card(&tx, project_id, actor, title, description.as_deref())?
                }
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
        let conn = self.conn()?;
        ensure_project(&conn, project_id)?;
        ensure_item(&conn, project_id, item_id)?;
        if plan.state == EMPTY_PLAN {
            conn.execute(
                "DELETE FROM item_plans WHERE project_id = ?1 AND item_id = ?2",
                params![project_id, item_id],
            )?;
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
        Ok(())
    }
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
        if !PLAN_AGENTS.contains(&agent) {
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

fn ensure_project(conn: &Connection, project_id: i64) -> Result<(), DbError> {
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
) -> Result<CardResult, DbError> {
    let title = normalized_title(title)?;
    ensure_title_free(conn, project_id, &title)?;
    let (id, position) = next_card_id(conn, project_id)?;
    let draft = CardDraft {
        title: title.clone(),
        kind: AGENT_CARD_TYPE,
        owner: actor.id.as_str(),
        description: normalized_description(description),
        origin: ORIGIN_AGENT,
        activity: vec![activity_entry(
            &id,
            1,
            format!("{} criou o card na conversa.", actor.name),
        )],
        links: Vec::new(),
    };
    insert_card(conn, project_id, &fresh_card(&id, draft), position)?;
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
    if column == AGENT_FORBIDDEN_COLUMN {
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
        "UPDATE backlog_items SET \"column\" = ?3, activity = ?4 WHERE project_id = ?1 AND id = ?2",
        params![
            project_id,
            card.id,
            column,
            to_json(&card.id, "activity", &card.activity)?,
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
    let owned =
        card.who == actor.id.as_str() || actor.current_card.as_deref() == Some(card.id.as_str());
    if !owned {
        return Err(DbError::CardNotOwned(card.id.clone()));
    }
    Ok(())
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
    if current == AGENT_FORBIDDEN_COLUMN {
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
        ],
    )?;
    Ok(())
}

fn read_items(conn: &Connection, project_id: i64) -> Result<Vec<BacklogItem>, DbError> {
    let mut statement = conn.prepare(&format!(
        "SELECT {ITEM_COLUMNS} FROM backlog_items WHERE project_id = ?1 ORDER BY position"
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
        children: None,
        who: row.get("who")?,
        by: row.get("by")?,
        origin: origin_from_row(row)?,
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
        if !PLAN_AGENTS.contains(&agent) {
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
