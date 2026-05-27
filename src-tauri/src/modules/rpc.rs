use std::fs;
use std::time::Duration;

use base64::prelude::*;
use serde::Serialize;

use crate::modules::files::{config_path, data_dir, ensure_config, parse_config};
use crate::modules::models::DashboardData;

#[derive(Debug, Serialize)]
pub struct RpcResult {
    pub success: bool,
    pub data: serde_json::Value,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RpcAuthMode {
    Cookie,
    LegacyUserpass,
    Unavailable,
}

#[derive(Debug, Clone, Serialize)]
pub struct RpcAuthInfo {
    pub auth_mode: RpcAuthMode,
    pub cookie_exists: bool,
    pub legacy_credentials_exist: bool,
    pub warning: String,
}

fn rpc_url() -> Result<String, String> {
    let config = parse_config(&config_path()?)?;
    let port = config.get("rpcport").map(|v| v.as_str()).unwrap_or("42068");
    Ok(format!("http://127.0.0.1:{port}"))
}

fn rpc_auth() -> Result<(String, RpcAuthMode), String> {
    let dir = data_dir()?;
    let cookie_path = dir.join(".cookie");
    rpc_auth_for_paths(&cookie_path, &config_path()?)
}

fn rpc_auth_for_paths(
    cookie_path: &std::path::Path,
    config_path: &std::path::Path,
) -> Result<(String, RpcAuthMode), String> {
    if cookie_path.exists() {
        let cookie = fs::read_to_string(cookie_path)
            .map_err(|e| format!("Failed to read RPC cookie: {e}"))?;
        let cookie = cookie.trim().to_string();
        if !cookie.is_empty() && cookie.contains(':') {
            return Ok((
                format!(
                    "Basic {}",
                    BASE64_STANDARD.encode(cookie.as_bytes())
                ),
                RpcAuthMode::Cookie,
            ));
        }
    }

    let config = if config_path.exists() {
        parse_config(config_path)?
    } else {
        return Err(
            "RPC authentication unavailable: no cookie file and no rpcuser/rpcpassword in hemp.conf"
                .to_string(),
        );
    };
    let user = config.get("rpcuser");
    let pass = config.get("rpcpassword");

    if let (Some(u), Some(p)) = (user, pass) {
        let auth = format!("{u}:{p}");
        return Ok((
            format!("Basic {}", BASE64_STANDARD.encode(auth.as_bytes())),
            RpcAuthMode::LegacyUserpass,
        ));
    }

    Err(
        "RPC authentication unavailable: no cookie file and no rpcuser/rpcpassword in hemp.conf"
            .to_string(),
    )
}

#[tauri::command]
pub fn get_rpc_auth_status() -> RpcAuthInfo {
    let (cookie_exists, legacy_exists) = detect_auth_sources();

    let (auth_mode, warning) = if cookie_exists {
        (RpcAuthMode::Cookie, String::new())
    } else if legacy_exists {
        (
            RpcAuthMode::LegacyUserpass,
            "Legacy RPC password auth is active. Cookie auth is recommended for Commander v2 and Core Next."
                .to_string(),
        )
    } else {
        (RpcAuthMode::Unavailable, String::new())
    };

    RpcAuthInfo {
        auth_mode,
        cookie_exists,
        legacy_credentials_exist: legacy_exists,
        warning,
    }
}

fn detect_auth_sources() -> (bool, bool) {
    let cookie_exists = data_dir()
        .map(|dir| {
            let cp = dir.join(".cookie");
            cp.exists()
                && fs::read_to_string(&cp)
                    .map(|c| !c.trim().is_empty() && c.contains(':'))
                    .unwrap_or(false)
        })
        .unwrap_or(false);

    let legacy_exists = config_path()
        .ok()
        .and_then(|p| parse_config(&p).ok())
        .map(|config| config.contains_key("rpcuser") && config.contains_key("rpcpassword"))
        .unwrap_or(false);

    (cookie_exists, legacy_exists)
}

pub(crate) struct RpcContext {
    url: String,
    auth: String,
    #[allow(dead_code)]
    pub auth_mode: RpcAuthMode,
}

pub(crate) fn rpc_context() -> Result<RpcContext, String> {
    let url = rpc_url()?;
    let (auth, auth_mode) = rpc_auth()?;
    Ok(RpcContext {
        url,
        auth,
        auth_mode,
    })
}

impl RpcContext {
    pub(crate) fn call(
        &self,
        method: &str,
        params: &[serde_json::Value],
    ) -> Result<serde_json::Value, String> {
        let body = serde_json::json!({
            "jsonrpc": "1.0",
            "id": "commander",
            "method": method,
            "params": params,
        });

        let response_result = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(5))
            .timeout_read(Duration::from_secs(15))
            .build()
            .post(&self.url)
            .set("Authorization", &self.auth)
            .set("Content-Type", "application/json")
            .send_json(&body);

        let raw: serde_json::Value = match response_result {
            Ok(resp) => resp
                .into_json()
                .map_err(|e| format!("RPC parse error ({method}): {e}"))?,
            Err(ureq::Error::Status(status, resp)) => resp.into_json().map_err(|e| {
                format!("RPC HTTP {status} with unreadable response body ({method}): {e}")
            })?,
            Err(e) => return Err(format!("RPC transport error ({method}): {e}")),
        };

        if let Some(err) = raw.get("error").filter(|e| !e.is_null()) {
            let msg = err
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("unknown RPC error");
            return Err(format!("RPC error ({method}): {msg}"));
        }

        let result = raw
            .get("result")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        Ok(result)
    }
}

pub(crate) fn call_rpc(method: &str, params: &[serde_json::Value]) -> Result<serde_json::Value, String> {
    let ctx = rpc_context()?;
    ctx.call(method, params)
}

fn rpc_command_with_result(method: &str, params: &[serde_json::Value]) -> RpcResult {
    match call_rpc(method, params) {
        Ok(data) => RpcResult {
            success: true,
            data,
            error: String::new(),
        },
        Err(e) => RpcResult {
            success: false,
            data: serde_json::Value::Null,
            error: e,
        },
    }
}

#[tauri::command]
pub fn rpc_get_blockchain_info() -> RpcResult {
    rpc_command_with_result("getblockchaininfo", &[])
}

#[tauri::command]
pub fn rpc_get_network_info() -> RpcResult {
    rpc_command_with_result("getnetworkinfo", &[])
}

#[tauri::command]
pub fn rpc_get_wallet_info() -> RpcResult {
    rpc_command_with_result("getwalletinfo", &[])
}

const ALLOWED_METHODS: &[&str] = &[
    "getassetdata",
    "getbestblockhash",
    "getblockchaininfo",
    "getblockcount",
    "getinfo",
    "getmempoolinfo",
    "getmessaginginfo",
    "getmininginfo",
    "getnetworkinfo",
    "getpeerinfo",
    "getwalletinfo",
    "listassets",
    "listtransactions",
    "listunspent",
    "estimatesmartfee",
    "viewallmessages",
    "viewallmessagechannels",
];

#[tauri::command]
pub fn rpc_call(method: String, params: Vec<serde_json::Value>) -> RpcResult {
    if !ALLOWED_METHODS.contains(&method.as_str()) {
        return RpcResult {
            success: false,
            data: serde_json::Value::Null,
            error: format!(
                "RPC method is not allowed through the generic read-only bridge: {method}"
            ),
        };
    }

    rpc_command_with_result(&method, &params)
}

pub(crate) fn build_dashboard_from_rpc(
    info: &serde_json::Value,
    bc: &serde_json::Value,
    tx_raw: &serde_json::Value,
) -> Result<DashboardData, String> {
    let blocks_info = info["blocks"].as_u64().unwrap_or(0);
    let peers = info["connections"].as_u64().unwrap_or(0);
    let diff_val = info["difficulty"].as_f64().unwrap_or(0.0);
    let balance_val = info["balance"].as_f64().unwrap_or(0.0);
    let pending_val = info["unconfirmed_balance"].as_f64().unwrap_or(0.0);
    let staked_val = info["immature_balance"].as_f64().unwrap_or(0.0);

    let unlocked_until = info["unlocked_until"].as_i64();
    let status = match unlocked_until {
        Some(0) => "LOCKED",
        Some(_) => "UNLOCKED",
        None => "UNENCRYPTED",
    };

    let b = bc["blocks"].as_u64().unwrap_or(blocks_info);
    let h = bc["headers"].as_u64().unwrap_or(0);
    let progress = bc["verificationprogress"].as_f64().unwrap_or(0.0);
    let initial_dl = bc["initialblockdownload"].as_bool().unwrap_or(false);
    let mtp = bc["mediantime"].as_i64().unwrap_or(0);
    let now = chrono::Local::now().timestamp();
    let is_synced = h > 0 && b >= h && progress >= 0.999 && !initial_dl && (now - mtp) < 5400;

    let node = crate::modules::models::NodeInfo {
        state: "RUNNING".to_string(),
        blocks: b,
        headers: h,
        peers,
        diff: format!("{:.4}", diff_val),
        synced: is_synced,
    };

    let wallet = crate::modules::models::WalletInfo {
        balance: format!("{:.3}", balance_val),
        pending: format!("{:.3}", pending_val),
        staked: format!("{:.3}", staked_val),
        status: status.to_string(),
    };

    let mut tx_vec: Vec<serde_json::Value> = tx_raw.as_array().unwrap_or(&Vec::new()).clone();
    tx_vec.sort_by(|a, b| {
        let time_a = a["time"].as_i64().unwrap_or(0);
        let time_b = b["time"].as_i64().unwrap_or(0);
        if time_a != time_b {
            return time_a.cmp(&time_b);
        }
        let cat_a = a["category"].as_str().unwrap_or("");
        let cat_b = b["category"].as_str().unwrap_or("");
        if cat_a == "send" && cat_b == "receive" {
            return std::cmp::Ordering::Less;
        }
        if cat_a == "receive" && cat_b == "send" {
            return std::cmp::Ordering::Greater;
        }
        cat_a.cmp(cat_b)
    });

    use chrono::{DateTime, Local, TimeZone};
    let mut txs = Vec::new();
    for tx in tx_vec.iter().rev().take(50) {
        let epoch = tx["time"].as_i64().unwrap_or(0);
        let dt: DateTime<Local> = Local
            .timestamp_opt(epoch, 0)
            .single()
            .unwrap_or_else(|| Local::now());
        let amount = tx["amount"].as_f64().unwrap_or(0.0);
        let item = crate::modules::models::TxItem {
            date: dt.format("%m/%d %H:%M").to_string(),
            tx_type: tx["category"].as_str().unwrap_or("unknown").to_string(),
            amount: format!("{:.7}", amount),
            conf: tx["confirmations"].as_u64().unwrap_or(0),
            txid: tx["txid"].as_str().unwrap_or("-").to_string(),
        };
        txs.push(item);
    }

    Ok(DashboardData {
        node,
        wallet,
        tx: txs,
    })
}

#[tauri::command]
pub fn rpc_dashboard() -> Result<DashboardData, String> {
    let _cfg = ensure_config()?;
    let ctx = rpc_context()?;

    let info = ctx.call("getinfo", &[])?;
    let bc = ctx.call("getblockchaininfo", &[])?;
    let tx_raw = ctx.call(
        "listtransactions",
        &[
            serde_json::Value::String("*".to_string()),
            serde_json::Value::Number(serde_json::value::Number::from(100)),
        ],
    )?;

    build_dashboard_from_rpc(&info, &bc, &tx_raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_info(
        blocks: u64,
        connections: u64,
        difficulty: f64,
        balance: f64,
    ) -> serde_json::Value {
        serde_json::json!({
            "blocks": blocks,
            "connections": connections,
            "difficulty": difficulty,
            "balance": balance,
            "unconfirmed_balance": 0.0,
            "immature_balance": 0.0,
            "unlocked_until": 0,
        })
    }

    fn fixture_bc(blocks: u64, headers: u64, progress: f64, mediantime: i64) -> serde_json::Value {
        serde_json::json!({
            "blocks": blocks,
            "headers": headers,
            "verificationprogress": progress,
            "initialblockdownload": false,
            "mediantime": mediantime,
        })
    }

    fn fixture_tx() -> serde_json::Value {
        serde_json::json!([
            {"time": 1716150000, "category": "receive", "amount": 1.5, "confirmations": 3, "txid": "abc123"},
            {"time": 1716140000, "category": "send", "amount": -0.5, "confirmations": 10, "txid": "def456"},
        ])
    }

    #[test]
    fn build_dashboard_parses_running_state() {
        let info = fixture_info(100, 8, 1.5, 42.0);
        let bc = fixture_bc(100, 100, 1.0, chrono::Local::now().timestamp() - 100);
        let txs = fixture_tx();
        let result = build_dashboard_from_rpc(&info, &bc, &txs).unwrap();
        assert_eq!(result.node.state, "RUNNING");
        assert_eq!(result.node.blocks, 100);
        assert_eq!(result.node.peers, 8);
        assert!(result.node.synced);
        assert_eq!(result.wallet.balance, "42.000");
        assert_eq!(result.wallet.status, "LOCKED");
        assert_eq!(result.tx.len(), 2);
    }

    #[test]
    fn build_dashboard_detects_unsynced() {
        let info = fixture_info(50, 1, 0.5, 0.0);
        let bc = fixture_bc(50, 200, 0.5, 0);
        let txs = fixture_tx();
        let result = build_dashboard_from_rpc(&info, &bc, &txs).unwrap();
        assert!(!result.node.synced);
    }

    #[test]
    fn build_dashboard_handles_empty_tx_list() {
        let info = fixture_info(1, 0, 0.0, 0.0);
        let bc = fixture_bc(1, 1, 1.0, chrono::Local::now().timestamp() - 50);
        let txs = serde_json::json!([]);
        let result = build_dashboard_from_rpc(&info, &bc, &txs).unwrap();
        assert_eq!(result.tx.len(), 0);
    }

    #[test]
    fn allowed_methods_include_informational_rpcs() {
        assert!(ALLOWED_METHODS.contains(&"getinfo"));
        assert!(ALLOWED_METHODS.contains(&"getblockchaininfo"));
        assert!(ALLOWED_METHODS.contains(&"getnetworkinfo"));
        assert!(ALLOWED_METHODS.contains(&"getwalletinfo"));
        assert!(ALLOWED_METHODS.contains(&"listtransactions"));
        assert!(ALLOWED_METHODS.contains(&"listunspent"));
        assert!(ALLOWED_METHODS.contains(&"estimatesmartfee"));
        assert!(ALLOWED_METHODS.contains(&"getmessaginginfo"));
        assert!(ALLOWED_METHODS.contains(&"viewallmessages"));
        assert!(ALLOWED_METHODS.contains(&"viewallmessagechannels"));
    }

    #[test]
    fn allowed_methods_exclude_destructive_rpcs() {
        assert!(!ALLOWED_METHODS.contains(&"sendtoaddress"));
        assert!(!ALLOWED_METHODS.contains(&"sendmany"));
        assert!(!ALLOWED_METHODS.contains(&"sendasset"));
        assert!(!ALLOWED_METHODS.contains(&"fundrawtransaction"));
        assert!(!ALLOWED_METHODS.contains(&"settxfee"));
        assert!(!ALLOWED_METHODS.contains(&"walletcreatefundedpsbt"));
        assert!(!ALLOWED_METHODS.contains(&"signrawtransaction"));
        assert!(!ALLOWED_METHODS.contains(&"issue"));
        assert!(!ALLOWED_METHODS.contains(&"transfer"));
        assert!(!ALLOWED_METHODS.contains(&"walletpassphrase"));
        assert!(!ALLOWED_METHODS.contains(&"encryptwallet"));
        assert!(!ALLOWED_METHODS.contains(&"dumpwallet"));
        assert!(!ALLOWED_METHODS.contains(&"sendmessage"));
        assert!(!ALLOWED_METHODS.contains(&"subscribetochannel"));
        assert!(!ALLOWED_METHODS.contains(&"unsubscribefromchannel"));
        assert!(!ALLOWED_METHODS.contains(&"clearmessages"));
    }

    #[test]
    fn rpc_result_success_constructor() {
        let r = RpcResult {
            success: true,
            data: serde_json::Value::String("ok".into()),
            error: String::new(),
        };
        assert!(r.success);
        assert_eq!(r.data, "ok");
        assert!(r.error.is_empty());
    }

    #[test]
    fn rpc_result_error_constructor() {
        let r = RpcResult {
            success: false,
            data: serde_json::Value::Null,
            error: "something went wrong".to_string(),
        };
        assert!(!r.success);
        assert_eq!(r.data, serde_json::Value::Null);
        assert_eq!(r.error, "something went wrong");
    }

    #[test]
    fn allowed_methods_exclude_mining_rpcs() {
        assert!(!ALLOWED_METHODS.contains(&"getblocktemplate"));
        assert!(!ALLOWED_METHODS.contains(&"getkawpowhash"));
        assert!(!ALLOWED_METHODS.contains(&"submitblock"));
        assert!(!ALLOWED_METHODS.contains(&"pprpcsb"));
        assert!(!ALLOWED_METHODS.contains(&"prioritisetransaction"));
    }

    fn make_temp_dir() -> std::path::PathBuf {
        static CNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = CNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "hemp0x_rpc_test_{:x}_{:x}",
            std::process::id(),
            n
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn temp_cookie_path(tmp: &std::path::Path) -> std::path::PathBuf {
        tmp.join(".cookie")
    }

    fn temp_config_path(tmp: &std::path::Path) -> std::path::PathBuf {
        tmp.join("hemp.conf")
    }

    #[test]
    fn auth_cookie_preferred_over_userpass_when_both_exist() {
        let tmp = make_temp_dir();
        let cookie_path = temp_cookie_path(&tmp);
        let config_path = temp_config_path(&tmp);

        fs::write(&cookie_path, "cookie_user:cookie_pass").unwrap();
        fs::write(&config_path, "rpcuser=config_user\nrpcpassword=config_pass\n").unwrap();

        let result = rpc_auth_for_paths(&cookie_path, &config_path);
        let _ = fs::remove_dir_all(&tmp);

        let (header, mode) = result.unwrap();
        assert_eq!(mode, RpcAuthMode::Cookie);
        assert!(header.starts_with("Basic "));
    }

    #[test]
    fn auth_falls_back_to_userpass_when_cookie_absent() {
        let tmp = make_temp_dir();
        let cookie_path = temp_cookie_path(&tmp);
        let config_path = temp_config_path(&tmp);

        fs::write(&config_path, "rpcuser=test_user\nrpcpassword=test_pass\n").unwrap();

        let result = rpc_auth_for_paths(&cookie_path, &config_path);
        let _ = fs::remove_dir_all(&tmp);

        let (header, mode) = result.unwrap();
        assert_eq!(mode, RpcAuthMode::LegacyUserpass);
        assert!(header.starts_with("Basic "));
    }

    #[test]
    fn auth_unavailable_when_neither_cookie_nor_userpass() {
        let tmp = make_temp_dir();
        let cookie_path = temp_cookie_path(&tmp);
        let config_path = temp_config_path(&tmp);

        fs::write(&config_path, "server=1\ndaemon=0\n").unwrap();

        let result = rpc_auth_for_paths(&cookie_path, &config_path);
        let _ = fs::remove_dir_all(&tmp);

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unavailable"));
    }

    #[test]
    fn auth_falls_back_on_empty_cookie() {
        let tmp = make_temp_dir();
        let cookie_path = temp_cookie_path(&tmp);
        let config_path = temp_config_path(&tmp);

        fs::write(&cookie_path, "").unwrap();
        fs::write(&config_path, "rpcuser=test_user\nrpcpassword=test_pass\n").unwrap();

        let result = rpc_auth_for_paths(&cookie_path, &config_path);
        let _ = fs::remove_dir_all(&tmp);

        let (_, mode) = result.unwrap();
        assert_eq!(mode, RpcAuthMode::LegacyUserpass);
    }

    #[test]
    fn auth_falls_back_on_malformed_cookie_no_colon() {
        let tmp = make_temp_dir();
        let cookie_path = temp_cookie_path(&tmp);
        let config_path = temp_config_path(&tmp);

        fs::write(&cookie_path, "no_colon_cookie_value").unwrap();
        fs::write(&config_path, "rpcuser=test_user\nrpcpassword=test_pass\n").unwrap();

        let result = rpc_auth_for_paths(&cookie_path, &config_path);
        let _ = fs::remove_dir_all(&tmp);

        let (_, mode) = result.unwrap();
        assert_eq!(mode, RpcAuthMode::LegacyUserpass);
    }

    #[test]
    fn auth_falls_back_on_whitespace_only_cookie() {
        let tmp = make_temp_dir();
        let cookie_path = temp_cookie_path(&tmp);
        let config_path = temp_config_path(&tmp);

        fs::write(&cookie_path, "   \n  ").unwrap();
        fs::write(&config_path, "rpcuser=test_user\nrpcpassword=test_pass\n").unwrap();

        let result = rpc_auth_for_paths(&cookie_path, &config_path);
        let _ = fs::remove_dir_all(&tmp);

        let (_, mode) = result.unwrap();
        assert_eq!(mode, RpcAuthMode::LegacyUserpass);
    }

    #[test]
    fn rpc_auth_mode_serialization() {
        let cookie_mode = serde_json::to_string(&RpcAuthMode::Cookie).unwrap();
        assert_eq!(cookie_mode, "\"cookie\"");
        let legacy_mode = serde_json::to_string(&RpcAuthMode::LegacyUserpass).unwrap();
        assert_eq!(legacy_mode, "\"legacy_userpass\"");
        let unavailable_mode = serde_json::to_string(&RpcAuthMode::Unavailable).unwrap();
        assert_eq!(unavailable_mode, "\"unavailable\"");
    }

    #[test]
    fn rpc_auth_info_warning_for_legacy() {
        let info = RpcAuthInfo {
            auth_mode: RpcAuthMode::LegacyUserpass,
            cookie_exists: false,
            legacy_credentials_exist: true,
            warning: "test warning".to_string(),
        };
        assert!(!info.warning.is_empty());
        assert_eq!(info.auth_mode, RpcAuthMode::LegacyUserpass);
    }

    #[test]
    fn rpc_auth_info_no_warning_for_cookie() {
        let info = RpcAuthInfo {
            auth_mode: RpcAuthMode::Cookie,
            cookie_exists: true,
            legacy_credentials_exist: true,
            warning: String::new(),
        };
        assert!(info.warning.is_empty());
        assert_eq!(info.auth_mode, RpcAuthMode::Cookie);
    }
}
