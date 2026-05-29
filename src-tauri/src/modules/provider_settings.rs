use std::fs;
use std::path::PathBuf;

use serde::{Serialize, Deserialize};

use crate::modules::files::data_dir;

#[derive(Clone, Serialize, Deserialize)]
pub struct ProviderGateways {
    pub viewing_gateways: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ProviderSettings {
    pub selected_publish_provider: String,
    pub gateways: ProviderGateways,
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
        }
    }
}

fn provider_settings_path() -> Result<PathBuf, String> {
    let dir = data_dir()?;
    Ok(dir.join("content-library").join("provider_settings.json"))
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

#[tauri::command]
pub fn ipfs_get_provider_settings() -> Result<ProviderSettings, String> {
    load_provider_settings()
}

#[tauri::command]
pub fn ipfs_update_provider_settings(settings: ProviderSettings) -> Result<ProviderSettings, String> {
    if settings.selected_publish_provider.is_empty() {
        return Err("Publish provider selection must not be empty".to_string());
    }
    let valid_providers = ["manual", "pinata", "web3.storage", "installed_kubo"];
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
    save_provider_settings(&settings)?;
    Ok(settings)
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
        };
        let json = serde_json::to_string(&settings).unwrap();
        let parsed: ProviderSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.selected_publish_provider, "pinata");
        assert_eq!(parsed.gateways.viewing_gateways.len(), 2);
    }

    #[test]
    fn provider_settings_json_includes_gateways() {
        let settings = ProviderSettings::default();
        let json = serde_json::to_string(&settings).unwrap();
        assert!(json.contains("selected_publish_provider"));
        assert!(json.contains("viewing_gateways"));
        assert!(json.contains("dweb.link"));
    }
}
