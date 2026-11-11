use std::fmt::{self, Display};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

use serde::{Deserialize, Serialize};

use crate::claude::DEFAULT_MODEL;

const NAME_MIN_CHARS: usize = 2;
const NAME_MAX_CHARS: usize = 20;
const ROLE_MIN_CHARS: usize = 2;
const ROLE_MAX_CHARS: usize = 40;
const MAX_AGENTS: usize = 12;
const RESERVED_IDS: [&str; 4] = ["todos", "time", "user", "voce"];
const AGENTS_FILE: &str = "agents.json";
const AGENTS_FILE_TMP: &str = "agents.json.tmp";
const FILE_VERSION: u32 = 1;
const AVATAR_MAX_CHARS: usize = 32;
const CLAUDE_CLI: &str = "claude";
const CLAUDE_CLI_LABEL: &str = "Claude Code";
const CLAUDE_MODELS: [&str; 3] = ["haiku", "sonnet", "opus"];
const IARA: &str = "iara";
const DAVI: &str = "davi";

const INITIAL_AGENTS: [(&str, &str, &str); 3] = [
    ("Nina", "Dev backend", "vesper"),
    ("Téo", "QA e revisão", "selene"),
    ("Rui", "Pesquisador", "ambar-ouro"),
];

const ACCENTS: &[(char, char)] = &[
    ('á', 'a'),
    ('à', 'a'),
    ('â', 'a'),
    ('ã', 'a'),
    ('ä', 'a'),
    ('é', 'e'),
    ('è', 'e'),
    ('ê', 'e'),
    ('ë', 'e'),
    ('í', 'i'),
    ('ì', 'i'),
    ('î', 'i'),
    ('ï', 'i'),
    ('ó', 'o'),
    ('ò', 'o'),
    ('ô', 'o'),
    ('õ', 'o'),
    ('ö', 'o'),
    ('ú', 'u'),
    ('ù', 'u'),
    ('û', 'u'),
    ('ü', 'u'),
    ('ç', 'c'),
    ('ñ', 'n'),
];

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Agent(String);

impl Agent {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn iara() -> Agent {
        Agent(IARA.to_string())
    }

    pub fn davi() -> Agent {
        Agent(DAVI.to_string())
    }

    pub(crate) fn from_stored(id: &str) -> Option<Agent> {
        is_agent_id(id).then(|| Agent(id.to_string()))
    }
}

impl Display for Agent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentProfile {
    pub id: Agent,
    pub name: String,
    pub role: String,
    pub avatar: String,
    pub cli: String,
    pub model: String,
    #[serde(default)]
    pub builtin: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentDraft {
    pub name: String,
    pub role: String,
    pub avatar: String,
    pub cli: String,
    pub model: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliPreset {
    pub id: &'static str,
    pub label: &'static str,
    pub models: &'static [&'static str],
    pub default_model: &'static str,
}

pub fn presets() -> Vec<CliPreset> {
    vec![CliPreset {
        id: CLAUDE_CLI,
        label: CLAUDE_CLI_LABEL,
        models: &CLAUDE_MODELS,
        default_model: DEFAULT_MODEL,
    }]
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamListing {
    pub agents: Vec<AgentProfile>,
    pub presets: Vec<CliPreset>,
}

#[derive(Debug)]
pub enum AgentError {
    LimitReached,
    InvalidName,
    ReservedName(String),
    NameInUse(String),
    InvalidRole,
    InvalidAvatar(String),
    UnknownCli(String),
    UnknownModel { cli: String, model: String },
    UnknownAgent(String),
    BuiltinLocked(String),
    LoadFailed(String),
    SaveFailed(String),
}

impl Display for AgentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentError::LimitReached => {
                write!(f, "O time já tem o máximo de {MAX_AGENTS} agentes.")
            }
            AgentError::InvalidName => write!(
                f,
                "Nome inválido: use de {NAME_MIN_CHARS} a {NAME_MAX_CHARS} letras ou dígitos, começando por letra, sem espaço, @ ou :."
            ),
            AgentError::ReservedName(name) => write!(f, "O nome {name} é reservado pelo app."),
            AgentError::NameInUse(name) => write!(f, "Já existe um agente chamado {name}."),
            AgentError::InvalidRole => write!(
                f,
                "Papel inválido: use de {ROLE_MIN_CHARS} a {ROLE_MAX_CHARS} caracteres, sem quebra de linha."
            ),
            AgentError::InvalidAvatar(id) => write!(f, "Avatar inválido: {id}."),
            AgentError::UnknownCli(cli) => write!(f, "CLI desconhecida: {cli}."),
            AgentError::UnknownModel { cli, model } => {
                write!(f, "O modelo {model} não existe para {cli}.")
            }
            AgentError::UnknownAgent(id) => write!(f, "Agente desconhecido: {id}."),
            AgentError::BuiltinLocked(name) => {
                write!(f, "{name} é um agente embutido e não pode ser alterado.")
            }
            AgentError::LoadFailed(message) | AgentError::SaveFailed(message) => {
                f.write_str(message)
            }
        }
    }
}

fn builtin(id: Agent, name: &str, role: &str, avatar: &str) -> AgentProfile {
    AgentProfile {
        id,
        name: name.to_string(),
        role: role.to_string(),
        avatar: avatar.to_string(),
        cli: CLAUDE_CLI.to_string(),
        model: DEFAULT_MODEL.to_string(),
        builtin: true,
    }
}

fn builtins() -> Vec<AgentProfile> {
    vec![
        builtin(Agent::iara(), "Iara", "Tech lead", "quarzo"),
        builtin(Agent::davi(), "Davi", "Dev frontend", "lume"),
    ]
}

fn initial_agents() -> Vec<AgentProfile> {
    INITIAL_AGENTS
        .iter()
        .map(|(name, role, avatar)| AgentProfile {
            id: Agent(name_key(name)),
            name: name.to_string(),
            role: role.to_string(),
            avatar: avatar.to_string(),
            cli: CLAUDE_CLI.to_string(),
            model: DEFAULT_MODEL.to_string(),
            builtin: false,
        })
        .collect()
}

fn lowercase(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

fn fold_accent(c: char) -> char {
    ACCENTS
        .iter()
        .find(|(accented, _)| *accented == c)
        .map_or(c, |(_, base)| *base)
}

fn is_name_letter(c: char) -> bool {
    let lower = lowercase(c);
    lower.is_ascii_alphabetic() || ACCENTS.iter().any(|(accented, _)| *accented == lower)
}

fn is_name_char(c: char) -> bool {
    is_name_letter(c) || c.is_ascii_digit()
}

fn is_agent_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
}

fn is_avatar_id(avatar: &str) -> bool {
    let count = avatar.chars().count();
    (1..=AVATAR_MAX_CHARS).contains(&count)
        && avatar
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !avatar.starts_with('-')
        && !avatar.ends_with('-')
        && !avatar.contains("--")
}

pub fn name_key(name: &str) -> String {
    name.chars()
        .flat_map(char::to_lowercase)
        .map(fold_accent)
        .collect()
}

fn validate(
    draft: &AgentDraft,
    team: &[AgentProfile],
    editing: Option<&Agent>,
) -> Result<AgentProfile, AgentError> {
    if editing.is_none() && team.len() >= MAX_AGENTS {
        return Err(AgentError::LimitReached);
    }

    let name = draft.name.trim();
    let name_chars = name.chars().count();
    let name_is_valid = (NAME_MIN_CHARS..=NAME_MAX_CHARS).contains(&name_chars)
        && name.chars().next().is_some_and(is_name_letter)
        && name.chars().all(is_name_char);
    if !name_is_valid {
        return Err(AgentError::InvalidName);
    }

    let key = name_key(name);
    if RESERVED_IDS.contains(&key.as_str()) {
        return Err(AgentError::ReservedName(name.to_string()));
    }
    let clash = team.iter().find(|profile| {
        Some(&profile.id) != editing
            && (profile.id.as_str() == key || name_key(&profile.name) == key)
    });
    if let Some(profile) = clash {
        return Err(AgentError::NameInUse(profile.name.clone()));
    }

    let role = draft.role.trim();
    let role_chars = role.chars().count();
    let role_is_valid =
        (ROLE_MIN_CHARS..=ROLE_MAX_CHARS).contains(&role_chars) && !role.contains(['\n', '\r']);
    if !role_is_valid {
        return Err(AgentError::InvalidRole);
    }

    let avatar = draft.avatar.trim();
    if !is_avatar_id(avatar) {
        return Err(AgentError::InvalidAvatar(avatar.to_string()));
    }

    let preset = presets()
        .into_iter()
        .find(|preset| preset.id == draft.cli)
        .ok_or_else(|| AgentError::UnknownCli(draft.cli.clone()))?;
    if !preset.models.contains(&draft.model.as_str()) {
        return Err(AgentError::UnknownModel {
            cli: draft.cli.clone(),
            model: draft.model.clone(),
        });
    }

    Ok(AgentProfile {
        id: editing.cloned().unwrap_or(Agent(key)),
        name: name.to_string(),
        role: role.to_string(),
        avatar: avatar.to_string(),
        cli: draft.cli.clone(),
        model: draft.model.clone(),
        builtin: false,
    })
}

fn team_of(stored: &[AgentProfile]) -> Vec<AgentProfile> {
    builtins()
        .into_iter()
        .chain(stored.iter().cloned())
        .collect()
}

pub struct Team(Vec<AgentProfile>);

impl Team {
    pub fn profiles(&self) -> &[AgentProfile] {
        &self.0
    }

    pub fn ids(&self) -> impl Iterator<Item = &Agent> {
        self.0.iter().map(|profile| &profile.id)
    }

    pub fn profile(&self, id: &Agent) -> Option<&AgentProfile> {
        self.0.iter().find(|profile| &profile.id == id)
    }

    pub fn name_of<'a>(&'a self, id: &'a Agent) -> &'a str {
        self.profile(id)
            .map_or_else(|| id.as_str(), |profile| profile.name.as_str())
    }

    pub fn named(&self, name: &str) -> Option<Agent> {
        let key = name_key(name.trim());
        self.0
            .iter()
            .find(|profile| name_key(&profile.name) == key)
            .map(|profile| profile.id.clone())
    }

    pub fn roster_line(&self) -> String {
        let items: Vec<String> = self
            .0
            .iter()
            .map(|profile| format!("{} ({})", profile.name, profile.role))
            .collect();
        let list = match items.split_last() {
            None => String::new(),
            Some((last, [])) => last.clone(),
            Some((last, rest)) => format!("{} e {last}", rest.join(", ")),
        };
        format!("Time desta sessão: {list}.")
    }
}

#[derive(Serialize, Deserialize)]
struct AgentsFile {
    version: u32,
    agents: Vec<AgentProfile>,
}

pub struct AgentRegistry {
    file: PathBuf,
    state: Mutex<Result<Vec<AgentProfile>, String>>,
}

impl AgentRegistry {
    pub fn load(dir: &Path) -> AgentRegistry {
        let file = dir.join(AGENTS_FILE);
        let state = read_or_seed(dir, &file).map_err(|error| error.to_string());
        AgentRegistry {
            file,
            state: Mutex::new(state),
        }
    }

    pub fn team(&self) -> Result<Team, AgentError> {
        match &*self.lock() {
            Ok(stored) => Ok(Team(team_of(stored))),
            Err(message) => Err(AgentError::LoadFailed(message.clone())),
        }
    }

    pub fn listing(&self) -> Result<TeamListing, AgentError> {
        let team = self.team()?;
        Ok(TeamListing {
            agents: team.profiles().to_vec(),
            presets: presets(),
        })
    }

    pub fn create(&self, draft: &AgentDraft) -> Result<AgentProfile, AgentError> {
        let mut state = self.lock();
        let stored = stored_mut(&mut state)?;
        let profile = validate(draft, &team_of(stored), None)?;
        let mut next = stored.clone();
        next.push(profile.clone());
        write_file(&self.file, next.clone()).map_err(AgentError::SaveFailed)?;
        *stored = next;
        Ok(profile)
    }

    pub fn update(&self, id: &Agent, draft: &AgentDraft) -> Result<AgentProfile, AgentError> {
        if let Some(builtin) = builtins().into_iter().find(|profile| &profile.id == id) {
            return Err(AgentError::BuiltinLocked(builtin.name));
        }
        let mut state = self.lock();
        let stored = stored_mut(&mut state)?;
        let index = stored
            .iter()
            .position(|profile| &profile.id == id)
            .ok_or_else(|| AgentError::UnknownAgent(id.to_string()))?;
        let profile = validate(draft, &team_of(stored), Some(id))?;
        let mut next = stored.clone();
        next[index] = profile.clone();
        write_file(&self.file, next.clone()).map_err(AgentError::SaveFailed)?;
        *stored = next;
        Ok(profile)
    }

    fn lock(&self) -> MutexGuard<'_, Result<Vec<AgentProfile>, String>> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn stored_mut<'a>(
    state: &'a mut MutexGuard<'_, Result<Vec<AgentProfile>, String>>,
) -> Result<&'a mut Vec<AgentProfile>, AgentError> {
    match &mut **state {
        Ok(stored) => Ok(stored),
        Err(message) => Err(AgentError::LoadFailed(message.clone())),
    }
}

fn load_failed(file: &Path, detail: impl Display) -> AgentError {
    AgentError::LoadFailed(format!(
        "Não foi possível carregar o time de {}: {detail}.",
        file.display()
    ))
}

fn read_or_seed(dir: &Path, file: &Path) -> Result<Vec<AgentProfile>, AgentError> {
    if !file.exists() {
        let seeds = initial_agents();
        fs::create_dir_all(dir).map_err(|error| load_failed(file, error))?;
        write_file(file, seeds.clone()).map_err(|detail| load_failed(file, detail))?;
        return Ok(seeds);
    }
    let text = fs::read_to_string(file).map_err(|error| load_failed(file, error))?;
    let parsed: AgentsFile =
        serde_json::from_str(&text).map_err(|error| load_failed(file, error))?;
    if parsed.version != FILE_VERSION {
        return Err(load_failed(
            file,
            format!("versão {} desconhecida", parsed.version),
        ));
    }
    let mut loaded: Vec<AgentProfile> = Vec::new();
    for profile in parsed.agents {
        let draft = AgentDraft {
            name: profile.name.clone(),
            role: profile.role.clone(),
            avatar: profile.avatar.clone(),
            cli: profile.cli.clone(),
            model: profile.model.clone(),
        };
        let team = team_of(&loaded);
        let validated = validate(&draft, &team, None).map_err(|error| load_failed(file, error))?;
        if validated.id != profile.id {
            return Err(load_failed(
                file,
                format!("o id {} não bate com o nome {}", profile.id, profile.name),
            ));
        }
        loaded.push(validated);
    }
    Ok(loaded)
}

fn write_file(file: &Path, agents: Vec<AgentProfile>) -> Result<(), String> {
    let text = serde_json::to_string_pretty(&AgentsFile {
        version: FILE_VERSION,
        agents,
    })
    .map_err(|error| error.to_string())?;
    let tmp = file.with_file_name(AGENTS_FILE_TMP);
    fs::write(&tmp, text).map_err(|error| error.to_string())?;
    fs::rename(&tmp, file).map_err(|error| error.to_string())
}
