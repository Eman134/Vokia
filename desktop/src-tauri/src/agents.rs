use std::collections::BTreeMap;
use std::fmt::{self, Display};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

use serde::{Deserialize, Serialize};

use crate::claude::DEFAULT_MODEL;
use crate::db::DbError;
use crate::orchestrator::fallback::LlmTarget;
use crate::providers::{ModelTier, ModelTiers, ProviderConfig, ProviderRegistry};

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
const MODEL_MAX_CHARS: usize = 120;
const CLAUDE_CLI: &str = "claude";
const EMBER: &str = "ember";
const AXIOM: &str = "axiom";
const VEXA: &str = "vexa";

const INITIAL_AGENTS: [(&str, &str, &str); 5] = [
    ("Axiom", "Tech Lead", "quarzo"),
    ("Vexa", "Frontend Developer", "lume"),
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
    pub model_tier: Option<ModelTier>,
    #[serde(default)]
    pub follow_default_llm: bool,
    #[serde(default)]
    pub fallback: Vec<LlmTarget>,
    #[serde(default)]
    pub command: String,
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
    #[serde(default)]
    pub model_tier: Option<ModelTier>,
    #[serde(default)]
    pub follow_default_llm: bool,
    #[serde(default)]
    pub fallback: Vec<LlmTarget>,
    pub command: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmberOverride {
    #[serde(default)]
    pub cli: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub model_tier: Option<ModelTier>,
    #[serde(default)]
    pub fallback: Vec<LlmTarget>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliPreset {
    pub id: String,
    pub label: String,
    pub models: Vec<String>,
    pub default_model: String,
    pub default_command: String,
    pub tiers: ModelTiers,
    pub supports_mcp: bool,
    pub supports_sandbox: bool,
    pub builtin_seed: bool,
}

#[allow(dead_code)]
pub fn presets() -> Vec<CliPreset> {
    crate::providers::initial_presets()
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
    InvalidModel(String),
    InvalidCommand,
    ReservedCommandFlag(String),
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
            AgentError::InvalidModel(model) => write!(f, "Modelo inválido: {model}."),
            AgentError::InvalidCommand => write!(
                f,
                "Comando inválido: informe um comando válido e com aspas fechadas."
            ),
            AgentError::ReservedCommandFlag(flag) => write!(
                f,
                "A flag {flag} é controlada pelo app e não pode ser usada no comando."
            ),
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
        model_tier: None,
        follow_default_llm: false,
        fallback: Vec::new(),
        command: CLAUDE_CLI.to_string(),
        builtin: true,
        extra_instructions: String::new(),
    }
}

fn builtins() -> Vec<AgentProfile> {
    vec![builtin(Agent::ember(), "Ember", "Orquestrador geral", "ember")]
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
            model_tier: None,
            follow_default_llm: false,
            fallback: Vec::new(),
            command: CLAUDE_CLI.to_string(),
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

fn default_command_for(cli: &str) -> String {
    crate::providers::initial_presets()
        .into_iter()
        .find(|preset| preset.id == cli)
        .map_or_else(|| CLAUDE_CLI.to_string(), |preset| preset.default_command)
}

fn validate_command(reserved: &[String], command: &str) -> Result<(), AgentError> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return Err(AgentError::InvalidCommand);
    }
    let Some(parts) = shlex::split(trimmed) else {
        return Err(AgentError::InvalidCommand);
    };
    if parts.is_empty() {
        return Err(AgentError::InvalidCommand);
    }
    for arg in parts {
        let flag_name = arg.split_once('=').map_or(arg.as_str(), |(name, _)| name);
        if reserved.iter().any(|flag| flag == flag_name) {
            return Err(AgentError::ReservedCommandFlag(flag_name.to_string()));
        }
    }
    Ok(())
}

fn validate(
    draft: &AgentDraft,
    team: &[AgentProfile],
    editing: Option<&Agent>,
    provider: Option<&ProviderConfig>,
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

    let resolved_model = match (draft.model.trim(), draft.model_tier, provider) {
        ("", Some(tier), Some(provider)) => provider.tiers.get(tier).to_string(),
        (other, _, _) => other.to_string(),
    };
    let model_chars = resolved_model.chars().count();
    let model_is_valid = (1..=MODEL_MAX_CHARS).contains(&model_chars)
        && !resolved_model.contains(['\n', '\r']);
    if !model_is_valid {
        return Err(AgentError::InvalidModel(resolved_model));
    }
    let reserved = provider.map_or(&[][..], |item| item.reserved_flags.as_slice());
    validate_command(reserved, &draft.command)?;

    Ok(AgentProfile {
        id: editing.cloned().unwrap_or(Agent(key)),
        name: name.to_string(),
        role: role.to_string(),
        avatar: avatar.to_string(),
        cli: draft.cli.clone(),
        model: resolved_model,
        model_tier: draft.model_tier,
        follow_default_llm: draft.follow_default_llm,
        fallback: draft.fallback.clone(),
        command: draft.command.trim().to_string(),
        builtin: false,
        extra_instructions: String::new(),
    })
}

fn team_of(stored: &[AgentProfile], ember_override: Option<&EmberOverride>) -> Vec<AgentProfile> {
    builtins()
        .into_iter()
        .map(|mut profile| {
            if profile.id == Agent::ember() {
                if let Some(override_cfg) = ember_override {
                    if let Some(cli) = &override_cfg.cli {
                        profile.cli = cli.clone();
                    }
                    if let Some(model) = &override_cfg.model {
                        profile.model = model.clone();
                    }
                    profile.model_tier = override_cfg.model_tier;
                    profile.fallback = override_cfg.fallback.clone();
                }
            }
            profile
        })
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
#[serde(rename_all = "camelCase")]
struct AgentsFile {
    version: u32,
    agents: Vec<AgentProfile>,
    #[serde(default)]
    instructions: BTreeMap<String, String>,
    #[serde(default)]
    roster_migrated: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    ember_override: Option<EmberOverride>,
}

// Agentes gravados e instruções extras por id (inclusive dos embutidos, que não ficam em `agents`).
#[derive(Clone, Default)]
struct Stored {
    agents: Vec<AgentProfile>,
    instructions: BTreeMap<String, String>,
    ember_override: Option<EmberOverride>,
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
                team_of(&stored.agents, stored.ember_override.as_ref())
                    .into_iter()
                    .map(|profile| with_instructions(profile, &stored.instructions))
                    .collect(),
            )),
            Err(message) => Err(AgentError::LoadFailed(message.clone())),
        }
    }

    pub fn listing(&self, providers: &ProviderRegistry) -> Result<TeamListing, AgentError> {
        let team = self.team()?;
        let presets = providers
            .presets()
            .map_err(|error| AgentError::LoadFailed(error.to_string()))?;
        Ok(TeamListing {
            agents: team.profiles().to_vec(),
            presets,
        })
    }

    pub fn create(
        &self,
        draft: &AgentDraft,
        providers: &ProviderRegistry,
    ) -> Result<AgentProfile, AgentError> {
        let provider = providers
            .get(&draft.cli)
            .map_err(|_| AgentError::UnknownCli(draft.cli.clone()))?;
        let mut state = self.lock();
        let stored = stored_mut(&mut state)?;
        let profile = validate(
            draft,
            &team_of(&stored.agents, stored.ember_override.as_ref()),
            None,
            Some(&provider),
        )?;
        let mut next = stored.agents.clone();
        next.push(profile.clone());
        write_file(
            &self.file,
            &next,
            &stored.instructions,
            stored.ember_override.as_ref(),
        )
        .map_err(AgentError::SaveFailed)?;
        stored.agents = next;
        Ok(profile)
    }

    pub fn update(
        &self,
        id: &Agent,
        draft: &AgentDraft,
        providers: &ProviderRegistry,
    ) -> Result<AgentProfile, AgentError> {
        if let Some(builtin) = builtins().into_iter().find(|profile| &profile.id == id) {
            return Err(AgentError::BuiltinLocked(builtin.name));
        }
        let provider = providers
            .get(&draft.cli)
            .map_err(|_| AgentError::UnknownCli(draft.cli.clone()))?;
        let mut state = self.lock();
        let stored = stored_mut(&mut state)?;
        let index = stored
            .agents
            .iter()
            .position(|profile| &profile.id == id)
            .ok_or_else(|| AgentError::UnknownAgent(id.to_string()))?;
        let profile = validate(
            draft,
            &team_of(&stored.agents, stored.ember_override.as_ref()),
            Some(id),
            Some(&provider),
        )?;
        let mut next = stored.agents.clone();
        next[index] = profile.clone();
        write_file(
            &self.file,
            &next,
            &stored.instructions,
            stored.ember_override.as_ref(),
        )
        .map_err(AgentError::SaveFailed)?;
        stored.agents = next;
        Ok(with_instructions(profile, &stored.instructions))
    }

    pub fn update_ember_llm(
        &self,
        cli: Option<String>,
        model_tier: Option<ModelTier>,
        model: Option<String>,
        fallback: Vec<LlmTarget>,
        providers: &ProviderRegistry,
    ) -> Result<AgentProfile, AgentError> {
        let cli_id = cli.unwrap_or_else(|| CLAUDE_CLI.to_string());
        let provider = providers
            .get(&cli_id)
            .map_err(|_| AgentError::UnknownCli(cli_id.clone()))?;
        if !provider.supports_mcp {
            return Err(AgentError::SaveFailed(
                "O agente Ember exige um provedor com suporte a MCP.".to_string(),
            ));
        }
        let resolved_model = crate::orchestrator::fallback::resolve_model_for_provider(
            &provider,
            model_tier,
            model.as_deref().unwrap_or(""),
        );
        let model_chars = resolved_model.chars().count();
        if !(1..=MODEL_MAX_CHARS).contains(&model_chars) || resolved_model.contains(['\n', '\r']) {
            return Err(AgentError::InvalidModel(resolved_model));
        }
        let override_cfg = EmberOverride {
            cli: Some(cli_id),
            model: Some(resolved_model),
            model_tier,
            fallback,
        };
        let mut state = self.lock();
        let stored = stored_mut(&mut state)?;
        write_file(
            &self.file,
            &stored.agents,
            &stored.instructions,
            Some(&override_cfg),
        )
        .map_err(AgentError::SaveFailed)?;
        stored.ember_override = Some(override_cfg);
        let ember = team_of(&stored.agents, stored.ember_override.as_ref())
            .into_iter()
            .find(|profile| profile.id == Agent::ember())
            .ok_or_else(|| AgentError::UnknownAgent(EMBER.to_string()))?;
        Ok(with_instructions(ember, &stored.instructions))
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
        write_file(
            &self.file,
            &agents,
            &instructions,
            stored.ember_override.as_ref(),
        )
        .map_err(AgentError::SaveFailed)?;
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
        let profile = team_of(&stored.agents, stored.ember_override.as_ref())
            .into_iter()
            .find(|profile| &profile.id == id)
            .ok_or_else(|| AgentError::UnknownAgent(id.to_string()))?;
        let mut instructions = stored.instructions.clone();
        if text.is_empty() {
            instructions.remove(id.as_str());
        } else {
            instructions.insert(id.to_string(), text.to_string());
        }
        write_file(
            &self.file,
            &stored.agents,
            &instructions,
            stored.ember_override.as_ref(),
        )
        .map_err(AgentError::SaveFailed)?;
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
        write_file(file, &seeds, &instructions, None).map_err(|detail| load_failed(file, detail))?;
        return Ok(Stored {
            agents: seeds,
            instructions,
            ember_override: None,
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
        let command = if profile.command.trim().is_empty() {
            rewrite = true;
            default_command_for(&profile.cli)
        } else {
            profile.command.clone()
        };
        let mut draft = AgentDraft {
            name: profile.name.clone(),
            role: profile.role.clone(),
            avatar: profile.avatar.clone(),
            cli: profile.cli.clone(),
            model: profile.model.clone(),
            model_tier: profile.model_tier,
            follow_default_llm: profile.follow_default_llm,
            fallback: profile.fallback.clone(),
            command,
        };
        let team = team_of(&loaded, parsed.ember_override.as_ref());
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
        let validated = validate(&draft, &team, None, None).map_err(|error| load_failed(file, error))?;
        if validated.id != profile.id {
            return Err(load_failed(
                file,
                format!("o id {} não bate com o nome {}", profile.id, profile.name),
            ));
        }
        loaded.push(validated);
    }
    if !parsed.roster_migrated {
        for (id, name, role, default_avatar) in [
            (Agent::axiom(), "Axiom", "Tech Lead", "quarzo"),
            (Agent::vexa(), "Vexa", "Frontend Developer", "lume"),
        ] {
            if loaded.iter().any(|profile| profile.id == id) {
                continue;
            }
            let team = team_of(&loaded, parsed.ember_override.as_ref());
            if team.len() >= MAX_AGENTS {
                break;
            }
            let avatar = if team.iter().any(|other| other.avatar == default_avatar) {
                match free_avatar(&team) {
                    Some(free) => free.to_string(),
                    None => continue,
                }
            } else {
                default_avatar.to_string()
            };
            loaded.push(AgentProfile {
                id,
                name: name.to_string(),
                role: role.to_string(),
                avatar,
                cli: CLAUDE_CLI.to_string(),
                model: DEFAULT_MODEL.to_string(),
                model_tier: None,
                follow_default_llm: false,
                fallback: Vec::new(),
                command: CLAUDE_CLI.to_string(),
                builtin: false,
                extra_instructions: String::new(),
            });
        }
        rewrite = true;
    }
    if rewrite {
        write_file(
            file,
            &loaded,
            &parsed.instructions,
            parsed.ember_override.as_ref(),
        )
        .map_err(|detail| load_failed(file, detail))?;
    }
    Ok(Stored {
        agents: loaded,
        instructions: parsed.instructions,
        ember_override: parsed.ember_override,
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
    ember_override: Option<&EmberOverride>,
) -> Result<(), String> {
    let text = serde_json::to_string_pretty(&AgentsFile {
        version: FILE_VERSION,
        agents: agents.to_vec(),
        instructions: instructions.clone(),
        roster_migrated: true,
        ember_override: ember_override.cloned(),
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
