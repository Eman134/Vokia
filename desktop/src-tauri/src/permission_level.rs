use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PermissionLevel {
    Liberado,
    Edicao,
    Leitura,
}

impl PermissionLevel {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "liberado" => Some(PermissionLevel::Liberado),
            "edicao" => Some(PermissionLevel::Edicao),
            "leitura" => Some(PermissionLevel::Leitura),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            PermissionLevel::Liberado => "liberado",
            PermissionLevel::Edicao => "edicao",
            PermissionLevel::Leitura => "leitura",
        }
    }
}
