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

fn rpc_url() -> Result<String, String> {
    let config = parse_config(&config_path()?)?;
    let port = config.get("rpcport").map(|v| v.as_str()).unwrap_or("42068");
    Ok(format!("http://127.0.0.1:{port}"))
}

fn rpc_auth() -> Result<String, String> {
    let dir = data_dir()?;
    let cookie_path = if cfg!(windows) {
        dir.join(".cookie")
    } else {
        dir.join(".cookie")
    };

    if cookie_path.exists() {
        let cookie = fs::read_to_string(&cookie_path)
            .map_err(|e| format!("Failed to read RPC cookie: {e}"))?;
        let cookie = cookie.trim().to_string();
        if cookie.is_empty() {
            return Err("RPC cookie is empty".to_string());
        }
        if !cookie.contains(':') {
            return Err("RPC cookie is malformed; expected username:password".to_string());
        }
        return Ok(format!(
            "Basic {}",
            BASE64_STANDARD.encode(cookie.as_bytes())
        ));
    }

    let config = parse_config(&config_path()?)?;
    let user = config.get("rpcuser");
    let pass = config.get("rpcpassword");

    if let (Some(u), Some(p)) = (user, pass) {
        let auth = format!("{u}:{p}");
        return Ok(format!("Basic {}", BASE64_STANDARD.encode(auth.as_bytes())));
    }

    Err(
        "RPC authentication unavailable: no cookie file and no rpcuser/rpcpassword in hemp.conf"
            .to_string(),
    )
}

pub(crate) struct RpcContext {
    url: String,
    auth: String,
}

pub(crate) fn rpc_context() -> Result<RpcContext, String> {
    let url = rpc_url()?;
    let auth = rpc_auth()?;
    Ok(RpcContext { url, auth })
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

fn call_rpc(method: &str, params: &[serde_json::Value]) -> Result<serde_json::Value, String> {
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
    "getmininginfo",
    "getnetworkinfo",
    "getpeerinfo",
    "getwalletinfo",
    "listassets",
    "listtransactions",
    "listunspent",
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowed_methods_include_informational_rpcs() {
        assert!(ALLOWED_METHODS.contains(&"getinfo"));
        assert!(ALLOWED_METHODS.contains(&"getblockchaininfo"));
        assert!(ALLOWED_METHODS.contains(&"getnetworkinfo"));
        assert!(ALLOWED_METHODS.contains(&"getwalletinfo"));
        assert!(ALLOWED_METHODS.contains(&"listtransactions"));
        assert!(ALLOWED_METHODS.contains(&"listunspent"));
    }

    #[test]
    fn allowed_methods_exclude_destructive_rpcs() {
        assert!(!ALLOWED_METHODS.contains(&"sendtoaddress"));
        assert!(!ALLOWED_METHODS.contains(&"sendmany"));
        assert!(!ALLOWED_METHODS.contains(&"sendasset"));
        assert!(!ALLOWED_METHODS.contains(&"fundrawtransaction"));
        assert!(!ALLOWED_METHODS.contains(&"signrawtransaction"));
        assert!(!ALLOWED_METHODS.contains(&"issue"));
        assert!(!ALLOWED_METHODS.contains(&"transfer"));
        assert!(!ALLOWED_METHODS.contains(&"walletpassphrase"));
        assert!(!ALLOWED_METHODS.contains(&"encryptwallet"));
        assert!(!ALLOWED_METHODS.contains(&"dumpwallet"));
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
}
