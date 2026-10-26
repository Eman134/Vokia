use serde::Serialize;

pub const EVENT_NAME: &str = "vokia://event";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Agent {
    Iara,
    Davi,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Speaker {
    User,
    Iara,
    Davi,
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
}
