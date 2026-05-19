use std::net::{TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;

use crate::modules::utils::resolve_bin;

const REQUIRED_CORE_NEXT_COMMIT: &str = "3aab5c068";
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
