use std::net::{TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::modules::rpc::rpc_context;
use crate::modules::utils::resolve_bin;

const REQUIRED_CORE_NEXT_COMMIT: &str = "192c6b5ce";
const REQUIRED_CORE_BASE_VERSION: &str = "4.7.0.0";
const DEFAULT_RPC_PORT: u16 = 42068;
const DEFAULT_P2P_PORT: u16 = 42069;

static DAEMON_OWNERSHIP: Mutex<bool> = Mutex::new(false);

#[derive(Serialize)]
pub struct BinaryVersion {
    pub path: String,
    pub exists: bool,
    pub raw: String,
    pub base_version: Option<String>,
    pub commit_hash: Option<String>,
    pub exact_core_next_match: bool,
}

#[derive(Clone, Serialize)]
pub struct DaemonProbe {
    pub rpc_port_open: bool,
    pub p2p_port_open: bool,
    pub default_rpc_port: u16,
    pub default_p2p_port: u16,
}

#[derive(Serialize)]
pub struct RuntimeStatus {
    pub required_base_version: String,
    pub required_commit_hash: String,
    pub daemon: BinaryVersion,
    pub cli: BinaryVersion,
    pub bundled_core_next_ready: bool,
    pub probe: DaemonProbe,
}

#[derive(Serialize)]
pub struct DaemonOwnership {
    pub commander_owns: bool,
}

fn command_version(path: &str) -> Result<String, String> {
    let output = Command::new(path)
        .arg("-version")
        .output()
        .map_err(|e| e.to_string())?;

    let mut text = String::new();
    if !output.stdout.is_empty() {
        text.push_str(&String::from_utf8_lossy(&output.stdout));
    }
    if !output.stderr.is_empty() {
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(&String::from_utf8_lossy(&output.stderr));
    }

    if text.trim().is_empty() {
        Err("Version command returned no output".to_string())
    } else {
        Ok(text.trim().to_string())
    }
}

fn parse_base_version(raw: &str) -> Option<String> {
    for token in raw.split_whitespace() {
        let trimmed =
            token.trim_matches(|c: char| c == 'v' || c == 'V' || c == ',' || c == ')' || c == '(');
        if trimmed.chars().filter(|c| *c == '.').count() >= 2
            && trimmed.chars().all(|c| c.is_ascii_digit() || c == '.')
        {
            return Some(trimmed.to_string());
        }
        if let Some((front, _)) = trimmed.split_once('-') {
            if front.chars().filter(|c| *c == '.').count() >= 2
                && front.chars().all(|c| c.is_ascii_digit() || c == '.')
            {
                return Some(front.to_string());
            }
        }
    }
    None
}

fn parse_commit_hash(raw: &str) -> Option<String> {
    let lower = raw.to_lowercase();
    if lower.contains(REQUIRED_CORE_NEXT_COMMIT) {
        return Some(REQUIRED_CORE_NEXT_COMMIT.to_string());
    }

    for token in lower.split(|c: char| !c.is_ascii_hexdigit()) {
        if token.len() >= 8 && token.len() <= 40 && token.chars().all(|c| c.is_ascii_hexdigit()) {
            return Some(token.to_string());
        }
    }
    None
}

fn binary_version(name: &str) -> BinaryVersion {
    let path = resolve_bin(name);
    let exists = PathBuf::from(&path).exists();
    let raw = if exists {
        command_version(&path).unwrap_or_else(|e| format!("Version check failed: {e}"))
    } else {
        String::new()
    };
    let base_version = parse_base_version(&raw);
    let commit_hash = parse_commit_hash(&raw);
    let exact_core_next_match = base_version.as_deref() == Some(REQUIRED_CORE_BASE_VERSION)
        && commit_hash.as_deref() == Some(REQUIRED_CORE_NEXT_COMMIT);

    BinaryVersion {
        path,
        exists,
        raw,
        base_version,
        commit_hash,
        exact_core_next_match,
    }
}

fn port_open(port: u16) -> bool {
    let addr = format!("127.0.0.1:{port}");
    let Ok(addrs) = addr.to_socket_addrs() else {
        return false;
    };
    for addr in addrs {
        if TcpStream::connect_timeout(&addr, Duration::from_millis(250)).is_ok() {
            return true;
        }
    }
    false
}

#[tauri::command]
pub fn probe_default_daemon() -> DaemonProbe {
    DaemonProbe {
        rpc_port_open: port_open(DEFAULT_RPC_PORT),
        p2p_port_open: port_open(DEFAULT_P2P_PORT),
        default_rpc_port: DEFAULT_RPC_PORT,
        default_p2p_port: DEFAULT_P2P_PORT,
    }
}

#[tauri::command]
pub fn get_runtime_status() -> RuntimeStatus {
    let daemon = binary_version("hemp0xd");
    let cli = binary_version("hemp0x-cli");
    let bundled_core_next_ready = daemon.exact_core_next_match && cli.exact_core_next_match;

    RuntimeStatus {
        required_base_version: REQUIRED_CORE_BASE_VERSION.to_string(),
        required_commit_hash: REQUIRED_CORE_NEXT_COMMIT.to_string(),
        daemon,
        cli,
        bundled_core_next_ready,
        probe: probe_default_daemon(),
    }
}

#[tauri::command]
pub fn take_daemon_ownership() -> DaemonOwnership {
    if let Ok(mut guard) = DAEMON_OWNERSHIP.lock() {
        *guard = true;
    }
    DaemonOwnership {
        commander_owns: true,
    }
}

#[tauri::command]
pub fn release_daemon_ownership() -> DaemonOwnership {
    if let Ok(mut guard) = DAEMON_OWNERSHIP.lock() {
        *guard = false;
    }
    DaemonOwnership {
        commander_owns: false,
    }
}

#[tauri::command]
pub fn get_daemon_ownership() -> DaemonOwnership {
    let owns = DAEMON_OWNERSHIP.lock().map(|g| *g).unwrap_or(false);
    DaemonOwnership {
        commander_owns: owns,
    }
}

#[derive(Clone, Serialize)]
pub struct RunningDaemonIdentity {
    pub rpc_authenticated: bool,
    pub base_version: Option<String>,
    pub subversion: Option<String>,
    pub protocol_version: Option<u64>,
    pub numeric_version: Option<u64>,
    pub is_required_core_next: bool,
    pub commit_match: bool,
    pub commit_available: bool,
    pub status: String,
}

fn parse_numeric_version(raw: u64) -> String {
    let major = raw / 1000000;
    let minor = (raw / 10000) % 100;
    let revision = (raw / 100) % 100;
    let build = raw % 100;
    format!("{major}.{minor}.{revision}.{build}")
}

#[tauri::command]
pub fn identify_running_daemon(allow_non_bundled: Option<bool>) -> RunningDaemonIdentity {
    let allow_override = allow_non_bundled.unwrap_or(false);
    let ctx = match rpc_context() {
        Ok(c) => c,
        Err(e) => {
            return RunningDaemonIdentity {
                rpc_authenticated: false,
                base_version: None,
                subversion: None,
                protocol_version: None,
                numeric_version: None,
                is_required_core_next: false,
                commit_match: false,
                commit_available: false,
                status: format!("RPC not configured: {e}"),
            };
        }
    };

    match ctx.call("getnetworkinfo", &[]) {
        Ok(data) => {
            let subver_str = data["subversion"]
                .as_str()
                .unwrap_or("")
                .to_string();
            let numeric_version = data["version"].as_u64();

            let base_version = numeric_version.map(parse_numeric_version);
            let is_required = base_version.as_deref() == Some(REQUIRED_CORE_BASE_VERSION);
            let commit_match = subver_str.contains(REQUIRED_CORE_NEXT_COMMIT);
            let commit_available = parse_commit_hash(&subver_str).is_some();
            let is_exact = is_required && numeric_version.is_some() && commit_match;

            let status = if is_exact {
                "A verified Core Next daemon is already running.".to_string()
            } else if allow_override {
                format!(
                    "A daemon is running (non-bundled override active). Version: {} / Subversion: {}",
                    base_version.as_deref().unwrap_or("?"),
                    subver_str
                )
            } else if is_required && commit_available {
                format!(
                    "A daemon is running, but it does not match the bundled Core Next build ({}).",
                    REQUIRED_CORE_NEXT_COMMIT
                )
            } else if is_required {
                format!(
                    "A daemon is running with the required base version, but Core RPC did not expose the commit hash needed to verify bundled build {}.",
                    REQUIRED_CORE_NEXT_COMMIT
                )
            } else {
                format!(
                    "A daemon is running, but Commander could not verify it is Core Next {} ({}).",
                    REQUIRED_CORE_BASE_VERSION,
                    REQUIRED_CORE_NEXT_COMMIT,
                )
            };

            RunningDaemonIdentity {
                rpc_authenticated: true,
                base_version,
                subversion: if subver_str.is_empty() { None } else { Some(subver_str) },
                protocol_version: data["protocolversion"].as_u64(),
                numeric_version,
                is_required_core_next: is_exact || (allow_override && is_required),
                commit_match,
                commit_available,
                status,
            }
        }
        Err(e) => RunningDaemonIdentity {
            rpc_authenticated: false,
            base_version: None,
            subversion: None,
            protocol_version: None,
            numeric_version: None,
            is_required_core_next: false,
            commit_match: false,
            commit_available: false,
            status: format!(
                "A daemon is listening on the default RPC port, but Commander could not verify its version: {e}"
            ),
        },
    }
}

#[derive(Clone, Serialize)]
pub struct DaemonReadiness {
    pub ready: bool,
    pub progress: String,
    pub elapsed_ms: u64,
    pub retries: u32,
    pub rpc_error: String,
}

#[tauri::command]
pub fn wait_for_daemon_ready(timeout_ms: Option<u64>) -> DaemonReadiness {
    let timeout_dur = Duration::from_millis(timeout_ms.unwrap_or(30_000));
    let poll_interval = Duration::from_millis(500);
    let start = Instant::now();

    let ctx = match rpc_context() {
        Ok(c) => c,
        Err(e) => {
            return DaemonReadiness {
                ready: false,
                progress: "RPC configuration failed".to_string(),
                elapsed_ms: start.elapsed().as_millis() as u64,
                retries: 0,
                rpc_error: e,
            };
        }
    };

    let mut retries: u32 = 0;
    loop {
        retries += 1;
        match ctx.call("getnetworkinfo", &[]) {
            Ok(data) => {
                let _ = data;
                return DaemonReadiness {
                    ready: true,
                    progress: "Daemon RPC is responding".to_string(),
                    elapsed_ms: start.elapsed().as_millis() as u64,
                    retries,
                    rpc_error: String::new(),
                };
            }
            Err(e) => {
                let elapsed = start.elapsed();
                if elapsed >= timeout_dur {
                    return DaemonReadiness {
                        ready: false,
                        progress: "Daemon did not become ready within timeout".to_string(),
                        elapsed_ms: elapsed.as_millis() as u64,
                        retries,
                        rpc_error: e,
                    };
                }
                if poll_interval > timeout_dur.saturating_sub(elapsed) {
                    std::thread::sleep(timeout_dur.saturating_sub(elapsed));
                } else {
                    std::thread::sleep(poll_interval);
                }
                continue;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_base_version_from_typical_output() {
        let raw = "Hemp0x Core Daemon version v4.7.0.0-192c6b5ce";
        let result = parse_base_version(raw);
        assert_eq!(result, Some("4.7.0.0".to_string()));
    }

    #[test]
    fn parses_base_version_without_v_prefix() {
        let raw = "Hemp0x Daemon 4.7.0-unk";
        let result = parse_base_version(raw);
        assert_eq!(result, Some("4.7.0".to_string()));
    }

    #[test]
    fn parses_base_version_from_alpha() {
        let raw = "Hemp0x RPC client version v4.7.0.0-alpha";
        assert_eq!(parse_base_version(raw), Some("4.7.0.0".to_string()));
    }

    #[test]
    fn parse_base_version_no_version() {
        assert_eq!(parse_base_version("Hemp0x no version here"), None);
    }

    #[test]
    fn parse_base_version_not_enough_dots() {
        let raw = "version 4.7 something";
        assert_eq!(parse_base_version(raw), None);
    }

    #[test]
    fn parse_base_version_two_digits() {
        let raw = "v4.70.0.5 (release)";
        assert_eq!(parse_base_version(raw), Some("4.70.0.5".to_string()));
    }

    #[test]
    fn parses_commit_hash_from_known_hash() {
        let raw = "Hemp0x Core Daemon version v4.7.0.0-192c6b5ce";
        let result = parse_commit_hash(raw);
        assert_eq!(result, Some("192c6b5ce".to_string()));
    }

    #[test]
    fn parses_commit_hash_from_arbitrary_hex() {
        let raw = "Hemp0x Daemon v1.2.3-abc123def456 (release build)";
        let result = parse_commit_hash(raw);
        assert!(result.is_some());
        assert!(result.unwrap().len() >= 8);
    }

    #[test]
    fn parse_commit_hash_short_hex_not_returned() {
        let raw = "Hemp0x Core v4.7.0.0-abc";
        assert_eq!(parse_commit_hash(raw), None);
    }

    #[test]
    fn parse_commit_hash_no_hash() {
        let raw = "Hemp0x Core v4.7.0.0";
        assert_eq!(parse_commit_hash(raw), None);
    }

    #[test]
    fn parse_numeric_version_standard() {
        assert_eq!(parse_numeric_version(4070000), "4.7.0.0");
    }

    #[test]
    fn parse_numeric_version_nonzero_build() {
        assert_eq!(parse_numeric_version(4070001), "4.7.0.1");
    }

    #[test]
    fn parse_numeric_version_minimal() {
        assert_eq!(parse_numeric_version(1), "0.0.0.1");
    }

    #[test]
    fn probe_default_daemon_returns_expected_structure() {
        let probe = probe_default_daemon();
        assert_eq!(probe.default_rpc_port, 42068);
        assert_eq!(probe.default_p2p_port, 42069);
    }

    #[test]
    fn daemon_ownership_cycle() {
        assert_eq!(get_daemon_ownership().commander_owns, false);
        let taken = take_daemon_ownership();
        assert!(taken.commander_owns);
        assert!(get_daemon_ownership().commander_owns);
        let released = release_daemon_ownership();
        assert!(!released.commander_owns);
        assert!(!get_daemon_ownership().commander_owns);
    }

    #[test]
    fn wait_for_daemon_ready_short_timeout_is_non_blocking() {
        let start = std::time::Instant::now();
        let result = wait_for_daemon_ready(Some(100));
        let elapsed = start.elapsed().as_millis();
        assert!(
            !result.ready || elapsed < 500,
            "if daemon is running, response should be sub-second"
        );
        assert!(elapsed < 5000, "function should not block for long");
    }
}
