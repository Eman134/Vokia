const PLAN_READY_MARKER: &str = "#plano-pronto";
const ESTIMATE_MARKER: &str = "#estimativa";
const LOT_MARKER: &str = "#lote";
const SCRIPT_MARKER: &str = "#roteiro";
const RISK_MARKER: &str = "#risco";
const TAGS_MARKER: &str = "#etiquetas";
const LOT_FIELD_SEPARATOR: char = '|';
const SCRIPT_FIELD_SEPARATOR: char = '~';
const SCRIPT_WHO_FIELD: &str = "quem:";
const SCRIPT_AFTER_FIELD: &str = "apos:";
const DEPENDENCY_SEPARATOR: char = ',';
const NO_DEPENDENCY: u32 = 0;
const TASK_SEPARATOR: char = ';';
const TAG_SEPARATOR: char = ',';
const LINE_SEPARATOR: &str = "\n";

// Front e backend leem a mesma regra: etapas pelo papel de cada agente do time, `apos:` marca paralelo ou sequência.
pub const PLAN_PROTOCOL_HINT: &str = "Formato do proposal: primeira linha #plano-pronto, depois um marcador por linha: #estimativa ~<tempo>, #lote <n>|<título>|<meta>|<tarefa>; <tarefa> (ao menos um), opcional #roteiro <n>|quem:<agente>~modelo:<modelo>~ctx:<contexto>~arq:<arquivo>~passo:<passo>~pronto:<critério>~nao:<limite>~apos:<n>, <n>, #risco <frase>, #etiquetas <e1>, <e2> e resumo livre. Com time selecionado: monte os lotes pelo papel e pelas instruções de cada agente, com #roteiro quem: só de agentes do time. apos: lista os lotes que precisam terminar antes (sem apos: vale o lote anterior; apos:0 começa junto do lote 1), então lotes com o mesmo apos: rodam em paralelo.";

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlanLot {
    pub n: u32,
    pub title: String,
    pub meta: String,
    pub tasks: Vec<String>,
    // Agente responsável pela etapa, vindo do #roteiro de mesmo número.
    pub who: Option<String>,
    // Lotes que precisam concluir antes deste; None é o lote anterior, Some(vazio) começa junto do primeiro.
    pub after: Option<Vec<u32>>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlanDraft {
    pub summary: String,
    pub estimate: String,
    pub lots: Vec<PlanLot>,
    pub risks: Vec<String>,
    pub tags: Vec<String>,
}

impl PlanDraft {
    // Lotes que cada lote espera concluir, na ordem do plano; sem `apos:` espera o lote anterior.
    pub fn dependencies(&self) -> Vec<(u32, Vec<u32>)> {
        self.lots
            .iter()
            .map(|lot| {
                let after = lot
                    .after
                    .clone()
                    .unwrap_or_else(|| if lot.n > 1 { vec![lot.n - 1] } else { Vec::new() });
                (lot.n, after)
            })
            .collect()
    }

    // Dependência só vale para lote anterior; isso impede ciclo e espera por etapa inexistente.
    pub fn dependency_refusal(&self) -> Option<String> {
        self.lots.iter().find_map(|lot| {
            let invalid = lot.after.iter().flatten().find(|dep| **dep >= lot.n)?;
            Some(format!("A etapa {} depende de apos:{invalid}, que não vem antes dela.", lot.n))
        })
    }
}

// Mesmo protocolo do parseProposal do front (planProposal.ts): linha sem marcador vai para o resumo.
pub fn parse_proposal(text: &str) -> PlanDraft {
    let mut draft = PlanDraft::default();
    let mut summary: Vec<&str> = Vec::new();
    let mut scripts: Vec<&str> = Vec::new();
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
        } else if let Some(rest) = line.strip_prefix(SCRIPT_MARKER) {
            scripts.push(rest);
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
    for script in scripts {
        let Some(parsed) = parse_script(script) else {
            continue;
        };
        if let Some(lot) = draft.lots.iter_mut().find(|lot| lot.n == parsed.n) {
            lot.who = parsed.who;
            lot.after = parsed.after;
        }
    }
    draft.summary = summary.join(LINE_SEPARATOR).trim().to_string();
    draft
}

struct ParsedScript {
    n: u32,
    who: Option<String>,
    after: Option<Vec<u32>>,
}

// `#roteiro <n>|quem:<agente>~...`: o número liga o roteiro ao lote; os demais campos seguem o front.
fn parse_script(rest: &str) -> Option<ParsedScript> {
    let (number, fields) = rest.trim().split_once(LOT_FIELD_SEPARATOR)?;
    let n = number.trim().parse::<u32>().ok()?;
    let field_value = |prefix: &str| {
        fields
            .split(SCRIPT_FIELD_SEPARATOR)
            .map(str::trim)
            .find_map(|field| field.strip_prefix(prefix))
            .map(str::trim)
            .filter(|value| !value.is_empty())
    };
    let who = field_value(SCRIPT_WHO_FIELD).map(str::to_string);
    let after = field_value(SCRIPT_AFTER_FIELD).map(parse_dependencies);
    Some(ParsedScript { n, who, after })
}

// `apos:0` zera a lista: o lote começa junto do primeiro.
fn parse_dependencies(value: &str) -> Vec<u32> {
    value
        .split(DEPENDENCY_SEPARATOR)
        .filter_map(|part| part.trim().parse::<u32>().ok())
        .filter(|n| *n != NO_DEPENDENCY)
        .collect()
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
        who: None,
        after: None,
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
