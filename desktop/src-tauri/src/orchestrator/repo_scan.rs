use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::agents::{Agent, AgentRegistry};
use crate::commands::learning::{emit_learning_changed, save_memory_graph_fact, MemoryGraphSaveArgs};
use crate::db::DbState;
use crate::events::Speaker;
use crate::state::AppState;

use super::emit::{emit_feed_note, now_millis};
use super::fallback::ask_project_readonly_with_fallback;

const MAX_SCAN_DEPTH: usize = 5;
const MAX_SCAN_FILES: usize = 260;
const MAX_MANIFEST_LINES: usize = 35;
const MAX_ENTRIES_PERSISTED: usize = 24;

const IGNORED_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "dist",
    "build",
    ".next",
    ".nuxt",
    ".venv",
    "venv",
    "__pycache__",
    "coverage",
    ".cache",
    ".turbo",
    "vendor",
];

const MANIFEST_NAMES: &[&str] = &[
    "README.md",
    "CLAUDE.md",
    "AGENTS.md",
    "package.json",
    "Cargo.toml",
    "pyproject.toml",
    "go.mod",
    "tsconfig.json",
];

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanProjectGraphOutcome {
    pub status: String,
    pub project_id: i64,
    pub agent: String,
    pub created_memories: usize,
    pub linked_files: usize,
    pub summary: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedRepoScanEntry {
    pub fact: String,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub wing: Option<String>,
    #[serde(default)]
    pub room: Option<String>,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub files: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedRepoScan {
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub entries: Vec<ParsedRepoScanEntry>,
}

#[derive(Clone, Debug, Default)]
pub struct RepoMapSnapshot {
    pub total_files: usize,
    pub top_dirs: BTreeMap<String, usize>,
    pub files: Vec<String>,
    pub manifests: Vec<(String, String)>,
}

impl RepoMapSnapshot {
    pub fn format_prompt_block(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "[mapa determinístico do repositório] {} arquivo(s) mapeados (limite {}).\n",
            self.total_files, MAX_SCAN_FILES
        ));
        if !self.top_dirs.is_empty() {
            out.push_str("\n[diretórios principais]\n");
            for (dir, count) in &self.top_dirs {
                out.push_str(&format!("- {dir}: {count} arquivo(s)\n"));
            }
        }
        if !self.files.is_empty() {
            out.push_str("\n[árvore resumida de arquivos]\n");
            for path in self.files.iter().take(160) {
                out.push_str(&format!("- {path}\n"));
            }
        }
        if !self.manifests.is_empty() {
            out.push_str("\n[manifestos e guias]\n");
            for (name, excerpt) in &self.manifests {
                out.push_str(&format!("--- {name} ---\n{excerpt}\n"));
            }
        }
        out
    }
}

pub fn build_repo_map_snapshot(root: &Path) -> RepoMapSnapshot {
    let mut snapshot = RepoMapSnapshot::default();
    walk_repo(root, root, 0, &mut snapshot);
    snapshot
}

fn walk_repo(root: &Path, dir: &Path, depth: usize, out: &mut RepoMapSnapshot) {
    if depth > MAX_SCAN_DEPTH || out.files.len() >= MAX_SCAN_FILES {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut items: Vec<_> = entries.filter_map(Result::ok).collect();
    items.sort_by_key(|entry| entry.file_name());

    for entry in items {
        if out.files.len() >= MAX_SCAN_FILES {
            break;
        }
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            if IGNORED_DIRS.contains(&name.as_str()) {
                continue;
            }
            walk_repo(root, &path, depth + 1, out);
            continue;
        }
        let Ok(rel) = path.strip_prefix(root) else {
            continue;
        };
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        if rel_str.ends_with(".lock")
            || rel_str.ends_with(".png")
            || rel_str.ends_with(".jpg")
            || rel_str.ends_with(".woff2")
            || rel_str.ends_with(".woff")
            || rel_str.ends_with(".db")
            || rel_str.ends_with(".sqlite")
        {
            continue;
        }
        out.total_files += 1;
        let top = rel_str.split('/').next().unwrap_or("raiz").to_string();
        *out.top_dirs.entry(top).or_insert(0) += 1;
        out.files.push(rel_str.clone());

        if (depth <= 1 || MANIFEST_NAMES.contains(&name.as_str()))
            && MANIFEST_NAMES.iter().any(|m| m.eq_ignore_ascii_case(&name))
            && out.manifests.len() < 8
        {
            if let Ok(content) = fs::read_to_string(&path) {
                let excerpt = content
                    .lines()
                    .take(MAX_MANIFEST_LINES)
                    .collect::<Vec<_>>()
                    .join("\n");
                if !excerpt.trim().is_empty() {
                    out.manifests.push((rel_str, excerpt));
                }
            }
        }
    }
}

pub fn parse_repo_scan_json(raw: &str) -> Option<ParsedRepoScan> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(parsed) = serde_json::from_str::<ParsedRepoScan>(trimmed) {
        return Some(parsed);
    }
    let start = trimmed.find('{')?;
    let end = trimmed.rfind('}')?;
    if end <= start {
        return None;
    }
    serde_json::from_str::<ParsedRepoScan>(&trimmed[start..=end]).ok()
}

pub(crate) async fn scan_project_graph_for_project(
    app: &AppHandle,
    project_id: i64,
) -> Result<ScanProjectGraphOutcome, String> {
    let project = {
        let db_state = app.state::<DbState>();
        let db = db_state.require().map_err(|e| e.to_string())?;
        db.project(project_id).map_err(|e| e.to_string())?
    };

    let root = Path::new(&project.path);
    if project.path.trim().is_empty() || !root.exists() {
        return Err("O projeto não possui um caminho de repositório válido no disco.".to_string());
    }

    let snapshot = build_repo_map_snapshot(root);
    let team = app
        .state::<AgentRegistry>()
        .team()
        .map_err(|e| e.to_string())?;
    let profile = team
        .profile(&Agent::ember())
        .or_else(|| team.profiles().into_iter().next())
        .cloned()
        .ok_or_else(|| "Nenhum agente configurado para varrer o repositório.".to_string())?;

    let state = app.state::<AppState>();
    let default_cli = state
        .cli
        .as_deref()
        .map_err(|problem| problem.reason.clone())?;

    let role = format!(
        "Você é {}, especialista em mapear repositórios em grafos de conhecimento (estilo MemPalace wings/rooms + ai-memory + Vesalius). \
        Explore o repositório em modo somente leitura para entender arquitetura, pontos de entrada, fluxos, módulos e convenções. \
        Responda EXCLUSIVAMENTE com um JSON válido, sem texto antes ou depois.",
        profile.name
    );

    let prompt = format!(
        "Projeto: {} (#{})\nCaminho: {}\n\n{}\n\
        Tarefa:\n\
        1. Analise a árvore de arquivos, manifestos e leia os pontos de entrada/módulos principais necessários para entender o sistema todo.\n\
        2. Estruture o conhecimento do repositório em memórias objetivas e interconectadas (entre 6 e 16 entradas).\n\
        3. Use `wing` para a camada/domínio (ex: arquitetura, frontend, backend, dados, infra, fluxos) e `room` para o submódulo/área.\n\
        4. Em `files`, liste apenas caminhos relativos reais observados no repositório.\n\
        5. Responda SOMENTE com este JSON:\n\
        {{\n  \"summary\": \"Resumo arquitetural objetivo em 1-2 frases\",\n  \"entries\": [\n    {{\n      \"fact\": \"Descrição clara do módulo, fluxo ou convenção com símbolos e arquivos reais\",\n      \"category\": \"macro | mapa-arquivos | decisao\",\n      \"wing\": \"arquitetura\",\n      \"room\": \"nome-do-modulo\",\n      \"keywords\": [\"palavra1\", \"palavra2\"],\n      \"files\": [\"caminho/real/arquivo.ext\"]\n    }}\n  ]\n}}",
        project.name,
        project.id,
        project.path,
        snapshot.format_prompt_block()
    );

    let reply = ask_project_readonly_with_fallback(
        app,
        &state,
        default_cli,
        &profile.id,
        &profile,
        project.id,
        root,
        &prompt,
        &role,
    )
    .await
    .map_err(|e| e.to_string())?;

    let parsed = parse_repo_scan_json(&reply.text).ok_or_else(|| {
        "O agente respondeu, mas não retornou um JSON válido para popular o grafo.".to_string()
    })?;

    let known_files: BTreeSet<String> = snapshot.files.iter().cloned().collect();
    let mut created_memories = 0usize;
    let mut linked_files_set = BTreeSet::new();

    {
        let db_state = app.state::<DbState>();
        let db = db_state.require().map_err(|e| e.to_string())?;
        for entry in parsed.entries.into_iter().take(MAX_ENTRIES_PERSISTED) {
            let fact = entry.fact.trim();
            if fact.is_empty() {
                continue;
            }
            let files: Vec<String> = entry
                .files
                .into_iter()
                .map(|f| f.trim().replace('\\', "/").trim_start_matches("./").to_string())
                .filter(|f| !f.is_empty() && (known_files.contains(f) || root.join(f).exists()))
                .collect();
            for f in &files {
                linked_files_set.insert(f.clone());
            }
            let args = MemoryGraphSaveArgs {
                project_id: project.id,
                fact: fact.to_string(),
                category: entry.category.or_else(|| Some("mapa-arquivos".to_string())),
                files,
                card_id: None,
                author: Some(profile.id.as_str().to_string()),
                wing: entry.wing.or_else(|| Some("arquitetura".to_string())),
                room: entry.room,
                supersede_id: None,
                keywords: entry.keywords,
            };
            if save_memory_graph_fact(db, args).is_ok() {
                created_memories += 1;
            }
        }
    }

    emit_learning_changed(app, project.id);
    let summary = if parsed.summary.trim().is_empty() {
        format!(
            "{} mapeou {created_memories} memórias e {} arquivos no grafo de {}.",
            profile.name,
            linked_files_set.len(),
            project.name
        )
    } else {
        parsed.summary.trim().to_string()
    };
    let _ = emit_feed_note(
        app,
        project.id,
        Speaker::Agent(profile.id.clone()),
        &format!(
            "Varredura do grafo concluída por {}: {created_memories} memórias e {} arquivos conectados.",
            profile.name,
            linked_files_set.len()
        ),
        now_millis().unwrap_or(0),
    );

    Ok(ScanProjectGraphOutcome {
        status: "completed".to_string(),
        project_id: project.id,
        agent: profile.name.clone(),
        created_memories,
        linked_files: linked_files_set.len(),
        summary,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static NEXT_DIR: AtomicU32 = AtomicU32::new(0);

    #[test]
    fn build_repo_map_snapshot_skips_ignored_dirs_and_reads_manifests() {
        let dir = std::env::temp_dir().join(format!(
            "vokia-repo-scan-test-{}-{}",
            std::process::id(),
            NEXT_DIR.fetch_add(1, Ordering::SeqCst)
        ));
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::create_dir_all(dir.join("node_modules/pkg")).unwrap();
        fs::write(dir.join("README.md"), "# Meu Projeto\nDescrição breve").unwrap();
        fs::write(dir.join("src/main.rs"), "fn main() {}").unwrap();
        fs::write(dir.join("node_modules/pkg/index.js"), "module.exports = 1").unwrap();

        let snap = build_repo_map_snapshot(&dir);
        assert!(snap.files.contains(&"README.md".to_string()));
        assert!(snap.files.contains(&"src/main.rs".to_string()));
        assert!(!snap.files.iter().any(|f| f.contains("node_modules")));
        assert_eq!(snap.manifests.len(), 1);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn parse_repo_scan_json_accepts_markdown_fences() {
        let raw = "```json\n{\"summary\":\"App Vue + Rust\",\"entries\":[{\"fact\":\"Orquestrador em Rust\",\"category\":\"macro\",\"wing\":\"backend\",\"room\":\"orchestrator\",\"keywords\":[\"rust\",\"orquestrador\"],\"files\":[\"src/main.rs\"]}]}\n```";
        let parsed = parse_repo_scan_json(raw).expect("should parse fenced json");
        assert_eq!(parsed.summary, "App Vue + Rust");
        assert_eq!(parsed.entries.len(), 1);
        assert_eq!(parsed.entries[0].wing.as_deref(), Some("backend"));
    }
}
