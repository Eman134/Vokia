use serde::{Deserialize, Serialize, Serializer};

use crate::claude::{ConsoleKind, LimitName, TurnActivity, TurnUsage};
use crate::orchestrator::fallback::TemporaryOverride;
use crate::orchestrator::RequestOutcome;

pub use crate::agents::Agent;

pub const EVENT_NAME: &str = "vokia://event";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Speaker {
    User,
    Agent(Agent),
    /// Id bruto de um remetente que não existe mais nesta instância (ex.: agente de outra instância excluída).
    Foreign(String),
}

impl Speaker {
    pub fn as_str(&self) -> &str {
        match self {
            Speaker::User => "user",
            Speaker::Agent(agent) => agent.as_str(),
            Speaker::Foreign(raw) => raw.as_str(),
        }
    }

    pub fn parse(value: &str) -> Option<Speaker> {
        if value == "user" {
            return Some(Speaker::User);
        }
        Agent::from_stored(value).map(Speaker::Agent)
    }
}

impl Serialize for Speaker {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl From<Agent> for Speaker {
    fn from(agent: Agent) -> Self {
        Speaker::Agent(agent)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentState {
    Sending,
    Thinking,
    Waiting,
    Idle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DecisionKind {
    Aprovacao,
    Escolha,
    Pergunta,
    Revisao,
    // Só o app abre (validação final do card): fora de ALL, para o agente não poder pedi-la.
    Validacao,
}

impl DecisionKind {
    pub const ALL: [DecisionKind; 4] = [
        DecisionKind::Aprovacao,
        DecisionKind::Escolha,
        DecisionKind::Pergunta,
        DecisionKind::Revisao,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            DecisionKind::Aprovacao => "aprovacao",
            DecisionKind::Escolha => "escolha",
            DecisionKind::Pergunta => "pergunta",
            DecisionKind::Revisao => "revisao",
            DecisionKind::Validacao => "validacao",
        }
    }

    pub fn parse(value: &str) -> Option<DecisionKind> {
        DecisionKind::ALL
            .into_iter()
            .find(|kind| kind.as_str().eq_ignore_ascii_case(value))
    }
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageWindow {
    pub percent: u8,
    pub resets_at: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestProposal {
    pub title: String,
    pub detail: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PermissionOutcome {
    Allowed,
    Denied,
    Expired,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GoalState {
    Atingido,
    Incompleto,
    SemAvanco,
    Repetindo,
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum VokiaEvent {
    #[serde(rename_all = "camelCase")]
    Message {
        from: Speaker,
        to: Speaker,
        text: String,
        time: u64,
        project_id: Option<i64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        channel_id: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        item_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        activity: Option<TurnActivity>,
        #[serde(skip_serializing_if = "Option::is_none")]
        request_proposal: Option<RequestProposal>,
        #[serde(skip_serializing_if = "Option::is_none")]
        usage: Option<TurnUsage>,
        #[serde(default, skip_serializing_if = "is_false")]
        is_error: bool,
        /// true quando algum lado da mensagem vem de uma instância excluída (o id bruto fica em `from`/`to`).
        #[serde(default, skip_serializing_if = "is_false")]
        foreign_instance: bool,
    },
    #[serde(rename_all = "camelCase")]
    AgentState {
        agent: Agent,
        state: AgentState,
        project_id: Option<i64>,
    },
    Error {
        message: String,
    },
    #[serde(rename_all = "camelCase")]
    UsageLimit {
        agent: Agent,
        project_id: i64,
        window: LimitName,
        resets_at: u64,
        run_id: Option<i64>,
    },
    #[serde(rename_all = "camelCase")]
    Permission {
        id: String,
        agent: Agent,
        tool: String,
        detail: String,
        project_id: Option<i64>,
        expires_at: u64,
        approved_before: u32,
        suggest_always: bool,
        root_command: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    PermissionClosed {
        id: String,
        project_id: Option<i64>,
        outcome: PermissionOutcome,
    },
    #[serde(rename_all = "camelCase")]
    PermissionByRule {
        agent: Agent,
        detail: String,
        project_id: Option<i64>,
    },
    #[serde(rename_all = "camelCase")]
    Decision {
        id: u64,
        agent: Agent,
        kind: DecisionKind,
        title: String,
        detail: String,
        options: Vec<String>,
        project_id: Option<i64>,
        item_id: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    PlanUsage {
        five_hour: UsageWindow,
        seven_day: UsageWindow,
    },
    #[serde(rename_all = "camelCase")]
    RequestSettled {
        run_id: u64,
        project_id: i64,
        outcome: RequestOutcome,
    },
    #[serde(rename_all = "camelCase")]
    ConsoleLine {
        agent: Agent,
        project_id: i64,
        seq: u64,
        kind: ConsoleKind,
        text: String,
        time: u64,
    },
    // Visual e não gravado: a atividade final segue na mensagem.
    #[serde(rename_all = "camelCase")]
    AgentActivity {
        agent: Agent,
        project_id: i64,
        activity: TurnActivity,
    },
    #[serde(rename_all = "camelCase")]
    GoalState {
        project_id: i64,
        state: Option<GoalState>,
        objective: String,
        reason: String,
        checks: u32,
    },
    #[serde(rename_all = "camelCase")]
    TaskResult {
        project_id: i64,
        objective: String,
        concluded: bool,
        turns: i64,
        agents: Vec<Agent>,
        time: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        channel_id: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        item_id: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    BacklogChanged {
        project_id: i64,
    },
    #[serde(rename_all = "camelCase")]
    ArtifactsChanged {
        project_id: i64,
    },
    OrchestrationChanged,
    #[serde(rename_all = "camelCase")]
    BatchProgress {
        project_id: i64,
        leader: Agent,
        returned: usize,
        total: usize,
        running: Vec<Agent>,
        done: bool,
    },
    #[serde(rename_all = "camelCase")]
    AgentNamed {
        project_id: i64,
        agent: Agent,
        name: String,
    },
    #[serde(rename_all = "camelCase")]
    TemporaryOverridesChanged {
        overrides: Vec<TemporaryOverride>,
    },
    #[serde(rename_all = "camelCase")]
    ModelUnavailable {
        agent: Agent,
        provider: String,
        model: String,
        detail: String,
    },
}
