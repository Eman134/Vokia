use std::path::Path;
use std::sync::atomic::Ordering;

use tauri::{AppHandle, Manager};

use crate::agents::{Agent, Team};
use crate::claude::{ProjectAccess, TurnActivity};
use crate::db::{DbState, HandoffDraft, OrchestrationSnapshot};
use crate::events::{AgentState, DecisionKind, GoalState, RequestProposal, Speaker, VokiaEvent};
use crate::mcp::{DelegationIntent, ExchangeOutcome, McpState, SlotDecision};
use crate::orchestration::counters::TaskCounters;
use crate::orchestration::policy::{self, Passage, Verdict};
use crate::orchestration::snapshot::placement_of;
use crate::orchestration::snapshot::ACTION_RETURN;
use crate::session_key::SessionKey;
use crate::state::{AppState, Batch, PendingDecision, RequestContext};
use crate::task::{Cache, Change, TaskDefinition, TaskOutcome};

use super::agent_session::{ask_step, StepReply};
use super::emit::{emit, emit_backlog_changed, emit_message, emit_state};
use super::error::FlowError;
use super::flow::{announce_batch, dispatch_request, open_batch, seal_batch, stop_with_error};
use super::goal::{achieved_message, emit_goal, settle, GoalTracker, Settlement, Trigger};
use super::loop_guard::{
    idle_limit_message, pass_limit_message, rejection_message, LoopGuard, PassCounter,
    LOOP_MESSAGE, LOOP_REPEAT_LIMIT, PASS_REPEAT_LIMIT, REJECTION_LIMIT, SEM_AVANCO_LIMIT,
};
use super::markers::ParsedDecision;
use super::prompts::{StepPrompts, ALTERNATE_INSTRUCTION, REJECTION_PREFIX};
use super::session_pool::ExchangeSessions;
use super::text_fallback::{read_reply, TextContext, TextReply};

pub(super) const NO_CONVERSATION: usize = 0;
pub(super) const USER_LABEL: &str = "Usuário";
const MOVEMENT_ERROR_PREFIX: &str = "Não foi possível registrar o movimento: ";
const KIND_LABELS: &[(&str, &str)] = &[
    ("planejar", "Pedido de plano"),
    ("implementar", "Pedido de implementação"),
    ("testar", "Pedido de testes"),
    ("revisar", "Pedido de revisão"),
    ("pesquisar", "Pedido de pesquisa"),
];

pub(super) struct Exchange<'a> {
    pub(super) app: &'a AppHandle,
    pub(super) state: &'a AppState,
    pub(super) cli: &'a Path,
    pub(super) project: ProjectAccess<'a>,
    pub(super) project_context: &'a str,
    pub(super) request: &'a str,
    pub(super) item_id: Option<&'a str>,
    pub(super) channel_id: Option<i64>,
    pub(super) key: SessionKey,
    pub(super) definition: &'a TaskDefinition,
    pub(super) team: &'a Team,
    pub(super) orchestration: Option<&'a OrchestrationSnapshot>,
    pub(super) run_id: Option<u64>,
}

pub(super) async fn run_turns(
    exchange: &Exchange<'_>,
    sessions: &mut ExchangeSessions<'_>,
    transcript: &mut Vec<String>,
) -> Result<TaskOutcome, FlowError> {
    let Exchange {
        app,
        state,
        request,
        item_id,
        channel_id,
        key,
        definition,
        team,
        ..
    } = exchange;
    let project_id = key.project_id;
    let origin = key.agent.clone();
    let mut turn = Turn::first(definition, &origin, request);
    let mut counters = Counters::default();
    // Um lote é a entrega de trabalho até o próximo fim de cadeia; só ele exige revisão, e cada lote novo volta a exigi-la.
    let mut lote_open = false;
    let mut rejections = 0;
    let with_tools = app.state::<McpState>().server().is_some();
    loop {
        if let Some(motive) = policy::task_limit_reached(exchange.orchestration, &counters.task) {
            let speaker = counters
                .task
                .last_speaker()
                .cloned()
                .unwrap_or_else(|| origin.clone());
            let escalation = Turn::new(speaker, "", "", turn.requesters.clone());
            return escalate_limit(
                exchange,
                transcript,
                &escalation,
                motive,
                String::new(),
                None,
                None,
            );
        }
        let prompts = with_cache(state, project_id, |cache| {
            Ok(StepPrompts::build(
                cache,
                definition,
                &turn.speaker,
                &turn.instruction(),
                with_tools,
            ))
        })?;
        let StepReply {
            text: reply,
            activity,
            outcome,
        } = ask_step(exchange, sessions, &turn.speaker, &prompts).await?;
        counters.task.register_turn(&turn.speaker);
        // Só a primeira mensagem do turno leva a atividade; turno sem mensagem a descarta.
        let mut activity = activity;
        let ExchangeOutcome {
            confirmations,
            decision,
            mut proposal,
            delegation: registered_delegation,
            delegation_intent,
            parallel_delegations,
            variables,
        } = outcome;
        let TextReply {
            visible,
            decision: text_decision,
            delegation: text_delegation,
            applied,
            cards,
            mut rejected,
        } = read_reply(
            with_tools,
            &TextContext {
                app,
                state,
                project_id,
                speaker: &turn.speaker,
                item_id: *item_id,
                team,
            },
            &reply,
        )?;
        let raised = decision
            .map(|decision| (decision, visible.clone()))
            .or_else(|| text_decision.map(ParsedDecision::split));
        if let Some((decision, shown)) = raised {
            let shown = Some(shown).filter(|shown| !shown.is_empty());
            let shown = with_confirmations(shown, &confirmations).unwrap_or_default();
            return raise_decision(
                exchange, transcript, &turn, decision, shown, activity, proposal,
            );
        }
        let intent = delegation_intent.filter(|_| registered_delegation.is_some());
        let delegation =
            merged_delegation(registered_delegation, text_delegation, definition.alternate).map(
                |(helper, request)| {
                    let request = match &intent {
                        Some(intent) => labeled_request(&request, intent),
                        None => request,
                    };
                    (helper, request)
                },
            );
        let mut passage: Option<Passage> = None;
        let mut escalation: Option<String> = None;
        if rejected.is_empty() {
            if let Some((helper, _)) = &delegation {
                match policy::check(
                    exchange.orchestration,
                    team,
                    &turn.speaker,
                    helper,
                    &counters.task,
                ) {
                    Verdict::Deliver(found) => passage = Some(found),
                    Verdict::Refuse(motive) => rejected.push(motive),
                    Verdict::Escalate(motive) => escalation = Some(motive),
                }
            }
        }
        if !rejected.is_empty() {
            let shown = with_confirmations(
                (!visible.is_empty()).then(|| visible.clone()),
                &confirmations,
            );
            if let Some(shown) = shown {
                let to = next_turn(definition, &turn, &visible, None)
                    .map_or(Speaker::User, |next| Speaker::from(next.speaker));
                emit_message(
                    app,
                    project_id,
                    Speaker::from(turn.speaker.clone()),
                    to,
                    &shown,
                    *channel_id,
                    activity.take(),
                    None,
                )?;
            }
            rejections += 1;
            if rejections >= REJECTION_LIMIT {
                let trigger = Trigger::Stall {
                    state: GoalState::SemAvanco,
                    message: rejection_message(),
                };
                match resolve(exchange, transcript, &mut counters, &turn.speaker, trigger).await? {
                    Step::Continue(continuation) => {
                        rejections = 0;
                        turn = continuation;
                        continue;
                    }
                    Step::End(outcome) => return Ok(outcome),
                }
            }
            sessions.review(&turn.speaker, rejections, 0).await;
            turn.rejection = Some(rejected.join("; "));
            continue;
        }
        rejections = 0;
        let mut confirmations = confirmations;
        confirmations.extend(cards.confirmations);
        let advanced = applied.created_variable
            || applied.changed_value
            || variables.created_variable
            || variables.changed_value;
        let accepted: Vec<Change> = variables
            .accepted
            .into_iter()
            .chain(applied.accepted)
            .collect();
        let feed = with_confirmations(feed_text(&visible, &accepted), &confirmations);
        let finished = with_cache(state, project_id, |cache| {
            Ok(definition
                .stop
                .as_ref()
                .is_some_and(|stop| stop.reached(cache)))
        })?;
        if finished {
            if let Some(text) = &feed {
                say(
                    app,
                    project_id,
                    transcript,
                    &turn.speaker,
                    Speaker::User,
                    text,
                    team,
                    *channel_id,
                    activity.take(),
                    proposal.take(),
                )?;
            }
            if cards.changed {
                emit_backlog_changed(app, project_id)?;
            }
            emit_goal(
                app,
                project_id,
                &definition.objetivo,
                Some(GoalState::Atingido),
                "",
                counters.goal.checks(),
            )?;
            let message = format!("Tarefa concluída: {}.", definition.objetivo);
            conclude(
                app,
                project_id,
                transcript,
                &turn.speaker,
                &origin,
                &message,
                team,
                *channel_id,
            )?;
            return Ok(TaskOutcome::Finished);
        }

        // Cada delegação em paralelo vai para a instância do helper (ou espera no teto); a delegação em série segue o caminho normal.
        if !parallel_delegations.is_empty() {
            let snapshot = exchange.orchestration;
            let max_parallel = snapshot.map_or(1, |snapshot| snapshot.settings.max_parallel);
            let batch = open_batch(
                state,
                Batch {
                    project_id,
                    leader: turn.speaker.clone(),
                    objective: exchange.definition.objetivo.clone(),
                    variables: exchange.definition.variables.clone(),
                    channel_id: *channel_id,
                    item_id: (*item_id).map(str::to_string),
                    // Contagem do próprio líder enquanto despacha: uma troca que termina cedo não dispara a junção.
                    pending: 1,
                    results: Vec::new(),
                    dispatched: Vec::new(),
                    started: Vec::new(),
                    returned: Vec::new(),
                    counters: counters.task.clone(),
                },
            )?;
            for (helper, request) in parallel_delegations {
                let max_instances = snapshot
                    .and_then(|snapshot| placement_of(snapshot, &helper))
                    .map_or(1, |placement| placement.max_instances);
                let mut context = RequestContext::new(
                    exchange.run_id,
                    (*item_id).map(str::to_string),
                    *channel_id,
                );
                context.batch = Some(batch);
                dispatch_request(
                    app,
                    state,
                    project_id,
                    &helper,
                    &request,
                    context,
                    max_instances,
                    max_parallel,
                )?;
                say(
                    app,
                    project_id,
                    transcript,
                    &turn.speaker,
                    Speaker::from(helper.clone()),
                    &request,
                    team,
                    *channel_id,
                    None,
                    None,
                )?;
            }
            announce_batch(app, state, batch);
            seal_batch(app, state, batch);
            if delegation.is_none() {
                if let Some(text) = &feed {
                    say(
                        app,
                        project_id,
                        transcript,
                        &turn.speaker,
                        Speaker::User,
                        text,
                        team,
                        *channel_id,
                        activity.take(),
                        proposal.take(),
                    )?;
                }
                emit_state(app, project_id, turn.speaker.clone(), AgentState::Waiting)?;
                return Ok(TaskOutcome::Finished);
            }
        }
        if let Some(motive) = escalation {
            return escalate_limit(
                exchange,
                transcript,
                &turn,
                motive,
                visible,
                activity.take(),
                proposal.take(),
            );
        }
        counters.goal.register_turn(advanced);
        let next = next_turn(
            definition,
            &turn,
            feed.as_deref().unwrap_or_default(),
            delegation.as_ref(),
        );
        let to = speech_recipient(definition, &turn, next.as_ref());
        if let Some(text) = &feed {
            say(
                app,
                project_id,
                transcript,
                &turn.speaker,
                to,
                text,
                team,
                *channel_id,
                activity.take(),
                proposal.take(),
            )?;
        }
        if cards.changed {
            emit_backlog_changed(app, project_id)?;
        }
        let Some(next) = next else {
            let lote_delivered = std::mem::take(&mut lote_open);
            let review = lote_delivered
                .then(|| policy::chain_review(exchange.orchestration, &turn.speaker))
                .flatten();
            if let Some(review) = review {
                let decision = SlotDecision {
                    kind: DecisionKind::Revisao,
                    title: review.title.to_string(),
                    detail: review.detail,
                    options: Vec::new(),
                };
                return raise_decision(
                    exchange,
                    transcript,
                    &turn,
                    decision,
                    String::new(),
                    None,
                    None,
                );
            }
            for requester in &turn.requesters {
                emit_state(app, project_id, requester.clone(), AgentState::Idle)?;
            }
            match resolve(
                exchange,
                transcript,
                &mut counters,
                &turn.speaker,
                Trigger::EndOfChain,
            )
            .await?
            {
                Step::Continue(continuation) => {
                    turn = continuation;
                    continue;
                }
                Step::End(outcome) => return Ok(outcome),
            }
        };

        let pair_repeats = if advanced {
            counters.passes.reset();
            0
        } else {
            counters.passes.register(&turn.speaker, &next.speaker)
        };
        let loop_repeats = counters
            .loop_guard
            .register(next.speaker.clone(), &next.received);
        let trigger = if pair_repeats >= PASS_REPEAT_LIMIT {
            Some(Trigger::Stall {
                state: GoalState::Repetindo,
                message: pass_limit_message(&counters.passes, &turn.speaker, &next.speaker, team),
            })
        } else if loop_repeats >= LOOP_REPEAT_LIMIT {
            Some(Trigger::Stall {
                state: GoalState::Repetindo,
                message: LOOP_MESSAGE.to_string(),
            })
        } else if !definition.variables.is_empty() && counters.goal.idle_turns() >= SEM_AVANCO_LIMIT
        {
            Some(Trigger::Stall {
                state: GoalState::SemAvanco,
                message: idle_limit_message(
                    counters.goal.last_advance_step(),
                    &turn.speaker,
                    &next.speaker,
                    team,
                ),
            })
        } else {
            None
        };
        if let Some(trigger) = trigger {
            match resolve(exchange, transcript, &mut counters, &turn.speaker, trigger).await? {
                Step::Continue(continuation) => {
                    turn = continuation;
                    continue;
                }
                Step::End(outcome) => return Ok(outcome),
            }
        }

        if let Some((helper, helper_request)) = &delegation {
            say(
                app,
                project_id,
                transcript,
                &turn.speaker,
                Speaker::from(helper.clone()),
                helper_request,
                team,
                *channel_id,
                activity.take(),
                proposal.take(),
            )?;
            if let Some(passage) = &passage {
                lote_open = true;
                counters.task.register_passage(&turn.speaker, helper);
                if passage.action == ACTION_RETURN {
                    counters.task.register_return(&turn.speaker, helper);
                }
                record_passage(
                    app,
                    exchange.run_id,
                    project_id,
                    intent
                        .as_ref()
                        .and_then(|intent| intent.item_id.as_deref())
                        .or(*item_id),
                    intent.as_ref().and_then(|intent| intent.event.as_deref()),
                    &turn.speaker,
                    helper,
                    passage,
                )?;
            }
        }
        let speaker_state = if delegation.is_some() {
            AgentState::Waiting
        } else {
            AgentState::Idle
        };
        emit_state(app, project_id, turn.speaker.clone(), speaker_state)?;
        if !advanced {
            sessions
                .review(&turn.speaker, counters.passes.by_speaker(&turn.speaker), 0)
                .await;
        }
        sessions.review(&next.speaker, 0, loop_repeats).await;
        turn = next;
    }
}

#[derive(Default)]
struct Counters {
    passes: PassCounter,
    loop_guard: LoopGuard,
    goal: GoalTracker,
    task: TaskCounters,
}

impl Counters {
    fn reset_stalls(&mut self) {
        self.passes.reset();
        self.loop_guard = LoopGuard::default();
    }
}

enum Step {
    Continue(Turn),
    End(TaskOutcome),
}

async fn resolve(
    exchange: &Exchange<'_>,
    transcript: &mut Vec<String>,
    counters: &mut Counters,
    speaker: &Agent,
    trigger: Trigger,
) -> Result<Step, FlowError> {
    let Exchange {
        app,
        channel_id,
        key,
        definition,
        team,
        ..
    } = exchange;
    let project_id = key.project_id;
    let origin = &key.agent;
    let settlement = settle(
        exchange,
        transcript.as_slice(),
        &mut counters.goal,
        &counters.passes,
        trigger,
    )
    .await?;
    Ok(match settlement {
        Settlement::Achieved { reason } => {
            let message = achieved_message(&definition.objetivo, &reason);
            conclude(
                app,
                project_id,
                transcript,
                speaker,
                origin,
                &message,
                team,
                *channel_id,
            )?;
            Step::End(TaskOutcome::Finished)
        }
        Settlement::Continue { instruction } => {
            counters.reset_stalls();
            Step::Continue(Turn::new(
                first_speaker(definition, origin),
                &instruction,
                &instruction,
                Vec::new(),
            ))
        }
        Settlement::Stalled { message } => {
            stop_with_error(app, project_id, message)?;
            Step::End(TaskOutcome::Stalled)
        }
        Settlement::Ended => {
            emit_state(app, project_id, speaker.clone(), AgentState::Idle)?;
            Step::End(TaskOutcome::Finished)
        }
    })
}

fn conclude(
    app: &AppHandle,
    project_id: i64,
    transcript: &mut Vec<String>,
    speaker: &Agent,
    origin: &Agent,
    message: &str,
    team: &Team,
    channel_id: Option<i64>,
) -> Result<(), FlowError> {
    emit_state(app, project_id, speaker.clone(), AgentState::Idle)?;
    say(
        app,
        project_id,
        transcript,
        origin,
        Speaker::User,
        message,
        team,
        channel_id,
        None,
        None,
    )
}

// Quem decide fica Waiting até a resposta virar um novo pedido; quem delegou a ele volta a Idle.
// Guarda a decisão pendente e avisa a tela; a resposta do usuário volta pelo pedido salvo aqui.
pub(super) fn open_decision(
    app: &AppHandle,
    state: &AppState,
    agent: &Agent,
    project_id: i64,
    item_id: Option<&str>,
    request: &str,
    decision: SlotDecision,
) -> Result<(), FlowError> {
    let id = state.next_decision_id.fetch_add(1, Ordering::Relaxed);
    state
        .decisions
        .lock()
        .map_err(|_| FlowError::DecisionsPoisoned)?
        .insert(
            id,
            PendingDecision {
                agent: agent.clone(),
                project_id,
                title: decision.title.clone(),
                request: request.to_string(),
                item_id: item_id.map(str::to_string),
            },
        );
    emit(
        app,
        VokiaEvent::Decision {
            id,
            agent: agent.clone(),
            kind: decision.kind,
            title: decision.title,
            detail: decision.detail,
            options: decision.options,
            project_id: Some(project_id),
            item_id: item_id.map(str::to_string),
        },
    )
}

fn raise_decision(
    exchange: &Exchange<'_>,
    transcript: &mut Vec<String>,
    turn: &Turn,
    decision: SlotDecision,
    visible: String,
    activity: Option<TurnActivity>,
    proposal: Option<RequestProposal>,
) -> Result<TaskOutcome, FlowError> {
    let Exchange {
        app,
        state,
        request,
        item_id,
        channel_id,
        key,
        team,
        ..
    } = exchange;
    let project_id = key.project_id;
    let SlotDecision {
        kind,
        title,
        detail,
        options,
    } = decision;
    let agent = turn.speaker.clone();
    if !visible.is_empty() {
        say(
            app,
            project_id,
            transcript,
            &agent,
            Speaker::User,
            &visible,
            team,
            *channel_id,
            activity,
            proposal,
        )?;
    }
    open_decision(
        app,
        state,
        &agent,
        project_id,
        *item_id,
        request,
        SlotDecision {
            kind,
            title,
            detail,
            options,
        },
    )?;
    for requester in &turn.requesters {
        emit_state(app, project_id, requester.clone(), AgentState::Idle)?;
    }
    emit_state(app, project_id, agent, AgentState::Waiting)?;
    Ok(TaskOutcome::Finished)
}

// Estouro de limite do mapa: sobe ao usuário em nome de quem falou, sem parar a tarefa com erro.
fn escalate_limit(
    exchange: &Exchange<'_>,
    transcript: &mut Vec<String>,
    turn: &Turn,
    motive: String,
    visible: String,
    activity: Option<TurnActivity>,
    proposal: Option<RequestProposal>,
) -> Result<TaskOutcome, FlowError> {
    let decision = SlotDecision {
        kind: DecisionKind::Pergunta,
        title: policy::LIMIT_TITLE.to_string(),
        detail: motive,
        options: Vec::new(),
    };
    raise_decision(
        exchange, transcript, turn, decision, visible, activity, proposal,
    )
}

// Falha de gravação vira aviso no feed e a troca segue (mesmo padrão de SessionStore::write).
// O texto entregue ao destinatário leva o tipo do pedido e o item, quando a ferramenta os informou.
fn labeled_request(request: &str, intent: &DelegationIntent) -> String {
    let label = intent
        .kind
        .as_deref()
        .and_then(|kind| KIND_LABELS.iter().find(|(name, _)| *name == kind))
        .map(|(_, label)| *label);
    match (label, intent.item_id.as_deref()) {
        (None, _) => request.to_string(),
        (Some(label), Some(item)) => format!("{label} de {item}: {request}"),
        (Some(label), None) => format!("{label}: {request}"),
    }
}

pub(super) fn record_passage(
    app: &AppHandle,
    run_id: Option<u64>,
    project_id: i64,
    item_id: Option<&str>,
    event: Option<&str>,
    from: &Agent,
    to: &Agent,
    passage: &Passage,
) -> Result<(), FlowError> {
    let state = app.state::<DbState>();
    let draft = HandoffDraft {
        run_id: run_id.and_then(|id| i64::try_from(id).ok()),
        from_agent: from.to_string(),
        to_agent: to.to_string(),
        from_layer: passage.from_layer.clone(),
        to_layer: passage.to_layer.clone(),
        action: passage.action.clone(),
        kind: passage.kind.clone(),
        item_id: item_id.map(str::to_string),
        return_count: passage.return_count,
        message_id: None,
        event: event.map(str::to_string),
    };
    match state
        .require()
        .and_then(|db| db.record_handoff(project_id, &draft))
    {
        Ok(()) => emit(app, VokiaEvent::OrchestrationChanged { project_id }),
        Err(error) => emit(
            app,
            VokiaEvent::Error {
                message: format!("{MOVEMENT_ERROR_PREFIX}{error}."),
            },
        ),
    }
}

pub(super) fn first_speaker(definition: &TaskDefinition, origin: &Agent) -> Agent {
    definition
        .participants
        .first()
        .cloned()
        .unwrap_or_else(|| origin.clone())
}

struct Turn {
    speaker: Agent,
    base: String,
    rejection: Option<String>,
    received: String,
    requesters: Vec<Agent>,
}

impl Turn {
    fn new(speaker: Agent, base: &str, received: &str, requesters: Vec<Agent>) -> Self {
        Self {
            speaker,
            base: base.to_string(),
            rejection: None,
            received: received.to_string(),
            requesters,
        }
    }

    fn first(definition: &TaskDefinition, origin: &Agent, request: &str) -> Self {
        let speaker = first_speaker(definition, origin);
        Self::new(
            speaker,
            &format!("Pedido do usuário: {request}"),
            request,
            Vec::new(),
        )
    }

    fn instruction(&self) -> String {
        match &self.rejection {
            Some(motive) => format!("{REJECTION_PREFIX}{motive}. {}", self.base),
            None => self.base.clone(),
        }
    }
}

// Sem revezamento, a resposta do helper fica na linha dele para quem pediu e não volta como turno.
fn next_turn(
    definition: &TaskDefinition,
    turn: &Turn,
    feed: &str,
    delegation: Option<&(Agent, String)>,
) -> Option<Turn> {
    let speaker = turn.speaker.clone();
    if definition.alternate {
        let agent = next_participant(&definition.participants, &speaker);
        return Some(Turn::new(agent, ALTERNATE_INSTRUCTION, feed, Vec::new()));
    }
    let (helper, request) = delegation?;
    let mut requesters = turn.requesters.clone();
    requesters.push(speaker);
    Some(Turn::new(helper.clone(), request, request, requesters))
}

// A fala de quem falou vai ao próximo do revezamento ou, sem revezamento, a quem pediu o turno.
fn speech_recipient(definition: &TaskDefinition, turn: &Turn, next: Option<&Turn>) -> Speaker {
    if definition.alternate {
        return next.map_or(Speaker::User, |next| Speaker::from(next.speaker.clone()));
    }
    turn.requesters
        .last()
        .cloned()
        .map_or(Speaker::User, Speaker::from)
}

fn next_participant(participants: &[Agent], speaker: &Agent) -> Agent {
    let index = participants
        .iter()
        .position(|agent| agent == speaker)
        .map_or(0, |index| index + 1);
    participants[index % participants.len()].clone()
}

fn feed_text(visible: &str, accepted: &[Change]) -> Option<String> {
    if !visible.is_empty() {
        return Some(visible.to_string());
    }
    match accepted {
        [only] => only.feed(),
        _ => None,
    }
}

// Um turno delega no máximo uma vez: pedido igual pela ferramenta e pela linha `@Nome:` conta uma vez,
// e a ferramenta vale se os dois divergirem. Em tarefa alternada o revezamento já decide quem fala.
fn merged_delegation(
    registered: Option<(Agent, String)>,
    from_text: Option<(Agent, String)>,
    alternate: bool,
) -> Option<(Agent, String)> {
    registered.or(from_text).filter(|_| !alternate)
}

fn with_confirmations(feed: Option<String>, confirmations: &[String]) -> Option<String> {
    if confirmations.is_empty() {
        return feed;
    }
    let lines = confirmations.join("\n");
    Some(match feed {
        Some(text) => format!("{text}\n\n{lines}"),
        None => lines,
    })
}

fn say(
    app: &AppHandle,
    project_id: i64,
    transcript: &mut Vec<String>,
    from: &Agent,
    to: Speaker,
    text: &str,
    team: &Team,
    channel_id: Option<i64>,
    activity: Option<TurnActivity>,
    request_proposal: Option<RequestProposal>,
) -> Result<(), FlowError> {
    emit_message(
        app,
        project_id,
        Speaker::from(from.clone()),
        to,
        text,
        channel_id,
        activity,
        request_proposal,
    )?;
    transcript.push(format!(
        "{}: {text}",
        crate::orchestration::instances::display_name(team, from)
    ));
    Ok(())
}

pub(crate) fn with_cache<T>(
    state: &AppState,
    project_id: i64,
    action: impl FnOnce(&mut Cache) -> Result<T, String>,
) -> Result<T, FlowError> {
    let mut caches = state.cache.lock().map_err(|_| FlowError::CachePoisoned)?;
    action(caches.entry(project_id).or_default()).map_err(FlowError::Cache)
}

#[cfg(test)]
#[path = "../../tests/backlog_cards_tests.rs"]
mod backlog_cards_tests;

#[cfg(test)]
#[path = "../../tests/mcp_cut_tests.rs"]
mod mcp_cut_tests;

#[cfg(test)]
#[path = "../../tests/turn_relay_tests.rs"]
mod turn_relay_tests;
