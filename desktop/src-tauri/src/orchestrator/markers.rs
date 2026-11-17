use crate::agents::{Agent, Team};
use crate::db::CardOperation;
use crate::events::DecisionKind;
use crate::task::{parse_update_line, Change};

pub(super) const DECISION_MARK: &str = "#decisao";
pub(super) const DECISION_FIELD_SEPARATOR: char = '|';
pub(super) const DECISION_OPTION_SEPARATOR: char = ';';
const DECISION_FIELD_COUNT: usize = 4;
const CARD_MARK: &str = "#card";
const CARD_FIELD_SEPARATOR: char = '|';
const CARD_CREATE: &str = "criar";
const CARD_MOVE: &str = "mover";
const CARD_MOVE_FIELD_COUNT: usize = 2;
const CARD_INCOMPLETE: &str = "a linha #card está incompleta (use \"#card criar <título>|<descrição>\" ou \"#card mover <id>|<coluna>\")";
const CARD_EMPTY_TITLE: &str = "o título do card não pode ser vazio";
const DELEGATION_MARK: char = '@';
const DELEGATION_SEPARATOR: char = ':';

pub(super) struct ParsedDecision {
    pub(super) kind: DecisionKind,
    pub(super) title: String,
    pub(super) detail: String,
    pub(super) options: Vec<String>,
    pub(super) visible: String,
}

pub(super) fn parse_decision(reply: &str) -> Option<ParsedDecision> {
    let lines: Vec<&str> = reply.lines().collect();
    let (index, decision) = lines
        .iter()
        .enumerate()
        .rev()
        .find_map(|(index, line)| decision_line(line).map(|decision| (index, decision)))?;
    let visible: Vec<&str> = lines
        .iter()
        .enumerate()
        .filter(|(position, _)| *position != index)
        .map(|(_, line)| *line)
        .collect();
    Some(ParsedDecision {
        visible: visible.join(
            "
",
        ),
        ..decision
    })
}

fn decision_line(line: &str) -> Option<ParsedDecision> {
    let rest = line.trim().strip_prefix(DECISION_MARK)?;
    let mut fields = rest.splitn(DECISION_FIELD_COUNT, DECISION_FIELD_SEPARATOR);
    let kind = DecisionKind::parse(fields.next()?.trim())?;
    let title = fields.next()?.trim();
    if title.is_empty() {
        return None;
    }
    let detail = fields.next().unwrap_or_default().trim();
    let options: Vec<String> = fields
        .next()
        .unwrap_or_default()
        .split(DECISION_OPTION_SEPARATOR)
        .map(str::trim)
        .filter(|option| !option.is_empty())
        .map(str::to_string)
        .collect();
    // Escolha sem opções não tem como ser respondida; a linha vira texto comum.
    if kind == DecisionKind::Escolha && options.is_empty() {
        return None;
    }
    Some(ParsedDecision {
        kind,
        title: title.to_string(),
        detail: detail.to_string(),
        options,
        visible: String::new(),
    })
}

pub(super) struct ParsedReply {
    pub(super) visible: String,
    pub(super) delegation: Option<(Agent, String)>,
    pub(super) updates: Vec<Result<Change, String>>,
    pub(super) cards: Vec<Result<CardOperation, String>>,
}

pub(super) fn parse_reply(reply: &str, from: &Agent, team: &Team) -> ParsedReply {
    let mut visible = Vec::new();
    let mut delegation = None;
    let mut updates = Vec::new();
    let mut cards = Vec::new();
    for line in reply.lines() {
        if let Some(card) = parse_card_line(line) {
            cards.push(card);
        } else if let Some(update) = parse_update_line(line) {
            updates.push(update);
        } else if let Some((agent, request)) = delegation_request(line, from, team) {
            delegation = Some((agent, request.to_string()));
        } else {
            visible.push(line);
        }
    }
    ParsedReply {
        visible: visible.join("\n").trim().to_string(),
        delegation,
        updates,
        cards,
    }
}

fn delegation_request<'a>(line: &'a str, from: &Agent, team: &Team) -> Option<(Agent, &'a str)> {
    let rest = line.trim().strip_prefix(DELEGATION_MARK)?;
    let (name, request) = rest.split_once(DELEGATION_SEPARATOR)?;
    let agent = team.named(name.trim())?;
    let request = request.trim();
    (agent != *from && !request.is_empty()).then_some((agent, request))
}

// Mesmo contrato de parse_update_line: None não é a linha; Err é a linha, mas inválida.
fn parse_card_line(line: &str) -> Option<Result<CardOperation, String>> {
    let rest = line.trim().strip_prefix(CARD_MARK)?;
    if !rest.is_empty() && !rest.starts_with(char::is_whitespace) {
        return None;
    }
    let rest = rest.trim();
    let (action, args) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
    Some(match action {
        CARD_CREATE => create_operation(args),
        CARD_MOVE => move_operation(args),
        _ => Err(CARD_INCOMPLETE.to_string()),
    })
}

fn create_operation(args: &str) -> Result<CardOperation, String> {
    let (title, description) = args.split_once(CARD_FIELD_SEPARATOR).unwrap_or((args, ""));
    let title = title.trim();
    if title.is_empty() {
        return Err(CARD_EMPTY_TITLE.to_string());
    }
    let description = description.trim();
    Ok(CardOperation::Create {
        title: title.to_string(),
        description: (!description.is_empty()).then(|| description.to_string()),
    })
}

fn move_operation(args: &str) -> Result<CardOperation, String> {
    let fields: Vec<&str> = args.split(CARD_FIELD_SEPARATOR).map(str::trim).collect();
    match fields.as_slice() {
        [id, column]
            if fields.len() == CARD_MOVE_FIELD_COUNT && !id.is_empty() && !column.is_empty() =>
        {
            Ok(CardOperation::Move {
                id: id.to_string(),
                column: column.to_string(),
            })
        }
        _ => Err(CARD_INCOMPLETE.to_string()),
    }
}
