use serde::{Deserialize, Serialize};

// Estilo de execução do projeto: define quanto do que os agentes pedem passa sem perguntar ao usuário.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExecutionMode {
    #[default]
    Manual,
    #[serde(rename = "planejamento")]
    Planning,
    Auto,
    Bypass,
}
