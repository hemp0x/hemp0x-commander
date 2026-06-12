use std::fs;
use std::path::PathBuf;

use serde::{Serialize, Deserialize};

use crate::modules::content_library::content_library_dir;

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
    trimmed.contains("...") && trimmed.len() >= 7
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

pub fn load_provider_settings() -> Result<ProviderSettings, String> {
    let path = provider_settings_path()?;
    if !path.exists() {
        return Ok(ProviderSettings::default());
    }
    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let settings: ProviderSettings = serde_json::from_str(&content).unwrap_or_default();
    Ok(settings)
}

pub fn viewing_gateways() -> Vec<String> {
    load_provider_settings()
        .map(|settings| settings.gateways.viewing_gateways)
        .unwrap_or_else(|_| ProviderSettings::default().gateways.viewing_gateways)
}

fn save_provider_settings(settings: &ProviderSettings) -> Result<(), String> {
    let path = provider_settings_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let content = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    fs::write(&path, content).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn mask_secrets(settings: &ProviderSettings) -> ProviderSettings {
    let mut masked = settings.clone();
    if !masked.pinata_api_token.is_empty() && masked.pinata_api_token.len() > 8 {
        let len = masked.pinata_api_token.len();
        masked.pinata_api_token = format!("{}...{}", &masked.pinata_api_token[..4], &masked.pinata_api_token[len-4..]);
    }
    if !masked.filebase_token.is_empty() && masked.filebase_token.len() > 8 {
        let len = masked.filebase_token.len();
        masked.filebase_token = format!("{}...{}", &masked.filebase_token[..4], &masked.filebase_token[len-4..]);
    }
    masked
}

fn merge_existing_secrets(mut incoming: ProviderSettings) -> Result<ProviderSettings, String> {
    let existing = load_provider_settings().unwrap_or_default();
    if is_masked_secret(&incoming.pinata_api_token) {
        incoming.pinata_api_token = existing.pinata_api_token;
    }
    if is_masked_secret(&incoming.filebase_token) {
        incoming.filebase_token = existing.filebase_token;
    }
    Ok(incoming)
}

#[tauri::command]
pub fn ipfs_get_provider_settings() -> Result<ProviderSettings, String> {
    let settings = load_provider_settings()?;
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
            return Err(format!("Gateway URL must start with http:// or https://: {}", trimmed));
        }
    }
    if !settings.kubo_endpoint.is_empty() {
        if !settings.kubo_endpoint.starts_with("http://") && !settings.kubo_endpoint.starts_with("https://") {
            return Err("Kubo endpoint must start with http:// or https://".to_string());
        }
    }
    if !settings.pinata_api_url.is_empty() {
        if !settings.pinata_api_url.starts_with("https://") {
            return Err("Pinata API URL must start with https://".to_string());
        }
    }
    if !settings.filebase_endpoint.is_empty() {
        if !settings.filebase_endpoint.starts_with("http://") && !settings.filebase_endpoint.starts_with("https://") {
            return Err("Filebase endpoint must start with http:// or https://".to_string());
        }
    }
    save_provider_settings(&settings)?;
    Ok(mask_secrets(&settings))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_settings_default_is_manual() {
        let settings = ProviderSettings::default();
        assert_eq!(settings.selected_publish_provider, "manual");
        assert!(!settings.gateways.viewing_gateways.is_empty());
    }

    #[test]
    fn provider_settings_serialization_roundtrip() {
        let settings = ProviderSettings {
            selected_publish_provider: "pinata".to_string(),
            gateways: ProviderGateways {
                viewing_gateways: vec![
                    "https://dweb.link/ipfs/".to_string(),
                    "https://ipfs.io/ipfs/".to_string(),
                ],
            },
            pinata_api_token: "test-token".to_string(),
            pinata_api_url: "https://api.pinata.cloud".to_string(),
            kubo_endpoint: "http://127.0.0.1:5001".to_string(),
            filebase_token: String::new(),
            filebase_endpoint: "https://rpc.filebase.io".to_string(),
        };
        let json = serde_json::to_string(&settings).unwrap();
        let parsed: ProviderSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.selected_publish_provider, "pinata");
        assert_eq!(parsed.gateways.viewing_gateways.len(), 2);
        assert_eq!(parsed.pinata_api_token, "test-token");
    }

    #[test]
    fn provider_settings_json_includes_gateways() {
        let settings = ProviderSettings::default();
        let json = serde_json::to_string(&settings).unwrap();
        assert!(json.contains("selected_publish_provider"));
        assert!(json.contains("viewing_gateways"));
        assert!(json.contains("dweb.link"));
    }

    #[test]
    fn mask_secrets_obscures_tokens() {
        let mut settings = ProviderSettings::default();
        settings.pinata_api_token = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.token_payload".to_string();
        settings.filebase_token = "abcd1234verylongtoken".to_string();
        let masked = mask_secrets(&settings);
        assert!(!masked.pinata_api_token.contains("token_payload"));
        assert!(masked.pinata_api_token.contains("..."));
        assert!(!masked.filebase_token.contains("verylong"));
    }

    #[test]
    fn mask_secrets_leaves_short_tokens() {
        let mut settings = ProviderSettings::default();
        settings.pinata_api_token = "short".to_string();
        let masked = mask_secrets(&settings);
        assert_eq!(masked.pinata_api_token, "short");
    }

    #[test]
    fn masked_secret_detection_is_conservative() {
        assert!(is_masked_secret("abcd...wxyz"));
        assert!(!is_masked_secret("short"));
        assert!(!is_masked_secret("real.token.value"));
    }
}
