const PLAN_READY_MARKER: &str = "#plano-pronto";
const ESTIMATE_MARKER: &str = "#estimativa";
const LOT_MARKER: &str = "#lote";
const SCRIPT_MARKER: &str = "#roteiro";
const RISK_MARKER: &str = "#risco";
const TAGS_MARKER: &str = "#etiquetas";
const LOT_FIELD_SEPARATOR: char = '|';
const TASK_SEPARATOR: char = ';';
const TAG_SEPARATOR: char = ',';
const LINE_SEPARATOR: &str = "\n";

pub const PLAN_PROTOCOL_HINT: &str = "Formato do proposal: primeira linha #plano-pronto, depois um marcador por linha: #estimativa ~<tempo>, #lote <n>|<título>|<meta>|<tarefa>; <tarefa> (ao menos um), opcional #roteiro <n>|quem:<agente>~modelo:<modelo>~ctx:<contexto>~arq:<arquivo>~passo:<passo>~pronto:<critério>~nao:<limite>, #risco <frase>, #etiquetas <e1>, <e2> e resumo livre.";

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlanLot {
    pub n: u32,
    pub title: String,
    pub meta: String,
    pub tasks: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlanDraft {
    pub summary: String,
    pub estimate: String,
    pub lots: Vec<PlanLot>,
    pub risks: Vec<String>,
    pub tags: Vec<String>,
}

// Mesmo protocolo do parseProposal do front (planProposal.ts): linha sem marcador vai para o resumo.
pub fn parse_proposal(text: &str) -> PlanDraft {
    let mut draft = PlanDraft::default();
    let mut summary: Vec<&str> = Vec::new();
    for raw in text.split(LINE_SEPARATOR) {
        let line = raw.trim();
        if line == PLAN_READY_MARKER {
            continue;
        }
        if let Some(rest) = line.strip_prefix(ESTIMATE_MARKER) {
            draft.estimate = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix(LOT_MARKER) {
            let n = draft.lots.len() as u32 + 1;
            if let Some(lot) = parse_lot(rest, n) {
                draft.lots.push(lot);
            }
        } else if line.starts_with(SCRIPT_MARKER) {
            // O roteiro detalhado é consumido só no front; o Rust ignora para não poluir o resumo.
            continue;
        } else if let Some(rest) = line.strip_prefix(RISK_MARKER) {
            let risk = rest.trim();
            if !risk.is_empty() {
                draft.risks.push(risk.to_string());
            }
        } else if let Some(rest) = line.strip_prefix(TAGS_MARKER) {
            draft.tags = split_trimmed(rest, TAG_SEPARATOR);
        } else {
            summary.push(raw);
        }
    }
    draft.summary = summary.join(LINE_SEPARATOR).trim().to_string();
    draft
}

fn parse_lot(rest: &str, n: u32) -> Option<PlanLot> {
    let mut fields: Vec<&str> = rest.split(LOT_FIELD_SEPARATOR).map(str::trim).collect();
    if fields.first().is_some_and(|first| is_lot_number(first)) {
        fields.remove(0);
    }
    let title = fields.first().copied().unwrap_or_default();
    if title.is_empty() {
        return None;
    }
    Some(PlanLot {
        n,
        title: title.to_string(),
        meta: fields.get(1).copied().unwrap_or_default().to_string(),
        tasks: split_trimmed(fields.get(2).copied().unwrap_or_default(), TASK_SEPARATOR),
    })
}

fn is_lot_number(field: &str) -> bool {
    !field.is_empty() && field.chars().all(|c| c.is_ascii_digit())
}

fn split_trimmed(text: &str, separator: char) -> Vec<String> {
    text.split(separator)
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
#[path = "../tests/plan_proposal_tests.rs"]
mod tests;
