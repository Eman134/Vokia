use std::path::Path;
use std::sync::atomic::Ordering;

use tauri::AppHandle;

use crate::agents::{Agent, Team};
use crate::claude::ProjectAccess;
use crate::events::{AgentState, GoalState, Speaker, VokiaEvent};
use crate::session_key::SessionKey;
use crate::state::{AppState, PendingDecision};
use crate::task::{Cache, Change, TaskDefinition, TaskOutcome};

use super::agent_session::ask_step;
use super::emit::{emit, emit_message, emit_state};
use super::error::FlowError;
use super::flow::stop_with_error;
use super::goal::{achieved_message, emit_goal, settle, GoalTracker, Settlement, Trigger};
use super::loop_guard::{
    idle_limit_message, pass_limit_message, rejection_message, LoopGuard, PassCounter,
    LOOP_MESSAGE, LOOP_REPEAT_LIMIT, PASS_REPEAT_LIMIT, REJECTION_LIMIT, SEM_AVANCO_LIMIT,
};
use super::markers::{parse_decision, parse_reply, ParsedDecision, ParsedReply};
use super::prompts::{StepPrompts, ALTERNATE_INSTRUCTION, REJECTION_PREFIX, REPLY_PREFIX};
use super::session_pool::ExchangeSessions;

pub(super) const NO_CONVERSATION: usize = 0;
pub(super) const USER_LABEL: &str = "Usuário";

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
}

pub(super) async fn run_turns(
    exchange: &Exchange<'_>,
    sessions: &mut ExchangeSessions,
    transcript: &mut Vec<String>,
) -> Result<TaskOutcome, FlowError> {
    let Exchange {
        app,
        state,
        request,
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
    let mut rejections = 0;
    loop {
        let instruction = turn.instruction();
        let prompts = with_cache(state, project_id, |cache| {
            Ok(StepPrompts::build(
                cache,
                definition,
                &turn.speaker,
                &instruction,
            ))
        })?;
        let reply = ask_step(exchange, sessions, &turn.speaker, &prompts).await?;
        if let Some(parsed) = parse_decision(&reply) {
            return raise_decision(exchange, transcript, &turn, parsed);
        }
        let ParsedReply {
            visible,
            delegation,
            updates,
        } = parse_reply(&reply, &turn.speaker, team);
        let applied = with_cache(state, project_id, |cache| {
            Ok(cache.apply_reply(&turn.speaker, updates, team))
        })?;
        let delegation = delegation.filter(|_| !definition.alternate);
        if !applied.rejected.is_empty() {
            if !visible.is_empty() {
                let to = next_turn(definition, &turn, &visible, None, team)
                    .map_or(Speaker::User, |next| Speaker::from(next.speaker));
                emit_message(
                    app,
                    project_id,
                    Speaker::from(turn.speaker.clone()),
                    to,
                    &visible,
                    *channel_id,
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
            sessions.review(&turn.speaker, rejections, 0, team)?.await;
            turn.rejection = Some(applied.rejected.join("; "));
            continue;
        }
        rejections = 0;

        let advanced = applied.created_variable || applied.changed_value;
        let feed = feed_text(&visible, &applied.accepted);
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
                )?;
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

        counters.goal.register_turn(advanced);
        let next = next_turn(
            definition,
            &turn,
            feed.as_deref().unwrap_or_default(),
            delegation.as_ref(),
            team,
        );
        let to = next
            .as_ref()
            .map_or(Speaker::User, |next| Speaker::from(next.speaker.clone()));
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
            )?;
        }
        let Some(next) = next else {
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
            )?;
        }
        let speaker_state = if delegation.is_some() {
            AgentState::Waiting
        } else {
            AgentState::Idle
        };
        emit_state(app, project_id, turn.speaker.clone(), speaker_state)?;
        if !advanced {
            sessions
                .review(
                    &turn.speaker,
                    counters.passes.by_speaker(&turn.speaker),
                    0,
                    team,
                )?
                .await;
        }
        sessions.review(&next.speaker, 0, loop_repeats, team)?.await;
        turn = next;
    }
}

#[derive(Default)]
struct Counters {
    passes: PassCounter,
    loop_guard: LoopGuard,
    goal: GoalTracker,
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
    )
}

// Quem decide fica Waiting até a resposta virar um novo pedido; quem delegou a ele volta a Idle.
fn raise_decision(
    exchange: &Exchange<'_>,
    transcript: &mut Vec<String>,
    turn: &Turn,
    parsed: ParsedDecision,
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
    let ParsedDecision {
        kind,
        title,
        detail,
        options,
        visible,
    } = parsed;
    let agent = turn.speaker.clone();
    let visible = parse_reply(&visible, &agent, team).visible;
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
        )?;
    }
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
                title: title.clone(),
                request: request.to_string(),
                item_id: item_id.map(str::to_string),
            },
        );
    emit(
        app,
        VokiaEvent::Decision {
            id,
            agent: agent.clone(),
            kind,
            title,
            detail,
            options,
            project_id: Some(project_id),
            item_id: item_id.map(str::to_string),
        },
    )?;
    for requester in &turn.requesters {
        emit_state(app, project_id, requester.clone(), AgentState::Idle)?;
    }
    emit_state(app, project_id, agent, AgentState::Waiting)?;
    Ok(TaskOutcome::Finished)
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

fn next_turn(
    definition: &TaskDefinition,
    turn: &Turn,
    feed: &str,
    delegation: Option<&(Agent, String)>,
    team: &Team,
) -> Option<Turn> {
    let speaker = turn.speaker.clone();
    if definition.alternate {
        let agent = next_participant(&definition.participants, &speaker);
        return Some(Turn::new(agent, ALTERNATE_INSTRUCTION, feed, Vec::new()));
    }
    if let Some((helper, request)) = delegation {
        let mut requesters = turn.requesters.clone();
        requesters.push(speaker);
        return Some(Turn::new(helper.clone(), request, request, requesters));
    }
    let (requester, remaining) = turn.requesters.split_last()?;
    let message = format!("{}{REPLY_PREFIX}{feed}", team.name_of(&speaker));
    Some(Turn::new(
        requester.clone(),
        &message,
        &message,
        remaining.to_vec(),
    ))
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

fn say(
    app: &AppHandle,
    project_id: i64,
    transcript: &mut Vec<String>,
    from: &Agent,
    to: Speaker,
    text: &str,
    team: &Team,
    channel_id: Option<i64>,
) -> Result<(), FlowError> {
    emit_message(
        app,
        project_id,
        Speaker::from(from.clone()),
        to,
        text,
        channel_id,
    )?;
    transcript.push(format!("{}: {text}", team.name_of(from)));
    Ok(())
}

pub(super) fn with_cache<T>(
    state: &AppState,
    project_id: i64,
    action: impl FnOnce(&mut Cache) -> Result<T, String>,
) -> Result<T, FlowError> {
    let mut caches = state.cache.lock().map_err(|_| FlowError::CachePoisoned)?;
    action(caches.entry(project_id).or_default()).map_err(FlowError::Cache)
}
