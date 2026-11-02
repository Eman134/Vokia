use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::db::{Db, DbError};

const DEFAULT_COLUMN: &str = "Backlog";
const BOARD_COLUMNS: &[&str] = &["Backlog", "Ready", "In progress", "In review", "Done"];
const MOVABLE_COLUMNS: &[&str] = &["Backlog", "Ready", "Done"];
const ITEM_TYPES: &[&str] = &["task", "feature"];
const ITEM_STATES: &[&str] = &["vazio", "triagem", "gerando", "proposto", "vinculado"];
const MICRO_TASK_STATUSES: &[&str] = &["proposta", "ready", "em andamento", "em revisão", "feito"];
const EMPTY_PLAN: &str = "vazio";
const PROPOSED_PLAN: &str = "proposto";
const LINKED_PLAN: &str = "vinculado";
const PLAN_AGENTS: &[&str] = &["iara", "davi"];
const PROPOSAL_REQUIRED: &str = "A proposta do plano não pode ser vazia.";
const LINKED_PLAN_REQUIRED: &str = "O plano vinculado precisa de id, nome e resumo.";
const LINKED_PLAN_INCOMPLETE: &str = "os dados do plano vinculado estão incompletos.";
const CHILDREN_INCOMPLETE: &str = "o total e os feitos dos filhos precisam vir juntos.";
const ITEM_WITH_ERROR: &str = "o item tem erro de leitura e não pode ser gravado de novo.";
const FALLBACK_ITEM_TYPE: &str = "task";
const FALLBACK_ITEM_STATE: &str = "vazio";

const ITEM_COLUMNS: &str = "id, title, \"type\", state, stage, triage_step, parent, feature, \
    description, plan, who, \"by\", is_new, \"column\", priority, estimate, tags, tasks, criteria, \
    questions, links, activity, children_total, children_done";

const UPSERT_ITEM_SQL: &str = "INSERT INTO backlog_items (project_id, id, title, \"type\", state, \
    stage, triage_step, parent, feature, description, plan, who, \"by\", is_new, \"column\", \
    priority, estimate, tags, tasks, criteria, questions, links, activity, children_total, \
    children_done, position) \
    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, \
    ?20, ?21, ?22, ?23, ?24, ?25, \
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

impl Db {
    pub fn list_backlog(&self, project_id: i64) -> Result<BacklogSnapshot, DbError> {
        let conn = self.conn()?;
        ensure_project(&conn, project_id)?;
        let items = read_items(&conn, project_id)?;
        let plans = read_plans(&conn, project_id)?;
        Ok(BacklogSnapshot { items, plans })
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
