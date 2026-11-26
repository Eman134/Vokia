use std::path::Path;
use std::sync::atomic::Ordering;

use tauri::{AppHandle, Manager};

use crate::agents::{Agent, Team};
use crate::claude::{ProjectAccess, ToolCategory, TurnActivity, TurnUsage};
use crate::db::{
    load_recent_work_steps_safe, work_steps_session_key, DbState, HandoffDraft, NewTurnWorkSteps,
    NewUsageRecord, OrchestrationSnapshot, StoredTaskResult,
};
use crate::events::{AgentState, DecisionKind, GoalState, RequestProposal, Speaker, VokiaEvent};
use crate::mcp::{Delegation, DelegationIntent, ExchangeOutcome, McpState, SlotDecision};
use crate::orchestration::counters::TaskCounters;
use crate::orchestration::policy::{self, Passage, Verdict};
use crate::orchestration::snapshot::placement_of;
use crate::orchestration::snapshot::{enforced, ACTION_CONSULT, ACTION_RETURN};
use crate::orchestration::stages::{self, STAGE_DONE, STAGE_WAITING};
use crate::session_key::SessionKey;
use crate::state::{AppState, Batch, PendingDecision, RequestContext};
use crate::task::{Cache, Change, TaskDefinition, TaskKind, TaskOutcome};

use super::agent_session::{ask_step, StepReply};
use super::continuation::{
    classify_turn_failure, executed_action_tool, has_verification_evidence,
    load_continuation_config, ContinuationDecision, ContinuationGuard, ContinuationTrigger,
    PlanContinuation, TurnEndInput, TurnFailureKind,
};
use super::emit::{
    emit, emit_backlog_changed, emit_feed_note, emit_message, emit_state, now_millis,
    reconcile_waiting, MessageScope,
};
use super::error::FlowError;
use super::flow::{
    announce_batch, count_branch, dispatch_flow, dispatch_request, emit_stage, is_batch_open, mark_flow_end,
    open_batch, seal_batch, set_stage_state, stop_with_error,
};
use super::plan_stages::advance_plan;
use super::goal::{achieved_message, emit_goal, settle, GoalTracker, Settlement, Trigger};
use super::loop_guard::{
    idle_limit_message, pass_limit_message, rejection_message, LoopGuard, PassCounter,
    LOOP_MESSAGE, LOOP_REPEAT_LIMIT, PASS_REPEAT_LIMIT, REJECTION_LIMIT, SEM_AVANCO_LIMIT,
};
use super::markers::ParsedDecision;
use super::prompts::{StepPrompts, ToolPromptMode, ALTERNATE_INSTRUCTION, REJECTION_PREFIX};
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
    // Orquestrador a quem a resposta final do fluxo é endereçada quando ninguém mais espera por ela.
    pub(super) reply_to: Option<&'a Agent>,
}

impl<'a> Exchange<'a> {
    pub(super) fn scope(&self) -> MessageScope<'a> {
        MessageScope {
            channel_id: self.channel_id,
            item_id: self.item_id,
        }
    }

    // O card no prompt poupa ao agente uma ida à API só para chamar read_card; sem banco ou card ilegível,
    // o agente segue podendo chamar read_card.
    pub(super) fn card_context(&self) -> Option<String> {
        let item_id = self.item_id?;
        let db_state = self.app.state::<DbState>();
        let db = db_state.db()?;
        crate::mcp::tools::card_text(db, self.key.project_id, item_id).ok()
    }
}

struct TurnWaitersGuard<'a> {
    state: &'a AppState,
    key: &'a SessionKey,
}

impl<'a> TurnWaitersGuard<'a> {
    fn set(&self, requesters: &[Agent]) {
        if let Ok(mut flow) = self.state.flow.lock() {
            if requesters.is_empty() {
                flow.turn_waiters.remove(self.key);
            } else {
                flow.turn_waiters
                    .insert(self.key.clone(), requesters.to_vec());
            }
        }
    }
}

impl Drop for TurnWaitersGuard<'_> {
    fn drop(&mut self) {
        if let Ok(mut flow) = self.state.flow.lock() {
            flow.turn_waiters.remove(self.key);
        }
    }
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
    let waiters_guard = TurnWaitersGuard { state, key };
    let continuation_guard = ContinuationGuard::register(&state.continuations, key);
    let mut continuation = PlanContinuation::new(load_continuation_config(app));
    let mut chain_verified = false;
    let scope = exchange.scope();
    let project_id = key.project_id;
    let origin = key.agent.clone();
    let mut turn = Turn::first(definition, &origin, request);
    let mut counters = Counters::default();
    counters.kind = definition.kind;
    counters.acted.push(origin.clone());
    // Um lote é a entrega de trabalho até o próximo fim de cadeia; só ele exige revisão, e cada lote novo volta a exigi-la.
    let mut lote_open = false;
    let mut rejections = 0;
    let has_mcp_server = app.state::<McpState>().server().is_some();
    loop {
        if super::runs::is_agent_paused(app, project_id, &turn.speaker) {
            emit_state(app, project_id, turn.speaker.clone(), AgentState::Idle, *item_id)?;
            return Ok(TaskOutcome::Paused);
        }
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
                None,
            );
        }
        let speaker_transport = super::fallback::effective_tool_transport_for(
            exchange.app,
            exchange.state,
            team,
            &turn.speaker,
            Some(exchange.project.level),
        );
        let tool_mode = if !has_mcp_server {
            ToolPromptMode::None
        } else {
            match speaker_transport {
                crate::providers::ToolTransport::McpNative => ToolPromptMode::Mcp,
                crate::providers::ToolTransport::VokiaCli => {
                    if super::adapters::resolve_vokia_cli_path().is_some() {
                        ToolPromptMode::VokiaCli
                    } else {
                        ToolPromptMode::None
                    }
                }
                crate::providers::ToolTransport::None => ToolPromptMode::None,
            }
        };
        let with_tools = tool_mode != ToolPromptMode::None;
        let card = exchange.card_context();
        let prompts = with_cache(state, project_id, |cache| {
            Ok(StepPrompts::build(
                cache,
                definition,
                &turn.speaker,
                &turn.instruction(),
                tool_mode,
                card.as_deref(),
            ))
        })?;
        let turn_started = std::time::Instant::now();
        let StepReply {
            text: reply,
            activity,
            mut usage,
            outcome,
        } = match ask_step(exchange, sessions, &turn.speaker, &prompts).await {
            Ok(step) => step,
            Err(error) => {
                let failure_kind = classify_turn_failure(&error);
                if matches!(
                    failure_kind,
                    TurnFailureKind::Compaction | TurnFailureKind::ProviderExit
                ) {
                    let recovery_input = TurnEndInput {
                        reply_text: String::new(),
                        turn_plan: state
                            .turn_plans
                            .get(&turn.speaker, Some(project_id), *item_id)
                            .map(|snap| snap.plan),
                        executed_action_tool: false,
                        has_pending_human_decision: has_pending_decision_for(
                            state,
                            project_id,
                            &turn.speaker,
                        ),
                        usage_limit_paused: speaker_usage_limit_paused(
                            exchange,
                            &turn.speaker,
                            None,
                            None,
                        ),
                        cancelled: continuation_guard.is_cancelled()
                            || super::runs::is_agent_paused(app, project_id, &turn.speaker)
                            || has_pending_queued_job(state, key),
                        pending_verification_items: Vec::new(),
                        has_verification_evidence: chain_verified,
                        compaction_failed: failure_kind == TurnFailureKind::Compaction,
                        provider_exited_abruptly: failure_kind == TurnFailureKind::ProviderExit,
                        work_steps_blocks: load_recent_work_steps_for(
                            app,
                            project_id,
                            &turn.speaker,
                            *item_id,
                        ),
                    };
                    if let ContinuationDecision::Continue {
                        trigger,
                        attempt,
                        max_attempts,
                        prompt,
                        marker,
                    } = continuation.consider(&recovery_input)
                    {
                        emit_turn_continuation(
                            exchange,
                            &turn.speaker,
                            trigger,
                            attempt,
                            max_attempts,
                            &marker,
                        )?;
                        turn = Turn::new(
                            turn.speaker.clone(),
                            &prompt,
                            &prompt,
                            turn.requesters.clone(),
                        );
                        continue;
                    }
                }
                if error.is_limit() || error.is_cli_failure() {
                    let _ = super::emit::emit_error_message(
                        app,
                        project_id,
                        Speaker::from(turn.speaker.clone()),
                        exchange.scope(),
                        &error.to_string(),
                    );
                }
                return Err(error);
            }
        };
        let turn_duration_ms = turn_started.elapsed().as_millis() as u64;
        if let Some(ref mut turn_usage) = usage {
            record_turn_usage(
                exchange,
                &turn.speaker,
                turn_usage,
                turn_duration_ms,
            );
        }
        record_turn_work_steps(
            exchange,
            &turn.speaker,
            activity.as_ref(),
            usage.as_ref().and_then(|u| u.provider.as_deref()),
        );
        super::memory_review::on_turn_completed_for_learning(
            app,
            project_id,
            &turn.speaker,
            *item_id,
        );
        let turn_executed_action = executed_action_tool(activity.as_ref());
        if has_verification_evidence(activity.as_ref()) {
            chain_verified = true;
        }
        let turn_activity_plan = activity.as_ref().and_then(|act| act.plan.clone());
        let turn_provider = usage.as_ref().and_then(|u| u.provider.clone());
        let turn_model = usage.as_ref().and_then(|u| u.model.clone());
        counters.task.register_turn(&turn.speaker);
        counters.act(&turn.speaker);
        // Só a primeira mensagem do turno leva a atividade; turno sem mensagem a descarta.
        let mut activity = activity;
        let ExchangeOutcome {
            confirmations,
            decision,
            mut proposal,
            delegations: registered_delegations,
            variables,
        } = outcome;
        // Líder com julgamento de fluxo ligado: a linha #fluxo grava o modo da task e não vai para a tela.
        let judging = exchange
            .orchestration
            .is_some_and(|snapshot| stages::judging_team(snapshot, &turn.speaker).is_some());
        let judgment = judging.then(|| stages::parse_judgment(&reply)).flatten();
        let reply = if judging {
            stages::without_judgment_lines(&reply)
        } else {
            reply
        };
        if let (Some(judgment), Some(item)) = (judgment, *item_id) {
            record_flow_judgment(app, project_id, item, &turn.speaker, judgment.mode)?;
        }
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
        // Decisão, proposta de card e ferramenta que escreve ou executa são trabalho, mesmo num pedido que parecia conversa.
        let worked = activity.as_ref().is_some_and(|activity| {
            activity.tools.iter().any(|tool| {
                matches!(tool.category, ToolCategory::Edit | ToolCategory::Command | ToolCategory::App)
            })
        });
        if raised.is_some() || proposal.is_some() || worked {
            counters.promote();
        }
        if let Some((decision, shown)) = raised {
            let shown = Some(shown).filter(|shown| !shown.is_empty());
            let shown = with_confirmations(shown, &confirmations).unwrap_or_default();
            return raise_decision(
                exchange,
                transcript,
                &turn,
                decision,
                shown,
                activity,
                proposal,
                usage,
            );
        }
        let (notices, work): (Vec<Delegation>, Vec<Delegation>) =
            resolve_delegations(registered_delegations, text_delegation, definition.alternate)
                .into_iter()
                .partition(|delegation| delegation.intent.notify);
        let mut passage: Option<Passage> = None;
        let mut escalation: Option<String> = None;
        let mut delegation: Option<(Agent, String)> = None;
        let mut intent: Option<DelegationIntent> = None;
        let mut parallel_checked: Vec<Branch> = Vec::new();
        if rejected.is_empty() {
            let mut deliveries: Vec<Branch> = Vec::new();
            // Gateway paralelo: a delegação vira uma entrega a cada saída, no mesmo lote.
            let mut forks: Vec<Branch> = Vec::new();
            for Delegation {
                agent: helper,
                request,
                intent: work_intent,
            } in work
            {
                let line_item = work_intent.item_id.as_deref().or(*item_id);
                match policy::check(
                    exchange.orchestration,
                    team,
                    &turn.speaker,
                    &helper,
                    &counters.task,
                    &edge_passages(app, project_id, line_item, &counters.task),
                    work_intent.event.as_deref(),
                ) {
                    Verdict::Deliver(found) => {
                        if let Some(motive) = skip_refusal(exchange, &turn.speaker, line_item, &found)
                        {
                            rejected.push(motive);
                            continue;
                        }
                        // Gateway exclusivo: o evento escolhe o destino, e o que o agente citou não vale.
                        let to = match found.routed.as_ref().filter(|routed| routed.to != helper) {
                            Some(routed) => {
                                let text = format!(
                                    "Gateway enviou a {} pelo evento {}.",
                                    team.name_of(&routed.to),
                                    routed.event
                                );
                                emit_feed_note(
                                    app,
                                    project_id,
                                    Speaker::from(turn.speaker.clone()),
                                    &text,
                                    now_millis()?,
                                )?;
                                routed.to.clone()
                            }
                            None => helper,
                        };
                        deliveries.push(Branch {
                            to,
                            request,
                            passage: found,
                            intent: work_intent.clone(),
                        });
                    }
                    Verdict::Fork(found) => {
                        let refused = found
                            .iter()
                            .find_map(|branch| skip_refusal(exchange, &turn.speaker, line_item, branch));
                        match refused {
                            Some(motive) => rejected.push(motive),
                            None => forks.extend(found.into_iter().filter_map(|branch| {
                                let to = branch.routed.as_ref()?.to.clone();
                                Some(Branch {
                                    to,
                                    request: request.clone(),
                                    passage: branch,
                                    intent: work_intent.clone(),
                                })
                            })),
                        }
                    }
                    Verdict::Refuse(motive) => rejected.push(motive),
                    Verdict::Escalate(motive) => escalation = Some(motive),
                }
            }
            // Uma entrega em série segue o caminho normal; várias, ou uma bifurcação, viram lote paralelo.
            if forks.is_empty() && deliveries.len() == 1 {
                if let Some(Branch {
                    to,
                    request,
                    passage: found,
                    intent: only,
                }) = deliveries.pop()
                {
                    delegation = Some((to, request));
                    passage = Some(found);
                    intent = Some(only);
                }
            } else {
                parallel_checked = deliveries.into_iter().chain(forks).collect();
            }
        }
        let line_item = intent
            .as_ref()
            .and_then(|intent| intent.item_id.as_deref())
            .or(*item_id);
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
                    scope,
                    activity.take(),
                    None,
                    usage.take(),
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
        for Delegation {
            agent,
            request,
            intent: notice,
        } in notices
        {
            let notice_item = notice.item_id.as_deref().or(*item_id);
            say(
                app,
                project_id,
                transcript,
                &turn.speaker,
                Speaker::from(agent),
                &request,
                team,
                MessageScope {
                    channel_id: scope.channel_id,
                    item_id: notice_item,
                },
                None,
                None,
                None,
            )?;
        }
        let mut confirmations = confirmations;
        confirmations.extend(cards.confirmations);
        let advanced = applied.created_variable
            || applied.changed_value
            || variables.created_variable
            || variables.changed_value;
        let helpers = delegation
            .iter()
            .map(|(helper, _)| helper)
            .chain(parallel_checked.iter().map(|branch| &branch.to));
        let mut delegated = false;
        for helper in helpers {
            counters.act(helper);
            delegated = true;
        }
        if delegated || cards.changed || !confirmations.is_empty() || advanced {
            counters.promote();
        }
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
                    scope,
                    activity.take(),
                    proposal.take(),
                    usage.take(),
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
            report_task_result(exchange, &counters, TaskOutcome::Finished)?;
            let message = format!("Tarefa concluída: {}.", definition.objetivo);
            conclude(
                app,
                project_id,
                transcript,
                &turn.speaker,
                &origin,
                &message,
                team,
                scope,
            )?;
            return Ok(TaskOutcome::Finished);
        }

        // Cada delegação em paralelo vai para a instância do helper (ou espera no teto); a delegação em série segue o caminho normal.
        if !parallel_checked.is_empty() {
            let snapshot = exchange.orchestration;
            let max_parallel = snapshot.map_or(1, |snapshot| snapshot.settings.max_parallel);
            for branch in &parallel_checked {
                count_passage(&mut counters.task, &turn.speaker, &branch.to, &branch.passage);
            }
            let batch = open_batch(
                state,
                Batch {
                    project_id,
                    leader: turn.speaker.clone(),
                    objective: exchange.definition.objetivo.clone(),
                    variables: exchange.definition.variables.clone(),
                    channel_id: *channel_id,
                    item_id: line_item.map(str::to_string),
                    // Contagem do próprio líder enquanto despacha: uma troca que termina cedo não dispara a junção.
                    pending: 1,
                    results: Vec::new(),
                    dispatched: Vec::new(),
                    started: Vec::new(),
                    returned: Vec::new(),
                    counters: counters.task.clone(),
                },
            )?;
            if super::runs::is_agent_paused(app, project_id, &turn.speaker) {
                seal_batch(app, state, batch);
                emit_state(app, project_id, turn.speaker.clone(), AgentState::Idle, *item_id)?;
                return Ok(TaskOutcome::Paused);
            }
            let fork_result: Result<(), FlowError> = (|| {
                for Branch {
                    to: helper,
                    request,
                    passage: found,
                    intent: branch_intent,
                } in parallel_checked
                {
                    let branch_item = branch_intent.item_id.as_deref().or(*item_id);
                    record_passage(
                        app,
                        exchange.run_id,
                        project_id,
                        branch_item,
                        None,
                        &turn.speaker,
                        &helper,
                        &found,
                    )?;
                    let max_instances = snapshot
                        .and_then(|snapshot| placement_of(snapshot, &helper))
                        .map_or(1, |placement| placement.max_instances);
                    let mut context = RequestContext::new(
                        exchange.run_id,
                        branch_item.map(str::to_string),
                        *channel_id,
                    );
                    context.batch = Some(batch);
                    context.ping_seconds = branch_intent.ping_seconds;
                    count_branch(state, project_id, branch_item)?;
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
                        MessageScope {
                            channel_id: scope.channel_id,
                            item_id: branch_item,
                        },
                        None,
                        None,
                        None,
                    )?;
                }
                Ok(())
            })();
            if let Err(error) = fork_result {
                seal_batch(app, state, batch);
                return Err(error);
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
                        scope,
                        activity.take(),
                        proposal.take(),
                        usage.take(),
                    )?;
                }
                if is_batch_open(state, batch) {
                    emit_state(app, project_id, turn.speaker.clone(), AgentState::Waiting, *item_id)?;
                } else {
                    reconcile_waiting(app, state, project_id);
                }
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
                usage.take(),
            );
        }
        counters.goal.register_turn(advanced);
        let next = next_turn(
            definition,
            &turn,
            feed.as_deref().unwrap_or_default(),
            delegation.as_ref(),
        );
        let to = final_recipient(definition, &turn, next.as_ref(), exchange.reply_to);
        if let Some(text) = &feed {
            say(
                app,
                project_id,
                transcript,
                &turn.speaker,
                to,
                text,
                team,
                scope,
                activity.take(),
                proposal.take(),
                usage.take(),
            )?;
        }
        if cards.changed {
            emit_backlog_changed(app, project_id)?;
        }
        let Some(next) = next else {
            let turn_plan = turn_activity_plan.or_else(|| {
                state
                    .turn_plans
                    .get(&turn.speaker, Some(project_id), *item_id)
                    .map(|snap| snap.plan)
            });
            let continuation_input = TurnEndInput {
                reply_text: visible.clone(),
                turn_plan,
                executed_action_tool: turn_executed_action,
                has_pending_human_decision: has_pending_decision_for(
                    state,
                    project_id,
                    &turn.speaker,
                ),
                usage_limit_paused: speaker_usage_limit_paused(
                    exchange,
                    &turn.speaker,
                    turn_provider.as_deref(),
                    turn_model.as_deref(),
                ),
                cancelled: continuation_guard.is_cancelled()
                    || super::runs::is_agent_paused(app, project_id, &turn.speaker)
                    || has_pending_queued_job(state, key),
                pending_verification_items: load_pending_verification_items(exchange),
                has_verification_evidence: chain_verified,
                compaction_failed: false,
                provider_exited_abruptly: false,
                work_steps_blocks: Vec::new(),
            };
            if let ContinuationDecision::Continue {
                trigger,
                attempt,
                max_attempts,
                prompt,
                marker,
            } = continuation.consider(&continuation_input)
            {
                emit_turn_continuation(
                    exchange,
                    &turn.speaker,
                    trigger,
                    attempt,
                    max_attempts,
                    &marker,
                )?;
                turn = Turn::new(
                    turn.speaker.clone(),
                    &prompt,
                    &prompt,
                    turn.requesters.clone(),
                );
                continue;
            }
            let lote_delivered = std::mem::take(&mut lote_open);
            // Chegar ao fim do fluxo com card vai para a validação final (quando o último ramo termina); a revisão
            // de lote só vale para o que não tem card.
            let ends_flow = match line_item {
                Some(item) if policy::reaches_flow_end(exchange.orchestration, &turn.speaker) => {
                    mark_flow_end(state, project_id, item, &turn.speaker)?;
                    true
                }
                _ => false,
            };
            let review = (lote_delivered && !ends_flow)
                .then(|| policy::chain_review(exchange.orchestration, exchange.team, &turn.speaker))
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
                    None,
                );
            }
            waiters_guard.set(&[]);
            for requester in &turn.requesters {
                emit_state(app, project_id, requester.clone(), AgentState::Idle, *item_id)?;
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
            if let Some(motive) = policy::task_limit_reached(exchange.orchestration, &counters.task)
            {
                let escalation = Turn::new(
                    turn.speaker.clone(),
                    "",
                    "",
                    turn.requesters.clone(),
                );
                return escalate_limit(
                    exchange,
                    transcript,
                    &escalation,
                    motive,
                    String::new(),
                    None,
                    None,
                    None,
                );
            }
            say(
                app,
                project_id,
                transcript,
                &turn.speaker,
                Speaker::from(helper.clone()),
                helper_request,
                team,
                MessageScope {
                    channel_id: scope.channel_id,
                    item_id: line_item,
                },
                activity.take(),
                proposal.take(),
                usage.take(),
            )?;
            if let Some(passage) = &passage {
                // Consulta não é entrega de etapa: não abre lote nem muda a revisão do fim da cadeia.
                lote_open |= passage.action != ACTION_CONSULT;
                count_passage(&mut counters.task, &turn.speaker, helper, passage);
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
                // O Ember só orquestra: despacha o líder e fica livre, quem espera o ajudante é o líder.
                let orchestrator_hands_off = turn.speaker == Agent::ember();
                if passage.asynchronous || orchestrator_hands_off {
                    let target_item = intent
                        .as_ref()
                        .and_then(|intent| intent.item_id.as_deref())
                        .or(line_item);
                    let ping_seconds = intent.as_ref().and_then(|intent| intent.ping_seconds);
                    let reply_to = orchestrator_hands_off.then(|| turn.speaker.clone());
                    dispatch_async_branch(
                        exchange,
                        target_item,
                        helper,
                        helper_request,
                        ping_seconds,
                        reply_to,
                    )?;
                    emit_state(app, project_id, turn.speaker.clone(), AgentState::Idle, *item_id)?;
                    return Ok(TaskOutcome::Finished);
                }
            }
        }
        let speaker_state = if delegation.is_some() {
            AgentState::Waiting
        } else {
            AgentState::Idle
        };
        emit_state(app, project_id, turn.speaker.clone(), speaker_state, *item_id)?;
        if !advanced {
            sessions
                .review(&turn.speaker, counters.passes.by_speaker(&turn.speaker), 0)
                .await;
        }
        sessions.review(&next.speaker, 0, loop_repeats).await;
        waiters_guard.set(&next.requesters);
        turn = next;
    }
}

#[derive(Default)]
struct Counters {
    passes: PassCounter,
    loop_guard: LoopGuard,
    goal: GoalTracker,
    task: TaskCounters,
    // Nasce da definição e só sobe: um pedido que parecia conversa vira demanda quando o fluxo mostra trabalho.
    kind: TaskKind,
    // Quem falou ou recebeu delegação na troca, sem repetir, para o cartão de conclusão.
    acted: Vec<Agent>,
}

impl Counters {
    fn promote(&mut self) {
        self.kind = TaskKind::Demanda;
    }

    fn act(&mut self, agent: &Agent) {
        if !self.acted.contains(agent) {
            self.acted.push(agent.clone());
        }
    }

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
        key,
        definition,
        team,
        ..
    } = exchange;
    let scope = exchange.scope();
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
                scope,
            )?;
            report_task_result(exchange, counters, TaskOutcome::Finished)?;
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
            report_task_result(exchange, counters, TaskOutcome::Stalled)?;
            Step::End(TaskOutcome::Stalled)
        }
        Settlement::Ended => {
            emit_state(app, project_id, speaker.clone(), AgentState::Idle, exchange.item_id)?;
            // Conversa simples termina só com a resposta do agente: sem cartão de conclusão.
            if counters.kind == TaskKind::Demanda {
                report_task_result(exchange, counters, TaskOutcome::Finished)?;
            }
            Step::End(TaskOutcome::Finished)
        }
    })
}

// Cartão para o usuário: grava no histórico e avisa a tela. Os agentes são a origem, os participantes definidos e quem agiu.
fn report_task_result(
    exchange: &Exchange<'_>,
    counters: &Counters,
    outcome: TaskOutcome,
) -> Result<(), FlowError> {
    let project_id = exchange.key.project_id;
    let mut agents = vec![exchange.key.agent.clone()];
    for agent in exchange.definition.participants.iter().chain(&counters.acted) {
        if !agents.contains(agent) {
            agents.push(agent.clone());
        }
    }
    let time = now_millis()?;
    let result = StoredTaskResult {
        objective: exchange.definition.objetivo.clone(),
        concluded: outcome == TaskOutcome::Finished,
        turns: counters.task.turns(),
        agents,
    };
    if let Some(db) = exchange.app.state::<DbState>().db() {
        db.insert_task_result(
            project_id,
            Speaker::from(exchange.key.agent.clone()),
            time,
            exchange.channel_id,
            exchange.item_id,
            &result,
        )?;
        if let (true, Some(item)) = (result.concluded, exchange.item_id) {
            set_stage_state(exchange.app, project_id, item, STAGE_DONE)?;
            advance_plan(exchange.app, project_id, item)?;
        }
    }
    emit(
        exchange.app,
        VokiaEvent::TaskResult {
            project_id,
            objective: result.objective,
            concluded: result.concluded,
            turns: result.turns,
            agents: result.agents,
            time,
            channel_id: exchange.channel_id,
            item_id: exchange.item_id.map(str::to_string),
        },
    )
}

#[allow(clippy::too_many_arguments)] // campos da mensagem final da tarefa
fn conclude(
    app: &AppHandle,
    project_id: i64,
    transcript: &mut Vec<String>,
    speaker: &Agent,
    origin: &Agent,
    message: &str,
    team: &Team,
    scope: MessageScope<'_>,
) -> Result<(), FlowError> {
    emit_state(app, project_id, speaker.clone(), AgentState::Idle, scope.item_id)?;
    say(
        app,
        project_id,
        transcript,
        origin,
        Speaker::User,
        message,
        team,
        scope,
        None,
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
                kind: decision.kind,
                title: decision.title.clone(),
                request: request.to_string(),
                item_id: item_id.map(str::to_string),
                options: decision.options.clone(),
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
    usage: Option<TurnUsage>,
) -> Result<TaskOutcome, FlowError> {
    let Exchange {
        app,
        state,
        request,
        item_id,
        key,
        team,
        ..
    } = exchange;
    let scope = exchange.scope();
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
            scope,
            activity,
            proposal,
            usage,
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
    // Decisão aberta: a task espera o usuário, então não conta como parada.
    if let Some(item) = *item_id {
        set_stage_state(app, project_id, item, STAGE_WAITING)?;
    }
    if let Ok(mut flow) = state.flow.lock() {
        flow.turn_waiters.remove(key);
    }
    for requester in &turn.requesters {
        emit_state(app, project_id, requester.clone(), AgentState::Idle, *item_id)?;
    }
    emit_state(app, project_id, agent, AgentState::Waiting, *item_id)?;
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
    usage: Option<TurnUsage>,
) -> Result<TaskOutcome, FlowError> {
    let decision = SlotDecision {
        kind: DecisionKind::Pergunta,
        title: policy::LIMIT_TITLE.to_string(),
        detail: motive,
        options: Vec::new(),
    };
    raise_decision(
        exchange, transcript, turn, decision, visible, activity, proposal, usage,
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

// Conta a passagem e, quando ela devolve trabalho, o retorno; a contagem vale para o limite de turnos e de passagens.
pub(super) fn count_passage(
    counters: &mut TaskCounters,
    from: &Agent,
    to: &Agent,
    passage: &Passage,
) {
    if let Some(edge_id) = passage.edge_id {
        counters.register_passage(edge_id);
    }
    if passage.action == ACTION_RETURN {
        counters.register_return(from, to);
    }
}

// Linha assíncrona: o pedido segue como ramo separado do mesmo card, e quem enviou termina o turno na entrega.
// Com `reply_to`, o ramo é um fluxo completo do líder (cadeia com os ajudantes) que responde a esse agente.
fn dispatch_async_branch(
    exchange: &Exchange<'_>,
    item_id: Option<&str>,
    helper: &Agent,
    request: &str,
    ping_seconds: Option<u64>,
    reply_to: Option<Agent>,
) -> Result<(), FlowError> {
    let snapshot = exchange.orchestration;
    let max_instances = snapshot
        .and_then(|snapshot| placement_of(snapshot, helper))
        .map_or(1, |placement| placement.max_instances);
    let max_parallel = snapshot.map_or(1, |snapshot| snapshot.settings.max_parallel);
    let mut context = RequestContext::new(
        exchange.run_id,
        item_id.map(str::to_string),
        exchange.channel_id,
    );
    context.ping_seconds = ping_seconds;
    count_branch(exchange.state, exchange.key.project_id, item_id)?;
    let dispatch = if reply_to.is_some() {
        dispatch_flow
    } else {
        dispatch_request
    };
    context.reply_to = reply_to;
    dispatch(
        exchange.app,
        exchange.state,
        exchange.key.project_id,
        helper,
        request,
        context,
        max_instances,
        max_parallel,
    )
}

// Passagens da ligação na task: com card, as gravadas desde o início do ciclo dele (devolver zera); sem card, as da troca.
pub(super) fn edge_passages<'a>(
    app: &'a AppHandle,
    project_id: i64,
    item_id: Option<&'a str>,
    counters: &'a TaskCounters,
) -> impl Fn(i64) -> i64 + 'a {
    move |edge_id| {
        let state = app.state::<DbState>();
        item_id
            .and_then(|item| {
                state
                    .db()
                    .and_then(|db| db.count_edge_passages(project_id, item, edge_id).ok())
            })
            .unwrap_or_else(|| counters.passages(edge_id))
    }
}

#[allow(clippy::too_many_arguments)] // campos da passagem gravada no banco
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
        edge_id: passage.edge_id,
    };
    match state
        .require()
        .and_then(|db| db.record_handoff(project_id, &draft))
    {
        // Nada na tela depende das passagens, exceto a etapa da task com card, que sai a cada movimento.
        Ok(()) => match item_id {
            Some(item) => emit_stage(app, project_id, item),
            None => Ok(()),
        },
        Err(error) => emit(
            app,
            VokiaEvent::Error {
                message: format!("{MOVEMENT_ERROR_PREFIX}{error}."),
            },
        ),
    }
}

// Pulo de camada recusado pelo julgamento do líder. Só vale com o julgamento ligado no time de quem envia.
fn skip_refusal(
    exchange: &Exchange<'_>,
    from: &Agent,
    item_id: Option<&str>,
    passage: &Passage,
) -> Option<String> {
    let snapshot = exchange.orchestration?;
    if !enforced(Some(snapshot)) {
        return None;
    }
    let judged = placement_of(snapshot, from)
        .and_then(|placement| snapshot.teams.iter().find(|team| team.id == placement.team_id))
        .is_some_and(|team| team.judge_full_flow);
    if !judged {
        return None;
    }
    let flow_mode = item_id.and_then(|item| {
        exchange
            .app
            .state::<DbState>()
            .db()
            .and_then(|db| db.task_stage(exchange.key.project_id, item).ok().flatten())
            .and_then(|stage| stage.flow_mode)
    });
    stages::skip_motive(
        &snapshot.layers,
        passage.from_layer.as_deref(),
        passage.to_layer.as_deref(),
        flow_mode.as_deref(),
    )
}

// Grava o julgamento do líder na task e leva a etapa à tela. Falha de gravação vira aviso e a troca segue.
fn record_flow_judgment(
    app: &AppHandle,
    project_id: i64,
    item_id: &str,
    leader: &Agent,
    mode: &str,
) -> Result<(), FlowError> {
    let state = app.state::<DbState>();
    match state
        .require()
        .and_then(|db| db.set_task_flow_mode(project_id, item_id, leader.as_str(), mode))
    {
        Ok(()) => emit_stage(app, project_id, item_id),
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

// A resposta que fecha o fluxo de uma delegação do orquestrador vai a ele (mensagem, sem turno); o resto segue a regra comum.
fn final_recipient(
    definition: &TaskDefinition,
    turn: &Turn,
    next: Option<&Turn>,
    reply_to: Option<&Agent>,
) -> Speaker {
    match (next, reply_to) {
        (None, Some(orchestrator)) if turn.requesters.is_empty() => {
            Speaker::from(orchestrator.clone())
        }
        _ => speech_recipient(definition, turn, next),
    }
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
// Delegações registradas valem; sem nenhuma, a linha `@Nome:` do texto vira uma. No modo alternado ninguém é delegado.
fn resolve_delegations(
    registered: Vec<Delegation>,
    from_text: Option<(Agent, String)>,
    alternate: bool,
) -> Vec<Delegation> {
    if alternate {
        return Vec::new();
    }
    if registered.is_empty() {
        return from_text
            .map(|(agent, request)| Delegation {
                agent,
                request,
                intent: DelegationIntent::default(),
            })
            .into_iter()
            .collect();
    }
    registered
        .into_iter()
        .map(|delegation| Delegation {
            request: labeled_request(&delegation.request, &delegation.intent),
            ..delegation
        })
        .collect()
}

// Saída de um turno que vai a um agente; `intent` guarda o item, o evento e o ping pedidos para ela.
struct Branch {
    to: Agent,
    request: String,
    passage: Passage,
    intent: DelegationIntent,
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

#[allow(clippy::too_many_arguments)] // campos da mensagem do turno
fn say(
    app: &AppHandle,
    project_id: i64,
    transcript: &mut Vec<String>,
    from: &Agent,
    to: Speaker,
    text: &str,
    team: &Team,
    scope: MessageScope<'_>,
    activity: Option<TurnActivity>,
    request_proposal: Option<RequestProposal>,
    usage: Option<TurnUsage>,
) -> Result<(), FlowError> {
    emit_message(
        app,
        project_id,
        Speaker::from(from.clone()),
        to,
        text,
        scope,
        activity,
        request_proposal,
        usage,
    )?;
    transcript.push(format!(
        "{}: {text}",
        crate::orchestration::instances::display_name(team, from)
    ));
    Ok(())
}

// Persiste métricas de uso desacopladas das mensagens; nunca grava texto de prompt nem resposta.
fn record_turn_usage(
    exchange: &Exchange<'_>,
    speaker: &Agent,
    usage: &mut TurnUsage,
    duration_ms: u64,
) {
    let occurred_at = match now_millis() {
        Ok(ms) => ms as i64,
        Err(_) => return,
    };
    let origin = usage_origin_for(exchange, speaker);
    let record = NewUsageRecord::from_turn_usage(
        occurred_at,
        Some(exchange.key.project_id),
        exchange.item_id.map(str::to_string),
        speaker.as_str(),
        origin,
        usage,
        duration_ms,
    );
    if usage.cost_usd.is_none() {
        usage.cost_usd = record.cost_usd_estimated;
    }
    if let Some(db) = exchange.app.state::<DbState>().db() {
        let _ = db.record_usage(&record);
    }
}

// Persiste resumo compacto [work steps] do turno no SQLite sem nunca bloquear a conversa.
pub(super) fn record_turn_work_steps(
    exchange: &Exchange<'_>,
    speaker: &Agent,
    activity: Option<&TurnActivity>,
    provider: Option<&str>,
) {
    let Some(activity) = activity else {
        return;
    };
    let collector = super::work_steps::WorkStepCollector::from_turn_activity(activity);
    let Some(formatted) = collector.format() else {
        return;
    };
    let created_at = match now_millis() {
        Ok(ms) => ms as i64,
        Err(_) => return,
    };
    let session_key = work_steps_session_key(
        exchange.key.project_id,
        speaker.as_str(),
        exchange.item_id,
    );
    let record = NewTurnWorkSteps {
        project_id: exchange.key.project_id,
        agent: speaker.as_str(),
        session_key: &session_key,
        card_id: exchange.item_id,
        provider,
        block: &formatted.block,
        step_count: formatted.step_count,
        omitted_count: formatted.omitted_count,
        created_at,
    };
    if let Some(db) = exchange.app.state::<DbState>().db() {
        let _ = db.insert_turn_work_steps(&record);
    }
}

fn usage_origin_for(exchange: &Exchange<'_>, speaker: &Agent) -> &'static str {
    let is_review_layer = exchange.orchestration.is_some_and(|snapshot| {
        placement_of(snapshot, speaker)
            .and_then(|placement| snapshot.layers.iter().find(|l| l.id == placement.layer_id))
            .is_some_and(|layer| layer.kind == "revisar")
    });
    if is_review_layer {
        return "review";
    }
    if exchange.item_id.is_some() {
        return "card";
    }
    if speaker == &Agent::ember() && exchange.definition.kind == TaskKind::Demanda {
        return "triage";
    }
    "quick"
}

fn emit_turn_continuation(
    exchange: &Exchange<'_>,
    speaker: &Agent,
    trigger: ContinuationTrigger,
    attempt: u32,
    max_attempts: u32,
    marker: &str,
) -> Result<(), FlowError> {
    let project_id = exchange.key.project_id;
    let time = now_millis()?;
    if let Some(item_id) = exchange.item_id {
        if let Some(db) = exchange.app.state::<DbState>().db() {
            if db.add_card_activity(project_id, item_id, marker).is_ok() {
                let _ = emit_backlog_changed(exchange.app, project_id);
            }
        }
    }
    emit(
        exchange.app,
        VokiaEvent::TurnContinuation {
            project_id,
            agent: speaker.clone(),
            trigger,
            attempt,
            max_attempts,
            marker: marker.to_string(),
            time,
            channel_id: exchange.channel_id,
            item_id: exchange.item_id.map(str::to_string),
        },
    )
}

fn speaker_usage_limit_paused(
    exchange: &Exchange<'_>,
    speaker: &Agent,
    turn_provider: Option<&str>,
    turn_model: Option<&str>,
) -> bool {
    let now_ms = crate::usage::now_millis();
    if let Some(provider) = turn_provider.filter(|p| !p.trim().is_empty()) {
        return exchange
            .state
            .usage
            .is_provider_paused_by_limit(provider, turn_model, now_ms);
    }
    let Some(profile) = exchange.team.profile(speaker) else {
        return false;
    };
    let providers = exchange.app.state::<crate::providers::ProviderRegistry>();
    let global_active = super::fallback::load_global_active_llm(exchange.app);
    let global_fallback = super::fallback::load_global_fallback(exchange.app);
    let candidates = super::fallback::resolve_candidates(
        speaker,
        profile,
        &providers,
        exchange.state,
        global_active.as_ref(),
        &global_fallback,
        Some(exchange.project.level),
    );
    if let Some(first) = candidates.first() {
        return exchange.state.usage.is_provider_paused_by_limit(
            &first.cli,
            Some(&first.model),
            now_ms,
        );
    }
    exchange
        .state
        .usage
        .is_provider_paused_by_limit(&profile.cli, Some(&profile.model), now_ms)
}

fn has_pending_decision_for(state: &AppState, project_id: i64, speaker: &Agent) -> bool {
    state.decisions.lock().is_ok_and(|decisions| {
        decisions
            .values()
            .any(|decision| decision.project_id == project_id && &decision.agent == speaker)
    })
}

fn has_pending_queued_job(state: &AppState, key: &SessionKey) -> bool {
    state
        .flow
        .lock()
        .is_ok_and(|flow| flow.pending.get(key).is_some_and(|queue| !queue.is_empty()))
}

fn load_pending_verification_items(exchange: &Exchange<'_>) -> Vec<String> {
    let mut items = Vec::new();
    if let (Some(item_id), Some(db)) = (exchange.item_id, exchange.app.state::<DbState>().db()) {
        if let Ok(criteria) = db.plan_criteria(exchange.key.project_id, item_id) {
            for criterion in criteria.into_iter().filter(|c| !c.done) {
                items.push(format!("{}: {}", criterion.id, criterion.text));
            }
        }
    }
    if items.is_empty()
        && exchange.definition.verify
        && !exchange.definition.criterion.trim().is_empty()
    {
        items.push(exchange.definition.criterion.trim().to_string());
    }
    items
}

fn load_recent_work_steps_for(
    app: &AppHandle,
    project_id: i64,
    speaker: &Agent,
    item_id: Option<&str>,
) -> Vec<String> {
    let db_state = app.state::<DbState>();
    let session_key = work_steps_session_key(project_id, speaker.as_str(), item_id);
    load_recent_work_steps_safe(db_state.db(), project_id, speaker.as_str(), &session_key, 10)
        .into_iter()
        .map(|record| record.block)
        .collect()
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
