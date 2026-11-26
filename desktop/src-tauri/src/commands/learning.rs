use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::db::{
    ArtifactSummary, BacklogItem, Db, DbState, LearningSuggestion, MemoryOriginType,
    NewProjectMemory, ProjectMemory, SkillScope, SuggestionStatus, TurnWorkStepsRecord,
};
use crate::events::VokiaEvent;
use crate::orchestrator::memory_review::{
    apply_learning_suggestion_on_db, dismiss_learning_suggestion_on_db,
    run_memory_review_for_project, ReviewOutcome, ReviewTrigger,
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunMemoryReviewArgs {
    pub project_id: i64,
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub card_id: Option<String>,
    #[serde(default)]
    pub trigger: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ListLearningSuggestionsArgs {
    #[serde(default)]
    pub project_id: Option<i64>,
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyLearningSuggestionArgs {
    pub id: String,
    #[serde(default)]
    pub scope: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DismissLearningSuggestionArgs {
    pub id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListProjectMemoriesArgs {
    pub project_id: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetContextGraphArgs {
    pub project_id: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryGraphQueryArgs {
    pub project_id: i64,
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryGraphSaveArgs {
    pub project_id: i64,
    pub fact: String,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(default)]
    pub card_id: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub wing: Option<String>,
    #[serde(default)]
    pub room: Option<String>,
    #[serde(default)]
    pub supersede_id: Option<String>,
    #[serde(default)]
    pub keywords: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryGraphQueryResult {
    pub memories: Vec<ProjectMemory>,
    pub decisions: Vec<String>,
    pub keywords: Vec<String>,
    pub compact_context: String,
    pub nodes: Vec<ContextGraphNode>,
    pub edges: Vec<ContextGraphEdge>,
    pub optimizations: Vec<ContextGraphOptimization>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextGraphNode {
    pub id: String,
    pub kind: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub card_id: Option<String>,
    #[serde(default)]
    pub agents: Vec<String>,
    #[serde(default)]
    pub card_ids: Vec<String>,
    pub conflict_risk: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wing: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub room: Option<String>,
    #[serde(default)]
    pub superseded: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextGraphEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextGraphOptimization {
    pub id: String,
    pub kind: String,
    pub badge: String,
    pub message: String,
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(default)]
    pub card_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextGraph {
    pub project_id: i64,
    pub nodes: Vec<ContextGraphNode>,
    pub edges: Vec<ContextGraphEdge>,
    pub optimizations: Vec<ContextGraphOptimization>,
}

fn now_millis_i64() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

fn emit_learning_changed(app: &AppHandle, project_id: i64) {
    let db_state = app.state::<DbState>();
    let pending_count = db_state
        .db()
        .and_then(|db| db.count_pending_learning_suggestions(Some(project_id)).ok())
        .unwrap_or(0);
    let _ = crate::orchestrator::emit(
        app,
        VokiaEvent::LearningSuggestionsChanged {
            project_id,
            pending_count,
            new_count: 0,
        },
    );
}

pub(crate) async fn run_memory_review_exec(
    app: &AppHandle,
    args: RunMemoryReviewArgs,
) -> Result<ReviewOutcome, String> {
    let trigger = ReviewTrigger::parse(args.trigger.as_deref());
    run_memory_review_for_project(
        app,
        args.project_id,
        trigger,
        args.agent.as_deref(),
        args.card_id.as_deref(),
        None,
    )
    .await
}

pub(crate) fn list_learning_suggestions_exec(
    app: &AppHandle,
    args: ListLearningSuggestionsArgs,
) -> Result<Vec<LearningSuggestion>, String> {
    let db_state = app.state::<DbState>();
    let db = db_state.require().map_err(|e| e.to_string())?;
    let status = args
        .status
        .as_deref()
        .and_then(SuggestionStatus::parse);
    db.list_learning_suggestions(args.project_id, status)
        .map_err(|e| e.to_string())
}

pub(crate) fn apply_learning_suggestion_exec(
    app: &AppHandle,
    args: ApplyLearningSuggestionArgs,
) -> Result<LearningSuggestion, String> {
    let db_state = app.state::<DbState>();
    let db = db_state.require().map_err(|e| e.to_string())?;
    let existing = db
        .get_learning_suggestion(&args.id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Sugestão não encontrada: {}", args.id))?;
    let project_path = db
        .project(existing.project_id)
        .ok()
        .map(|p| PathBuf::from(p.path));
    let app_data_dir = app.path().app_data_dir().ok();
    let scope_override = args.scope.as_deref().map(SkillScope::parse);
    let updated = apply_learning_suggestion_on_db(
        db,
        &args.id,
        scope_override,
        project_path.as_deref(),
        app_data_dir.as_deref(),
        now_millis_i64(),
    )?;
    emit_learning_changed(app, updated.project_id);
    Ok(updated)
}

pub(crate) fn dismiss_learning_suggestion_exec(
    app: &AppHandle,
    args: DismissLearningSuggestionArgs,
) -> Result<LearningSuggestion, String> {
    let db_state = app.state::<DbState>();
    let db = db_state.require().map_err(|e| e.to_string())?;
    let updated = dismiss_learning_suggestion_on_db(db, &args.id, now_millis_i64())?;
    emit_learning_changed(app, updated.project_id);
    Ok(updated)
}

pub(crate) fn list_project_memories_exec(
    app: &AppHandle,
    args: ListProjectMemoriesArgs,
) -> Result<Vec<ProjectMemory>, String> {
    let db_state = app.state::<DbState>();
    let db = db_state.require().map_err(|e| e.to_string())?;
    db.list_project_memories(args.project_id)
        .map_err(|e| e.to_string())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveProjectMemoryArgs {
    pub project_id: i64,
    pub fact: String,
    #[serde(default)]
    pub source: Option<String>,
}

pub(crate) fn save_project_memory_exec(
    app: &AppHandle,
    args: SaveProjectMemoryArgs,
) -> Result<ProjectMemory, String> {
    let fact = args.fact.trim();
    if fact.is_empty() {
        return Err("Informe o fato a salvar após /lembrar.".to_string());
    }
    let db_state = app.state::<DbState>();
    let db = db_state.require().map_err(|e| e.to_string())?;
    let now_ms = now_millis_i64();
    let id = format!("mem-manual-{now_ms}");
    let source = args.source.as_deref().unwrap_or("comando /lembrar");
    db.insert_project_memory(&NewProjectMemory {
        id: &id,
        project_id: args.project_id,
        fact,
        source,
        origin_type: MemoryOriginType::Manual,
        pinned: true,
        author: Some("você"),
        now_ms,
    })
    .map_err(|e| e.to_string())?;
    emit_learning_changed(app, args.project_id);
    let (wing, room) = crate::db::learning::infer_wing_and_room(source, fact);
    Ok(ProjectMemory {
        id,
        project_id: args.project_id,
        fact: fact.to_string(),
        source: source.to_string(),
        origin_type: MemoryOriginType::Manual,
        pinned: true,
        author: Some("você".to_string()),
        valid_from: Some(now_ms),
        valid_until: None,
        superseded_by: None,
        wing: Some(wing),
        room: Some(room),
        created_at: now_ms,
        updated_at: now_ms,
    })
}

pub(crate) fn get_context_graph_exec(
    app: &AppHandle,
    args: GetContextGraphArgs,
) -> Result<ContextGraph, String> {
    let db_state = app.state::<DbState>();
    let db = db_state.require().map_err(|e| e.to_string())?;
    build_context_graph(db, args.project_id)
}

const MAX_WORK_STEPS_SCAN: usize = 80;
const MAX_FILE_NODES: usize = 16;
const MAX_ARTIFACT_NODES: usize = 10;

#[derive(Default)]
struct FileTouch {
    agents: BTreeSet<String>,
    card_ids: BTreeSet<String>,
    recency_rank: usize,
}

fn normalize_graph_path(raw: &str) -> Option<String> {
    let normalized = raw.trim().replace('\\', "/");
    let trimmed = normalized.trim_start_matches("./").trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn file_display_label(path: &str) -> String {
    let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
    match parts.as_slice() {
        [] => path.to_string(),
        [single] => (*single).to_string(),
        [.., parent, file] if *parent == "api" || *parent == "src" || *parent == "db" => {
            format!("{parent}/{file}")
        }
        [.., file] => (*file).to_string(),
    }
}

fn extract_files_from_work_step_block(block: &str) -> Vec<String> {
    let mut files = Vec::new();
    for line in block.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("files (") {
            continue;
        }
        let Some((_, rest)) = trimmed.split_once("):") else {
            continue;
        };
        for entry in rest.split(',') {
            let entry = entry.trim();
            let raw_path = entry
                .strip_prefix("edit ")
                .or_else(|| entry.strip_prefix("create "))
                .or_else(|| entry.strip_prefix("delete "))
                .unwrap_or(entry);
            if let Some(path) = normalize_graph_path(raw_path) {
                files.push(path);
            }
        }
    }
    files
}

fn is_active_card_column(column: Option<&str>) -> bool {
    matches!(column, Some("In progress") | Some("In review"))
}

fn record_file_touch(
    map: &mut BTreeMap<String, FileTouch>,
    path: String,
    agent: Option<&str>,
    card_id: Option<&str>,
    rank: usize,
) {
    let entry = map.entry(path).or_insert_with(|| FileTouch {
        recency_rank: rank,
        ..FileTouch::default()
    });
    entry.recency_rank = entry.recency_rank.min(rank);
    if let Some(a) = agent.map(str::trim).filter(|s| !s.is_empty()) {
        entry.agents.insert(a.to_string());
    }
    if let Some(c) = card_id.map(str::trim).filter(|s| !s.is_empty()) {
        entry.card_ids.insert(c.to_string());
    }
}

fn collect_file_touches(
    work_steps: &[TurnWorkStepsRecord],
    active_cards: &[&BacklogItem],
) -> BTreeMap<String, FileTouch> {
    let mut touches = BTreeMap::new();
    for (idx, step) in work_steps.iter().enumerate() {
        for path in extract_files_from_work_step_block(&step.block) {
            record_file_touch(
                &mut touches,
                path,
                Some(step.agent.as_str()),
                step.card_id.as_deref(),
                idx,
            );
        }
    }
    let base_rank = work_steps.len();
    for (idx, card) in active_cards.iter().enumerate() {
        for micro in &card.micro {
            let agent = if micro.who.trim().is_empty() {
                card.who.as_str()
            } else {
                micro.who.as_str()
            };
            for raw_file in &micro.script.files {
                if let Some(path) = normalize_graph_path(raw_file) {
                    record_file_touch(
                        &mut touches,
                        path,
                        Some(agent),
                        Some(card.id.as_str()),
                        base_rank + idx,
                    );
                }
            }
        }
    }
    touches
}

fn artifact_mentions_file(artifact: &ArtifactSummary, file_path: &str, file_label: &str) -> bool {
    let needle_full = file_path.to_ascii_lowercase();
    let needle_label = file_label.to_ascii_lowercase();
    let needle_base = file_path
        .rsplit('/')
        .next()
        .unwrap_or(file_path)
        .to_ascii_lowercase();
    if needle_base.len() < 4 {
        return false;
    }
    let haystack = format!(
        "{} {} {}",
        artifact.title.to_ascii_lowercase(),
        artifact.summary.to_ascii_lowercase(),
        artifact.tags.join(" ").to_ascii_lowercase()
    );
    haystack.contains(&needle_full)
        || haystack.contains(&needle_label)
        || haystack.contains(&needle_base)
}

fn push_edge_unique(
    edges: &mut Vec<ContextGraphEdge>,
    seen: &mut HashSet<(String, String, String)>,
    source: &str,
    target: &str,
    kind: &str,
    label: Option<String>,
) {
    let key = (source.to_string(), target.to_string(), kind.to_string());
    if !seen.insert(key) {
        return;
    }
    let idx = edges.len() + 1;
    edges.push(ContextGraphEdge {
        id: format!("edge-{idx}"),
        source: source.to_string(),
        target: target.to_string(),
        kind: kind.to_string(),
        label,
    });
}

/// Monta o grafo de contexto do projeto a partir de cards ativos, passos recentes e artefatos.
pub(crate) fn build_context_graph(db: &Db, project_id: i64) -> Result<ContextGraph, String> {
    let backlog = db.list_backlog(project_id).map_err(|e| e.to_string())?;
    let (work_steps, _) = db
        .list_history_work_steps(project_id, None, None, MAX_WORK_STEPS_SCAN, 0)
        .map_err(|e| e.to_string())?;
    let artifacts = db
        .list_artifact_summaries(project_id)
        .map_err(|e| e.to_string())?;

    let active_cards: Vec<&BacklogItem> = backlog
        .items
        .iter()
        .filter(|item| is_active_card_column(item.column.as_deref()))
        .collect();
    let active_card_ids: HashSet<&str> = active_cards.iter().map(|c| c.id.as_str()).collect();

    let touches = collect_file_touches(&work_steps, &active_cards);
    let mut ranked_files: Vec<(String, FileTouch)> = touches.into_iter().collect();
    ranked_files.sort_by(|(pa, ta), (pb, tb)| {
        let ca = ta.card_ids.len() >= 2 || ta.agents.len() >= 2;
        let cb = tb.card_ids.len() >= 2 || tb.agents.len() >= 2;
        cb.cmp(&ca)
            .then_with(|| ta.recency_rank.cmp(&tb.recency_rank))
            .then_with(|| pa.cmp(pb))
    });
    ranked_files.truncate(MAX_FILE_NODES);

    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut seen_edges = HashSet::new();

    for card in &active_cards {
        nodes.push(ContextGraphNode {
            id: format!("card:{}", card.id),
            kind: "card".to_string(),
            label: card.id.clone(),
            subtitle: Some(card.title.clone()),
            path: None,
            status: card.column.clone(),
            artifact_kind: None,
            card_id: Some(card.id.clone()),
            agents: if card.who.trim().is_empty() {
                Vec::new()
            } else {
                vec![card.who.clone()]
            },
            card_ids: vec![card.id.clone()],
            conflict_risk: false,
            wing: None,
            room: None,
            superseded: false,
        });
    }

    let mut card_to_files: HashMap<String, Vec<String>> = HashMap::new();
    let mut conflict_files: Vec<(String, Vec<String>, Vec<String>)> = Vec::new();

    for (path, touch) in &ranked_files {
        let conflict_risk = touch.card_ids.len() >= 2 || touch.agents.len() >= 2;
        let file_node_id = format!("file:{path}");
        let label = file_display_label(path);
        let agents: Vec<String> = touch.agents.iter().cloned().collect();
        let card_ids: Vec<String> = touch.card_ids.iter().cloned().collect();

        nodes.push(ContextGraphNode {
            id: file_node_id.clone(),
            kind: "file".to_string(),
            label,
            subtitle: Some(path.clone()),
            path: Some(path.clone()),
            status: None,
            artifact_kind: None,
            card_id: card_ids.first().cloned(),
            agents: agents.clone(),
            card_ids: card_ids.clone(),
            conflict_risk,
            wing: None,
            room: None,
            superseded: false,
        });

        for cid in &card_ids {
            card_to_files
                .entry(cid.clone())
                .or_default()
                .push(path.clone());
            if active_card_ids.contains(cid.as_str()) {
                let card_node_id = format!("card:{cid}");
                let edge_kind = if conflict_risk {
                    "conflict_risk"
                } else {
                    "task_path"
                };
                push_edge_unique(
                    &mut edges,
                    &mut seen_edges,
                    &card_node_id,
                    &file_node_id,
                    edge_kind,
                    Some(cid.clone()),
                );
            }
        }

        if conflict_risk {
            conflict_files.push((path.clone(), agents, card_ids));
        }
    }

    let mut linked_file_ids: HashSet<String> = HashSet::new();
    for art in artifacts.iter().take(MAX_ARTIFACT_NODES) {
        let art_node_id = format!("artifact:{}", art.id);
        nodes.push(ContextGraphNode {
            id: art_node_id.clone(),
            kind: "artifact".to_string(),
            label: art.title.clone(),
            subtitle: if art.summary.trim().is_empty() {
                None
            } else {
                Some(art.summary.clone())
            },
            path: None,
            status: None,
            artifact_kind: Some(art.kind.clone()),
            card_id: art.item_id.clone(),
            agents: art.author.clone().into_iter().collect(),
            card_ids: art.item_id.clone().into_iter().collect(),
            conflict_risk: false,
            wing: None,
            room: None,
            superseded: false,
        });

        if let Some(item_id) = art.item_id.as_deref() {
            if active_card_ids.contains(item_id) {
                push_edge_unique(
                    &mut edges,
                    &mut seen_edges,
                    &art_node_id,
                    &format!("card:{item_id}"),
                    "artifact_link",
                    None,
                );
            }
            if let Some(files) = card_to_files.get(item_id) {
                for path in files {
                    let file_node_id = format!("file:{path}");
                    linked_file_ids.insert(file_node_id.clone());
                    push_edge_unique(
                        &mut edges,
                        &mut seen_edges,
                        &art_node_id,
                        &file_node_id,
                        "artifact_link",
                        None,
                    );
                }
            }
        }

        for (path, _) in &ranked_files {
            let label = file_display_label(path);
            if artifact_mentions_file(art, path, &label) {
                let file_node_id = format!("file:{path}");
                linked_file_ids.insert(file_node_id.clone());
                push_edge_unique(
                    &mut edges,
                    &mut seen_edges,
                    &art_node_id,
                    &file_node_id,
                    "artifact_link",
                    None,
                );
            }
        }
    }

    let mut optimizations = Vec::new();
    for (idx, (path, agents, card_ids)) in conflict_files.iter().take(2).enumerate() {
        let who = if agents.is_empty() {
            "Múltiplos fluxos".to_string()
        } else {
            agents.join(" e ")
        };
        let cards_text = if card_ids.is_empty() {
            "as tarefas ativas".to_string()
        } else {
            card_ids.join(" e ")
        };
        optimizations.push(ContextGraphOptimization {
            id: format!("opt-conflict-{}", idx + 1),
            kind: "conflict".to_string(),
            badge: "conflito".to_string(),
            message: format!(
                "{who} tocam {path} em {cards_text}. Coordenar as alterações evita retrabalho."
            ),
            files: vec![path.clone()],
            card_ids: card_ids.clone(),
        });
    }

    if let Some((card_id, files)) = card_to_files
        .iter()
        .filter(|(cid, files)| active_card_ids.contains(cid.as_str()) && !files.is_empty())
        .max_by_key(|(_, files)| files.len())
    {
        let count = files.len();
        optimizations.push(ContextGraphOptimization {
            id: "opt-context-1".to_string(),
            kind: "context".to_string(),
            badge: "contexto".to_string(),
            message: format!(
                "Para {card_id} bastam {count} arquivo(s) do caminho marcado, sem carregar o repositório inteiro."
            ),
            files: files.clone(),
            card_ids: vec![card_id.clone()],
        });
    }

    let unlinked_files: Vec<String> = ranked_files
        .iter()
        .filter(|(path, _)| !linked_file_ids.contains(&format!("file:{path}")))
        .map(|(path, _)| path.clone())
        .collect();
    if !unlinked_files.is_empty() && !ranked_files.is_empty() {
        let count = unlinked_files.len();
        optimizations.push(ContextGraphOptimization {
            id: "opt-artifact-1".to_string(),
            kind: "artifact".to_string(),
            badge: "artefato".to_string(),
            message: format!(
                "{count} arquivo(s) ativo(s) ainda sem artefato vinculado no grafo. Registrar decisão ou spec reduz leitura repetida."
            ),
            files: unlinked_files.into_iter().take(4).collect(),
            card_ids: Vec::new(),
        });
    }

    if let Some((path, _, card_ids)) = conflict_files.first() {
        if card_ids.len() >= 2 {
            optimizations.push(ContextGraphOptimization {
                id: "opt-order-1".to_string(),
                kind: "order".to_string(),
                badge: "ordem".to_string(),
                message: format!(
                    "{} depende de {path}, também tocado por {}. Concluir {} primeiro reduz conflitos de merge.",
                    card_ids[1], card_ids[0], card_ids[0]
                ),
                files: vec![path.clone()],
                card_ids: card_ids.clone(),
            });
        }
    }

    let memories = db.list_project_memories(project_id).unwrap_or_default();
    let active_memories: Vec<&ProjectMemory> =
        memories.iter().filter(|m| m.is_active()).take(24).collect();
    let mut keyword_freq: HashMap<String, Vec<String>> = HashMap::new();
    for mem in &active_memories {
        let mem_node_id = format!("memory:{}", mem.id);
        let is_decision = matches!(
            mem.source.to_ascii_lowercase().as_str(),
            "decisao" | "decisão" | "arquitetura"
        );
        let kind = if is_decision { "decision" } else { "memory" };
        let label: String = if mem.fact.chars().count() > 54 {
            let clipped: String = mem.fact.chars().take(53).collect();
            format!("{clipped}…")
        } else {
            mem.fact.clone()
        };
        let wing_room = match (mem.wing.as_deref(), mem.room.as_deref()) {
            (Some(w), Some(r)) => format!("{w} · {r}"),
            (Some(w), None) => w.to_string(),
            _ => mem.source.clone(),
        };
        nodes.push(ContextGraphNode {
            id: mem_node_id.clone(),
            kind: kind.to_string(),
            label,
            subtitle: Some(format!("{wing_room} — {}", mem.fact)),
            path: None,
            status: Some(mem.source.clone()),
            artifact_kind: None,
            card_id: None,
            agents: mem.author.clone().into_iter().collect(),
            card_ids: Vec::new(),
            conflict_risk: false,
            wing: mem.wing.clone(),
            room: mem.room.clone(),
            superseded: !mem.is_active(),
        });
        for kw in extract_domain_keywords(&mem.fact, 4) {
            keyword_freq.entry(kw).or_default().push(mem_node_id.clone());
        }
    }
    for card in &active_cards {
        let card_node_id = format!("card:{}", card.id);
        for kw in extract_domain_keywords(&card.title, 3) {
            keyword_freq.entry(kw).or_default().push(card_node_id.clone());
        }
    }
    for art in artifacts.iter().take(MAX_ARTIFACT_NODES) {
        let art_node_id = format!("artifact:{}", art.id);
        let art_text = format!("{} {}", art.title, art.summary);
        for kw in extract_domain_keywords(&art_text, 3) {
            keyword_freq.entry(kw).or_default().push(art_node_id.clone());
        }
    }

    let mut ranked_keywords: Vec<(String, Vec<String>)> = keyword_freq.into_iter().collect();
    ranked_keywords.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then_with(|| a.0.cmp(&b.0)));
    for (kw, linked_sources) in ranked_keywords.into_iter().take(12) {
        let kw_node_id = format!("keyword:{kw}");
        nodes.push(ContextGraphNode {
            id: kw_node_id.clone(),
            kind: "keyword".to_string(),
            label: kw.clone(),
            subtitle: Some(format!("Palavra-chave conectando {} item(ns)", linked_sources.len())),
            path: None,
            status: None,
            artifact_kind: None,
            card_id: None,
            agents: Vec::new(),
            card_ids: Vec::new(),
            conflict_risk: false,
            wing: None,
            room: None,
            superseded: false,
        });
        for src in linked_sources {
            push_edge_unique(
                &mut edges,
                &mut seen_edges,
                &src,
                &kw_node_id,
                "has_keyword",
                None,
            );
        }
    }

    if let Ok(mem_edges) = db.list_memory_edges(project_id) {
        let mut existing_node_ids: HashSet<String> = nodes.iter().map(|n| n.id.clone()).collect();
        for edge in mem_edges {
            if edge.target_node.starts_with("keyword:") && !existing_node_ids.contains(&edge.target_node) {
                let kw = edge.target_node.trim_start_matches("keyword:").to_string();
                existing_node_ids.insert(edge.target_node.clone());
                nodes.push(ContextGraphNode {
                    id: edge.target_node.clone(),
                    kind: "keyword".to_string(),
                    label: kw,
                    subtitle: Some("Palavra-chave registrada".to_string()),
                    path: None,
                    status: None,
                    artifact_kind: None,
                    card_id: None,
                    agents: Vec::new(),
                    card_ids: Vec::new(),
                    conflict_risk: false,
                    wing: None,
                    room: None,
                    superseded: false,
                });
            }
            if existing_node_ids.contains(&edge.source_node)
                && existing_node_ids.contains(&edge.target_node)
            {
                push_edge_unique(
                    &mut edges,
                    &mut seen_edges,
                    &edge.source_node,
                    &edge.target_node,
                    &edge.relation,
                    Some(edge.relation.clone()),
                );
            }
        }
    }

    Ok(ContextGraph {
        project_id,
        nodes,
        edges,
        optimizations,
    })
}

#[tauri::command]
pub async fn run_memory_review(
    app: AppHandle,
    args: RunMemoryReviewArgs,
) -> Result<ReviewOutcome, String> {
    run_memory_review_exec(&app, args).await
}

#[tauri::command]
pub fn list_learning_suggestions(
    app: AppHandle,
    args: Option<ListLearningSuggestionsArgs>,
) -> Result<Vec<LearningSuggestion>, String> {
    list_learning_suggestions_exec(&app, args.unwrap_or_default())
}

#[tauri::command]
pub fn apply_learning_suggestion(
    app: AppHandle,
    args: ApplyLearningSuggestionArgs,
) -> Result<LearningSuggestion, String> {
    apply_learning_suggestion_exec(&app, args)
}

#[tauri::command]
pub fn dismiss_learning_suggestion(
    app: AppHandle,
    args: DismissLearningSuggestionArgs,
) -> Result<LearningSuggestion, String> {
    dismiss_learning_suggestion_exec(&app, args)
}

#[tauri::command]
pub fn list_project_memories(
    app: AppHandle,
    args: ListProjectMemoriesArgs,
) -> Result<Vec<ProjectMemory>, String> {
    list_project_memories_exec(&app, args)
}

#[tauri::command]
pub fn save_project_memory(
    app: AppHandle,
    args: SaveProjectMemoryArgs,
) -> Result<ProjectMemory, String> {
    save_project_memory_exec(&app, args)
}

fn extract_domain_keywords(text: &str, max: usize) -> Vec<String> {
    const STOPWORDS: &[&str] = &[
        "para", "como", "com", "sem", "uma", "uns", "umas", "que", "por", "mais", "menos",
        "sobre", "entre", "após", "antes", "quando", "onde", "qual", "quais", "esse", "essa",
        "isso", "isto", "este", "esta", "aqui", "ali", "tudo", "nada", "cada", "deve", "devem",
        "pode", "podem", "fazer", "feito", "sendo", "será", "foram", "tem", "ter", "tinha",
        "this", "that", "with", "from", "into", "when", "where", "should", "must", "will",
        "the", "and", "for", "are", "was", "were", "have", "has", "not", "but", "use",
    ];
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for raw in text.split(|c: char| !c.is_alphanumeric() && c != '_' && c != '-') {
        let kw = raw.trim_matches(|c: char| c == '-' || c == '_').to_lowercase();
        if kw.chars().count() < 4 || kw.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        if STOPWORDS.contains(&kw.as_str()) {
            continue;
        }
        if seen.insert(kw.clone()) {
            out.push(kw);
            if out.len() >= max {
                break;
            }
        }
    }
    out
}

fn summarize_query_subgraph(
    memories: &[ProjectMemory],
    nodes: &[ContextGraphNode],
) -> (Vec<String>, Vec<String>, String) {
    let decisions: Vec<String> = nodes
        .iter()
        .filter(|n| n.kind == "decision")
        .map(|n| n.subtitle.clone().unwrap_or_else(|| n.label.clone()))
        .take(6)
        .collect();
    let keywords: Vec<String> = nodes
        .iter()
        .filter(|n| n.kind == "keyword")
        .map(|n| n.label.clone())
        .take(10)
        .collect();
    let mut parts = Vec::new();
    if !decisions.is_empty() {
        parts.push(format!("[decisões] {}", decisions.join(" | ")));
    }
    if !keywords.is_empty() {
        parts.push(format!("[palavras-chave] {}", keywords.join(", ")));
    }
    if !memories.is_empty() {
        let mems: Vec<String> = memories.iter().take(5).map(|m| m.fact.clone()).collect();
        parts.push(format!("[memórias] {}", mems.join(" | ")));
    }
    let files: Vec<&str> = nodes
        .iter()
        .filter(|n| n.kind == "file")
        .map(|n| n.path.as_deref().unwrap_or(n.label.as_str()))
        .take(8)
        .collect();
    if !files.is_empty() {
        parts.push(format!("[arquivos] {}", files.join(", ")));
    }
    (decisions, keywords, parts.join("\n"))
}

pub(crate) fn query_memory_graph(
    db: &Db,
    project_id: i64,
    query: &str,
    limit: usize,
) -> Result<MemoryGraphQueryResult, String> {
    let cap = limit.clamp(1, 50);
    let memories = db
        .search_project_memories(project_id, query, cap)
        .map_err(|e| e.to_string())?;
    let graph = build_context_graph(db, project_id)?;
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        let (decisions, keywords, compact_context) = summarize_query_subgraph(&memories, &graph.nodes);
        return Ok(MemoryGraphQueryResult {
            memories,
            decisions,
            keywords,
            compact_context,
            nodes: graph.nodes,
            edges: graph.edges,
            optimizations: graph.optimizations,
        });
    }
    let terms: Vec<&str> = needle.split_whitespace().filter(|t| !t.is_empty()).collect();
    let matched_ids: HashSet<String> = graph
        .nodes
        .iter()
        .filter(|n| {
            let hay = format!("{} {} {}", n.id, n.label, n.subtitle.as_deref().unwrap_or("")).to_lowercase();
            terms.iter().any(|t| hay.contains(*t))
        })
        .map(|n| n.id.clone())
        .collect();
    let mut hop1 = matched_ids.clone();
    for edge in &graph.edges {
        if matched_ids.contains(&edge.source) {
            hop1.insert(edge.target.clone());
        }
        if matched_ids.contains(&edge.target) {
            hop1.insert(edge.source.clone());
        }
    }
    let mut neighbor_ids = hop1.clone();
    for edge in &graph.edges {
        if hop1.contains(&edge.source) {
            neighbor_ids.insert(edge.target.clone());
        }
        if hop1.contains(&edge.target) {
            neighbor_ids.insert(edge.source.clone());
        }
    }
    let nodes: Vec<ContextGraphNode> = graph
        .nodes
        .into_iter()
        .filter(|n| neighbor_ids.contains(&n.id))
        .collect();
    let edges = graph
        .edges
        .into_iter()
        .filter(|e| neighbor_ids.contains(&e.source) && neighbor_ids.contains(&e.target))
        .collect();
    let (decisions, keywords, compact_context) = summarize_query_subgraph(&memories, &nodes);
    Ok(MemoryGraphQueryResult {
        memories,
        decisions,
        keywords,
        compact_context,
        nodes,
        edges,
        optimizations: graph.optimizations,
    })
}

fn is_noise_memory_fact(fact: &str) -> bool {
    let trimmed = fact.trim();
    if trimmed.chars().count() < 12 {
        return true;
    }
    let lower = trimmed.to_ascii_lowercase();
    let transient_markers = [
        "não respondeu em",
        "timed out",
        "exit code",
        "stack trace",
        "cargo test failed",
        "erro temporário",
        "tentando novamente",
    ];
    transient_markers.iter().any(|m| lower.contains(m))
        && !lower.contains("regra")
        && !lower.contains("sempre")
        && !lower.contains("nunca")
        && !lower.contains("arquitetura")
}

fn token_jaccard_similarity(a: &str, b: &str) -> f32 {
    let norm = |s: &str| -> HashSet<String> {
        s.to_ascii_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| w.len() >= 3)
            .map(str::to_string)
            .collect()
    };
    let set_a = norm(a);
    let set_b = norm(b);
    if set_a.is_empty() || set_b.is_empty() {
        return 0.0;
    }
    let inter = set_a.intersection(&set_b).count() as f32;
    let union = set_a.union(&set_b).count() as f32;
    inter / union
}

fn link_memory_graph_edges(
    db: &Db,
    project_id: i64,
    memory_id: &str,
    fact: &str,
    keywords: &[String],
    files: &[String],
    card_id: Option<&str>,
    now_ms: i64,
) {
    let mem_node = format!("memory:{memory_id}");
    for file in files {
        let trimmed = file.trim();
        if !trimmed.is_empty() {
            let _ = db.upsert_memory_edge(
                project_id,
                &mem_node,
                &format!("file:{trimmed}"),
                "touches_file",
                now_ms,
            );
        }
    }
    if let Some(cid) = card_id.map(str::trim).filter(|s| !s.is_empty()) {
        let _ = db.upsert_memory_edge(
            project_id,
            &mem_node,
            &format!("card:{cid}"),
            "about_card",
            now_ms,
        );
    }
    let mut kw_set: Vec<String> = keywords
        .iter()
        .map(|k| k.trim().to_lowercase())
        .filter(|k| k.chars().count() >= 3)
        .collect();
    for auto_kw in extract_domain_keywords(fact, 4) {
        if !kw_set.contains(&auto_kw) {
            kw_set.push(auto_kw);
        }
    }
    for kw in kw_set.into_iter().take(6) {
        let _ = db.upsert_memory_edge(
            project_id,
            &mem_node,
            &format!("keyword:{kw}"),
            "has_keyword",
            now_ms,
        );
    }
}

pub(crate) fn save_memory_graph_fact(
    db: &Db,
    args: MemoryGraphSaveArgs,
) -> Result<ProjectMemory, String> {
    let fact = args.fact.trim();
    if fact.is_empty() {
        return Err("O fato da memória não pode ser vazio.".to_string());
    }
    if is_noise_memory_fact(fact) {
        return Err(
            "Memória rejeitada pelo Write Gate: informe uma decisão durável, regra ou mapa de arquivos (não erros transitórios)."
                .to_string(),
        );
    }
    // Fail-closed: quando o projeto existe no disco e `files` foram informados, valida os caminhos.
    if !args.files.is_empty() {
        if let Ok(project) = db.project(args.project_id) {
            let root = std::path::Path::new(&project.path);
            if root.is_dir() {
                for f in &args.files {
                    let rel = f.trim();
                    if rel.is_empty() || rel.contains('*') {
                        continue;
                    }
                    let candidate = root.join(rel);
                    if !candidate.exists() {
                        return Err(format!(
                            "Contrato fail-closed: arquivo '{rel}' não existe no projeto."
                        ));
                    }
                }
            }
        }
    }

    let now_ms = now_millis_i64();
    let existing = db.list_project_memories(args.project_id).unwrap_or_default();

    // Deduplicação semântica: se já existe memória ativa muito parecida (>= 0.80), reforça arestas.
    if args.supersede_id.is_none() {
        if let Some(dup) = existing
            .iter()
            .filter(|m| m.is_active())
            .find(|m| token_jaccard_similarity(&m.fact, fact) >= 0.80)
        {
            let _ = db.touch_project_memory(args.project_id, &dup.id, now_ms);
            link_memory_graph_edges(
                db,
                args.project_id,
                &dup.id,
                fact,
                &args.keywords,
                &args.files,
                args.card_id.as_deref(),
                now_ms,
            );
            let _ = db.record_orchestration_event(
                args.project_id,
                "memory_reinforced",
                args.author.as_deref(),
                args.card_id.as_deref(),
                &format!(r#"{{"memoryId":"{}"}}"#, dup.id),
                now_ms,
            );
            return db
                .get_project_memory(args.project_id, &dup.id)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| "Memória reforçada não encontrada.".to_string());
        }
    }

    let id = format!("mem-{}", now_ms);
    let source = args
        .category
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("macro")
        .to_string();
    db.insert_project_memory(&NewProjectMemory {
        id: &id,
        project_id: args.project_id,
        fact,
        source: &source,
        origin_type: MemoryOriginType::Auto,
        pinned: source == "macro" || source == "arquitetura",
        author: args.author.as_deref(),
        now_ms,
    })
    .map_err(|e| e.to_string())?;
    if args.wing.is_some() || args.room.is_some() {
        let _ = db.set_memory_palace_location(
            args.project_id,
            &id,
            args.wing.as_deref(),
            args.room.as_deref(),
        );
    }
    if let Some(old_id) = args.supersede_id.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        let _ = db.supersede_project_memory(args.project_id, old_id, &id, now_ms);
    }
    link_memory_graph_edges(
        db,
        args.project_id,
        &id,
        fact,
        &args.keywords,
        &args.files,
        args.card_id.as_deref(),
        now_ms,
    );
    let _ = db.record_orchestration_event(
        args.project_id,
        "memory_saved",
        args.author.as_deref(),
        args.card_id.as_deref(),
        &format!(r#"{{"memoryId":"{id}","source":"{source}"}}"#),
        now_ms,
    );
    db.get_project_memory(args.project_id, &id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Memória salva não encontrada.".to_string())
}

pub(crate) fn build_compact_memory_context(
    db: &Db,
    project_id: i64,
    item_id: Option<&str>,
    agent: Option<&str>,
    max_chars: usize,
) -> String {
    let memories: Vec<ProjectMemory> = db
        .list_project_memories(project_id)
        .unwrap_or_default()
        .into_iter()
        .filter(|m| m.is_active())
        .collect();
    let graph = build_context_graph(db, project_id).ok();
    if memories.is_empty() && graph.as_ref().is_none_or(|g| g.nodes.is_empty()) {
        return String::new();
    }
    let mut lines: Vec<String> = vec!["[memória e grafo do projeto — camadas]".to_string()];
    let mut prioritized: Vec<&ProjectMemory> = memories
        .iter()
        .filter(|m| {
            m.pinned
                || m.source == "macro"
                || m.source == "arquitetura"
                || m.source == "mapa-arquivos"
                || agent.is_some_and(|a| m.author.as_deref() == Some(a))
        })
        .take(6)
        .collect();
    if prioritized.len() < 4 {
        for mem in memories.iter().take(6) {
            if !prioritized.iter().any(|p| p.id == mem.id) {
                prioritized.push(mem);
            }
            if prioritized.len() >= 6 {
                break;
            }
        }
    }
    if !prioritized.is_empty() {
        lines.push("[núcleo] Visão macro e fatos ativos:".to_string());
        for mem in prioritized {
            let palace = match (mem.wing.as_deref(), mem.room.as_deref()) {
                (Some(w), Some(r)) => format!("{w}/{r}"),
                _ => mem.source.clone(),
            };
            lines.push(format!("- [{palace}] {}", mem.fact.trim()));
        }
    }
    if let Some(graph) = &graph {
        let keywords: Vec<&str> = graph
            .nodes
            .iter()
            .filter(|n| n.kind == "keyword")
            .map(|n| n.label.as_str())
            .take(10)
            .collect();
        if !keywords.is_empty() {
            lines.push(format!("[palavras-chave] {}", keywords.join(", ")));
        }
        let files: Vec<&ContextGraphNode> = graph
            .nodes
            .iter()
            .filter(|n| n.kind == "file")
            .take(10)
            .collect();
        if !files.is_empty() {
            let file_list = files
                .iter()
                .map(|f| f.label.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            lines.push(format!("[palácio/mapa] Arquivos ativos: {file_list}"));
        }
        if let Some(target_card) = item_id {
            let card_node = format!("card:{target_card}");
            let related_files: Vec<&str> = graph
                .edges
                .iter()
                .filter(|e| e.source == card_node && e.target.starts_with("file:"))
                .map(|e| e.target.trim_start_matches("file:"))
                .take(8)
                .collect();
            if !related_files.is_empty() {
                lines.push(format!(
                    "[grafo] {target_card} toca: {}",
                    related_files.join(", ")
                ));
            }
        }
        for opt in graph.optimizations.iter().take(2) {
            lines.push(format!("- [{}] {}", opt.kind, opt.message));
        }
    }
    lines.push("[ferramentas] Antes de implementar ou decidir, chame memory_graph_query(query=\"palavra-chave ou item-XX\") para puxar decisões e arquivos ligados; ao concluir/decidir algo durável, salve com memory_graph_save(fact, category=\"decisao|macro|mapa-arquivos\", keywords=[...], files=[...], card_id).".to_string());
    let joined = lines.join("\n");
    if joined.chars().count() <= max_chars {
        joined
    } else {
        let clipped: String = joined.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{clipped}…")
    }
}

pub(crate) fn memory_graph_query_exec(
    app: &AppHandle,
    args: MemoryGraphQueryArgs,
) -> Result<MemoryGraphQueryResult, String> {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|e| e.to_string())?;
    query_memory_graph(db, args.project_id, &args.query, args.limit.unwrap_or(12))
}

pub(crate) fn memory_graph_save_exec(
    app: &AppHandle,
    args: MemoryGraphSaveArgs,
) -> Result<ProjectMemory, String> {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|e| e.to_string())?;
    save_memory_graph_fact(db, args)
}

#[tauri::command]
pub fn get_context_graph(
    app: AppHandle,
    args: GetContextGraphArgs,
) -> Result<ContextGraph, String> {
    get_context_graph_exec(&app, args)
}

#[tauri::command]
pub fn memory_graph_query(
    app: AppHandle,
    args: MemoryGraphQueryArgs,
) -> Result<MemoryGraphQueryResult, String> {
    memory_graph_query_exec(&app, args)
}

#[tauri::command]
pub fn memory_graph_save(
    app: AppHandle,
    args: MemoryGraphSaveArgs,
) -> Result<ProjectMemory, String> {
    let saved = memory_graph_save_exec(&app, args)?;
    emit_learning_changed(&app, saved.project_id);
    Ok(saved)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    use super::*;
    use crate::db::{
        work_steps_session_key, BacklogItem, MicroTask, NewArtifactDocument, NewProject,
        NewTurnWorkSteps, ScriptBlock,
    };

    static NEXT_DIR: AtomicU32 = AtomicU32::new(0);

    struct TempDbDir(PathBuf);

    impl TempDbDir {
        fn new() -> Self {
            Self(std::env::temp_dir().join(format!(
                "vokia-context-graph-test-{}-{}",
                std::process::id(),
                NEXT_DIR.fetch_add(1, Ordering::SeqCst)
            )))
        }

        fn open(&self) -> Db {
            Db::open(&self.0).expect("temp db should open")
        }
    }

    impl Drop for TempDbDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn create_test_project(db: &Db, name: &str) -> i64 {
        db.insert_project(&NewProject {
            name,
            symbol: "folder",
            description: "",
            path: &format!("C:/tmp/{name}"),
            origin_kind: "folder",
            origin_repo: None,
        })
        .expect("project should be created")
        .id
    }

    fn make_active_card(id: &str, title: &str, who: &str, column: &str, files: Vec<&str>) -> BacklogItem {
        BacklogItem {
            id: id.to_string(),
            public_id: None,
            title: title.to_string(),
            kind: "task".to_string(),
            state: "vinculado".to_string(),
            stage: None,
            triage_step: None,
            parent: None,
            feature: None,
            tags: Vec::new(),
            description: None,
            plan: None,
            plan_note: None,
            children: None,
            who: who.to_string(),
            by: "voce".to_string(),
            is_new: None,
            column: Some(column.to_string()),
            priority: None,
            estimate: None,
            questions: Vec::new(),
            micro: if files.is_empty() {
                Vec::new()
            } else {
                vec![MicroTask {
                    id: format!("{id}-m1"),
                    title: "Executar etapa".to_string(),
                    who: who.to_string(),
                    model: "sonnet".to_string(),
                    status: "em andamento".to_string(),
                    script: ScriptBlock {
                        context: "ctx".to_string(),
                        files: files.into_iter().map(str::to_string).collect(),
                        steps: vec!["step".to_string()],
                        done: vec!["done".to_string()],
                        limits: Vec::new(),
                    },
                    returned: None,
                    learned: None,
                }]
            },
            criteria: Vec::new(),
            activity: Vec::new(),
            links: Vec::new(),
            origin: "usuario".to_string(),
            branch: None,
            worktree_path: None,
            closed_by: None,
            auto_attempts: 0,
            error: None,
        }
    }

    #[test]
    fn empty_project_returns_empty_graph_gracefully() {
        let dir = TempDbDir::new();
        let db = dir.open();
        let project_id = create_test_project(&db, "empty-graph");

        let graph = build_context_graph(&db, project_id).expect("graph should build");
        assert_eq!(graph.project_id, project_id);
        assert!(graph.nodes.is_empty());
        assert!(graph.edges.is_empty());
        assert!(graph.optimizations.is_empty());
    }

    #[test]
    fn detects_conflict_when_two_cards_touch_same_file() {
        let dir = TempDbDir::new();
        let db = dir.open();
        let project_id = create_test_project(&db, "conflict-graph");

        let card_a = make_active_card("PIX-3", "Webhook Pix", "nina", "In progress", vec![]);
        let card_b = make_active_card("PAY-12", "Confirmar Pix", "leo", "In review", vec![]);
        db.save_backlog_item(project_id, &card_a).unwrap();
        db.save_backlog_item(project_id, &card_b).unwrap();

        let key_a = work_steps_session_key(project_id, "nina", Some("PIX-3"));
        db.insert_turn_work_steps(&NewTurnWorkSteps {
            project_id,
            agent: "nina",
            session_key: &key_a,
            card_id: Some("PIX-3"),
            provider: Some("claude"),
            block: "[work steps]\nfiles (ok): edit src/api/pix.ts\n[end work steps]",
            step_count: 1,
            omitted_count: 0,
            created_at: 1_000,
        })
        .unwrap();

        let key_b = work_steps_session_key(project_id, "leo", Some("PAY-12"));
        db.insert_turn_work_steps(&NewTurnWorkSteps {
            project_id,
            agent: "leo",
            session_key: &key_b,
            card_id: Some("PAY-12"),
            provider: Some("claude"),
            block: "[work steps]\nfiles (ok): edit src/api/pix.ts, create src/api/confirmacao.ts\n[end work steps]",
            step_count: 1,
            omitted_count: 0,
            created_at: 2_000,
        })
        .unwrap();

        let graph = build_context_graph(&db, project_id).expect("graph should build");

        let pix_node = graph
            .nodes
            .iter()
            .find(|n| n.kind == "file" && n.path.as_deref() == Some("src/api/pix.ts"))
            .expect("pix.ts node should exist");
        assert!(pix_node.conflict_risk);
        assert_eq!(pix_node.agents, vec!["leo", "nina"]);
        assert_eq!(pix_node.card_ids, vec!["PAY-12", "PIX-3"]);

        let conf_node = graph
            .nodes
            .iter()
            .find(|n| n.kind == "file" && n.path.as_deref() == Some("src/api/confirmacao.ts"))
            .expect("confirmacao.ts node should exist");
        assert!(!conf_node.conflict_risk);

        assert!(graph
            .edges
            .iter()
            .any(|e| e.kind == "conflict_risk" && e.target == pix_node.id));
        assert!(graph
            .optimizations
            .iter()
            .any(|o| o.kind == "conflict" && o.files.contains(&"src/api/pix.ts".to_string())));
        assert!(graph.optimizations.iter().any(|o| o.kind == "order"));
    }

    #[test]
    fn builds_nodes_edges_and_artifact_links_for_active_cards() {
        let dir = TempDbDir::new();
        let db = dir.open();
        let project_id = create_test_project(&db, "full-graph");

        let active = make_active_card(
            "WEB-21",
            "Parcelas no checkout",
            "davi",
            "In progress",
            vec!["src/CheckoutPix.tsx", "src/parcelas.ts"],
        );
        let done_card = make_active_card(
            "WEB-10",
            "Antigo",
            "nina",
            "Done",
            vec!["src/legacy.ts"],
        );
        db.save_backlog_item(project_id, &active).unwrap();
        db.save_backlog_item(project_id, &done_card).unwrap();

        db.create_artifact_document(
            project_id,
            NewArtifactDocument {
                kind: "especificacao".to_string(),
                title: "Spec CheckoutPix.tsx".to_string(),
                summary: Some("Detalha o fluxo de parcelas".to_string()),
                body: None,
                tags: vec!["checkout".to_string()],
                author: Some("iara".to_string()),
            },
        )
        .unwrap();

        let graph = build_context_graph(&db, project_id).expect("graph should build");

        assert!(graph.nodes.iter().any(|n| n.kind == "card" && n.label == "WEB-21"));
        assert!(!graph.nodes.iter().any(|n| n.kind == "card" && n.label == "WEB-10"));
        assert!(graph.nodes.iter().any(|n| n.kind == "artifact" && n.label == "Spec CheckoutPix.tsx"));
        assert!(graph.edges.iter().any(|e| e.kind == "task_path"));
        assert!(graph.edges.iter().any(|e| e.kind == "artifact_link"));
        assert!(graph.optimizations.iter().any(|o| o.kind == "context"));
        assert!(graph.optimizations.iter().any(|o| o.kind == "artifact"));
    }

    #[test]
    fn e2e_full_card_triage_plan_and_memory_graph_flow() {
        let dir = TempDbDir::new();
        let db = dir.open();
        let project_id = create_test_project(&db, "e2e-flow");

        // 1. Card criado em triagem com plano multi-etapa (2 micro-etapas)
        let mut card = make_active_card(
            "E2E-1",
            "Implementar autenticação com refresh token",
            "kairo",
            "Backlog",
            vec!["src/auth/token.rs", "src/auth/middleware.ts"],
        );
        let mut step2 = card.micro[0].clone();
        step2.id = "E2E-1-m2".to_string();
        step2.title = "Integrar middleware e testes".to_string();
        step2.status = "ready".to_string();
        card.micro.push(step2);
        db.save_backlog_item(project_id, &card).unwrap();

        // 2. Agente salva decisão arquitetural conectada a palavras-chave e card
        let saved = save_memory_graph_fact(
            &db,
            MemoryGraphSaveArgs {
                project_id,
                fact: "Usar rotação de refresh token com revogação em SQLite".to_string(),
                category: Some("decisao".to_string()),
                files: vec!["src/auth/token.rs".to_string()],
                card_id: Some("E2E-1".to_string()),
                author: Some("kairo".to_string()),
                wing: None,
                room: None,
                supersede_id: None,
                keywords: vec!["autenticacao".to_string(), "refresh".to_string()],
            },
        )
        .expect("memory_graph_save should succeed");
        assert_eq!(saved.source, "decisao");

        // 3. Aceita o plano e inicia desenvolvimento (In progress + atualização de etapa)
        card.column = Some("In progress".to_string());
        card.micro[0].status = "feito".to_string();
        card.micro[1].status = "em andamento".to_string();
        db.save_backlog_item(project_id, &card).unwrap();

        // 4. Outro agente consulta o grafo via palavra-chave (2-hop)
        let query_res = query_memory_graph(&db, project_id, "autenticacao", 10)
            .expect("memory_graph_query should succeed");
        assert!(query_res.nodes.iter().any(|n| n.kind == "decision"));
        assert!(query_res.nodes.iter().any(|n| n.kind == "keyword" && n.label == "autenticacao"));
        assert!(query_res.compact_context.contains("[decisões]"));

        // 5. Contexto compacto injetado nas delegações contém decisões, palavras-chave e instruções
        let compact = build_compact_memory_context(&db, project_id, Some("E2E-1"), Some("kairo"), 2200);
        assert!(compact.contains("[núcleo]"));
        assert!(compact.contains("[palavras-chave]"));
        assert!(compact.contains("memory_graph_query"));
    }
}
