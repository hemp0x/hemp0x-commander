use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use serde::{Serialize, Deserialize};
use zeroize::Zeroizing;

use crate::modules::content_library::content_library_dir;
use crate::modules::vault;

static VAULT_PASSPHRASE: OnceLock<Mutex<Option<Zeroizing<String>>>> = OnceLock::new();

fn vault_passphrase_cache() -> &'static Mutex<Option<Zeroizing<String>>> {
    VAULT_PASSPHRASE.get_or_init(|| Mutex::new(None))
}

pub fn set_vault_passphrase(passphrase: String) {
    if let Ok(mut cache) = vault_passphrase_cache().lock() {
        *cache = Some(Zeroizing::new(passphrase));
    }
}

pub fn clear_vault_passphrase() {
    if let Ok(mut cache) = vault_passphrase_cache().lock() {
        *cache = None;
    }
}

fn get_cached_passphrase() -> Option<String> {
    vault_passphrase_cache()
        .lock()
        .ok()
        .and_then(|c| c.as_ref().map(|z| (**z).clone()))
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ProviderGateways {
    pub viewing_gateways: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ProviderSettings {
    pub selected_publish_provider: String,
    pub gateways: ProviderGateways,
    #[serde(default)]
    pub pinata_api_token: String,
    #[serde(default = "default_pinata_api_url")]
    pub pinata_api_url: String,
    #[serde(default = "default_kubo_endpoint")]
    pub kubo_endpoint: String,
    #[serde(default)]
    pub filebase_token: String,
    #[serde(default = "default_filebase_endpoint")]
    pub filebase_endpoint: String,
}

fn default_pinata_api_url() -> String {
    "https://api.pinata.cloud".to_string()
}

fn default_kubo_endpoint() -> String {
    "http://127.0.0.1:5001".to_string()
}

fn default_filebase_endpoint() -> String {
    "https://rpc.filebase.io".to_string()
}

fn is_masked_secret(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.len() >= 9
        && trimmed.len() <= 13
        && trimmed.starts_with(|c: char| c.is_alphanumeric())
        && trimmed.ends_with(|c: char| c.is_alphanumeric())
        && trimmed.contains("...")
}

fn is_vault_placeholder(value: &str) -> bool {
    value.trim() == "[in vault]"
}

fn is_real_token_value(value: &str) -> bool {
    let trimmed = value.trim();
    !trimmed.is_empty() && !is_masked_secret(trimmed) && !is_vault_placeholder(trimmed)
}

impl Default for ProviderSettings {
    fn default() -> Self {
        Self {
            selected_publish_provider: "manual".to_string(),
            gateways: ProviderGateways {
                viewing_gateways: vec![
                    "https://dweb.link/ipfs/".to_string(),
                    "https://ipfs.io/ipfs/".to_string(),
                ],
            },
            pinata_api_token: String::new(),
            pinata_api_url: default_pinata_api_url(),
            kubo_endpoint: default_kubo_endpoint(),
            filebase_token: String::new(),
            filebase_endpoint: default_filebase_endpoint(),
        }
    }
}

fn provider_settings_path() -> Result<PathBuf, String> {
    Ok(content_library_dir()?.join("provider_settings.json"))
}

fn provider_settings_temp_path() -> Result<PathBuf, String> {
    Ok(content_library_dir()?.join("provider_settings.json.tmp"))
}

fn legacy_provider_settings_path() -> Result<PathBuf, String> {
    Ok(crate::modules::files::data_dir()?.join("content-library").join("provider_settings.json"))
}

pub fn load_provider_settings() -> Result<ProviderSettings, String> {
    let path = provider_settings_path()?;
    if path.exists() {
        let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        return serde_json::from_str(&content).map_err(|e| format!("Corrupt provider settings: {e}"));
    }
    let legacy = legacy_provider_settings_path()?;
    if legacy.exists() {
        let content = fs::read_to_string(&legacy).map_err(|e| e.to_string())?;
        let settings: ProviderSettings = serde_json::from_str(&content).map_err(|e| format!("Corrupt legacy provider settings: {e}"))?;
        save_provider_settings_atomic(&settings)?;
        return Ok(settings);
    }
    Ok(ProviderSettings::default())
}

pub fn has_plaintext_tokens() -> bool {
    load_provider_settings()
        .map(|s| is_real_token_value(&s.pinata_api_token) || is_real_token_value(&s.filebase_token))
        .unwrap_or(false)
}

pub fn load_provider_tokens() -> Result<(String, String), String> {
    if vault::vault_exists() {
        if let Some(passphrase) = get_cached_passphrase() {
            match vault::load_vault_tokens(&passphrase) {
                Ok(payload) => {
                    return Ok((vault::payload_pinata_token(&payload), vault::payload_filebase_token(&payload)));
                }
                Err(e) => {
                    clear_vault_passphrase();
                    return Err(format!("Vault decrypt failed: {e}. Vault has been locked."));
                }
            }
        }
        return Err("Provider token vault is locked. Unlock it in IPFS settings.".to_string());
    }
    let settings = load_provider_settings()?;
    Ok((settings.pinata_api_token.clone(), settings.filebase_token.clone()))
}

pub fn viewing_gateways() -> Vec<String> {
    load_provider_settings()
        .map(|settings| settings.gateways.viewing_gateways)
        .unwrap_or_else(|_| ProviderSettings::default().gateways.viewing_gateways)
}

fn save_provider_settings_atomic(settings: &ProviderSettings) -> Result<(), String> {
    let path = provider_settings_path()?;
    let tmp = provider_settings_temp_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let content = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    fs::write(&tmp, content).map_err(|e| e.to_string())?;
    fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
    Ok(())
}

fn clean_temp_files() {
    let _ = provider_settings_temp_path().map(|tmp| { let _ = fs::remove_file(&tmp); });
    let _ = crate::modules::files::commander_dir().map(|d| {
        let vault_tmp = d.join("vault.json.tmp");
        let _ = fs::remove_file(&vault_tmp);
    });
}

pub fn mask_secrets(settings: &ProviderSettings) -> ProviderSettings {
    let mut masked = settings.clone();
    if is_real_token_value(&masked.pinata_api_token) && masked.pinata_api_token.len() > 8 {
        let len = masked.pinata_api_token.len();
        masked.pinata_api_token = format!("{}...{}", &masked.pinata_api_token[..4], &masked.pinata_api_token[len-4..]);
    }
    if is_real_token_value(&masked.filebase_token) && masked.filebase_token.len() > 8 {
        let len = masked.filebase_token.len();
        masked.filebase_token = format!("{}...{}", &masked.filebase_token[..4], &masked.filebase_token[len-4..]);
    }
    masked
}

fn merge_existing_secrets(mut incoming: ProviderSettings) -> Result<ProviderSettings, String> {
    let existing = load_provider_settings().unwrap_or_default();
    if is_masked_secret(&incoming.pinata_api_token) || is_vault_placeholder(&incoming.pinata_api_token) {
        incoming.pinata_api_token = existing.pinata_api_token;
    }
    if is_masked_secret(&incoming.filebase_token) || is_vault_placeholder(&incoming.filebase_token) {
        incoming.filebase_token = existing.filebase_token;
    }
    Ok(incoming)
}

fn token_fields_changed(incoming: &ProviderSettings, existing: &ProviderSettings) -> bool {
    incoming.pinata_api_token != existing.pinata_api_token
        || incoming.filebase_token != existing.filebase_token
}

#[tauri::command]
pub fn ipfs_get_provider_settings() -> Result<ProviderSettings, String> {
    clean_temp_files();
    let mut settings = load_provider_settings()?;
    if vault::vault_exists() {
        settings.pinata_api_token = "[in vault]".to_string();
        settings.filebase_token = "[in vault]".to_string();
    }
    Ok(mask_secrets(&settings))
}

#[tauri::command]
pub fn ipfs_update_provider_settings(settings: ProviderSettings) -> Result<ProviderSettings, String> {
    let settings = merge_existing_secrets(settings)?;
    if settings.selected_publish_provider.is_empty() {
        return Err("Publish provider selection must not be empty".to_string());
    }
    let valid_providers = ["manual", "pinata", "installed_kubo", "filebase"];
    if !valid_providers.contains(&settings.selected_publish_provider.as_str()) {
        return Err(format!("Invalid publish provider: {}. Must be one of: {}", settings.selected_publish_provider, valid_providers.join(", ")));
    }
    if settings.gateways.viewing_gateways.is_empty() {
        return Err("At least one viewing gateway is required".to_string());
    }
    for gw in &settings.gateways.viewing_gateways {
        let trimmed = gw.trim();
        if trimmed.is_empty() {
            return Err("Gateway URLs must not be empty".to_string());
        }
        if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
            return Err(format!("Gateway URL must start with http:// or https://: {}", trimmed));
        }
    }
    if !settings.kubo_endpoint.is_empty() && !settings.kubo_endpoint.starts_with("http://") && !settings.kubo_endpoint.starts_with("https://") {
        return Err("Kubo endpoint must start with http:// or https://".to_string());
    }
    if !settings.pinata_api_url.is_empty() && !settings.pinata_api_url.starts_with("https://") {
        return Err("Pinata API URL must start with https://".to_string());
    }
    if !settings.filebase_endpoint.is_empty() && !settings.filebase_endpoint.starts_with("http://") && !settings.filebase_endpoint.starts_with("https://") {
        return Err("Filebase endpoint must start with http:// or https://".to_string());
    }

    let existing = load_provider_settings().unwrap_or_default();
    let mut to_save = settings.clone();

    if vault::vault_exists() {
        let tokens_changed = token_fields_changed(&settings, &existing);
        if tokens_changed {
            if let Some(passphrase) = get_cached_passphrase() {
                vault::update_vault_tokens(
                    &passphrase,
                    &settings.pinata_api_token,
                    &settings.filebase_token,
                    &settings.pinata_api_url,
                    &settings.filebase_endpoint,
                )
                    .map_err(|e| format!("Failed to update vault tokens: {e}"))?;
                to_save.pinata_api_token = String::new();
                to_save.filebase_token = String::new();
            } else {
                return Err("Provider token vault is locked. Unlock it in IPFS settings before saving tokens.".to_string());
            }
        }
    }

    save_provider_settings_atomic(&to_save)?;

    let mut result = to_save.clone();
    if vault::vault_exists() {
        result.pinata_api_token = "[in vault]".to_string();
        result.filebase_token = "[in vault]".to_string();
    }
    Ok(mask_secrets(&result))
}

#[tauri::command]
pub fn ipfs_migrate_provider_tokens_to_vault(passphrase: String) -> Result<serde_json::Value, String> {
    if !vault::vault_exists() {
        return Err("Vault does not exist. Set up a vault first.".to_string());
    }
    let valid = vault::verify_vault_passphrase(&passphrase)?;
    if !valid {
        return Err("Incorrect vault passphrase.".to_string());
    }

    let settings = load_provider_settings()?;
    let pinata = settings.pinata_api_token.clone();
    let filebase = settings.filebase_token.clone();

    if pinata.is_empty() && filebase.is_empty() {
        return Ok(serde_json::json!({
            "migrated": false,
            "message": "No plaintext tokens to migrate.",
        }));
    }

    vault::update_vault_tokens(
        &passphrase,
        &pinata,
        &filebase,
        &settings.pinata_api_url,
        &settings.filebase_endpoint,
    )?;

    let mut cleaned = settings;
    cleaned.pinata_api_token = String::new();
    cleaned.filebase_token = String::new();
    match save_provider_settings_atomic(&cleaned) {
        Ok(()) => {
            set_vault_passphrase(passphrase);
            Ok(serde_json::json!({
                "migrated": true,
                "message": "Provider tokens migrated to encrypted vault. Plaintext JSON fields cleared.",
            }))
        }
        Err(e) => Err(format!(
            "Tokens were encrypted in the vault, but clearing plaintext settings failed: {e}. Your tokens are safe but the plaintext settings file still contains them. You may need to manually clear the token fields in IPFS Settings.",
        )),
    }
}

#[tauri::command]
pub fn ipfs_unlock_vault(passphrase: String) -> Result<bool, String> {
    if !vault::vault_exists() {
        return Err("Vault does not exist. Set up a vault first.".to_string());
    }
    let valid = vault::verify_vault_passphrase(&passphrase)?;
    if valid {
        set_vault_passphrase(passphrase);
    }
    Ok(valid)
}

#[tauri::command]
pub fn ipfs_lock_vault() -> Result<(), String> {
    clear_vault_passphrase();
    Ok(())
}

#[tauri::command]
pub fn ipfs_vault_status() -> Result<serde_json::Value, String> {
    let exists = vault::vault_exists();
    let unlocked = get_cached_passphrase().is_some();
    let has_plaintext = has_plaintext_tokens();
    let info = vault::vault_get_info()?;
    let vault_path = vault::vault_get_vault_path().unwrap_or_default();
    Ok(serde_json::json!({
        "exists": exists,
        "unlocked": unlocked,
        "has_plaintext_tokens": has_plaintext,
        "vault_path": vault_path,
        "info": info,
    }))
}

#[tauri::command]
pub fn ipfs_vault_provider_status() -> Result<serde_json::Value, String> {
    let passphrase = get_cached_passphrase()
        .ok_or("Vault is locked. Unlock it first.")?;
    vault::check_provider_token_records(&passphrase)
}

#[tauri::command]
pub fn ipfs_vault_import_bundle_replace(path: String, passphrase: Option<String>) -> Result<serde_json::Value, String> {
    let result = vault::import_bundle_replace_from_path(&path, passphrase.as_deref())?;
    clear_vault_passphrase();
    Ok(result)
}

#[tauri::command]
pub fn ipfs_vault_remove_provider_token(provider_id: String) -> Result<serde_json::Value, String> {
    let passphrase = get_cached_passphrase()
        .ok_or("Vault is locked. Unlock it first.")?;
    let record_id = match provider_id.as_str() {
        "pinata" => "provider.pinata.api_token",
        "filebase" => "provider.filebase.token",
        other => return Err(format!("Unknown provider id: {other}")),
    };
    vault::remove_provider_token_from_vault(&passphrase, record_id)?;
    Ok(serde_json::json!({
        "removed": true,
        "provider_id": provider_id,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masked_secret_detection_rejects_real_tokens_with_dots() {
        assert!(!is_masked_secret("real.token.with.dots.but.long"));
        assert!(!is_masked_secret(""));
        assert!(!is_masked_secret("short"));
    }

    #[test]
    fn masked_secret_detection_accepts_masked_pattern() {
        assert!(is_masked_secret("abcd...wxyz"));
        assert!(is_masked_secret("12ab...89xy"));
    }

    #[test]
    fn vault_placeholder_detection() {
        assert!(is_vault_placeholder("[in vault]"));
        assert!(!is_vault_placeholder("real-token"));
        assert!(!is_vault_placeholder(""));
    }

    #[test]
    fn real_token_detection() {
        assert!(is_real_token_value("real-jwt-token-value"));
        assert!(!is_real_token_value(""));
        assert!(!is_real_token_value("[in vault]"));
        assert!(!is_real_token_value("abcd...wxyz"));
        assert!(is_real_token_value("real.token.with.dots.but.long"));
    }
}
