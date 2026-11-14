use crate::agents::{Agent, Team};
use crate::cache::{parse_update_line, Change};
use crate::events::DecisionKind;

pub(super) const DECISION_MARK: &str = "#decisao";
pub(super) const DECISION_FIELD_SEPARATOR: char = '|';
pub(super) const DECISION_OPTION_SEPARATOR: char = ';';
const DECISION_FIELD_COUNT: usize = 4;
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
}

pub(super) fn parse_reply(reply: &str, from: &Agent, team: &Team) -> ParsedReply {
    let mut visible = Vec::new();
    let mut delegation = None;
    let mut updates = Vec::new();
    for line in reply.lines() {
        if let Some(update) = parse_update_line(line) {
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
    }
}

fn delegation_request<'a>(line: &'a str, from: &Agent, team: &Team) -> Option<(Agent, &'a str)> {
    let rest = line.trim().strip_prefix(DELEGATION_MARK)?;
    let (name, request) = rest.split_once(DELEGATION_SEPARATOR)?;
    let agent = team.named(name.trim())?;
    let request = request.trim();
    (agent != *from && !request.is_empty()).then_some((agent, request))
}
