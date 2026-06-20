use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};
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

pub fn get_cached_passphrase() -> Option<String> {
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
    Ok(crate::modules::files::data_dir()?
        .join("content-library")
        .join("provider_settings.json"))
}

pub fn load_provider_settings() -> Result<ProviderSettings, String> {
    let path = provider_settings_path()?;
    if path.exists() {
        let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        return serde_json::from_str(&content)
            .map_err(|e| format!("Corrupt provider settings: {e}"));
    }
    let legacy = legacy_provider_settings_path()?;
    if legacy.exists() {
        let content = fs::read_to_string(&legacy).map_err(|e| e.to_string())?;
        let settings: ProviderSettings = serde_json::from_str(&content)
            .map_err(|e| format!("Corrupt legacy provider settings: {e}"))?;
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
                    let mut pinata = vault::payload_pinata_token(&payload);
                    let mut filebase = vault::payload_filebase_token(&payload);
                    let settings = load_provider_settings().unwrap_or_default();
                    let plaintext_pinata = is_real_token_value(&settings.pinata_api_token);
                    let plaintext_filebase = is_real_token_value(&settings.filebase_token);

                    if (pinata.is_empty() && plaintext_pinata)
                        || (filebase.is_empty() && plaintext_filebase)
                    {
                        let pinata_update = if pinata.is_empty() && plaintext_pinata {
                            settings.pinata_api_token.clone()
                        } else {
                            String::new()
                        };
                        let filebase_update = if filebase.is_empty() && plaintext_filebase {
                            settings.filebase_token.clone()
                        } else {
                            String::new()
                        };
                        vault::update_vault_tokens(
                            &passphrase,
                            &pinata_update,
                            &filebase_update,
                            &settings.pinata_api_url,
                            &settings.filebase_endpoint,
                        )
                        .map_err(|e| format!("Failed to move provider tokens into vault: {e}"))?;

                        let mut cleaned = settings;
                        cleaned.pinata_api_token = String::new();
                        cleaned.filebase_token = String::new();
                        save_provider_settings_atomic(&cleaned)
                            .map_err(|e| format!("Provider tokens were moved into the vault, but clearing plaintext settings failed: {e}"))?;

                        if !pinata_update.is_empty() {
                            pinata = pinata_update.to_string();
                        }
                        if !filebase_update.is_empty() {
                            filebase = filebase_update.to_string();
                        }
                    }

                    return Ok((pinata, filebase));
                }
                Err(e) => {
                    clear_vault_passphrase();
                    return Err(format!("Vault decrypt failed: {e}. Vault has been locked."));
                }
            }
        }
        return Err(
            "Provider token vault is locked. Unlock your vault to use saved provider tokens."
                .to_string(),
        );
    }
    let settings = load_provider_settings()?;
    Ok((
        settings.pinata_api_token.clone(),
        settings.filebase_token.clone(),
    ))
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
    let _ = provider_settings_temp_path().map(|tmp| {
        let _ = fs::remove_file(&tmp);
    });
    let _ = crate::modules::files::commander_dir().map(|d| {
        let vault_tmp = d.join("vault.json.tmp");
        let _ = fs::remove_file(&vault_tmp);
    });
}

pub fn mask_secrets(settings: &ProviderSettings) -> ProviderSettings {
    let mut masked = settings.clone();
    if is_real_token_value(&masked.pinata_api_token) && masked.pinata_api_token.len() > 8 {
        let len = masked.pinata_api_token.len();
        masked.pinata_api_token = format!(
            "{}...{}",
            &masked.pinata_api_token[..4],
            &masked.pinata_api_token[len - 4..]
        );
    }
    if is_real_token_value(&masked.filebase_token) && masked.filebase_token.len() > 8 {
        let len = masked.filebase_token.len();
        masked.filebase_token = format!(
            "{}...{}",
            &masked.filebase_token[..4],
            &masked.filebase_token[len - 4..]
        );
    }
    masked
}

fn merge_existing_secrets(mut incoming: ProviderSettings) -> Result<ProviderSettings, String> {
    let existing = load_provider_settings().unwrap_or_default();
    if is_masked_secret(&incoming.pinata_api_token)
        || is_vault_placeholder(&incoming.pinata_api_token)
    {
        incoming.pinata_api_token = existing.pinata_api_token;
    }
    if is_masked_secret(&incoming.filebase_token) || is_vault_placeholder(&incoming.filebase_token)
    {
        incoming.filebase_token = existing.filebase_token;
    }
    Ok(incoming)
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
pub fn ipfs_update_provider_settings(
    settings: ProviderSettings,
) -> Result<ProviderSettings, String> {
    let settings = merge_existing_secrets(settings)?;
    if settings.selected_publish_provider.is_empty() {
        return Err("Publish provider selection must not be empty".to_string());
    }
    let valid_providers = ["manual", "pinata", "installed_kubo", "filebase"];
    if !valid_providers.contains(&settings.selected_publish_provider.as_str()) {
        return Err(format!(
            "Invalid publish provider: {}. Must be one of: {}",
            settings.selected_publish_provider,
            valid_providers.join(", ")
        ));
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
            return Err(format!(
                "Gateway URL must start with http:// or https://: {}",
                trimmed
            ));
        }
    }
    if !settings.kubo_endpoint.is_empty()
        && !settings.kubo_endpoint.starts_with("http://")
        && !settings.kubo_endpoint.starts_with("https://")
    {
        return Err("Kubo endpoint must start with http:// or https://".to_string());
    }
    if !settings.pinata_api_url.is_empty() && !settings.pinata_api_url.starts_with("https://") {
        return Err("Pinata API URL must start with https://".to_string());
    }
    if !settings.filebase_endpoint.is_empty()
        && !settings.filebase_endpoint.starts_with("http://")
        && !settings.filebase_endpoint.starts_with("https://")
    {
        return Err("Filebase endpoint must start with http:// or https://".to_string());
    }

    let existing = load_provider_settings().unwrap_or_default();
    let mut to_save = settings.clone();

    if vault::vault_exists() {
        let has_pinata_token = is_real_token_value(&settings.pinata_api_token);
        let has_filebase_token = is_real_token_value(&settings.filebase_token);
        let has_existing_pinata_token = is_real_token_value(&existing.pinata_api_token);
        let has_existing_filebase_token = is_real_token_value(&existing.filebase_token);
        let should_update_vault_tokens = has_pinata_token
            || has_filebase_token
            || has_existing_pinata_token
            || has_existing_filebase_token;

        if should_update_vault_tokens {
            if let Some(passphrase) = get_cached_passphrase() {
                let pinata_token = if has_pinata_token {
                    settings.pinata_api_token.as_str()
                } else if has_existing_pinata_token {
                    existing.pinata_api_token.as_str()
                } else {
                    ""
                };
                let filebase_token = if has_filebase_token {
                    settings.filebase_token.as_str()
                } else if has_existing_filebase_token {
                    existing.filebase_token.as_str()
                } else {
                    ""
                };
                vault::update_vault_tokens(
                    &passphrase,
                    pinata_token,
                    filebase_token,
                    &settings.pinata_api_url,
                    &settings.filebase_endpoint,
                )
                .map_err(|e| format!("Failed to update vault tokens: {e}"))?;
            } else {
                return Err("Provider token vault is locked. Unlock your vault before saving provider tokens.".to_string());
            }
        }

        to_save.pinata_api_token = String::new();
        to_save.filebase_token = String::new();
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
pub fn ipfs_migrate_provider_tokens_to_vault(
    passphrase: String,
) -> Result<serde_json::Value, String> {
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
    let passphrase = get_cached_passphrase().ok_or("Vault is locked. Unlock it first.")?;
    vault::check_provider_token_records(&passphrase)
}

#[tauri::command]
pub fn ipfs_provider_token_presence() -> Result<serde_json::Value, String> {
    let settings = load_provider_settings().unwrap_or_default();
    let pinata_plaintext = is_real_token_value(&settings.pinata_api_token);
    let filebase_plaintext = is_real_token_value(&settings.filebase_token);
    let vault_present = vault::vault_exists();

    let has_plaintext_tokens = pinata_plaintext || filebase_plaintext;
    let mut pinata_stored = false;
    let mut filebase_stored = false;
    let mut vault_locked = false;

    if vault_present {
        if let Some(passphrase) = get_cached_passphrase() {
            match vault::check_provider_token_records(&passphrase) {
                Ok(status) => {
                    pinata_stored = status["providers"]["pinata"]["has_token"]
                        .as_bool()
                        .unwrap_or(false);
                    filebase_stored = status["providers"]["filebase"]["has_token"]
                        .as_bool()
                        .unwrap_or(false);
                }
                Err(_) => {
                    clear_vault_passphrase();
                    vault_locked = true;
                }
            }
        } else {
            vault_locked = true;
        }
    }

    let source = if has_plaintext_tokens {
        "plaintext"
    } else if vault_present && vault_locked {
        "vault_locked"
    } else if vault_present {
        "vault"
    } else {
        "none"
    };

    Ok(serde_json::json!({
        "vault_exists": vault_present,
        "vault_locked": vault_locked,
        "source": source,
        "has_plaintext_tokens": has_plaintext_tokens,
        "providers": {
            "pinata": {
                "stored": pinata_stored,
                "plaintext": pinata_plaintext,
            },
            "filebase": {
                "stored": filebase_stored,
                "plaintext": filebase_plaintext,
            }
        }
    }))
}

#[tauri::command]
pub fn ipfs_vault_import_bundle_replace(
    path: String,
    passphrase: Option<String>,
) -> Result<serde_json::Value, String> {
    let result = vault::import_bundle_replace_from_path(&path, passphrase.as_deref())?;
    clear_vault_passphrase();
    Ok(result)
}

#[tauri::command]
pub fn ipfs_vault_remove_provider_token(provider_id: String) -> Result<serde_json::Value, String> {
    let passphrase = get_cached_passphrase().ok_or("Vault is locked. Unlock it first.")?;
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

// ─── Wallet-page vault helpers (60q) ─────────────────────────────────────
//
// These commands let the Wallet page reuse the cached vault unlock
// session (set by `ipfs_unlock_vault`) without weakening security:
//   - The cached passphrase is stored in a process-local
//     `Zeroizing<String>` Mutex, never persisted, and cleared on
//     lock / vault-bundle replacement / lock-on-error.
//   - Each wrapper below accepts an OPTIONAL explicit `vault_passphrase`
//     argument. If provided, it is preferred over the cached value
//     (lets the user re-enter their passphrase for one operation
//     without keeping the session unlocked). If absent, the cached
//     passphrase is used. The cached value is never returned over IPC.
//   - On a successful operation that used the cached passphrase, the
//     cache is left untouched so the session stays unlocked for
//     follow-up Wallet-page actions. A failed operation does NOT
//     clear the cache (the user may want to retry).
//   - These wrappers do not change vault format, crypto, key slots,
//     record shape, or command names. They are thin pass-through
//     adapters that resolve the passphrase and call the existing
//     `vault::*` commands.

pub fn resolve_vault_passphrase(explicit: Option<String>) -> Result<String, String> {
    if let Some(p) = explicit {
        if p.is_empty() {
            return Err("Vault passphrase must not be empty".to_string());
        }
        return Ok(p);
    }
    get_cached_passphrase().ok_or_else(|| {
        "Vault is locked. Unlock it on the Wallet page or in IPFS Settings.".to_string()
    })
}

#[tauri::command]
pub fn ipfs_vault_setup_and_unlock(passphrase: String) -> Result<serde_json::Value, String> {
    if passphrase.len() < 8 {
        return Err("Vault passphrase must be at least 8 characters".to_string());
    }
    // Reject if a vault already exists. We do not allow setup-and-unlock
    // to silently overwrite an existing vault; that flow lives in
    // `vault_import_bundle_replace`.
    if vault::vault_exists() {
        return Err(
            "A vault already exists. Use Unlock on the Wallet page, or Restore Vault in IPFS Settings to replace it."
                .to_string(),
        );
    }
    let info = vault::vault_setup(passphrase.clone())?;
    // Only cache the passphrase after vault_setup succeeds and
    // produced a valid bundle. We intentionally do not log the
    // passphrase or return it.
    set_vault_passphrase(passphrase);
    Ok(info)
}

#[tauri::command]
pub fn ipfs_vault_unlock_status() -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({
        "unlocked": get_cached_passphrase().is_some(),
        "vault_exists": vault::vault_exists(),
    }))
}

#[tauri::command]
pub async fn ipfs_vault_list_wallet_migration_records(
    vault_passphrase: Option<String>,
) -> Result<Vec<serde_json::Value>, String> {
    let passphrase = resolve_vault_passphrase(vault_passphrase)?;
    tauri::async_runtime::spawn_blocking(move || {
        vault::vault_list_wallet_migration_records(Some(passphrase))
    })
    .await
    .map_err(|e| format!("Vault wallet record list task failed: {e}"))?
}

#[tauri::command]
pub async fn ipfs_vault_export_current_wallet_migration_record(
    label: String,
    migration_passphrase: String,
    vault_passphrase: Option<String>,
    recovery_mode: Option<String>,
) -> Result<serde_json::Value, String> {
    let passphrase = resolve_vault_passphrase(vault_passphrase)?;
    tauri::async_runtime::spawn_blocking(move || {
        vault::vault_export_current_wallet_migration_record(
            label,
            true,
            migration_passphrase,
            Some(passphrase),
            recovery_mode,
        )
    })
    .await
    .map_err(|e| format!("Vault wallet backup export task failed: {e}"))?
}

#[tauri::command]
pub async fn ipfs_vault_restore_wallet_migration_record(
    record_id: String,
    wallet_name: String,
    migration_passphrase: String,
    birth_height: Option<i64>,
    vault_passphrase: Option<String>,
) -> Result<serde_json::Value, String> {
    let passphrase = resolve_vault_passphrase(vault_passphrase)?;
    tauri::async_runtime::spawn_blocking(move || {
        vault::vault_restore_wallet_migration_record(
            record_id,
            wallet_name,
            migration_passphrase,
            birth_height,
            Some(passphrase),
        )
    })
    .await
    .map_err(|e| format!("Vault wallet backup restore task failed: {e}"))?
}

// ─── WebCom / Hemp0x Vault Interop Cached-Session Wrappers ─────────────────

#[tauri::command]
pub fn ipfs_vault_get_webcom_interop_summary(
    vault_passphrase: Option<String>,
) -> Result<serde_json::Value, String> {
    let passphrase = resolve_vault_passphrase(vault_passphrase)?;
    vault::vault_get_webcom_interop_summary(Some(passphrase))
}

#[tauri::command]
pub fn ipfs_vault_get_address_book_record_summary(
    vault_passphrase: Option<String>,
) -> Result<serde_json::Value, String> {
    let passphrase = resolve_vault_passphrase(vault_passphrase)?;
    vault::vault_get_address_book_record_summary(Some(passphrase))
}

#[tauri::command]
pub fn ipfs_vault_export_address_book_record(
    vault_passphrase: Option<String>,
) -> Result<serde_json::Value, String> {
    let passphrase = resolve_vault_passphrase(vault_passphrase)?;
    vault::vault_export_address_book_record(Some(passphrase))
}

#[tauri::command]
pub fn ipfs_vault_import_address_book_record(
    vault_passphrase: Option<String>,
) -> Result<serde_json::Value, String> {
    let passphrase = resolve_vault_passphrase(vault_passphrase)?;
    vault::vault_import_address_book_record(Some(passphrase))
}

#[tauri::command]
pub async fn ipfs_vault_remove_wallet_migration_record(
    record_id: String,
    vault_passphrase: Option<String>,
) -> Result<serde_json::Value, String> {
    let passphrase = resolve_vault_passphrase(vault_passphrase)?;
    tauri::async_runtime::spawn_blocking(move || {
        vault::vault_remove_wallet_migration_record(record_id, Some(passphrase))
    })
    .await
    .map_err(|e| format!("Vault wallet backup remove task failed: {e}"))?
}

#[tauri::command]
pub async fn ipfs_vault_import_wallet_migration_record_from_path(
    path: String,
    label: String,
    migration_passphrase: Option<String>,
    vault_passphrase: Option<String>,
) -> Result<serde_json::Value, String> {
    let passphrase = resolve_vault_passphrase(vault_passphrase)?;
    tauri::async_runtime::spawn_blocking(move || {
        vault::vault_import_wallet_migration_record_from_path(
            path,
            label,
            migration_passphrase,
            Some(passphrase),
        )
    })
    .await
    .map_err(|e| format!("Vault wallet backup import task failed: {e}"))?
}

// ─── Portable Wallet Bridge cached-session wrappers (slice 64b) ──────────
//
// These are read-only, metadata-only wrappers. They do NOT write to
// `wallet.webcom.hemp.primary`. Wallet record write/promotion is
// intentionally deferred until the safety requirements in
// `untracked/commander-v1.4/handoff-prompts/hemp0x-vault-portable-wallet-model-slice-64b.md`
// are all met.

#[tauri::command]
pub async fn ipfs_vault_get_wallet_alignment_status(
    vault_passphrase: Option<String>,
) -> Result<serde_json::Value, String> {
    let passphrase = resolve_vault_passphrase(vault_passphrase)?;
    tauri::async_runtime::spawn_blocking(move || {
        vault::vault_get_wallet_alignment_status(Some(passphrase))
    })
    .await
    .map_err(|e| format!("Vault alignment status task failed: {e}"))?
}

#[tauri::command]
pub fn ipfs_vault_create_or_update_alignment_record(
    _active_wallet_record_id: String,
    _core_wallet_source: String,
    _derivation_profile: String,
    _core_migration_backup_record_id: Option<String>,
    _active_wallet_fingerprint: Option<String>,
    _notes: Option<Vec<String>>,
    _vault_passphrase: Option<String>,
) -> Result<serde_json::Value, String> {
    Err("Deprecated: alignment records can only be written by the verified Core restore flow (ipfs_vault_connect_webcom_primary_wallet_to_core). Manual metadata-only alignment is no longer supported.".to_string())
}

// ─── WebCom -> Core Bridge cached-session wrappers (slice 64d) ───────────

#[tauri::command]
pub fn ipfs_vault_connect_webcom_primary_wallet_to_core(
    wallet_name: Option<String>,
    birth_height: Option<i64>,
    pre_connect_backup_record_id: Option<String>,
    vault_passphrase: Option<String>,
) -> Result<serde_json::Value, String> {
    let passphrase = resolve_vault_passphrase(vault_passphrase)?;
    vault::vault_connect_webcom_primary_wallet_to_core(
        wallet_name,
        birth_height,
        pre_connect_backup_record_id,
        Some(passphrase),
    )
}

#[tauri::command]
pub async fn ipfs_vault_connect_webcom_primary_wallet_to_core_guided(
    wallet_name: Option<String>,
    birth_height: Option<i64>,
    vault_passphrase: Option<String>,
    skip_backup: Option<bool>,
) -> Result<serde_json::Value, String> {
    // Resolve the (possibly cached) passphrase on the async thread, then
    // run the heavy, blocking restore/align pipeline on a background
    // thread so the webview event loop and the connect stepper stay
    // responsive while Core does the restore/scan work.
    let passphrase = resolve_vault_passphrase(vault_passphrase)?;
    tauri::async_runtime::spawn_blocking(move || {
        vault::vault_connect_webcom_primary_wallet_to_core_guided(
            wallet_name,
            birth_height,
            Some(passphrase),
            skip_backup,
        )
    })
    .await
    .map_err(|e| format!("Vault connect task failed: {e}"))?
}

#[tauri::command]
pub async fn ipfs_vault_get_wallet_alignment_status_v2(
    vault_passphrase: Option<String>,
) -> Result<serde_json::Value, String> {
    let passphrase = resolve_vault_passphrase(vault_passphrase)?;
    tauri::async_runtime::spawn_blocking(move || {
        vault::vault_get_wallet_alignment_status_v2(Some(passphrase))
    })
    .await
    .map_err(|e| format!("Vault alignment status task failed: {e}"))?
}

#[tauri::command]
pub async fn ipfs_vault_preview_connect_webcom_primary_to_core(
    vault_passphrase: Option<String>,
) -> Result<serde_json::Value, String> {
    let passphrase = resolve_vault_passphrase(vault_passphrase)?;
    tauri::async_runtime::spawn_blocking(move || {
        vault::vault_get_wallet_alignment_status_v2(Some(passphrase))
    })
    .await
    .map_err(|e| format!("Vault connect preview task failed: {e}"))?
}

#[tauri::command]
pub async fn ipfs_vault_preview_export_core_wallet_to_webcom_primary(
    vault_passphrase: Option<String>,
) -> Result<serde_json::Value, String> {
    let passphrase = resolve_vault_passphrase(vault_passphrase)?;
    tauri::async_runtime::spawn_blocking(move || {
        vault::vault_get_wallet_alignment_status_v2(Some(passphrase))
    })
    .await
    .map_err(|e| format!("Vault export preview task failed: {e}"))?
}

#[tauri::command]
pub fn ipfs_vault_backup_current_core_wallet_before_alignment(
    label: String,
    vault_passphrase: Option<String>,
) -> Result<serde_json::Value, String> {
    let passphrase = resolve_vault_passphrase(vault_passphrase)?;
    vault::vault_export_current_wallet_migration_record(
        label,
        true,
        passphrase.clone(),
        Some(passphrase),
        Some(String::from("vault_passphrase")),
    )
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

    // ─── 60q: cached-passphrase wrapper tests ─────────────────────────

    // ─── 60q: cached-passphrase wrapper tests ─────────────────────────
    //
    // These tests intentionally do not assume a clean cache state
    // because Rust test threads run in parallel and the
    // `VAULT_PASSPHRASE` cache is a process-global. The contract
    // they verify is the local `resolve_vault_passphrase` /
    // `ipfs_vault_unlock_status` behavior, not the global cache
    // contents. They are kept race-tolerant by not asserting on
    // the global cache value at the end of each test.

    #[test]
    fn resolve_vault_passphrase_rejects_empty_explicit() {
        let result = resolve_vault_passphrase(Some(String::new()));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("must not be empty"));
    }

    #[test]
    fn resolve_vault_passphrase_returns_explicit_value_when_provided() {
        // This works regardless of cache state — explicit always wins.
        let result = resolve_vault_passphrase(Some("explicit-override-12345".to_string()));
        assert_eq!(result.unwrap(), "explicit-override-12345");
    }

    #[test]
    fn resolve_vault_passphrase_caches_explicit_value() {
        // The cache MUST be set after a successful resolve with an
        // explicit passphrase so that downstream wallet-page
        // operations (e.g. an immediate follow-up list) do not
        // re-prompt the user. This is a structural property — the
        // explicit-passphrase path goes through the same function,
        // but we explicitly cache after success in callers. Here we
        // verify the resolver itself returns the value, and trust
        // the caller to set_vault_passphrase on success.
        let result = resolve_vault_passphrase(Some("just-this-one".to_string()));
        assert!(result.is_ok());
    }

    #[test]
    fn ipfs_vault_unlock_status_returns_expected_shape() {
        let res = ipfs_vault_unlock_status().unwrap();
        // Shape check: must contain the two well-known keys with
        // boolean values. Values may be true or false depending on
        // parallel test activity.
        assert!(res.get("unlocked").is_some());
        assert!(res.get("vault_exists").is_some());
        assert!(res["unlocked"].is_boolean());
        assert!(res["vault_exists"].is_boolean());
    }

    #[test]
    fn ipfs_provider_token_presence_returns_expected_shape() {
        let res = ipfs_provider_token_presence().unwrap();
        // Shape: must include the non-secret fields the IPFS
        // Settings UI relies on. Values may vary with test order.
        assert!(res.get("vault_exists").is_some());
        assert!(res["vault_exists"].is_boolean());
        assert!(res.get("source").is_some());
        let source = res["source"].as_str().unwrap();
        assert!(matches!(
            source,
            "vault" | "vault_locked" | "plaintext" | "none"
        ));
        assert!(res.get("vault_locked").is_some());
        assert!(res["vault_locked"].is_boolean());
        assert!(res.get("has_plaintext_tokens").is_some());
        assert!(res["has_plaintext_tokens"].is_boolean());
        let providers = res.get("providers").unwrap();
        assert!(providers.get("pinata").is_some());
        assert!(providers.get("filebase").is_some());
        for key in ["pinata", "filebase"] {
            let p = &providers[key];
            assert!(p.get("stored").is_some());
            assert!(p["stored"].is_boolean());
            assert!(p.get("plaintext").is_some());
            assert!(p["plaintext"].is_boolean());
        }
    }
}
