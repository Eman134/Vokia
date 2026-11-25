use crate::claude::{ToolActivity, ToolCategory, ToolDetail, ToolStatus, TurnActivity};

use super::step_labels::redact_secrets;

pub const MAX_STEP_CHARS: usize = 1_000;
pub const MAX_TURN_WORK_STEPS_CHARS: usize = 4_000;
pub const MAX_COMMAND_OUTPUT_TAIL_CHARS: usize = 600;

pub const WORK_STEPS_OPEN: &str = "[work steps]";
pub const WORK_STEPS_CLOSE: &str = "[end work steps]";

pub const HANDOFF_MAX_CHARS: usize = 32_000;
pub const HANDOFF_RECENT_BUDGET: usize = 24_000;
pub const HANDOFF_OLDER_BUDGET: usize = 4_000;
pub const HANDOFF_WORK_STEPS_BUDGET: usize = 2_000;
pub const HANDOFF_CONTINUITY_BUDGET: usize = 2_000;

const REDACTED: &str = "[oculto]";
const ELLIPSIS: &str = "…";
const MID_OMISSION_MARKER: &str = "\n[... trecho omitido ...]\n";
const MAX_OUTPUT_REDACT_WINDOW_CHARS: usize = 8_192;
const DEFAULT_OLDER_EXCERPT_CHARS: usize = 480;

const COMPACT_SECRET_KEYS: [&str; 13] = [
    "TOKEN",
    "SECRET",
    "PASSWORD",
    "PASSWD",
    "API_KEY",
    "APIKEY",
    "PRIVATE_KEY",
    "CREDENTIAL",
    "AUTHORIZATION",
    "ACCESS_KEY",
    "AUTH_KEY",
    "SIGNING_KEY",
    "CLIENT_SECRET",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileChangeKind {
    Edit,
    Create,
    Delete,
}

impl FileChangeKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Edit => "edit",
            Self::Create => "create",
            Self::Delete => "delete",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileChange {
    pub kind: FileChangeKind,
    pub path: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkStep {
    Command {
        command: String,
        exit_code: Option<i32>,
        status: String,
        output_tail: Option<String>,
    },
    Files {
        status: String,
        changes: Vec<FileChange>,
    },
    Tool {
        server: Option<String>,
        name: String,
        status: String,
    },
    WebSearch {
        query: String,
    },
    Note {
        text: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormattedWorkSteps {
    pub block: String,
    pub step_count: usize,
    pub omitted_count: usize,
}

#[derive(Clone, Debug, Default)]
pub struct WorkStepCollector {
    steps: Vec<String>,
}

impl WorkStepCollector {
    pub fn new() -> Self {
        Self::default()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn len(&self) -> usize {
        self.steps.len()
    }

    pub fn record(&mut self, step: WorkStep) {
        if let Some(formatted) = format_single_step(step) {
            self.steps.push(formatted);
        }
    }

    pub fn record_command(
        &mut self,
        command: &str,
        exit_code: Option<i32>,
        status: &str,
        output: Option<&str>,
    ) {
        self.record(WorkStep::Command {
            command: command.to_string(),
            exit_code,
            status: status.to_string(),
            output_tail: output.map(str::to_string),
        });
    }

    pub fn record_files(&mut self, status: &str, changes: &[FileChange]) {
        self.record(WorkStep::Files {
            status: status.to_string(),
            changes: changes.to_vec(),
        });
    }

    pub fn record_tool(&mut self, server: Option<&str>, name: &str, status: &str) {
        self.record(WorkStep::Tool {
            server: server.map(str::to_string),
            name: name.to_string(),
            status: status.to_string(),
        });
    }

    pub fn record_web_search(&mut self, query: &str) {
        self.record(WorkStep::WebSearch {
            query: query.to_string(),
        });
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn record_note(&mut self, text: &str) {
        self.record(WorkStep::Note {
            text: text.to_string(),
        });
    }

    // Constrói passos normalizados a partir de TurnActivity, ignorando explicitamente reasoning cru
    // (`thinking_text` / `thinking_ms`) e leituras puras ruidosas.
    pub fn from_turn_activity(activity: &TurnActivity) -> Self {
        let mut collector = Self::new();
        let mut pending_files: Option<(String, Vec<FileChange>)> = None;

        let flush_files =
            |collector: &mut WorkStepCollector, pending: &mut Option<(String, Vec<FileChange>)>| {
                if let Some((status, changes)) = pending.take() {
                    if !changes.is_empty() {
                        collector.record_files(&status, &changes);
                    }
                }
            };

        for tool in &activity.tools {
            let status = status_label(tool.status);
            match tool.category {
                ToolCategory::Edit => {
                    let path = tool.target.trim();
                    if path.is_empty() {
                        continue;
                    }
                    let kind = infer_file_change_kind(tool);
                    let change = FileChange {
                        kind,
                        path: path.to_string(),
                    };
                    match &mut pending_files {
                        Some((current_status, changes)) if current_status == status => {
                            if !changes.iter().any(|c| c.kind == change.kind && c.path == change.path) {
                                changes.push(change);
                            }
                        }
                        _ => {
                            flush_files(&mut collector, &mut pending_files);
                            pending_files = Some((status.to_string(), vec![change]));
                        }
                    }
                }
                ToolCategory::Command => {
                    flush_files(&mut collector, &mut pending_files);
                    let cmd = tool.target.trim();
                    if !cmd.is_empty() {
                        collector.record_command(cmd, tool.exit_code, status, tool.result.as_deref());
                    }
                }
                ToolCategory::Mcp => {
                    flush_files(&mut collector, &mut pending_files);
                    let (server, name) = split_mcp_tool_name(&tool.name);
                    collector.record_tool(server.as_deref(), &name, status);
                }
                ToolCategory::App => {
                    flush_files(&mut collector, &mut pending_files);
                    let name = match &tool.detail {
                        Some(ToolDetail::AppCommand { command, .. }) if !command.trim().is_empty() => {
                            command.as_str()
                        }
                        _ if !tool.target.trim().is_empty() => tool.target.as_str(),
                        _ => tool.name.as_str(),
                    };
                    collector.record_tool(Some("vokia"), name, status);
                }
                ToolCategory::Web => {
                    flush_files(&mut collector, &mut pending_files);
                    let lower = tool.name.to_ascii_lowercase();
                    if lower.contains("search") {
                        let query = tool.target.trim();
                        if !query.is_empty() {
                            collector.record_web_search(query);
                        }
                    } else {
                        collector.record_tool(Some("web"), &tool.name, status);
                    }
                }
                ToolCategory::Plan | ToolCategory::Agent | ToolCategory::Other => {
                    flush_files(&mut collector, &mut pending_files);
                    if !tool.name.trim().is_empty() {
                        collector.record_tool(None, &tool.name, status);
                    }
                }
                ToolCategory::Read | ToolCategory::List | ToolCategory::Search => {
                    // Leituras e buscas locais não entram no resumo compacto de etapas executadas.
                }
            }
        }
        flush_files(&mut collector, &mut pending_files);
        collector
    }

    // Formata o bloco `[work steps] ... [end work steps]` respeitando o teto rígido de 4.000 chars.
    // Quando excede, descarta os passos mais antigos e inclui `(<N> passos anteriores omitidos)`.
    pub fn format(&self) -> Option<FormattedWorkSteps> {
        if self.steps.is_empty() {
            return None;
        }
        let total = self.steps.len();
        let full = render_work_steps_block(&self.steps, 0);
        if full.chars().count() <= MAX_TURN_WORK_STEPS_CHARS {
            return Some(FormattedWorkSteps {
                block: full,
                step_count: total,
                omitted_count: 0,
            });
        }

        for omitted in 1..total {
            let candidate = render_work_steps_block(&self.steps[omitted..], omitted);
            if candidate.chars().count() <= MAX_TURN_WORK_STEPS_CHARS {
                return Some(FormattedWorkSteps {
                    block: candidate,
                    step_count: total,
                    omitted_count: omitted,
                });
            }
        }

        // Fallback extremo: garante o teto mesmo se um passo individual tivesse tamanho limite.
        let omitted = total.saturating_sub(1);
        let notice = omitted_line(omitted);
        let overhead = WORK_STEPS_OPEN.chars().count()
            + 1
            + notice.chars().count()
            + 1
            + 1
            + WORK_STEPS_CLOSE.chars().count();
        let allowed = MAX_TURN_WORK_STEPS_CHARS.saturating_sub(overhead);
        let last = truncate_chars(self.steps.last().map(String::as_str).unwrap_or(""), allowed);
        let block = format!("{WORK_STEPS_OPEN}\n{notice}\n{last}\n{WORK_STEPS_CLOSE}");
        Some(FormattedWorkSteps {
            block,
            step_count: total,
            omitted_count: omitted,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HandoffMessage {
    pub author: String,
    pub text: String,
}

#[derive(Clone, Debug, Default)]
pub struct HandoffInput {
    pub from_provider: Option<String>,
    pub to_provider: Option<String>,
    pub reason: Option<String>,
    pub objective: Option<String>,
    pub current_request: Option<String>,
    pub card_summary: Option<String>,
    pub open_plan_steps: Vec<String>,
    pub recent_messages: Vec<HandoffMessage>,
    pub older_messages: Vec<HandoffMessage>,
    pub work_steps_blocks: Vec<String>,
    pub history_pointers: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HandoffPacket {
    pub text: String,
    pub total_chars: usize,
    pub recent_chars: usize,
    pub older_chars: usize,
    pub work_steps_chars: usize,
    pub continuity_chars: usize,
}

// Monta pacote determinístico de passagem de contexto com teto de 32.000 caracteres e orçamentos
// reservados (24k recentes, 4k antigos, 2k [work steps], 2k continuidade/plano/ponteiros).
// Espaço não utilizado em qualquer bloco reservado é reaproveitado para mais texto recente.
pub fn build_handoff_packet(input: &HandoffInput) -> HandoffPacket {
    const WRAPPER_OPEN: &str = "[handoff]\n";
    const WRAPPER_CLOSE: &str = "\n[end handoff]";
    const SECTION_SEP: &str = "\n\n";

    let wrapper_overhead = WRAPPER_OPEN.chars().count() + WRAPPER_CLOSE.chars().count();
    let max_body = HANDOFF_MAX_CHARS.saturating_sub(wrapper_overhead);

    // 1. Continuidade, objetivo, etapas abertas do plano e ponteiros para consulta de histórico (<= 2.000).
    let continuity_section = build_continuity_section(input, HANDOFF_CONTINUITY_BUDGET);
    let continuity_chars = continuity_section.chars().count();
    let unused_continuity = HANDOFF_CONTINUITY_BUDGET.saturating_sub(continuity_chars);

    // 2. Blocos [work steps] capturados (<= 2.000).
    let work_steps_section = build_work_steps_section(&input.work_steps_blocks, HANDOFF_WORK_STEPS_BUDGET);
    let work_steps_chars = work_steps_section.chars().count();
    let unused_work_steps = HANDOFF_WORK_STEPS_BUDGET.saturating_sub(work_steps_chars);

    // 3. Excertos mais antigos (<= 4.000). Quando `older_messages` vier vazio e `recent_messages`
    // estourar o orçamento primário de recentes, os excedentes mais antigos alimentam este bloco.
    let explicit_older = !input.older_messages.is_empty();
    let (older_section, recent_section) = if explicit_older {
        let older = build_older_section(&input.older_messages, HANDOFF_OLDER_BUDGET);
        let older_chars = older.chars().count();
        let unused_older = HANDOFF_OLDER_BUDGET.saturating_sub(older_chars);
        let sep_overhead = section_separator_overhead(
            !continuity_section.is_empty(),
            !older.is_empty(),
            !work_steps_section.is_empty(),
            !input.recent_messages.is_empty(),
        );
        let recent_budget = (HANDOFF_RECENT_BUDGET + unused_continuity + unused_work_steps + unused_older)
            .min(max_body.saturating_sub(continuity_chars + work_steps_chars + older_chars + sep_overhead));
        let (recent, _) = build_recent_section(&input.recent_messages, recent_budget);
        (older, recent)
    } else {
        // Primeiro tenta encaixar tudo com o orçamento de recentes + sobras (sem older).
        let sep_without_older = section_separator_overhead(
            !continuity_section.is_empty(),
            false,
            !work_steps_section.is_empty(),
            !input.recent_messages.is_empty(),
        );
        let primary_recent_budget = (HANDOFF_RECENT_BUDGET + unused_continuity + unused_work_steps)
            .min(max_body.saturating_sub(continuity_chars + work_steps_chars + sep_without_older));
        let (primary_recent, overflow_count) =
            build_recent_section(&input.recent_messages, primary_recent_budget);

        if overflow_count == 0 {
            (String::new(), primary_recent)
        } else {
            // Mensagens mais antigas que não couberam entram como excertos no orçamento de 4.000 chars.
            let older = build_older_section(
                &input.recent_messages[..overflow_count],
                HANDOFF_OLDER_BUDGET,
            );
            let older_chars = older.chars().count();
            let unused_older = HANDOFF_OLDER_BUDGET.saturating_sub(older_chars);
            let sep_with_older = section_separator_overhead(
                !continuity_section.is_empty(),
                !older.is_empty(),
                !work_steps_section.is_empty(),
                true,
            );
            let final_recent_budget =
                (HANDOFF_RECENT_BUDGET + unused_continuity + unused_work_steps + unused_older)
                    .min(
                        max_body
                            .saturating_sub(continuity_chars + work_steps_chars + older_chars + sep_with_older),
                    );
            let (recent, _) = build_recent_section(&input.recent_messages[overflow_count..], final_recent_budget);
            (older, recent)
        }
    };

    let older_chars = older_section.chars().count();
    let recent_chars = recent_section.chars().count();

    let mut sections: Vec<&str> = Vec::with_capacity(4);
    if !continuity_section.is_empty() {
        sections.push(&continuity_section);
    }
    if !older_section.is_empty() {
        sections.push(&older_section);
    }
    if !work_steps_section.is_empty() {
        sections.push(&work_steps_section);
    }
    if !recent_section.is_empty() {
        sections.push(&recent_section);
    }

    let body = sections.join(SECTION_SEP);
    let mut text = if body.is_empty() {
        format!("{}{}", WRAPPER_OPEN.trim_end(), WRAPPER_CLOSE)
    } else {
        format!("{WRAPPER_OPEN}{body}{WRAPPER_CLOSE}")
    };

    if text.chars().count() > HANDOFF_MAX_CHARS {
        let allowed_body = HANDOFF_MAX_CHARS.saturating_sub(wrapper_overhead);
        let clipped_body = truncate_chars(&body, allowed_body);
        text = format!("{WRAPPER_OPEN}{clipped_body}{WRAPPER_CLOSE}");
    }

    let total_chars = text.chars().count();
    HandoffPacket {
        text,
        total_chars,
        recent_chars,
        older_chars,
        work_steps_chars,
        continuity_chars,
    }
}

fn section_separator_overhead(a: bool, b: bool, c: bool, d: bool) -> usize {
    let count = usize::from(a) + usize::from(b) + usize::from(c) + usize::from(d);
    count.saturating_sub(1) * 2
}

fn build_continuity_section(input: &HandoffInput, budget: usize) -> String {
    let mut lines: Vec<String> = Vec::new();
    match (&input.from_provider, &input.to_provider) {
        (Some(from), Some(to)) => {
            let reason_suffix = input
                .reason
                .as_deref()
                .map(str::trim)
                .filter(|r| !r.is_empty())
                .map(|r| format!(" ({})", redact_work_step_text(r)))
                .unwrap_or_default();
            lines.push(format!(
                "troca_provedor: {} -> {}{reason_suffix}",
                redact_work_step_text(from.trim()),
                redact_work_step_text(to.trim())
            ));
        }
        (None, Some(to)) => {
            lines.push(format!(
                "sessao_substituta: {}",
                redact_work_step_text(to.trim())
            ));
        }
        _ => {}
    }

    if let Some(objective) = input
        .objective
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        lines.push(format!(
            "objetivo: {}",
            truncate_chars(&collapse_whitespace(&redact_work_step_text(objective)), 280)
        ));
    }

    if !input.open_plan_steps.is_empty() {
        lines.push("etapas_abertas:".to_string());
        for step in input.open_plan_steps.iter().take(10) {
            let clean = collapse_whitespace(&redact_work_step_text(step));
            if !clean.is_empty() {
                lines.push(format!("- {}", truncate_chars(&clean, 140)));
            }
        }
    }

    if let Some(card) = input
        .card_summary
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        lines.push(format!(
            "card: {}",
            truncate_chars(&collapse_whitespace(&redact_work_step_text(card)), 360)
        ));
    }

    if let Some(request) = input
        .current_request
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        lines.push(format!(
            "pedido_atual: {}",
            truncate_chars(&collapse_whitespace(&redact_work_step_text(request)), 360)
        ));
    }

    lines.push(
        "continuidade: Retome a tarefa a partir do estado acima sem repetir comandos ou edições já concluídos."
            .to_string(),
    );

    if input.history_pointers.is_empty() {
        lines.push(
            "historico: Use history_search ou history_read (includeWorkSteps=true, workStepsOffset) para consultar turnos anteriores."
                .to_string(),
        );
    } else {
        for ptr in &input.history_pointers {
            let clean = collapse_whitespace(&redact_work_step_text(ptr));
            if !clean.is_empty() {
                lines.push(format!("historico: {}", truncate_chars(&clean, 200)));
            }
        }
    }

    fit_lines_to_budget(&lines, budget)
}

fn build_work_steps_section(blocks: &[String], budget: usize) -> String {
    if blocks.is_empty() || budget == 0 {
        return String::new();
    }
    let cleaned: Vec<String> = blocks
        .iter()
        .map(|b| redact_work_step_text(b.trim()))
        .filter(|b| !b.is_empty())
        .collect();
    if cleaned.is_empty() {
        return String::new();
    }

    let mut chosen_rev: Vec<String> = Vec::new();
    let mut used = 0usize;

    for block in cleaned.iter().rev() {
        let len = block.chars().count();
        let sep = if chosen_rev.is_empty() { 0 } else { 1 };
        if used + sep + len <= budget {
            used += sep + len;
            chosen_rev.push(block.clone());
        } else if chosen_rev.is_empty() {
            // Mesmo o bloco mais recente excede 2.000 chars (um turno pode ter até 4.000):
            // compacta preservando os passos mais recentes dentro do orçamento.
            if let Some(compacted) = trim_work_steps_block_to_budget(block, budget) {
                chosen_rev.push(compacted);
            }
            break;
        } else {
            break;
        }
    }

    chosen_rev.reverse();
    chosen_rev.join("\n")
}

fn trim_work_steps_block_to_budget(block: &str, max_chars: usize) -> Option<String> {
    let min_wrapper = WORK_STEPS_OPEN.chars().count() + 1 + WORK_STEPS_CLOSE.chars().count();
    if max_chars <= min_wrapper {
        return None;
    }
    let mut existing_omitted = 0usize;
    let mut step_lines: Vec<String> = Vec::new();

    for line in block.lines() {
        let trimmed = line.trim();
        if trimmed == WORK_STEPS_OPEN || trimmed == WORK_STEPS_CLOSE || trimmed.is_empty() {
            continue;
        }
        if let Some(count) = parse_omitted_count(trimmed) {
            existing_omitted += count;
            continue;
        }
        if line.starts_with("  ") {
            if let Some(last) = step_lines.last_mut() {
                last.push('\n');
                last.push_str(line);
                continue;
            }
        }
        step_lines.push(line.to_string());
    }

    if step_lines.is_empty() {
        return None;
    }

    for drop in 0..step_lines.len() {
        let omitted = existing_omitted + drop;
        let candidate = render_work_steps_block(&step_lines[drop..], omitted);
        if candidate.chars().count() <= max_chars {
            return Some(candidate);
        }
    }

    let omitted = existing_omitted + step_lines.len().saturating_sub(1);
    let notice = if omitted > 0 {
        format!("{}\n", omitted_line(omitted))
    } else {
        String::new()
    };
    let overhead = WORK_STEPS_OPEN.chars().count()
        + 1
        + notice.chars().count()
        + 1
        + WORK_STEPS_CLOSE.chars().count();
    let allowed = max_chars.saturating_sub(overhead);
    let last = truncate_chars(step_lines.last().map(String::as_str).unwrap_or(""), allowed);
    Some(format!("{WORK_STEPS_OPEN}\n{notice}{last}\n{WORK_STEPS_CLOSE}"))
}

fn parse_omitted_count(line: &str) -> Option<usize> {
    let inner = line.strip_prefix('(')?.strip_suffix(')')?;
    let num = inner.split_whitespace().next()?;
    if inner.contains("omitido") {
        num.parse::<usize>().ok()
    } else {
        None
    }
}

fn build_older_section(messages: &[HandoffMessage], budget: usize) -> String {
    if messages.is_empty() || budget == 0 {
        return String::new();
    }
    const HEADER: &str = "excertos_anteriores:\n";
    let header_len = HEADER.chars().count();
    if budget <= header_len + 16 {
        return String::new();
    }
    let body_budget = budget - header_len;
    let mut chosen_rev: Vec<String> = Vec::new();
    let mut used = 0usize;

    for msg in messages.iter().rev() {
        let author = collapse_whitespace(&redact_work_step_text(&msg.author));
        let text = collapse_whitespace(&redact_work_step_text(&msg.text));
        if text.is_empty() {
            continue;
        }
        let prefix = if author.is_empty() {
            "- ".to_string()
        } else {
            format!("- {author}: ")
        };
        let remaining = body_budget.saturating_sub(used + usize::from(!chosen_rev.is_empty()));
        if remaining <= prefix.chars().count() + 8 {
            break;
        }
        let max_text = remaining
            .saturating_sub(prefix.chars().count())
            .min(DEFAULT_OLDER_EXCERPT_CHARS);
        let excerpt = preserve_head_and_tail(&text, max_text);
        let entry = format!("{prefix}{excerpt}");
        let entry_len = entry.chars().count();
        let sep = usize::from(!chosen_rev.is_empty());
        if used + sep + entry_len <= body_budget {
            used += sep + entry_len;
            chosen_rev.push(entry);
        } else {
            break;
        }
    }

    if chosen_rev.is_empty() {
        return String::new();
    }
    chosen_rev.reverse();
    format!("{HEADER}{}", chosen_rev.join("\n"))
}

// Retorna `(seção_formatada, quantidade_de_mensagens_mais_antigas_não_incluídas)`.
fn build_recent_section(messages: &[HandoffMessage], budget: usize) -> (String, usize) {
    if messages.is_empty() || budget == 0 {
        return (String::new(), messages.len());
    }
    const HEADER: &str = "mensagens_recentes:\n";
    let header_len = HEADER.chars().count();
    if budget <= header_len + 16 {
        return (String::new(), messages.len());
    }
    let body_budget = budget - header_len;
    let mut chosen_rev: Vec<String> = Vec::new();
    let mut used = 0usize;
    let mut included = 0usize;

    for msg in messages.iter().rev() {
        let author = collapse_whitespace(&redact_work_step_text(&msg.author));
        let text = redact_work_step_text(msg.text.trim());
        if text.is_empty() {
            included += 1;
            continue;
        }
        let prefix = if author.is_empty() {
            "- ".to_string()
        } else {
            format!("- {author}: ")
        };
        let sep = usize::from(!chosen_rev.is_empty());
        let remaining = body_budget.saturating_sub(used + sep);
        let prefix_len = prefix.chars().count();
        if remaining <= prefix_len + 8 {
            break;
        }
        let full_len = prefix_len + text.chars().count();
        let entry = if full_len <= remaining {
            format!("{prefix}{text}")
        } else if chosen_rev.is_empty() || remaining >= 240 {
            // Preserva início e fim de mensagens muito grandes.
            let allowed_text = remaining.saturating_sub(prefix_len);
            let trimmed = preserve_head_and_tail(&text, allowed_text);
            format!("{prefix}{trimmed}")
        } else {
            break;
        };
        let entry_len = entry.chars().count();
        if used + sep + entry_len <= body_budget {
            used += sep + entry_len;
            chosen_rev.push(entry);
            included += 1;
        } else {
            break;
        }
    }

    if chosen_rev.is_empty() {
        return (String::new(), messages.len());
    }
    chosen_rev.reverse();
    let overflow = messages.len().saturating_sub(included);
    (format!("{HEADER}{}", chosen_rev.join("\n")), overflow)
}

pub fn preserve_head_and_tail(text: &str, max_chars: usize) -> String {
    let total = text.chars().count();
    if total <= max_chars {
        return text.to_string();
    }
    let marker_len = MID_OMISSION_MARKER.chars().count();
    if max_chars <= marker_len + 4 {
        return truncate_chars(text, max_chars);
    }
    let content_budget = max_chars - marker_len;
    let head_len = (content_budget * 3) / 5;
    let tail_len = content_budget.saturating_sub(head_len);
    let head: String = text.chars().take(head_len).collect();
    let tail: String = text.chars().skip(total.saturating_sub(tail_len)).collect();
    format!("{head}{MID_OMISSION_MARKER}{tail}")
}

fn fit_lines_to_budget(lines: &[String], budget: usize) -> String {
    if budget == 0 {
        return String::new();
    }
    let mut out = String::new();
    let mut used = 0usize;
    for line in lines {
        let len = line.chars().count();
        let sep = usize::from(!out.is_empty());
        if used + sep + len <= budget {
            if sep == 1 {
                out.push('\n');
            }
            out.push_str(line);
            used += sep + len;
        } else {
            let remaining = budget.saturating_sub(used + sep);
            if remaining >= 16 {
                if sep == 1 {
                    out.push('\n');
                }
                out.push_str(&truncate_chars(line, remaining));
            }
            break;
        }
    }
    out
}

fn format_single_step(step: WorkStep) -> Option<String> {
    let formatted = match step {
        WorkStep::Command {
            command,
            exit_code,
            status,
            output_tail,
        } => {
            let cmd = collapse_whitespace(&redact_work_step_text(&command));
            if cmd.is_empty() {
                return None;
            }
            let status_part = match exit_code {
                Some(code) => format!("exit {code}"),
                None => {
                    let clean_status = collapse_whitespace(&status);
                    if clean_status.is_empty() {
                        "ok".to_string()
                    } else {
                        clean_status
                    }
                }
            };
            let mut line = format!("$ {cmd} ({status_part})");
            if let Some(raw_output) = output_tail {
                let tail = sanitize_command_output_tail(&raw_output);
                if !tail.is_empty() {
                    let indented = tail
                        .lines()
                        .map(|l| format!("  {l}"))
                        .collect::<Vec<_>>()
                        .join("\n");
                    line.push('\n');
                    line.push_str(&indented);
                }
            }
            line
        }
        WorkStep::Files { status, changes } => {
            let items: Vec<String> = changes
                .into_iter()
                .filter_map(|change| {
                    let path = collapse_whitespace(&redact_work_step_text(&change.path));
                    (!path.is_empty()).then(|| format!("{} {path}", change.kind.as_str()))
                })
                .collect();
            if items.is_empty() {
                return None;
            }
            let clean_status = collapse_whitespace(&status);
            let status_text = if clean_status.is_empty() {
                "ok"
            } else {
                clean_status.as_str()
            };
            format!("files ({status_text}): {}", items.join(", "))
        }
        WorkStep::Tool {
            server,
            name,
            status,
        } => {
            let clean_name = collapse_whitespace(&redact_work_step_text(&name));
            if clean_name.is_empty() {
                return None;
            }
            let qualified = match server
                .as_deref()
                .map(|s| collapse_whitespace(&redact_work_step_text(s)))
                .filter(|s| !s.is_empty())
            {
                Some(srv) => format!("{srv}/{clean_name}"),
                None => clean_name,
            };
            let clean_status = collapse_whitespace(&status);
            let status_text = if clean_status.is_empty() {
                "ok"
            } else {
                clean_status.as_str()
            };
            format!("tool {qualified} ({status_text})")
        }
        WorkStep::WebSearch { query } => {
            let clean = collapse_whitespace(&redact_work_step_text(&query));
            if clean.is_empty() {
                return None;
            }
            format!("searched: {clean}")
        }
        WorkStep::Note { text } => {
            let clean = collapse_whitespace(&redact_work_step_text(&text));
            if clean.is_empty() {
                return None;
            }
            format!("note: {clean}")
        }
    };

    Some(truncate_chars(&formatted, MAX_STEP_CHARS))
}

fn sanitize_command_output_tail(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let window = tail_chars(trimmed, MAX_OUTPUT_REDACT_WINDOW_CHARS);
    let redacted = redact_work_step_text(&window);
    let tail = tail_chars(redacted.trim(), MAX_COMMAND_OUTPUT_TAIL_CHARS);
    redact_work_step_text(tail.trim())
}

fn render_work_steps_block(steps: &[String], omitted: usize) -> String {
    let mut out = String::new();
    out.push_str(WORK_STEPS_OPEN);
    out.push('\n');
    if omitted > 0 {
        out.push_str(&omitted_line(omitted));
        out.push('\n');
    }
    for (idx, step) in steps.iter().enumerate() {
        if idx > 0 {
            out.push('\n');
        }
        out.push_str(step);
    }
    out.push('\n');
    out.push_str(WORK_STEPS_CLOSE);
    out
}

fn omitted_line(count: usize) -> String {
    if count == 1 {
        "(1 passo anterior omitido)".to_string()
    } else {
        format!("({count} passos anteriores omitidos)")
    }
}

fn status_label(status: ToolStatus) -> &'static str {
    match status {
        ToolStatus::Ok => "ok",
        ToolStatus::Failed => "failed",
        ToolStatus::Running => "running",
    }
}

fn infer_file_change_kind(tool: &ToolActivity) -> FileChangeKind {
    let lower = tool.name.to_ascii_lowercase();
    if lower.contains("delete") || lower.contains("remove") {
        return FileChangeKind::Delete;
    }
    if matches!(
        lower.as_str(),
        "write" | "write_file" | "write_to_file" | "create" | "create_file"
    ) && tool.removed.unwrap_or(0) == 0
    {
        return FileChangeKind::Create;
    }
    FileChangeKind::Edit
}

fn split_mcp_tool_name(raw: &str) -> (Option<String>, String) {
    if let Some(rest) = raw.strip_prefix("mcp__") {
        if let Some((server, name)) = rest.split_once("__") {
            if !server.is_empty() && !name.is_empty() {
                return (Some(server.to_string()), name.to_string());
            }
        }
    }
    if let Some((server, name)) = raw.split_once('/') {
        if !server.is_empty() && !name.is_empty() {
            return (Some(server.to_string()), name.to_string());
        }
    }
    (None, raw.to_string())
}

pub fn redact_work_step_text(text: &str) -> String {
    let without_pem = redact_pem_blocks(text);
    let base = redact_secrets(&without_pem);
    redact_compact_key_values(&base)
}

fn redact_pem_blocks(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("-----BEGIN ") {
        let after_begin = &rest[start..];
        let Some(header_end) = after_begin.find("PRIVATE KEY-----") else {
            out.push_str(&rest[..start + 11]);
            rest = &rest[start + 11..];
            continue;
        };
        out.push_str(&rest[..start]);
        out.push_str(REDACTED);
        let search_from = header_end + "PRIVATE KEY-----".len();
        if let Some(end_rel) = after_begin[search_from..].find("-----END ") {
            let end_tail = &after_begin[search_from + end_rel..];
            if let Some(closing) = end_tail.find("-----") {
                // Avança até o final da linha `-----END ... PRIVATE KEY-----`.
                let after_prefix = &end_tail[closing + 5..];
                if let Some(final_dashes) = after_prefix.find("-----") {
                    rest = &after_prefix[final_dashes + 5..];
                    continue;
                }
            }
        }
        rest = "";
        break;
    }
    out.push_str(rest);
    out
}

// Mascara padrões compactos sem espaço após `:` como `"api_key":"secret"` ou `token:abc123`.
fn redact_compact_key_values(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for piece in text.split_inclusive(char::is_whitespace) {
        let word = piece.trim_end();
        let tail = &piece[word.len()..];
        if let Some(masked) = mask_compact_colon_pair(word) {
            out.push_str(&masked);
            out.push_str(tail);
        } else {
            out.push_str(piece);
        }
    }
    out
}

fn mask_compact_colon_pair(word: &str) -> Option<String> {
    // Evita quebrar URLs (`https://...`) e caminhos Windows (`C:\...`).
    if word.contains("://") {
        return None;
    }
    let colon = word.find(':')?;
    let key_part = &word[..colon];
    let val_part = &word[colon + 1..];
    if val_part.is_empty() || val_part.contains(REDACTED) {
        return None;
    }
    let clean_key = key_part
        .trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-')
        .to_ascii_uppercase();
    if !COMPACT_SECRET_KEYS.iter().any(|hint| clean_key.contains(hint)) {
        return None;
    }
    let trimmed_val = val_part.trim_matches(|c: char| matches!(c, '"' | '\'' | ',' | '}' | ']' | ';'));
    if trimmed_val.is_empty() {
        return None;
    }
    let idx = val_part.find(trimmed_val)?;
    Some(format!(
        "{key_part}:{}{REDACTED}{}",
        &val_part[..idx],
        &val_part[idx + trimmed_val.len()..]
    ))
}

fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    if max_chars == 1 {
        return ELLIPSIS.to_string();
    }
    let mut out: String = text.chars().take(max_chars - 1).collect();
    out.push_str(ELLIPSIS);
    out
}

fn tail_chars(text: &str, max_chars: usize) -> String {
    let total = text.chars().count();
    if total <= max_chars {
        return text.to_string();
    }
    text.chars().skip(total - max_chars).collect()
}

#[cfg(test)]
#[path = "../../tests/work_steps_tests.rs"]
mod tests;
