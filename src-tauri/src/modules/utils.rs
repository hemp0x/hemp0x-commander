use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub fn bin_name(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

fn target_triple() -> &'static str {
    if cfg!(target_os = "windows") {
        "x86_64-pc-windows-msvc"
    } else {
        "x86_64-unknown-linux-gnu"
    }
}

pub fn add_bin_candidates(candidates: &mut Vec<PathBuf>, base: PathBuf, name: &str, depth: usize) {
    let suffixed_name = format!("{}-{}", name, target_triple());

    let mut current = Some(base);
    for _ in 0..=depth {
        if let Some(path) = current {
            candidates.push(path.join(bin_name(name)));
            // Also check for Tauri sidecar style
            if cfg!(unix) {
                candidates.push(path.join(&suffixed_name));
            }
            candidates.push(path.join("binaries").join(bin_name(name)));
            if cfg!(unix) {
                candidates.push(path.join("binaries").join(&suffixed_name));
            }
            current = path.parent().map(|p| p.to_path_buf());
        } else {
            break;
        }
    }
}

pub fn resolve_bin(name: &str) -> String {
    resolve_bin_with_override(name, None)
}

pub fn resolve_bin_with_override(name: &str, override_dir: Option<&str>) -> String {
    if let Some(dir) = override_dir {
        let p = PathBuf::from(dir).join(bin_name(name));
        if p.exists() {
            return p.to_string_lossy().to_string();
        }
    }

    let suffixed_name = format!("{}-{}", name, target_triple());

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            if cfg!(unix) {
                let candidate = dir.join(&suffixed_name);
                if candidate.exists() {
                    return candidate.to_string_lossy().to_string();
                }
                let resources = dir.join("resources").join(&suffixed_name);
                if resources.exists() {
                    return resources.to_string_lossy().to_string();
                }
            }
            let candidate = dir.join(bin_name(name));
            if candidate.exists() {
                return candidate.to_string_lossy().to_string();
            }

            let resources = dir.join("resources").join(bin_name(name));
            if resources.exists() {
                return resources.to_string_lossy().to_string();
            }
        }
    }

    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        add_bin_candidates(&mut candidates, cwd, name, 4);
    }

    // NOTE: CARGO_MANIFEST_DIR is compile time check, might need passing in or logic change if not available in library?

    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| String::from("."));
    add_bin_candidates(&mut candidates, PathBuf::from(manifest), name, 5);

    if !cfg!(windows) {
        if let Some(home) = dirs::home_dir() {
            let candidate = home
                .join("hemp0x-deploy")
                .join("hemp0x-core")
                .join("src")
                .join(bin_name(name));
            candidates.push(candidate);
        }
    }

    for candidate in candidates {
        if candidate.exists() {
            return candidate.to_string_lossy().to_string();
        }
    }

    name.to_string()
}

pub fn split_args(input: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut quote_char = '\0';
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if in_quotes {
            if ch == quote_char {
                in_quotes = false;
            } else if ch == '\\' {
                if let Some(next) = chars.next() {
                    current.push(next);
                }
            } else {
                current.push(ch);
            }
        } else if ch == '"' || ch == '\'' {
            in_quotes = true;
            quote_char = ch;
        } else if ch.is_whitespace() {
            if !current.is_empty() {
                args.push(current.clone());
                current.clear();
            }
        } else {
            current.push(ch);
        }
    }
    if !current.is_empty() {
        args.push(current);
    }
    args
}

pub fn parse_balances(value: &serde_json::Value, map: &mut HashMap<String, f64>) {
    if let Some(arr) = value.as_array() {
        for item in arr {
            if let Some(row) = item.as_array() {
                if row.len() >= 2 {
                    if let (Some(addr), Some(amount)) = (row[0].as_str(), row[1].as_f64()) {
                        map.insert(addr.to_string(), amount);
                    }
                }
                parse_balances(item, map);
            }
        }
    }
}

pub fn calculate_dir_size(path: &Path) -> u64 {
    let mut total = 0u64;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() {
                total += std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
            } else if p.is_dir() {
                total += calculate_dir_size(&p);
            }
        }
    }
    total
}

pub fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} bytes", bytes)
    }
}

const MIN_VERSION: (u32, u32, u32) = (4, 7, 0);

pub fn parse_version(subver: &str) -> Option<(u32, u32, u32)> {
    let stripped = subver.trim_matches('/');
    if !stripped.to_lowercase().contains("hemp0x") {
        return None;
    }

    for token in stripped.split(|c: char| !(c.is_ascii_digit() || c == '.')) {
        if token.matches('.').count() < 2 {
            continue;
        }
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() >= 3
            && parts[0].chars().all(|c| c.is_ascii_digit())
            && parts[1].chars().all(|c| c.is_ascii_digit())
            && parts[2].chars().all(|c| c.is_ascii_digit())
        {
            let major = parts[0].parse().ok()?;
            let minor = parts[1].parse().ok()?;
            let patch = parts[2].parse().ok()?;
            return Some((major, minor, patch));
        }
    }
    None
}

pub fn version_is_old(subver: &str) -> bool {
    if let Some((major, minor, patch)) = parse_version(subver) {
        if major < MIN_VERSION.0 {
            return true;
        }
        if major > MIN_VERSION.0 {
            return false;
        }
        if minor < MIN_VERSION.1 {
            return true;
        }
        if minor > MIN_VERSION.1 {
            return false;
        }
        if patch < MIN_VERSION.2 {
            return true;
        }
        return false;
    }
    true
}

// ─── Core wallet filename validation ──────────────────────────────────────
//
// Any user-entered wallet name that becomes a Core `-wallet=<name>` filename
// must pass through `validate_core_wallet_filename`. This is the single
// shared backend gate used by every vault-wallet flow (recovery phrase
// restore, create vault wallet, guided connect, backup-record restore, and
// the migration restore path).

/// Maximum length for a user-entered wallet name that becomes a Core
/// `-wallet=<name>` filename.
pub const CORE_WALLET_NAME_MAX_LEN: usize = 64;

/// Validate a user-entered wallet name before it is used as a Core
/// `-wallet=<name>` filename.
///
/// Rules:
/// - Non-empty after trimming.
/// - Only ASCII letters, digits, `_`, and `-`. No spaces, no `.`, no path
///   separators (`/`, `\`), no drive separators (`:`), no control/glob
///   characters.
/// - Not `.` or `..`.
/// - At most [`CORE_WALLET_NAME_MAX_LEN`] characters.
/// - Not a reserved Windows device name (CON, PRN, AUX, NUL, COM1.., LPT1..).
///
/// Returns the trimmed name on success so callers always store the
/// canonical form.
pub fn validate_core_wallet_filename(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("Wallet name is required".to_string());
    }
    if trimmed == "." || trimmed == ".." {
        return Err("Wallet name cannot be '.' or '..'".to_string());
    }
    if trimmed.contains('/') || trimmed.contains('\\') || trimmed.contains(':') {
        return Err("Wallet name cannot contain path separators or drive separators".to_string());
    }
    if trimmed
        .chars()
        .any(|c| !c.is_ascii_alphanumeric() && c != '_' && c != '-')
    {
        return Err(
            "Wallet name can only use letters, numbers, hyphen, and underscore.".to_string(),
        );
    }
    if trimmed.len() > CORE_WALLET_NAME_MAX_LEN {
        return Err(format!(
            "Wallet name must be at most {} characters",
            CORE_WALLET_NAME_MAX_LEN
        ));
    }
    let upper = trimmed.to_uppercase();
    let reserved = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    if reserved.contains(&upper.as_str()) {
        return Err("Wallet name cannot be a reserved device name".to_string());
    }
    Ok(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_version_accepts_common_hemp0x_subver_formats() {
        assert_eq!(parse_version("/Hemp0x:4.7.0/"), Some((4, 7, 0)));
        assert_eq!(parse_version("/Hemp0x:4.8.0.0/"), Some((4, 8, 0)));
        assert_eq!(parse_version("/Hemp0x Core:4.7.1/"), Some((4, 7, 1)));
    }

    #[test]
    fn version_is_old_rejects_old_or_non_hemp0x_peers() {
        assert!(version_is_old("/Hemp0x:4.6.9/"));
        assert!(version_is_old("/Satoshi:4.7.0/"));
        assert!(!version_is_old("/Hemp0x:4.7.0/"));
        assert!(!version_is_old("/Hemp0x Core:4.8.0/"));
    }

    #[test]
    fn core_wallet_filename_accepts_valid_names() {
        assert_eq!(
            validate_core_wallet_filename("test-recovery-wallet").unwrap(),
            "test-recovery-wallet"
        );
        assert_eq!(
            validate_core_wallet_filename("test_recovery_wallet").unwrap(),
            "test_recovery_wallet"
        );
        assert_eq!(validate_core_wallet_filename("Main123").unwrap(), "Main123");
        assert_eq!(
            validate_core_wallet_filename("  hemp0x-vault-main  ").unwrap(),
            "hemp0x-vault-main"
        );
    }

    #[test]
    fn core_wallet_filename_rejects_spaces_and_path_separators() {
        assert!(validate_core_wallet_filename("New Wallet").is_err());
        assert!(validate_core_wallet_filename("bad/name").is_err());
        assert!(validate_core_wallet_filename("bad\\name").is_err());
        assert!(validate_core_wallet_filename("C:wallet").is_err());
        assert!(validate_core_wallet_filename("wallet.dat").is_err());
        assert!(validate_core_wallet_filename("./wallet").is_err());
        assert!(validate_core_wallet_filename("../wallet").is_err());
        assert!(validate_core_wallet_filename(".").is_err());
        assert!(validate_core_wallet_filename("..").is_err());
    }

    #[test]
    fn core_wallet_filename_rejects_empty_and_garbage() {
        assert!(validate_core_wallet_filename("").is_err());
        assert!(validate_core_wallet_filename("   ").is_err());
        assert!(validate_core_wallet_filename("wallet?name").is_err());
        assert!(validate_core_wallet_filename("wallet*name").is_err());
        assert!(validate_core_wallet_filename("wallet\nname").is_err());
    }

    #[test]
    fn core_wallet_filename_rejects_reserved_and_overlong() {
        assert!(validate_core_wallet_filename("CON").is_err());
        assert!(validate_core_wallet_filename("nul").is_err());
        assert!(validate_core_wallet_filename("LPT1").is_err());
        let long = "a".repeat(CORE_WALLET_NAME_MAX_LEN + 1);
        assert!(validate_core_wallet_filename(&long).is_err());
        let max = "a".repeat(CORE_WALLET_NAME_MAX_LEN);
        assert!(validate_core_wallet_filename(&max).is_ok());
    }
}
