use crate::events::Agent;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SessionKey {
    pub project_id: i64,
    pub agent: Agent,
}
