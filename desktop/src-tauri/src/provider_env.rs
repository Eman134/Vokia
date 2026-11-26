use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::agents::write_atomic;

const ENV_FILE: &str = "provider_env.json";
const ENV_FILE_TMP: &str = "provider_env.json.tmp";
const FILE_VERSION: u32 = 1;
const MIN_SECRET_REDACT_LEN: usize = 4;
const REDACTED_MARKER: &str = "[REDACTED]";

pub const RESERVED_ENV_VARS: &[&str] = &[
    "PATH",
    "HOME",
    "VOKIA_URL",
    "VOKIA_TOKEN",
    "VOKIA_CLI",
];

static REGISTERED_SECRETS: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
static ACTIVE_PROVIDER_ENV_CACHE: OnceLock<Mutex<BTreeMap<String, BTreeMap<String, String>>>> =
    OnceLock::new();

fn secrets_registry() -> &'static Mutex<Vec<String>> {
    REGISTERED_SECRETS.get_or_init(|| Mutex::new(Vec::new()))
}

fn global_env_cache() -> &'static Mutex<BTreeMap<String, BTreeMap<String, String>>> {
    ACTIVE_PROVIDER_ENV_CACHE.get_or_init(|| Mutex::new(BTreeMap::new()))
}

pub fn apply_global_provider_env(provider_id: &str, cmd: &mut tokio::process::Command) {
    let env_map = {
        let guard = global_env_cache()
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        guard.get(provider_id).cloned()
    };
    if let Some(map) = env_map {
        apply_provider_env(cmd, &map);
    }
}

pub fn register_sensitive_secret(secret: &str) {
    let trimmed = secret.trim();
    if trimmed.len() < MIN_SECRET_REDACT_LEN {
        return;
    }
    let mut guard = secrets_registry()
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    if !guard.iter().any(|existing| existing == trimmed) {
        guard.push(trimmed.to_string());
        // Longer secrets first so substrings don't partially redact a longer token.
        guard.sort_by(|a, b| b.len().cmp(&a.len()));
    }
}

pub fn unregister_sensitive_secret(secret: &str) {
    let trimmed = secret.trim();
    if trimmed.is_empty() {
        return;
    }
    let mut guard = secrets_registry()
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    guard.retain(|existing| existing != trimmed);
}

pub fn redact_registered_secrets(text: &str) -> String {
    let secrets: Vec<String> = {
        let guard = secrets_registry()
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        guard.clone()
    };
    let mut out = text.to_string();
    for secret in secrets {
        if out.contains(&secret) {
            out = out.replace(&secret, REDACTED_MARKER);
        }
    }
    out
}

pub fn validate_env_var_name(raw_key: &str) -> Result<String, String> {
    let key = raw_key.trim();
    if key.is_empty() {
        return Err("Nome da variável de ambiente não pode ficar vazio.".to_string());
    }
    let upper = key.to_ascii_uppercase();
    if RESERVED_ENV_VARS.contains(&upper.as_str()) {
        return Err(format!(
            "A variável '{key}' é reservada pelo Vokia/sistema e não pode ser configurada."
        ));
    }
    let mut chars = key.chars();
    let Some(first) = chars.next() else {
        return Err("Nome da variável de ambiente inválido.".to_string());
    };
    if !(first.is_ascii_alphabetic() || first == '_')
        || !chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    {
        return Err(format!(
            "Nome de variável inválido '{key}': use apenas letras, números e '_' (sem começar por número)."
        ));
    }
    Ok(key.to_string())
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderEnvVarView {
    pub key: String,
    pub sensitive: bool,
    pub stored: bool,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_kind: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredEnvEntry {
    key: String,
    sensitive: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    plain_value: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sealed_hex: Option<String>,
    #[serde(default)]
    storage_kind: String,
    updated_at_ms: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProviderEnvFile {
    version: u32,
    #[serde(default)]
    providers: BTreeMap<String, Vec<StoredEnvEntry>>,
}

pub struct ProviderEnvRegistry {
    file: Option<PathBuf>,
    state: Mutex<ProviderEnvFile>,
}

impl ProviderEnvRegistry {
    pub fn load(dir: &Path) -> Self {
        let file = dir.join(ENV_FILE);
        let state = read_env_file(&file).unwrap_or_else(|_| ProviderEnvFile {
            version: FILE_VERSION,
            providers: BTreeMap::new(),
        });
        let registry = Self {
            file: Some(file),
            state: Mutex::new(state),
        };
        registry.register_all_secrets_for_redaction();
        registry.sync_all_global_cache();
        registry
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn in_memory() -> Self {
        Self {
            file: None,
            state: Mutex::new(ProviderEnvFile {
                version: FILE_VERSION,
                providers: BTreeMap::new(),
            }),
        }
    }

    fn register_all_secrets_for_redaction(&self) {
        let entries: Vec<(String, StoredEnvEntry)> = {
            let guard = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            guard
                .providers
                .iter()
                .flat_map(|(pid, list)| list.iter().map(move |e| (pid.clone(), e.clone())))
                .collect()
        };
        for (pid, entry) in entries {
            if entry.sensitive {
                if let Ok(Some(val)) = unseal_entry(&pid, &entry) {
                    register_sensitive_secret(&val);
                }
            }
        }
    }

    pub fn list_vars(&self, provider_id: &str) -> Vec<ProviderEnvVarView> {
        let entries = {
            let guard = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            guard
                .providers
                .get(provider_id)
                .cloned()
                .unwrap_or_default()
        };
        entries
            .into_iter()
            .map(|entry| {
                if entry.sensitive {
                    match unseal_entry(provider_id, &entry) {
                        Ok(Some(secret)) => {
                            register_sensitive_secret(&secret);
                            ProviderEnvVarView {
                                key: entry.key,
                                sensitive: true,
                                stored: true,
                                status: "saved".to_string(),
                                value: None,
                                storage_kind: Some(entry.storage_kind),
                            }
                        }
                        _ => ProviderEnvVarView {
                            key: entry.key,
                            sensitive: true,
                            stored: false,
                            status: "unreadable".to_string(),
                            value: None,
                            storage_kind: Some(entry.storage_kind),
                        },
                    }
                } else {
                    ProviderEnvVarView {
                        key: entry.key,
                        sensitive: false,
                        stored: entry.plain_value.is_some(),
                        status: if entry.plain_value.is_some() {
                            "saved".to_string()
                        } else {
                            "missing".to_string()
                        },
                        value: entry.plain_value,
                        storage_kind: Some("plain".to_string()),
                    }
                }
            })
            .collect()
    }

    pub fn set_var(
        &self,
        provider_id: &str,
        raw_key: &str,
        value: &str,
        sensitive: bool,
    ) -> Result<ProviderEnvVarView, String> {
        let pid = provider_id.trim();
        if pid.is_empty() {
            return Err("ID do provedor é obrigatório.".to_string());
        }
        let key = validate_env_var_name(raw_key)?;
        let trimmed_val = value.trim();
        if trimmed_val.is_empty() {
            return Err(format!("O valor da variável '{key}' não pode ficar vazio."));
        }

        let now_ms = now_millis();
        let entry = if sensitive {
            register_sensitive_secret(trimmed_val);
            let (storage_kind, sealed_hex) = seal_secret(pid, &key, trimmed_val)?;
            StoredEnvEntry {
                key: key.clone(),
                sensitive: true,
                plain_value: None,
                sealed_hex,
                storage_kind,
                updated_at_ms: now_ms,
            }
        } else {
            StoredEnvEntry {
                key: key.clone(),
                sensitive: false,
                plain_value: Some(trimmed_val.to_string()),
                sealed_hex: None,
                storage_kind: "plain".to_string(),
                updated_at_ms: now_ms,
            }
        };

        let mut guard = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let mut next = guard.clone();
        let list = next.providers.entry(pid.to_string()).or_default();
        if let Some(existing) = list.iter_mut().find(|item| item.key == key) {
            *existing = entry.clone();
        } else {
            list.push(entry.clone());
        }
        list.sort_by(|a, b| a.key.cmp(&b.key));
        if let Some(file) = &self.file {
            write_env_file(file, &next)?;
        }
        *guard = next;
        drop(guard);
        self.sync_provider_global_cache(pid);

        Ok(ProviderEnvVarView {
            key,
            sensitive,
            stored: true,
            status: "saved".to_string(),
            value: if sensitive {
                None
            } else {
                Some(trimmed_val.to_string())
            },
            storage_kind: Some(entry.storage_kind),
        })
    }

    pub fn delete_var(&self, provider_id: &str, raw_key: &str) -> Result<bool, String> {
        let pid = provider_id.trim();
        let key = raw_key.trim();
        let mut guard = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let mut next = guard.clone();
        let removed_entry = next.providers.get_mut(pid).and_then(|list| {
            let pos = list.iter().position(|item| item.key == key)?;
            Some(list.remove(pos))
        });
        let Some(entry) = removed_entry else {
            return Ok(false);
        };
        if entry.sensitive {
            if let Ok(Some(secret)) = unseal_entry(pid, &entry) {
                unregister_sensitive_secret(&secret);
            }
            delete_os_secret(pid, &entry.key, &entry.storage_kind);
        }
        if let Some(file) = &self.file {
            write_env_file(file, &next)?;
        }
        *guard = next;
        drop(guard);
        self.sync_provider_global_cache(pid);
        Ok(true)
    }

    fn sync_all_global_cache(&self) {
        let provider_ids: Vec<String> = {
            let guard = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            guard.providers.keys().cloned().collect()
        };
        for pid in provider_ids {
            self.sync_provider_global_cache(&pid);
        }
    }

    fn sync_provider_global_cache(&self, provider_id: &str) {
        let resolved = self.resolve_env_for_provider(provider_id);
        let mut cache = global_env_cache()
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if resolved.is_empty() {
            cache.remove(provider_id);
        } else {
            cache.insert(provider_id.to_string(), resolved);
        }
    }

    pub fn resolve_env_for_provider(&self, provider_id: &str) -> BTreeMap<String, String> {
        let entries = {
            let guard = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            guard
                .providers
                .get(provider_id)
                .cloned()
                .unwrap_or_default()
        };
        let mut map = BTreeMap::new();
        for entry in entries {
            if validate_env_var_name(&entry.key).is_err() {
                continue;
            }
            if entry.sensitive {
                if let Ok(Some(secret)) = unseal_entry(provider_id, &entry) {
                    register_sensitive_secret(&secret);
                    map.insert(entry.key, secret);
                }
            } else if let Some(val) = entry.plain_value {
                map.insert(entry.key, val);
            }
        }
        map
    }

    pub fn has_saved_var(&self, provider_id: &str, keys: &[String]) -> bool {
        let entries = {
            let guard = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            guard
                .providers
                .get(provider_id)
                .cloned()
                .unwrap_or_default()
        };
        entries.iter().any(|entry| {
            keys.iter().any(|k| k == &entry.key)
                && if entry.sensitive {
                    unseal_entry(provider_id, entry)
                        .ok()
                        .flatten()
                        .is_some_and(|v| !v.trim().is_empty())
                } else {
                    entry
                        .plain_value
                        .as_deref()
                        .is_some_and(|v| !v.trim().is_empty())
                }
        })
    }
}

pub fn apply_provider_env(cmd: &mut tokio::process::Command, env_map: &BTreeMap<String, String>) {
    for (key, value) in env_map {
        if validate_env_var_name(key).is_ok() {
            cmd.env(key, value);
        }
    }
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

fn read_env_file(file: &Path) -> Result<ProviderEnvFile, String> {
    if !file.exists() {
        return Ok(ProviderEnvFile {
            version: FILE_VERSION,
            providers: BTreeMap::new(),
        });
    }
    let text = fs::read_to_string(file).map_err(|e| e.to_string())?;
    let parsed: ProviderEnvFile = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    if parsed.version != FILE_VERSION {
        return Err(format!("Versão desconhecida em provider_env.json: {}", parsed.version));
    }
    Ok(parsed)
}

fn write_env_file(file: &Path, data: &ProviderEnvFile) -> Result<(), String> {
    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(data).map_err(|e| e.to_string())?;
    write_atomic(file, ENV_FILE_TMP, &text)
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

fn decode_hex(hex: &str) -> Result<Vec<u8>, String> {
    let clean = hex.trim();
    if clean.len() % 2 != 0 {
        return Err("Hexadecimal inválido.".to_string());
    }
    let bytes = clean.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() / 2);
    for pair in bytes.chunks_exact(2) {
        let hi = (pair[0] as char)
            .to_digit(16)
            .ok_or_else(|| "Caractere hexadecimal inválido.".to_string())?;
        let lo = (pair[1] as char)
            .to_digit(16)
            .ok_or_else(|| "Caractere hexadecimal inválido.".to_string())?;
        out.push(((hi << 4) | lo) as u8);
    }
    Ok(out)
}

#[cfg(windows)]
mod platform_seal {
    use std::ffi::c_void;
    use std::ptr;

    const CRYPTPROTECT_UI_FORBIDDEN: u32 = 0x1;
    const ENTROPY: &[u8] = b"vokia-provider-env-v1";

    #[repr(C)]
    struct DataBlob {
        cb_data: u32,
        pb_data: *mut u8,
    }

    #[link(name = "crypt32")]
    extern "system" {
        fn CryptProtectData(
            p_data_in: *const DataBlob,
            sz_data_descr: *const u16,
            p_optional_entropy: *const DataBlob,
            pv_reserved: *mut c_void,
            p_prompt_struct: *mut c_void,
            dw_flags: u32,
            p_data_out: *mut DataBlob,
        ) -> i32;

        fn CryptUnprotectData(
            p_data_in: *const DataBlob,
            ppsz_data_descr: *mut *mut u16,
            p_optional_entropy: *const DataBlob,
            pv_reserved: *mut c_void,
            p_prompt_struct: *mut c_void,
            dw_flags: u32,
            p_data_out: *mut DataBlob,
        ) -> i32;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn LocalFree(h_mem: *mut c_void) -> *mut c_void;
    }

    pub fn seal(secret: &str) -> Result<Vec<u8>, String> {
        let mut input = secret.as_bytes().to_vec();
        let mut entropy = ENTROPY.to_vec();
        let data_in = DataBlob {
            cb_data: input.len() as u32,
            pb_data: input.as_mut_ptr(),
        };
        let entropy_blob = DataBlob {
            cb_data: entropy.len() as u32,
            pb_data: entropy.as_mut_ptr(),
        };
        let mut data_out = DataBlob {
            cb_data: 0,
            pb_data: ptr::null_mut(),
        };
        let ok = unsafe {
            CryptProtectData(
                &data_in,
                ptr::null(),
                &entropy_blob,
                ptr::null_mut(),
                ptr::null_mut(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut data_out,
            )
        };
        if ok == 0 || data_out.pb_data.is_null() {
            return Err("Falha ao proteger credencial com DPAPI do Windows.".to_string());
        }
        let slice = unsafe { std::slice::from_raw_parts(data_out.pb_data, data_out.cb_data as usize) };
        let result = slice.to_vec();
        unsafe {
            LocalFree(data_out.pb_data.cast::<c_void>());
        }
        Ok(result)
    }

    pub fn unseal(cipher: &[u8]) -> Result<String, String> {
        let mut input = cipher.to_vec();
        let mut entropy = ENTROPY.to_vec();
        let data_in = DataBlob {
            cb_data: input.len() as u32,
            pb_data: input.as_mut_ptr(),
        };
        let entropy_blob = DataBlob {
            cb_data: entropy.len() as u32,
            pb_data: entropy.as_mut_ptr(),
        };
        let mut data_out = DataBlob {
            cb_data: 0,
            pb_data: ptr::null_mut(),
        };
        let ok = unsafe {
            CryptUnprotectData(
                &data_in,
                ptr::null_mut(),
                &entropy_blob,
                ptr::null_mut(),
                ptr::null_mut(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut data_out,
            )
        };
        if ok == 0 || data_out.pb_data.is_null() {
            return Err("Falha ao descriptografar credencial via DPAPI do Windows.".to_string());
        }
        let slice = unsafe { std::slice::from_raw_parts(data_out.pb_data, data_out.cb_data as usize) };
        let out = String::from_utf8(slice.to_vec())
            .map_err(|_| "Credencial descriptografada não é UTF-8 válido.".to_string());
        unsafe {
            LocalFree(data_out.pb_data.cast::<c_void>());
        }
        out
    }
}

fn seal_secret(_provider_id: &str, _key: &str, value: &str) -> Result<(String, Option<String>), String> {
    #[cfg(windows)]
    {
        let blob = platform_seal::seal(value)?;
        return Ok(("dpapi".to_string(), Some(encode_hex(&blob))));
    }

    #[cfg(target_os = "macos")]
    {
        let account = format!("{_provider_id}:{_key}");
        let status = std::process::Command::new("security")
            .args([
                "add-generic-password",
                "-U",
                "-a",
                &account,
                "-s",
                "vokia.provider_env",
                "-w",
                value,
            ])
            .status();
        if status.is_ok_and(|s| s.success()) {
            return Ok(("keychain".to_string(), None));
        }
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let account = format!("{_provider_id}:{_key}");
        let mut child = std::process::Command::new("secret-tool")
            .args([
                "store",
                "--label",
                &format!("Vokia {account}"),
                "service",
                "vokia.provider_env",
                "account",
                &account,
            ])
            .stdin(std::process::Stdio::piped())
            .spawn()
            .ok();
        if let Some(mut proc) = child {
            use std::io::Write;
            if let Some(mut stdin) = proc.stdin.take() {
                let _ = stdin.write_all(value.as_bytes());
            }
            if proc.wait().is_ok_and(|s| s.success()) {
                return Ok(("secret-tool".to_string(), None));
            }
        }
    }

    #[cfg(not(windows))]
    {
        let sealed = fallback_xor_seal(value.as_bytes())?;
        Ok(("sealed-file".to_string(), Some(encode_hex(&sealed))))
    }
}

fn unseal_entry(_provider_id: &str, entry: &StoredEnvEntry) -> Result<Option<String>, String> {
    if !entry.sensitive {
        return Ok(entry.plain_value.clone());
    }
    match entry.storage_kind.as_str() {
        #[cfg(windows)]
        "dpapi" => {
            let Some(hex) = entry.sealed_hex.as_deref() else {
                return Ok(None);
            };
            let blob = decode_hex(hex)?;
            platform_seal::unseal(&blob).map(Some)
        }
        #[cfg(target_os = "macos")]
        "keychain" => {
            let account = format!("{_provider_id}:{}", entry.key);
            let output = std::process::Command::new("security")
                .args([
                    "find-generic-password",
                    "-a",
                    &account,
                    "-s",
                    "vokia.provider_env",
                    "-w",
                ])
                .output()
                .map_err(|e| e.to_string())?;
            if output.status.success() {
                Ok(Some(
                    String::from_utf8_lossy(&output.stdout)
                        .trim_end_matches(&['\r', '\n'][..])
                        .to_string(),
                ))
            } else {
                Err("Segredo não encontrado no Keychain.".to_string())
            }
        }
        #[cfg(all(unix, not(target_os = "macos")))]
        "secret-tool" => {
            let account = format!("{_provider_id}:{}", entry.key);
            let output = std::process::Command::new("secret-tool")
                .args([
                    "lookup",
                    "service",
                    "vokia.provider_env",
                    "account",
                    &account,
                ])
                .output()
                .map_err(|e| e.to_string())?;
            if output.status.success() {
                Ok(Some(
                    String::from_utf8_lossy(&output.stdout)
                        .trim_end_matches(&['\r', '\n'][..])
                        .to_string(),
                ))
            } else {
                Err("Segredo não encontrado no Secret Service.".to_string())
            }
        }
        #[cfg(not(windows))]
        "sealed-file" => {
            let Some(hex) = entry.sealed_hex.as_deref() else {
                return Ok(None);
            };
            let blob = decode_hex(hex)?;
            let plain = fallback_xor_unseal(&blob)?;
            String::from_utf8(plain)
                .map(Some)
                .map_err(|_| "Credencial inválida.".to_string())
        }
        _ => Err("Tipo de armazenamento seguro não suportado.".to_string()),
    }
}

fn delete_os_secret(_provider_id: &str, _key: &str, _storage_kind: &str) {
    #[cfg(target_os = "macos")]
    if _storage_kind == "keychain" {
        let account = format!("{_provider_id}:{_key}");
        let _ = std::process::Command::new("security")
            .args([
                "delete-generic-password",
                "-a",
                &account,
                "-s",
                "vokia.provider_env",
            ])
            .status();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    if _storage_kind == "secret-tool" {
        let account = format!("{_provider_id}:{_key}");
        let _ = std::process::Command::new("secret-tool")
            .args([
                "clear",
                "service",
                "vokia.provider_env",
                "account",
                &account,
            ])
            .status();
    }
}

#[cfg(not(windows))]
fn fallback_machine_key() -> [u8; 32] {
    let mut key = [0u8; 32];
    let seed = format!(
        "{}:{}:vokia-fallback-seal-v1",
        std::env::var("USER").unwrap_or_default(),
        std::env::var("HOME").unwrap_or_default()
    );
    let bytes = seed.as_bytes();
    for (idx, slot) in key.iter_mut().enumerate() {
        *slot = bytes[idx % bytes.len()].wrapping_mul((idx as u8).wrapping_add(17)) ^ 0x5a;
    }
    key
}

#[cfg(not(windows))]
fn fallback_xor_seal(plain: &[u8]) -> Result<Vec<u8>, String> {
    let mut nonce = [0u8; 16];
    getrandom::fill(&mut nonce).map_err(|e| format!("Falha ao gerar entropia: {e}"))?;
    let key = fallback_machine_key();
    let mut out = Vec::with_capacity(16 + plain.len());
    out.extend_from_slice(&nonce);
    for (i, &b) in plain.iter().enumerate() {
        let mask = key[i % key.len()] ^ nonce[i % nonce.len()];
        out.push(b ^ mask);
    }
    Ok(out)
}

#[cfg(not(windows))]
fn fallback_xor_unseal(blob: &[u8]) -> Result<Vec<u8>, String> {
    if blob.len() < 16 {
        return Err("Bloco criptografado truncado.".to_string());
    }
    let (nonce, cipher) = blob.split_at(16);
    let key = fallback_machine_key();
    let mut out = Vec::with_capacity(cipher.len());
    for (i, &b) in cipher.iter().enumerate() {
        let mask = key[i % key.len()] ^ nonce[i % nonce.len()];
        out.push(b ^ mask);
    }
    Ok(out)
}
