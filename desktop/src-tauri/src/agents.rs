use std::collections::BTreeMap;
use std::fmt::{self, Display};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

use serde::{Deserialize, Serialize};

use crate::claude::DEFAULT_MODEL;
use crate::db::DbError;

const NAME_MIN_CHARS: usize = 2;
const NAME_MAX_CHARS: usize = 20;
pub(crate) const ROLE_MIN_CHARS: usize = 2;
pub(crate) const ROLE_MAX_CHARS: usize = 40;
const MAX_AGENTS: usize = 12;
// Ids antigos ficam reservados: a rotina de renomeação dos dados roda a cada início e não pode alcançar um agente novo.
const RESERVED_IDS: [&str; 8] = ["todos", "time", "user", "voce", "davi", "nina", "teo", "rui"];
const AGENTS_FILE: &str = "agents.json";
const AGENTS_FILE_TMP: &str = "agents.json.tmp";
const FILE_VERSION: u32 = 1;
const AVATAR_MAX_CHARS: usize = 32;
const INSTRUCTIONS_MAX_CHARS: usize = 4000;
const CLAUDE_CLI: &str = "claude";
const CLAUDE_CLI_LABEL: &str = "Claude Code";
const CLAUDE_MODELS: [&str; 3] = ["haiku", "sonnet", "opus"];
const EMBER: &str = "ember";
const AXIOM: &str = "axiom";
const VEXA: &str = "vexa";

const INITIAL_AGENTS: [(&str, &str, &str); 3] = [
    ("Kairo", "Backend Developer", "vesper"),
    ("Sentry", "QA Engineer", "selene"),
    ("Oracle", "Researcher", "ambar-ouro"),
];

// Id antigo → id novo, nos dados gravados. A mesma tabela alimenta a rotina de migração do banco.
pub(crate) const AGENT_ID_RENAMES: [(&str, &str); 5] = [
    ("iara", EMBER),
    ("davi", VEXA),
    ("nina", "kairo"),
    ("teo", "sentry"),
    ("rui", "oracle"),
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

    pub fn ember() -> Agent {
        Agent(EMBER.to_string())
    }

    pub fn axiom() -> Agent {
        Agent(AXIOM.to_string())
    }

    pub fn vexa() -> Agent {
        Agent(VEXA.to_string())
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
    // Vem do mapa de instruções do registro, não do registro de cada agente; vazio quando não há.
    #[serde(default)]
    pub extra_instructions: String,
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
    AvatarInUse(String),
    UnknownCli(String),
    UnknownModel { cli: String, model: String },
    UnknownAgent(String),
    BuiltinLocked(String),
    LoadFailed(String),
    SaveFailed(String),
    InstanceSuffixReserved(String),
    PositionHeld(String),
    InstructionsTooLong,
}

impl From<DbError> for AgentError {
    fn from(error: DbError) -> Self {
        match error {
            DbError::AgentHasPosition(message) => AgentError::PositionHeld(message),
            other => AgentError::SaveFailed(other.to_string()),
        }
    }
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
            AgentError::AvatarInUse(name) => write!(f, "Esse avatar já é usado por {name}."),
            AgentError::UnknownCli(cli) => write!(f, "CLI desconhecida: {cli}."),
            AgentError::UnknownModel { cli, model } => {
                write!(f, "O modelo {model} não existe para {cli}.")
            }
            AgentError::UnknownAgent(id) => write!(f, "Agente desconhecido: {id}."),
            AgentError::BuiltinLocked(name) => {
                write!(f, "{name} é um agente embutido e não pode ser alterado.")
            }
            AgentError::InstanceSuffixReserved(id) => write!(
                f,
                "O id {id} termina em -<número>, sufixo reservado para as réplicas de agentes."
            ),
            AgentError::InstructionsTooLong => write!(
                f,
                "As instruções extras passam de {INSTRUCTIONS_MAX_CHARS} caracteres."
            ),
            AgentError::PositionHeld(message) | AgentError::LoadFailed(message) | AgentError::SaveFailed(message) => {
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
        extra_instructions: String::new(),
    }
}

fn builtins() -> Vec<AgentProfile> {
    vec![
        builtin(Agent::ember(), "Ember", "Orquestrador geral", "ember"),
        builtin(Agent::axiom(), "Axiom", "Tech Lead", "quarzo"),
        builtin(Agent::vexa(), "Vexa", "Frontend Developer", "lume"),
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
            extra_instructions: String::new(),
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

// Réplica é <base>-<n> com n ≥ 2; o id do usuário nunca usa essa forma (is_agent_id continua recusando o hífen).
pub(crate) fn replica_of(id: &str) -> Option<(&str, u32)> {
    let (base, number) = id.rsplit_once('-')?;
    let n: u32 = number.parse().ok()?;
    (n >= 2 && is_agent_id(base)).then_some((base, n))
}

pub(crate) fn replica_id(base: &Agent, n: u32) -> Agent {
    Agent(format!("{}-{n}", base.as_str()))
}

fn ends_with_number_suffix(id: &str) -> bool {
    id.rsplit_once('-')
        .is_some_and(|(_, digits)| !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()))
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
    if ends_with_number_suffix(&key) {
        return Err(AgentError::InstanceSuffixReserved(key));
    }
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
    let avatar_owner = team
        .iter()
        .find(|profile| Some(&profile.id) != editing && profile.avatar == avatar);
    if let Some(profile) = avatar_owner {
        return Err(AgentError::AvatarInUse(profile.name.clone()));
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
        extra_instructions: String::new(),
    })
}

fn team_of(stored: &[AgentProfile]) -> Vec<AgentProfile> {
    builtins()
        .into_iter()
        .chain(stored.iter().cloned())
        .collect()
}

fn with_instructions(profile: AgentProfile, instructions: &BTreeMap<String, String>) -> AgentProfile {
    let extra_instructions = instructions
        .get(profile.id.as_str())
        .cloned()
        .unwrap_or_default();
    AgentProfile {
        extra_instructions,
        ..profile
    }
}

pub struct Team(Vec<AgentProfile>);

impl Team {
    pub fn profiles(&self) -> &[AgentProfile] {
        &self.0
    }

    pub fn ids(&self) -> impl Iterator<Item = &Agent> {
        self.0.iter().map(|profile| &profile.id)
    }

    // Réplica (<base>-<n>) usa o perfil do agente base; o nome de exibição sai de instances::display_name.
    pub fn profile(&self, id: &Agent) -> Option<&AgentProfile> {
        self.0.iter().find(|profile| &profile.id == id).or_else(|| {
            let (base, _) = replica_of(id.as_str())?;
            self.0.iter().find(|profile| profile.id.as_str() == base)
        })
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
        roster_text(self.0.iter())
    }

    pub fn roster_line_among(&self, ids: &[Agent]) -> String {
        roster_text(self.0.iter().filter(|profile| ids.contains(&profile.id)))
    }
}

fn roster_text<'a>(profiles: impl Iterator<Item = &'a AgentProfile>) -> String {
    let items: Vec<String> = profiles
        .map(|profile| format!("{} ({})", profile.name, profile.role))
        .collect();
    format!("Time desta sessão: {}.", join_items(&items))
}

pub(crate) fn join_items(items: &[String]) -> String {
    match items.split_last() {
        None => String::new(),
        Some((last, [])) => last.clone(),
        Some((last, rest)) => format!("{} e {last}", rest.join(", ")),
    }
}

#[derive(Serialize, Deserialize)]
struct AgentsFile {
    version: u32,
    agents: Vec<AgentProfile>,
    #[serde(default)]
    instructions: BTreeMap<String, String>,
}

// Agentes gravados e instruções extras por id (inclusive dos embutidos, que não ficam em `agents`).
#[derive(Clone, Default)]
struct Stored {
    agents: Vec<AgentProfile>,
    instructions: BTreeMap<String, String>,
}

pub struct AgentRegistry {
    file: PathBuf,
    state: Mutex<Result<Stored, String>>,
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
            Ok(stored) => Ok(Team(
                team_of(&stored.agents)
                    .into_iter()
                    .map(|profile| with_instructions(profile, &stored.instructions))
                    .collect(),
            )),
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
        let profile = validate(draft, &team_of(&stored.agents), None)?;
        let mut next = stored.agents.clone();
        next.push(profile.clone());
        write_file(&self.file, &next, &stored.instructions).map_err(AgentError::SaveFailed)?;
        stored.agents = next;
        Ok(profile)
    }

    pub fn update(&self, id: &Agent, draft: &AgentDraft) -> Result<AgentProfile, AgentError> {
        if let Some(builtin) = builtins().into_iter().find(|profile| &profile.id == id) {
            return Err(AgentError::BuiltinLocked(builtin.name));
        }
        let mut state = self.lock();
        let stored = stored_mut(&mut state)?;
        let index = stored
            .agents
            .iter()
            .position(|profile| &profile.id == id)
            .ok_or_else(|| AgentError::UnknownAgent(id.to_string()))?;
        let profile = validate(draft, &team_of(&stored.agents), Some(id))?;
        let mut next = stored.agents.clone();
        next[index] = profile.clone();
        write_file(&self.file, &next, &stored.instructions).map_err(AgentError::SaveFailed)?;
        stored.agents = next;
        Ok(with_instructions(profile, &stored.instructions))
    }

    // Só tira o registro: posições e cards são tratados antes, na mesma operação do banco.
    pub fn remove(&self, id: &Agent) -> Result<(), AgentError> {
        if let Some(builtin) = builtins().into_iter().find(|profile| &profile.id == id) {
            return Err(AgentError::BuiltinLocked(builtin.name));
        }
        let mut state = self.lock();
        let stored = stored_mut(&mut state)?;
        let index = stored
            .agents
            .iter()
            .position(|profile| &profile.id == id)
            .ok_or_else(|| AgentError::UnknownAgent(id.to_string()))?;
        let mut agents = stored.agents.clone();
        agents.remove(index);
        let mut instructions = stored.instructions.clone();
        instructions.remove(id.as_str());
        write_file(&self.file, &agents, &instructions).map_err(AgentError::SaveFailed)?;
        stored.agents = agents;
        stored.instructions = instructions;
        Ok(())
    }

    // Texto vazio apaga as instruções; o embutido é aceito, porque o campo não altera a identidade do agente.
    pub fn set_instructions(&self, id: &Agent, text: &str) -> Result<AgentProfile, AgentError> {
        let text = text.trim();
        if text.chars().count() > INSTRUCTIONS_MAX_CHARS {
            return Err(AgentError::InstructionsTooLong);
        }
        let mut state = self.lock();
        let stored = stored_mut(&mut state)?;
        let profile = team_of(&stored.agents)
            .into_iter()
            .find(|profile| &profile.id == id)
            .ok_or_else(|| AgentError::UnknownAgent(id.to_string()))?;
        let mut instructions = stored.instructions.clone();
        if text.is_empty() {
            instructions.remove(id.as_str());
        } else {
            instructions.insert(id.to_string(), text.to_string());
        }
        write_file(&self.file, &stored.agents, &instructions).map_err(AgentError::SaveFailed)?;
        stored.instructions = instructions;
        Ok(with_instructions(profile, &stored.instructions))
    }

    fn lock(&self) -> MutexGuard<'_, Result<Stored, String>> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn stored_mut<'a>(
    state: &'a mut MutexGuard<'_, Result<Stored, String>>,
) -> Result<&'a mut Stored, AgentError> {
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

fn read_or_seed(dir: &Path, file: &Path) -> Result<Stored, AgentError> {
    if !file.exists() {
        let seeds = initial_agents();
        let instructions = BTreeMap::new();
        fs::create_dir_all(dir).map_err(|error| load_failed(file, error))?;
        write_file(file, &seeds, &instructions).map_err(|detail| load_failed(file, detail))?;
        return Ok(Stored {
            agents: seeds,
            instructions,
        });
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
    let mut rewrite = false;
    for stored in parsed.agents {
        let profile = renamed_stored(stored.clone());
        rewrite |= profile.id != stored.id;
        let mut draft = AgentDraft {
            name: profile.name.clone(),
            role: profile.role.clone(),
            avatar: profile.avatar.clone(),
            cli: profile.cli.clone(),
            model: profile.model.clone(),
        };
        let team = team_of(&loaded);
        // Avatar repetido já gravado não derruba o carregamento: o segundo recebe o próximo livre.
        if team.iter().any(|other| other.avatar == draft.avatar) {
            let free = free_avatar(&team).ok_or_else(|| {
                load_failed(file, format!("não há avatar livre para {}", profile.name))
            })?;
            eprintln!(
                "Avatar {} de {} já era usado por outro agente; {} passa a usar {free}.",
                draft.avatar, profile.name, profile.name
            );
            draft.avatar = free.to_string();
            rewrite = true;
        }
        let validated = validate(&draft, &team, None).map_err(|error| load_failed(file, error))?;
        if validated.id != profile.id {
            return Err(load_failed(
                file,
                format!("o id {} não bate com o nome {}", profile.id, profile.name),
            ));
        }
        loaded.push(validated);
    }
    if rewrite {
        write_file(file, &loaded, &parsed.instructions).map_err(|detail| load_failed(file, detail))?;
    }
    Ok(Stored {
        agents: loaded,
        instructions: parsed.instructions,
    })
}

// Catálogo de avatares do app: espelha os ids de identities/avatarProfiles.ts.
const AVATAR_IDS: [&str; 13] = [
    "lume",
    "ambar",
    "vesper",
    "quarzo",
    "onix",
    "selene",
    "ambar-ouro",
    "ambar-rubi",
    "ambar-rosa",
    "vesper-agua",
    "onix-negro",
    "selene-ouro",
    "ember",
];

fn free_avatar(team: &[AgentProfile]) -> Option<&'static str> {
    AVATAR_IDS
        .iter()
        .copied()
        .find(|avatar| team.iter().all(|profile| profile.avatar != *avatar))
}

// Perfis do elenco inicial gravados com o id antigo passam ao nome novo; o resto fica como está.
fn renamed_stored(profile: AgentProfile) -> AgentProfile {
    let Some((_, new_id)) = AGENT_ID_RENAMES
        .iter()
        .find(|(old_id, _)| *old_id == profile.id.as_str())
    else {
        return profile;
    };
    let Some((name, role, _)) = INITIAL_AGENTS
        .iter()
        .find(|(name, _, _)| name_key(name) == *new_id)
    else {
        return profile;
    };
    AgentProfile {
        id: Agent(new_id.to_string()),
        name: name.to_string(),
        role: role.to_string(),
        ..profile
    }
}

fn write_file(
    file: &Path,
    agents: &[AgentProfile],
    instructions: &BTreeMap<String, String>,
) -> Result<(), String> {
    let text = serde_json::to_string_pretty(&AgentsFile {
        version: FILE_VERSION,
        agents: agents.to_vec(),
        instructions: instructions.clone(),
    })
    .map_err(|error| error.to_string())?;
    write_atomic(file, AGENTS_FILE_TMP, &text)
}

// Grava num arquivo temporário ao lado e troca pelo definitivo: uma queda no meio não deixa JSON pela metade.
pub(crate) fn write_atomic(file: &Path, tmp_name: &str, text: &str) -> Result<(), String> {
    let tmp = file.with_file_name(tmp_name);
    fs::write(&tmp, text).map_err(|error| error.to_string())?;
    fs::rename(&tmp, file).map_err(|error| error.to_string())
}
