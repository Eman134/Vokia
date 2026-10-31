use serde::{Deserialize, Serialize};

pub const EVENT_NAME: &str = "vokia://event";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Agent {
    Iara,
    Davi,
}

impl Agent {
    pub const ALL: [Agent; 2] = [Agent::Iara, Agent::Davi];

    pub fn name(self) -> &'static str {
        match self {
            Agent::Iara => "Iara",
            Agent::Davi => "Davi",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Speaker {
    User,
    Iara,
    Davi,
}

impl Speaker {
    pub const ALL: [Speaker; 3] = [Speaker::User, Speaker::Iara, Speaker::Davi];

    pub fn as_str(self) -> &'static str {
        match self {
            Speaker::User => "user",
            Speaker::Iara => "iara",
            Speaker::Davi => "davi",
        }
    }

    pub fn parse(value: &str) -> Option<Speaker> {
        Speaker::ALL
            .into_iter()
            .find(|speaker| speaker.as_str() == value)
    }
}

impl From<Agent> for Speaker {
    fn from(agent: Agent) -> Self {
        match agent {
            Agent::Iara => Speaker::Iara,
            Agent::Davi => Speaker::Davi,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentState {
    Thinking,
    Waiting,
    Idle,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageWindow {
    pub percent: u8,
    pub resets_at: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum VokiaEvent {
    Message {
        from: Speaker,
        to: Speaker,
        text: String,
        time: u64,
    },
    AgentState {
        agent: Agent,
        state: AgentState,
    },
    Error {
        message: String,
    },
    #[serde(rename_all = "camelCase")]
    PlanUsage {
        five_hour: UsageWindow,
        seven_day: UsageWindow,
    },
}
