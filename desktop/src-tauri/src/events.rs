use serde::{Serialize, Serializer};

use crate::orchestrator::RequestOutcome;

pub use crate::agents::Agent;

pub const EVENT_NAME: &str = "vokia://event";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Speaker {
    User,
    Agent(Agent),
}

impl Speaker {
    pub fn as_str(&self) -> &str {
        match self {
            Speaker::User => "user",
            Speaker::Agent(agent) => agent.as_str(),
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
}

impl DecisionKind {
    const ALL: [DecisionKind; 4] = [
        DecisionKind::Aprovacao,
        DecisionKind::Escolha,
        DecisionKind::Pergunta,
        DecisionKind::Revisao,
    ];

    fn as_str(self) -> &'static str {
        match self {
            DecisionKind::Aprovacao => "aprovacao",
            DecisionKind::Escolha => "escolha",
            DecisionKind::Pergunta => "pergunta",
            DecisionKind::Revisao => "revisao",
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PermissionOutcome {
    Allowed,
    Denied,
    Expired,
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
    Permission {
        id: String,
        agent: Agent,
        tool: String,
        detail: String,
        project_id: Option<i64>,
        expires_at: u64,
        approved_before: u32,
    },
    #[serde(rename_all = "camelCase")]
    PermissionClosed {
        id: String,
        project_id: Option<i64>,
        outcome: PermissionOutcome,
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
}
