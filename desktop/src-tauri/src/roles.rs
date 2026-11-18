use std::fmt::{self, Display};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

use serde::{Deserialize, Serialize};

use crate::agents::{
    join_items, name_key, write_atomic, AgentProfile, AgentRegistry, Team, ROLE_MAX_CHARS,
    ROLE_MIN_CHARS,
};

const ROLES_FILE: &str = "roles.json";
const ROLES_FILE_TMP: &str = "roles.json.tmp";
const ROLES_FILE_VERSION: u32 = 1;
const DESCRIPTION_MAX_CHARS: usize = 500;
const INSTRUCTIONS_MAX_CHARS: usize = 4000;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Role {
    pub id: String,
    pub name: String,
    pub description: String,
    pub instructions: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleDraft {
    pub name: String,
    pub description: String,
    pub instructions: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleEdit {
    pub description: String,
    pub instructions: String,
}

#[derive(Serialize, Deserialize)]
struct RolesFile {
    version: u32,
    roles: Vec<Role>,
}

#[derive(Debug)]
pub enum RoleError {
    InvalidName,
    NameInUse(String),
    DescriptionTooLong,
    InstructionsTooLong,
    NotFound(String),
    UnknownRole(String),
    InUse { role: String, agents: Vec<String> },
    LoadFailed(String),
    SaveFailed(String),
}

impl Display for RoleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RoleError::InvalidName => write!(
                f,
                "Nome de papel inválido: use de {ROLE_MIN_CHARS} a {ROLE_MAX_CHARS} caracteres, sem quebra de linha."
            ),
            RoleError::NameInUse(name) => write!(f, "Já existe um papel chamado {name}."),
            RoleError::DescriptionTooLong => {
                write!(f, "A descrição do papel passa de {DESCRIPTION_MAX_CHARS} caracteres.")
            }
            RoleError::InstructionsTooLong => {
                write!(f, "As instruções do papel passam de {INSTRUCTIONS_MAX_CHARS} caracteres.")
            }
            RoleError::NotFound(id) => write!(f, "O papel {id} não existe."),
            RoleError::UnknownRole(name) => {
                write!(f, "O papel {name} não existe. Crie o papel antes de usá-lo.")
            }
            RoleError::InUse { role, agents } => write!(
                f,
                "O papel {role} está em uso por {}; troque o papel desses agentes antes de excluir.",
                join_items(agents)
            ),
            RoleError::LoadFailed(message) | RoleError::SaveFailed(message) => f.write_str(message),
        }
    }
}

pub struct RoleRegistry {
    file: PathBuf,
    state: Mutex<Result<Vec<Role>, String>>,
}

impl RoleRegistry {
    pub fn load(dir: &Path, agents: &AgentRegistry) -> RoleRegistry {
        let file = dir.join(ROLES_FILE);
        let state = read_or_seed(&file, agents).map_err(|error| error.to_string());
        RoleRegistry {
            file,
            state: Mutex::new(state),
        }
    }

    pub fn list(&self) -> Result<Vec<Role>, RoleError> {
        Ok(loaded(&self.lock())?.clone())
    }

    pub fn ensure_exists(&self, name: &str) -> Result<(), RoleError> {
        let name = name.trim();
        if loaded(&self.lock())?.iter().any(|role| role.name == name) {
            return Ok(());
        }
        Err(RoleError::UnknownRole(name.to_string()))
    }

    pub fn create(&self, draft: &RoleDraft) -> Result<Role, RoleError> {
        let mut state = self.lock();
        let roles = loaded_mut(&mut state)?;
        let name = draft.name.trim();
        let id = role_id(name);
        if !is_valid_name(name) || id.is_empty() {
            return Err(RoleError::InvalidName);
        }
        let key = name_key(name);
        if roles
            .iter()
            .any(|role| role.id == id || name_key(&role.name) == key)
        {
            return Err(RoleError::NameInUse(name.to_string()));
        }
        let (description, instructions) = validated_texts(&draft.description, &draft.instructions)?;
        let role = Role {
            id,
            name: name.to_string(),
            description,
            instructions,
        };
        let mut next = roles.clone();
        next.push(role.clone());
        write_roles(&self.file, &next).map_err(RoleError::SaveFailed)?;
        *roles = next;
        Ok(role)
    }

    pub fn update(&self, id: &str, edit: &RoleEdit) -> Result<Role, RoleError> {
        let mut state = self.lock();
        let roles = loaded_mut(&mut state)?;
        let index = roles
            .iter()
            .position(|role| role.id == id)
            .ok_or_else(|| RoleError::NotFound(id.to_string()))?;
        let (description, instructions) = validated_texts(&edit.description, &edit.instructions)?;
        let mut next = roles.clone();
        next[index].description = description;
        next[index].instructions = instructions;
        let updated = next[index].clone();
        write_roles(&self.file, &next).map_err(RoleError::SaveFailed)?;
        *roles = next;
        Ok(updated)
    }

    // O team é lido antes do lock de papéis: o cadastro de agentes não depende do de papéis.
    pub fn delete(&self, id: &str, team: &Team) -> Result<(), RoleError> {
        let mut state = self.lock();
        let roles = loaded_mut(&mut state)?;
        let role = roles
            .iter()
            .find(|role| role.id == id)
            .ok_or_else(|| RoleError::NotFound(id.to_string()))?;
        let users: Vec<String> = team
            .profiles()
            .iter()
            .filter(|profile| profile.role == role.name)
            .map(|profile| profile.name.clone())
            .collect();
        if !users.is_empty() {
            return Err(RoleError::InUse {
                role: role.name.clone(),
                agents: users,
            });
        }
        let next: Vec<Role> = roles.iter().filter(|role| role.id != id).cloned().collect();
        write_roles(&self.file, &next).map_err(RoleError::SaveFailed)?;
        *roles = next;
        Ok(())
    }

    fn lock(&self) -> MutexGuard<'_, Result<Vec<Role>, String>> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn loaded(state: &Result<Vec<Role>, String>) -> Result<&Vec<Role>, RoleError> {
    match state {
        Ok(roles) => Ok(roles),
        Err(message) => Err(RoleError::LoadFailed(message.clone())),
    }
}

fn loaded_mut<'a>(
    state: &'a mut MutexGuard<'_, Result<Vec<Role>, String>>,
) -> Result<&'a mut Vec<Role>, RoleError> {
    match &mut **state {
        Ok(roles) => Ok(roles),
        Err(message) => Err(RoleError::LoadFailed(message.clone())),
    }
}

fn is_valid_name(name: &str) -> bool {
    let chars = name.chars().count();
    (ROLE_MIN_CHARS..=ROLE_MAX_CHARS).contains(&chars) && !name.contains(['\n', '\r'])
}

// Identificador estável derivado do nome: sem acento, minúsculas, separado por hífen.
fn role_id(name: &str) -> String {
    let mut id = String::new();
    for c in name_key(name).chars() {
        if c.is_ascii_alphanumeric() {
            id.push(c);
        } else if !id.ends_with('-') {
            id.push('-');
        }
    }
    id.trim_matches('-').to_string()
}

fn validated_texts(description: &str, instructions: &str) -> Result<(String, String), RoleError> {
    let description = description.trim();
    if description.chars().count() > DESCRIPTION_MAX_CHARS {
        return Err(RoleError::DescriptionTooLong);
    }
    let instructions = instructions.trim();
    if instructions.chars().count() > INSTRUCTIONS_MAX_CHARS {
        return Err(RoleError::InstructionsTooLong);
    }
    Ok((description.to_string(), instructions.to_string()))
}

// Um papel por valor distinto de AgentProfile.role: o cadastro começa com os papéis já em uso.
fn seed_roles(profiles: &[AgentProfile]) -> Vec<Role> {
    let mut names: Vec<&str> = Vec::new();
    for profile in profiles {
        if !names.contains(&profile.role.as_str()) {
            names.push(profile.role.as_str());
        }
    }
    names
        .into_iter()
        .map(|name| Role {
            id: role_id(name),
            name: name.to_string(),
            description: String::new(),
            instructions: String::new(),
        })
        .collect()
}

fn read_or_seed(file: &Path, agents: &AgentRegistry) -> Result<Vec<Role>, RoleError> {
    if !file.exists() {
        let team = agents
            .team()
            .map_err(|error| RoleError::LoadFailed(error.to_string()))?;
        let seeds = seed_roles(team.profiles());
        write_roles(file, &seeds).map_err(RoleError::SaveFailed)?;
        return Ok(seeds);
    }
    let text = fs::read_to_string(file).map_err(|error| load_failed(file, error))?;
    let parsed: RolesFile =
        serde_json::from_str(&text).map_err(|error| load_failed(file, error))?;
    if parsed.version != ROLES_FILE_VERSION {
        return Err(load_failed(
            file,
            format!("versão {} desconhecida", parsed.version),
        ));
    }
    Ok(parsed.roles)
}

fn load_failed(file: &Path, detail: impl Display) -> RoleError {
    RoleError::LoadFailed(format!(
        "Não foi possível carregar os papéis de {}: {detail}.",
        file.display()
    ))
}

fn write_roles(file: &Path, roles: &[Role]) -> Result<(), String> {
    let text = serde_json::to_string_pretty(&RolesFile {
        version: ROLES_FILE_VERSION,
        roles: roles.to_vec(),
    })
    .map_err(|error| error.to_string())?;
    write_atomic(file, ROLES_FILE_TMP, &text)
}
