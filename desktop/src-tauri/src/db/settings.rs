use std::collections::HashMap;

use rusqlite::params;

use super::connection::Db;
use super::error::DbError;

const SETTING_THEME: &str = "theme";
// Espelha THEME_CHOICES de src/features/theme/useTheme.ts.
const THEME_VALUES: &[&str] = &["light", "dark", "system"];
pub const SETTING_PERMISSION_GRANT: &str = "permission_grant";
pub const SETTING_AUTO_TRIAGE: &str = "auto_triage";
pub const SETTING_ACTIVE_LLM: &str = "active_llm";
pub const SETTING_LLM_FALLBACK: &str = "llm_fallback";
pub const SETTING_WEB_SEARCH: &str = "web_search";
pub const SETTING_AUTO_PAUSE_ON_LIMIT: &str = "auto_pause_on_limit";
pub const SETTING_USAGE_PAUSE_THRESHOLD: &str = "usage_pause_threshold";
pub const SETTING_CONTINUATION_ENABLED: &str = "continuationEnabled";
pub const SETTING_CONTINUATION_MAX_COUNT: &str = "continuationMaxCount";
pub const SETTING_LEARNING_MODE: &str = "learning_mode";
pub const SETTING_LEARNING_TURN_CADENCE: &str = "learning_turn_cadence";
pub const LEARNING_MODE_ASK: &str = "ask";
pub const LEARNING_MODE_OFF: &str = "off";
pub const LEARNING_MODE_AUTO: &str = "auto";
const LEARNING_MODE_VALUES: &[&str] = &[LEARNING_MODE_ASK, LEARNING_MODE_OFF, LEARNING_MODE_AUTO];
const LEARNING_CADENCE_VALUES: &[&str] = &["10", "20", "40", "0"];
const CONTINUATION_MAX_COUNT_MIN: u32 = 0;
const CONTINUATION_MAX_COUNT_MAX: u32 = 50;
const BOOL_VALUES: &[&str] = &["true", "false"];
const USAGE_PAUSE_THRESHOLD_VALUES: &[&str] = &["85", "90", "95", "off"];
pub const GRANT_SESSION: &str = "session";
pub const GRANT_ONCE: &str = "once";
const PERMISSION_GRANT_VALUES: &[&str] = &[GRANT_SESSION, GRANT_ONCE];
const AUTO_TRIAGE_VALUES: &[&str] = &["true", "false"];
const SETTING_AVATAR_PROFILES: &str = "avatar_profiles";
const SETTING_USER_NAME: &str = "user_name";
const SETTING_USER_AVATAR: &str = "user_avatar";
// Mesmos limites de NAME_MIN_CHARS/NAME_MAX_CHARS dos agentes (src/agents.rs, onde são privados).
const USER_NAME_MIN_CHARS: usize = 2;
const USER_NAME_MAX_CHARS: usize = 20;

enum SettingRule {
    OneOf(&'static [&'static str]),
    // O formato do JSON é validado no front (useAvatarProfiles); aqui só se recusa texto que não é JSON.
    Json,
    // Conta caracteres (não bytes) do texto sem as pontas e recusa quebra de linha.
    Text { min: usize, max: usize },
    // Inteiro não negativo dentro do intervalo fechado [min, max].
    Uint { min: u32, max: u32 },
}

const SETTINGS_SCHEMA: &[(&str, SettingRule)] = &[
    (SETTING_THEME, SettingRule::OneOf(THEME_VALUES)),
    (
        SETTING_PERMISSION_GRANT,
        SettingRule::OneOf(PERMISSION_GRANT_VALUES),
    ),
    (SETTING_AUTO_TRIAGE, SettingRule::OneOf(AUTO_TRIAGE_VALUES)),
    (SETTING_AVATAR_PROFILES, SettingRule::Json),
    (
        SETTING_USER_NAME,
        SettingRule::Text {
            min: USER_NAME_MIN_CHARS,
            max: USER_NAME_MAX_CHARS,
        },
    ),
    (SETTING_USER_AVATAR, SettingRule::Json),
    (SETTING_ACTIVE_LLM, SettingRule::Json),
    (SETTING_LLM_FALLBACK, SettingRule::Json),
    (SETTING_WEB_SEARCH, SettingRule::Json),
    (SETTING_AUTO_PAUSE_ON_LIMIT, SettingRule::OneOf(BOOL_VALUES)),
    (
        SETTING_USAGE_PAUSE_THRESHOLD,
        SettingRule::OneOf(USAGE_PAUSE_THRESHOLD_VALUES),
    ),
    (
        SETTING_CONTINUATION_ENABLED,
        SettingRule::OneOf(BOOL_VALUES),
    ),
    (
        SETTING_CONTINUATION_MAX_COUNT,
        SettingRule::Uint {
            min: CONTINUATION_MAX_COUNT_MIN,
            max: CONTINUATION_MAX_COUNT_MAX,
        },
    ),
    (
        SETTING_LEARNING_MODE,
        SettingRule::OneOf(LEARNING_MODE_VALUES),
    ),
    (
        SETTING_LEARNING_TURN_CADENCE,
        SettingRule::OneOf(LEARNING_CADENCE_VALUES),
    ),
];

// Só entram aqui as configurações que o backend precisa resolver; o tema tem o padrão no front.
const SETTING_DEFAULTS: &[(&str, &str)] = &[
    (SETTING_PERMISSION_GRANT, GRANT_SESSION),
    (SETTING_AUTO_PAUSE_ON_LIMIT, "false"),
    (SETTING_USAGE_PAUSE_THRESHOLD, "90"),
    (SETTING_CONTINUATION_ENABLED, "true"),
    (SETTING_CONTINUATION_MAX_COUNT, "5"),
    (SETTING_LEARNING_MODE, LEARNING_MODE_ASK),
    (SETTING_LEARNING_TURN_CADENCE, "20"),
];

impl Db {
    pub fn load_settings(&self) -> Result<HashMap<String, String>, DbError> {
        let conn = self.conn()?;
        let mut statement = conn.prepare("SELECT key, value FROM settings")?;
        let rows = statement.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut settings = HashMap::new();
        for row in rows {
            let (key, value) = row?;
            if is_loadable_setting(&key, &value) {
                settings.insert(key, value);
            }
        }
        for (key, value) in SETTING_DEFAULTS {
            settings
                .entry((*key).to_string())
                .or_insert_with(|| (*value).to_string());
        }
        Ok(settings)
    }

    pub fn save_setting(&self, key: &str, value: &str) -> Result<(), DbError> {
        if !is_valid_setting(key, value) {
            return Err(DbError::InvalidSetting(key.to_string(), value.to_string()));
        }
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO settings(key, value) VALUES (?1, ?2) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn auto_pause_on_limit(&self) -> Result<bool, DbError> {
        let settings = self.load_settings()?;
        Ok(settings
            .get(SETTING_AUTO_PAUSE_ON_LIMIT)
            .is_some_and(|value| value == "true"))
    }

    pub fn usage_pause_threshold(&self) -> Result<Option<u8>, DbError> {
        let settings = self.load_settings()?;
        Ok(parse_usage_pause_threshold(
            settings
                .get(SETTING_USAGE_PAUSE_THRESHOLD)
                .map(String::as_str)
                .unwrap_or("90"),
        ))
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn continuation_enabled(&self) -> Result<bool, DbError> {
        let settings = self.load_settings()?;
        Ok(settings
            .get(SETTING_CONTINUATION_ENABLED)
            .map_or(true, |value| value == "true"))
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn continuation_max_count(&self) -> Result<u32, DbError> {
        let settings = self.load_settings()?;
        Ok(settings
            .get(SETTING_CONTINUATION_MAX_COUNT)
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(5))
    }

    pub fn learning_mode(&self) -> Result<String, DbError> {
        let settings = self.load_settings()?;
        Ok(settings
            .get(SETTING_LEARNING_MODE)
            .cloned()
            .unwrap_or_else(|| LEARNING_MODE_ASK.to_string()))
    }

    pub fn learning_turn_cadence(&self) -> Result<u32, DbError> {
        let settings = self.load_settings()?;
        Ok(settings
            .get(SETTING_LEARNING_TURN_CADENCE)
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(20))
    }
}

pub fn parse_usage_pause_threshold(raw: &str) -> Option<u8> {
    match raw.trim() {
        "off" | "0" => None,
        "85" => Some(85),
        "95" => Some(95),
        _ => Some(90),
    }
}

impl SettingRule {
    fn accepts(&self, value: &str) -> bool {
        match self {
            SettingRule::OneOf(allowed) => allowed.contains(&value),
            SettingRule::Json => serde_json::from_str::<serde_json::Value>(value).is_ok(),
            SettingRule::Text { min, max } => {
                !value.contains(['\n', '\r'])
                    && (*min..=*max).contains(&value.trim().chars().count())
            }
            SettingRule::Uint { min, max } => {
                value.parse::<u32>().is_ok_and(|n| (*min..=*max).contains(&n))
            }
        }
    }
}

// Valor JSON inválido sai bruto: o front precisa vê-lo para avisar e não gravar por cima.
fn is_loadable_setting(key: &str, value: &str) -> bool {
    SETTINGS_SCHEMA.iter().any(|(known, rule)| {
        *known == key && (matches!(rule, SettingRule::Json) || rule.accepts(value))
    })
}

fn is_valid_setting(key: &str, value: &str) -> bool {
    SETTINGS_SCHEMA
        .iter()
        .any(|(known, rule)| *known == key && rule.accepts(value))
}
