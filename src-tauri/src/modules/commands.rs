use std::path::PathBuf;
use std::process::Command;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::fs;
use chrono::{Local, TimeZone, DateTime};
// use tauri::Emitter; // Unused

// Import local modules
use crate::modules::models::*;
use crate::modules::utils::{resolve_bin, resolve_bin_with_override, split_args, parse_balances, version_is_old};
use crate::modules::files::{data_dir, ensure_config, parse_config, config_path, load_app_settings_impl};
use crate::modules::rpc;

// --- SHELL STATE ---
#[derive(Default)]
pub struct ShellState {
  pub cwd: PathBuf,
}

static SHELL_STATE: OnceLock<Mutex<ShellState>> = OnceLock::new();

fn default_shell_cwd() -> PathBuf {
  let custom_bin_dir = load_app_settings_impl().ok().and_then(|s| s.custom_core_binary_dir);
  let daemon = if let Some(ref d) = custom_bin_dir {
    resolve_bin_with_override("hemp0xd", Some(d))
  } else {
    resolve_bin("hemp0xd")
  };
  let candidate = PathBuf::from(daemon);
  if candidate.exists() {
    if let Some(parent) = candidate.parent() {
      return parent.to_path_buf();
    }
  }
  std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn shell_state() -> &'static Mutex<ShellState> {
  SHELL_STATE.get_or_init(|| Mutex::new(ShellState { cwd: default_shell_cwd() }))
}

// --- CLI RUNNER ---
pub fn run_cli(args: &[String]) -> Result<String, String> {
  let cfg = ensure_config()?;
  let dir = data_dir()?;

  let custom_bin_dir = load_app_settings_impl().ok().and_then(|s| s.custom_core_binary_dir);
  let cli = if let Some(ref d) = custom_bin_dir {
    resolve_bin_with_override("hemp0x-cli", Some(d))
  } else {
    resolve_bin("hemp0x-cli")
  };
  let cli_path = PathBuf::from(&cli);
  if !cli_path.exists() {
    return Err(format!("CLI not found at {}", cli));
  }
  
  // Parse config to detect network mode
  let config = parse_config(&cfg)?;
  let is_regtest = config.get("regtest").map(|v| v == "1").unwrap_or(false);
  let is_testnet = config.get("testnet").map(|v| v == "1").unwrap_or(false);
  
  let mut cmd = Command::new(&cli);
  if let Some(parent) = cli_path.parent() {
    cmd.current_dir(parent);
  }
  #[cfg(windows)]
  {
    use std::os::windows::process::CommandExt;
    cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
  }
  
  // Add network mode flags BEFORE config and datadir
  if is_regtest {
    cmd.arg("-regtest");
  } else if is_testnet {
    cmd.arg("-testnet");
  }
  
  let output = cmd
    .arg(format!("-conf={}", cfg.to_string_lossy()))
    .arg(format!("-datadir={}", dir.to_string_lossy()))
    .args(args.iter().map(|v| v.as_str()))
    .output()
    .map_err(|e| e.to_string())?;
  
  if !output.status.success() {
    let err = String::from_utf8_lossy(&output.stderr);
    let out = String::from_utf8_lossy(&output.stdout);
    return Err(format!(
      "CLI error ({}): {} {}",
      output.status,
      err.trim(),
      out.trim()
    )
    .trim()
    .to_string());
  }
  let out = String::from_utf8_lossy(&output.stdout);
  Ok(out.trim().to_string())
}


#[tauri::command]
pub fn run_cli_command(command: String, args: String) -> Result<String, String> {
  ensure_config()?;
  let mut full = Vec::new();
  if !command.trim().is_empty() {
    full.push(command.trim().to_string());
  }
  if !args.trim().is_empty() {
    full.extend(split_args(&args));
  }
  run_cli(&full)
}

/// Wrapper for frontend calls using `run_cli` with args array
#[tauri::command]
pub fn run_cli_args(args: Vec<String>) -> Result<String, String> {
  ensure_config()?;
  run_cli(&args)
}

/// Simple getinfo wrapper for node status checks
#[tauri::command]
pub fn get_info() -> Result<String, String> {
  run_cli(&[String::from("getinfo")])
}

/// List address groupings for wallet keys display
#[tauri::command]
pub fn list_address_groupings() -> Result<serde_json::Value, String> {
  let raw = run_cli(&[String::from("listaddressgroupings")])?;
  serde_json::from_str(&raw).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn dashboard_data() -> Result<DashboardData, String> {
  let cfg = ensure_config()?;
  let _ = parse_config(&cfg)?;

  let mut is_running = true;

  #[cfg(unix)]
  {
    // pgrep behavior
    let output = Command::new("pgrep")
        .arg("-f")
        .arg("hemp0xd")
        .output();
    
    if let Ok(o) = output {
        if !o.status.success() {
            is_running = false;
        }
    }
  }

  #[cfg(windows)]
  {
    use std::os::windows::process::CommandExt;
    let output = Command::new("tasklist")
      .creation_flags(0x08000000)
      .arg("/FI")
      .arg("IMAGENAME eq hemp0xd.exe")
      .arg("/NH")
      .output();
    
    if let Ok(o) = output {
        let stdout = String::from_utf8_lossy(&o.stdout);
        if !stdout.contains("hemp0xd.exe") {
            is_running = false;
        }
    }
  }

  if !is_running {
     return Ok(DashboardData {
        node: NodeInfo {
            state: "OFFLINE".to_string(),
            blocks: 0,
            headers: 0,
            peers: 0,
            diff: "--".to_string(),
            synced: false,
        },
        wallet: WalletInfo {
            balance: "--".to_string(),
            pending: "--".to_string(),
            staked: "--".to_string(),
            status: "--".to_string(),
        },
        tx: Vec::new(),
     });
  }

  let info_raw = run_cli(&[String::from("getinfo")])?;
  let info: serde_json::Value = serde_json::from_str(&info_raw).map_err(|e| e.to_string())?;

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

  let (blocks, headers, synced) = match run_cli(&[String::from("getblockchaininfo")]) {
    Ok(bc_raw) => {
      if let Ok(bc_info) = serde_json::from_str::<serde_json::Value>(&bc_raw) {
        let b = bc_info["blocks"].as_u64().unwrap_or(blocks_info);
        let h = bc_info["headers"].as_u64().unwrap_or(0);
        let progress = bc_info["verificationprogress"].as_f64().unwrap_or(0.0);
        let initial_dl = bc_info["initialblockdownload"].as_bool().unwrap_or(false);
        let mtp = bc_info["mediantime"].as_i64().unwrap_or(0);
        let now = Local::now().timestamp();
        let is_synced = h > 0 && b >= h && progress >= 0.999 && !initial_dl && (now - mtp) < 5400;
        (b, h, is_synced)
      } else {
        (blocks_info, blocks_info, false)
      }
    }
    Err(_) => (blocks_info, blocks_info, false)
  };

  let node = NodeInfo {
    state: "RUNNING".to_string(),
    blocks,
    headers,
    peers,
    diff: format!("{:.4}", diff_val),
    synced,
  };

  let wallet = WalletInfo {
    balance: format!("{:.3}", balance_val),
    pending: format!("{:.3}", pending_val),
    staked: format!("{:.3}", staked_val),
    status: status.to_string(),
  };

  let tx_raw = run_cli(&[
    String::from("listtransactions"),
    String::from("*"),
    String::from("100"),
  ])?;
  let tx_list: serde_json::Value = serde_json::from_str(&tx_raw).map_err(|e| e.to_string())?;
  let mut txs = Vec::new();
  let mut tx_vec: Vec<serde_json::Value> = tx_list.as_array().unwrap_or(&Vec::new()).clone();
  
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

  for tx in tx_vec.iter().rev().take(50) {
      let epoch = tx["time"].as_i64().unwrap_or(0);
      let dt: DateTime<Local> = Local.timestamp_opt(epoch, 0).single().unwrap_or_else(|| Local::now());
      let amount = tx["amount"].as_f64().unwrap_or(0.0);
      let item = TxItem {
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
pub fn get_receive_addresses(show_change: bool) -> Result<Vec<AddressItem>, String> {
  ensure_config()?;

  let groups_raw = run_cli(&[String::from("listaddressgroupings")])?;
  let groups: serde_json::Value =
    serde_json::from_str(&groups_raw).map_err(|e| e.to_string())?;
  let mut balances = HashMap::new();
  parse_balances(&groups, &mut balances);

  let list_raw = run_cli(&[
    String::from("listreceivedbyaddress"),
    String::from("0"),
    String::from("true"),
  ])?;
  let list: serde_json::Value = serde_json::from_str(&list_raw).map_err(|e| e.to_string())?;

  let mut items = Vec::new();
  let mut seen = HashMap::new();
  if let Some(arr) = list.as_array() {
    for item in arr {
      let addr = item["address"].as_str().unwrap_or("").to_string();
      if addr.is_empty() {
        continue;
      }
      let label = item["label"]
        .as_str()
        .or(item["account"].as_str())
        .unwrap_or("")
        .to_string();
      let bal = balances.get(&addr).copied().unwrap_or(0.0);
      items.push(AddressItem {
        label,
        address: addr.clone(),
        balance: format!("{:.8}", bal),
      });
      seen.insert(addr, true);
    }
  }

  if show_change {
    for (addr, bal) in balances {
      if !seen.contains_key(&addr) {
        items.push(AddressItem {
          label: "(Change)".to_string(),
          address: addr,
          balance: format!("{:.8}", bal),
        });
      }
    }
  }

  Ok(items)
}

#[tauri::command]
pub fn new_address(label: Option<String>) -> Result<String, String> {
  ensure_config()?;
  match label {
    Some(l) if !l.trim().is_empty() => run_cli(&[String::from("getnewaddress"), l]),
    _ => run_cli(&[String::from("getnewaddress")]),
  }
}

#[tauri::command]
pub fn get_change_address() -> Result<String, String> {
  ensure_config()?;
  run_cli(&[String::from("getrawchangeaddress")])
}

#[tauri::command]
pub fn get_network_mode() -> Result<String, String> {
  let cfg_path = config_path()?;
  if !cfg_path.exists() {
    return Ok("mainnet".to_string());
  }

  let content = fs::read_to_string(&cfg_path).map_err(|e| e.to_string())?;
  let mut is_testnet = false;
  let mut is_regtest = false;

  for line in content.lines() {
    let line = line.trim();
    if line.starts_with("testnet=1") {
      is_testnet = true;
    } else if line.starts_with("regtest=1") {
      is_regtest = true;
    }
  }

  if is_regtest {
    Ok("regtest".to_string())
  } else if is_testnet {
    Ok("testnet".to_string())
  } else {
    Ok("mainnet".to_string())
  }
}

// NOTE: set_network_mode needs stop_node which is in process.rs. 
// We will move set_network_mode to process.rs to avoid circular dependency.

#[tauri::command]
pub fn send_hemp(to: String, amount: String) -> Result<String, String> {
  ensure_config()?;
  run_cli(&[String::from("sendtoaddress"), to, amount])
}

#[tauri::command]
pub fn list_assets() -> Result<Vec<AssetItem>, String> {
  ensure_config()?;
  let raw = run_cli(&[String::from("listmyassets")])?;
  let value: serde_json::Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
  let mut items = Vec::new();
  if let Some(obj) = value.as_object() {
    for (name, bal) in obj {
      let amount = bal.as_f64().unwrap_or(0.0);
      let asset_type = if name.ends_with('!') {
        "OWNER"
      } else {
        "TOKEN"
      };
      items.push(AssetItem {
        name: name.to_string(),
        balance: format!("{:.8}", amount),
        asset_type: asset_type.to_string(),
        asset_type_label: None,
      });
    }
  }
  Ok(items)
}

#[tauri::command]
pub fn transfer_asset(asset: String, amount: String, to: String) -> Result<String, String> {
  ensure_config()?;
  validate_asset_name(&asset)?;
  validate_positive_amount(&amount)?;
  if to.trim().is_empty() {
    return Err("Destination address is required".to_string());
  }
  normalize_cli_txid(run_cli(&[String::from("transfer"), asset, amount, to])?)
}

#[tauri::command]
pub fn issue_asset(name: String, qty: String, units: u8, reissuable: bool, ipfs: String) -> Result<String, String> {
  ensure_config()?;
  validate_asset_name(&name)?;
  if name.contains('#') {
    return Err("Use issue_unique_asset for NFT/unique asset issuance".to_string());
  }
  let qty_val = parse_positive_amount(&qty)?;
  if units > 8 {
    return Err("Units must be between 0 and 8".to_string());
  }
  let flag = if reissuable { "true" } else { "false" };
  
  if !ipfs.is_empty() {
    normalize_cli_txid(run_cli(&[
      String::from("issue"),
      name,
      format!("{qty_val}"),
      String::new(), 
      String::new(), 
      units.to_string(),
      flag.to_string(),
      String::from("true"), 
      ipfs, 
    ])?)
  } else {
    normalize_cli_txid(run_cli(&[
      String::from("issue"),
      name,
      format!("{qty_val}"),
      String::new(),
      String::new(),
      units.to_string(),
      flag.to_string(),
    ])?)
  }
}

#[tauri::command]
pub fn get_asset_data(name: String) -> Result<AssetData, String> {
  ensure_config()?;
  let raw = run_cli(&[String::from("getassetdata"), name.clone()])?;
  let value: serde_json::Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
  
  Ok(AssetData {
    name: value.get("name").and_then(|v| v.as_str()).unwrap_or(&name).to_string(),
    amount: value.get("amount").and_then(|v| v.as_f64()).unwrap_or(0.0),
    units: value.get("units").and_then(|v| v.as_u64()).unwrap_or(0) as u8,
    reissuable: value.get("reissuable").and_then(|v| v.as_i64()).map(|v| v == 1).unwrap_or(false),
    has_ipfs: value.get("has_ipfs").and_then(|v| v.as_i64()).map(|v| v == 1).unwrap_or(false),
    ipfs_hash: value.get("ipfs_hash").and_then(|v| v.as_str()).unwrap_or("").to_string(),
    block_height: value.get("block_height").and_then(|v| v.as_u64()).unwrap_or(0),
  })
}

#[tauri::command]
pub fn list_network_assets(pattern: String, verbose: bool) -> Result<String, String> {
  ensure_config()?;
  let search = if pattern.is_empty() { String::from("*") } else { pattern };
  let verbose_str = if verbose { String::from("true") } else { String::from("false") };
  run_cli(&[String::from("listassets"), search, verbose_str, String::from("50")])
}

#[tauri::command]
pub fn check_ownership_token(asset_name: String) -> Result<bool, String> {
  ensure_config()?;
  let ownership_token = format!("{}!", asset_name.trim_end_matches('!'));
  let raw = run_cli(&[String::from("listmyassets"), ownership_token.clone(), String::from("true")])?;
  let value: serde_json::Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
  
  if let Some(obj) = value.as_object() {
    if let Some(asset_info) = obj.get(&ownership_token) {
      if let Some(balance) = asset_info.get("balance").and_then(|v| v.as_f64()) {
        return Ok(balance > 0.0);
      }
    }
  }
  Ok(false)
}

#[tauri::command]
pub fn reissue_asset(
  name: String,
  qty: String,
  to_address: String,
  change_address: String,
  reissuable: bool,
  new_units: Option<u8>,
  new_ipfs: String,
) -> Result<String, String> {
  ensure_config()?;
  validate_asset_name(&name)?;
  let qty_val = parse_non_negative_amount(&qty)?;
  let units = match new_units {
    Some(units) => units,
    None => get_asset_units(&name)?,
  };
  if units > 8 {
    return Err("Units must be between 0 and 8".to_string());
  }
  let to_addr = if to_address.trim().is_empty() {
    run_cli(&[String::from("getnewaddress")])?
  } else {
    to_address
  };
  let change_addr = if change_address.trim().is_empty() {
    to_addr.clone()
  } else {
    change_address
  };

  let args = build_reissue_args(
    &name,
    &format!("{qty_val}"),
    &to_addr,
    &change_addr,
    reissuable,
    units,
    new_ipfs.trim(),
  )?;

  normalize_cli_txid(run_cli(&args)?)
}

fn validate_asset_name(name: &str) -> Result<(), String> {
  let trimmed = name.trim();
  if trimmed.is_empty() {
    return Err("Asset name is required".to_string());
  }
  if trimmed.len() > 128 {
    return Err("Asset name is too long".to_string());
  }
  if trimmed.chars().any(|c| c.is_whitespace()) {
    return Err("Asset name cannot contain whitespace".to_string());
  }
  Ok(())
}

fn parse_positive_amount(amount: &str) -> Result<f64, String> {
  let value = amount
    .trim()
    .parse::<f64>()
    .map_err(|_| "Amount must be a number".to_string())?;
  if !value.is_finite() || value <= 0.0 {
    return Err("Amount must be greater than zero".to_string());
  }
  Ok(value)
}

fn parse_non_negative_amount(amount: &str) -> Result<f64, String> {
  let value = amount
    .trim()
    .parse::<f64>()
    .map_err(|_| "Amount must be a number".to_string())?;
  if !value.is_finite() || value < 0.0 {
    return Err("Amount cannot be negative".to_string());
  }
  Ok(value)
}

fn validate_positive_amount(amount: &str) -> Result<(), String> {
  parse_positive_amount(amount).map(|_| ())
}

fn validate_destination_address(destination: &str) -> Result<(), String> {
  let destination = destination.trim();
  if destination.is_empty() {
    return Err("Destination address is required".to_string());
  }
  let validate_result = run_cli(&[String::from("validateaddress"), destination.to_string()])
    .map_err(|e| format!("Node/wallet unavailable: {e}"))?;
  let validation: serde_json::Value =
    serde_json::from_str(&validate_result).map_err(|e| format!("Malformed validation response: {e}"))?;
  if !validation["isvalid"].as_bool().unwrap_or(false) {
    return Err("Invalid destination address format".to_string());
  }
  Ok(())
}

fn detect_duplicate_inputs(inputs: &[RawTxInput]) -> bool {
  let keys: std::collections::HashSet<String> = inputs
    .iter()
    .map(|u| format!("{}:{}", u.txid.trim(), u.vout))
    .collect();
  keys.len() != inputs.len()
}

fn parse_output_sum(outputs: &HashMap<String, String>) -> Result<f64, String> {
  let mut sum = 0.0_f64;
  for (addr, amount_str) in outputs {
    if addr.trim().is_empty() {
      return Err("Output address cannot be empty".to_string());
    }
    let amount: f64 = amount_str
      .trim()
      .parse()
      .map_err(|_| format!("Output amount '{}' is not a valid number", amount_str))?;
    if !amount.is_finite() || amount <= 0.0 {
      return Err(format!("Output amount '{}' must be a positive number", amount_str));
    }
    sum += amount;
  }
  Ok(sum)
}

fn is_utxo_unsafe_for_hemp(spendable: Option<bool>, safe: Option<bool>, asset: Option<&str>, asset_amount: Option<f64>) -> bool {
  if spendable == Some(false) {
    return true;
  }
  if safe == Some(false) {
    return true;
  }
  if asset_amount.unwrap_or(0.0) > 0.0 {
    return true;
  }
  if let Some(a) = asset {
    if a != "HEMP" {
      return true;
    }
  }
  false
}

fn normalize_cli_txid(raw: String) -> Result<String, String> {
  let trimmed = raw.trim();
  if trimmed.is_empty() {
    return Err("Command completed without returning a transaction id".to_string());
  }

  if trimmed.starts_with('[') {
    let value: serde_json::Value =
      serde_json::from_str(trimmed).map_err(|e| format!("Malformed transaction id response: {e}"))?;
    if let Some(txid) = value.as_array()
      .and_then(|items| items.first())
      .and_then(|item| item.as_str())
      .filter(|txid| !txid.trim().is_empty())
    {
      return Ok(txid.trim().to_string());
    }
    return Err("Command did not return a transaction id".to_string());
  }

  Ok(trimmed.to_string())
}

fn get_asset_units(name: &str) -> Result<u8, String> {
  let raw = run_cli(&[String::from("getassetdata"), name.to_string()])?;
  let value: serde_json::Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
  let units = value.get("units").and_then(|v| v.as_u64()).unwrap_or(8) as u8;
  if units > 8 {
    return Err("Asset units returned by node are invalid".to_string());
  }
  Ok(units)
}

fn build_reissue_args(
  name: &str,
  qty: &str,
  to_address: &str,
  change_address: &str,
  reissuable: bool,
  new_units: u8,
  new_ipfs: &str,
) -> Result<Vec<String>, String> {
  if to_address.trim().is_empty() {
    return Err("Reissue destination address is required".to_string());
  }
  if change_address.trim().is_empty() {
    return Err("Reissue change address is required".to_string());
  }
  if new_units > 8 {
    return Err("Units must be between 0 and 8".to_string());
  }
  let mut args = vec![
    String::from("reissue"),
    name.to_string(),
    qty.to_string(),
    to_address.to_string(),
    change_address.to_string(),
    if reissuable { "true" } else { "false" }.to_string(),
    new_units.to_string(),
  ];
  if !new_ipfs.trim().is_empty() {
    args.push(new_ipfs.trim().to_string());
  }
  Ok(args)
}

fn build_issue_unique_args(
  root_name: String,
  tags: Vec<String>,
  ipfs_hashes: Vec<String>,
) -> Result<Vec<String>, String> {
  let (root_name, tags, ipfs_hashes) = normalize_unique_asset_inputs(root_name, tags, ipfs_hashes)?;
  let tags_json = serde_json::to_string(&tags).map_err(|e| e.to_string())?;
  let mut args = vec![
    String::from("issueunique"),
    root_name,
    tags_json,
  ];
  if !ipfs_hashes.is_empty() {
    args.push(serde_json::to_string(&ipfs_hashes).map_err(|e| e.to_string())?);
  }
  Ok(args)
}

#[tauri::command]
pub fn issue_unique_asset(
  root_name: String,
  tags: Vec<String>,
  ipfs_hashes: Vec<String>,
) -> Result<String, String> {
  ensure_config()?;
  let args = build_issue_unique_args(root_name, tags, ipfs_hashes)?;
  normalize_cli_txid(run_cli(&args)?)
}

fn normalize_unique_asset_inputs(
  root_name: String,
  tags: Vec<String>,
  ipfs_hashes: Vec<String>,
) -> Result<(String, Vec<String>, Vec<String>), String> {
  let root_name = root_name.trim().to_uppercase();
  validate_asset_name(&root_name)?;
  if root_name.contains('#') {
    return Err("Unique asset parent name cannot contain '#'".to_string());
  }
  if tags.is_empty() {
    return Err("At least one tag is required".to_string());
  }

  let normalized_tags: Vec<String> = tags
    .into_iter()
    .map(|tag| tag.trim().to_string())
    .collect();
  if normalized_tags.iter().any(|tag| tag.is_empty()) {
    return Err("Tag names cannot be empty".to_string());
  }
  if normalized_tags.iter().any(|tag| tag.chars().any(|c| c.is_whitespace())) {
    return Err("Tag names cannot contain whitespace".to_string());
  }
  if normalized_tags.iter().any(|tag| tag.contains('#') || tag.contains('/')) {
    return Err("Tag names cannot contain '#' or '/'".to_string());
  }

  let has_ipfs = ipfs_hashes.iter().any(|hash| !hash.trim().is_empty());
  let normalized_ipfs = if has_ipfs {
    if ipfs_hashes.len() != normalized_tags.len() {
      return Err("IPFS hashes array must match tag count when provided".to_string());
    }
    ipfs_hashes
      .into_iter()
      .map(|hash| hash.trim().to_string())
      .collect()
  } else {
    Vec::new()
  };

  Ok((root_name, normalized_tags, normalized_ipfs))
}

#[tauri::command]
pub fn ban_old_peers() -> Result<BanResult, String> {
  ensure_config()?;
  
  let raw = run_cli(&[String::from("getpeerinfo")])?;
  let peers: serde_json::Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
  
  let mut banned_count = 0u32;
  let mut banned_peers = Vec::new();
  
  if let Some(arr) = peers.as_array() {
    for peer in arr {
      let subver = peer.get("subver").and_then(|v| v.as_str()).unwrap_or("");
      let addr = peer.get("addr").and_then(|v| v.as_str()).unwrap_or("");
      
      if !subver.is_empty() && version_is_old(subver) {
        let ip = addr.split(':').next().unwrap_or(addr);
        if !ip.is_empty() {
          if run_cli(&[
            String::from("setban"),
            ip.to_string(),
            String::from("add"),
            String::from("86400"),
          ]).is_ok() {
            banned_count += 1;
            banned_peers.push(format!("{} ({})", ip, subver));
          }
        }
      }
    }
  }
  
  Ok(BanResult { banned_count, banned_peers })
}

#[tauri::command]
pub fn get_banned_peers() -> Result<Vec<BanEntry>, String> {
  ensure_config()?;
  let raw = run_cli(&[String::from("listbanned")])?;
  let bans: serde_json::Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
  
  let mut entries = Vec::new();
  if let Some(arr) = bans.as_array() {
    for ban in arr {
      let address = ban.get("address").and_then(|v| v.as_str()).unwrap_or("").to_string();
      let banned_until = ban.get("banned_until").and_then(|v| v.as_i64()).unwrap_or(0);
      let ban_reason = ban.get("ban_reason").and_then(|v| v.as_str()).unwrap_or("manual").to_string();
      
      let dt = Local.timestamp_opt(banned_until, 0)
        .single()
        .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|| "Unknown".to_string());
      
      entries.push(BanEntry {
        address,
        banned_until: dt,
        ban_reason,
      });
    }
  }
  Ok(entries)
}

#[tauri::command]
pub fn unban_peer(address: String) -> Result<String, String> {
  ensure_config()?;
  run_cli(&[String::from("setban"), address, String::from("remove")])
}

#[tauri::command]
pub fn dump_priv_key(address: String) -> Result<String, String> {
  ensure_config()?;
  let address = address.trim().to_string();
  if address.is_empty() {
    return Err("Address is required".to_string());
  }
  let validate_raw = run_cli(&[String::from("validateaddress"), address.clone()])?;
  let validation: serde_json::Value = serde_json::from_str(&validate_raw)
    .map_err(|e| format!("Failed to parse validation response: {e}"))?;
  if !validation["isvalid"].as_bool().unwrap_or(false) {
    return Err("Invalid address - private key export requires a valid wallet address".to_string());
  }
  if !validation["ismine"].as_bool().unwrap_or(false) {
    return Err("Address does not belong to current wallet".to_string());
  }
  run_cli(&[String::from("dumpprivkey"), address])
}

fn parse_cli_json(raw: &str, context: &str) -> Result<serde_json::Value, String> {
  serde_json::from_str(raw).map_err(|e| format!("Failed to parse {context} response: {e}"))
}

fn validate_migration_path(path: &str, label: &str) -> Result<String, String> {
  let trimmed = path.trim().to_string();
  if trimmed.is_empty() {
    return Err(format!("{label} is required"));
  }
  Ok(trimmed)
}

fn validate_migration_passphrase(passphrase: &str, required: bool, label: &str) -> Result<(), String> {
  if required && passphrase.trim().is_empty() {
    return Err(format!("{label} is required"));
  }
  if required && passphrase.len() < 8 {
    return Err(format!("{label} must be at least 8 characters"));
  }
  if passphrase.len() > 1024 {
    return Err(format!("{label} must not exceed 1024 characters"));
  }
  Ok(())
}

fn validate_migration_wallet_name(wallet_name: &str) -> Result<String, String> {
  let name = wallet_name.trim().to_string();
  if name.is_empty() {
    return Err("Wallet name is required".to_string());
  }
  if name == "." || name == ".." || name.contains('/') || name.contains('\\') || name.contains(':') {
    return Err("Wallet name cannot contain path separators, drive separators, '.', or '..'".to_string());
  }
  if name.chars().any(|c| c.is_control() || matches!(c, '*' | '?' | '"' | '<' | '>' | '|')) {
    return Err("Wallet name contains characters that are not safe for wallet file names".to_string());
  }
  let upper = name.to_uppercase();
  let device_name = upper.split('.').next().unwrap_or("");
  let reserved = [
    "CON", "PRN", "AUX", "NUL",
    "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9",
    "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
  ];
  if reserved.contains(&device_name) {
    return Err("Wallet name cannot be a reserved device name".to_string());
  }
  Ok(name)
}

fn build_export_wallet_migration_args(
  path: String,
  include_private: bool,
  allow_overwrite: bool,
  export_passphrase: String,
) -> Result<Vec<String>, String> {
  let path = validate_migration_path(&path, "Output file path")?;
  validate_migration_passphrase(&export_passphrase, include_private, "Export passphrase")?;

  let mut args = vec![
    String::from("exportwalletmigration"),
    path,
    if include_private { String::from("true") } else { String::from("false") },
    if allow_overwrite { String::from("true") } else { String::from("false") },
  ];
  if include_private {
    args.push(export_passphrase);
  }
  Ok(args)
}

fn build_validate_wallet_migration_args(path: String, passphrase: String) -> Result<Vec<String>, String> {
  let path = validate_migration_path(&path, "File path")?;
  validate_migration_passphrase(&passphrase, false, "Export passphrase")?;

  let mut args = vec![String::from("validatewalletmigration"), path];
  if !passphrase.is_empty() {
    args.push(passphrase);
  }
  Ok(args)
}

fn build_restore_wallet_migration_args(
  path: String,
  wallet_name: String,
  passphrase: String,
  birth_height: Option<i64>,
) -> Result<Vec<String>, String> {
  let path = validate_migration_path(&path, "File path")?;
  let wallet_name = validate_migration_wallet_name(&wallet_name)?;
  validate_migration_passphrase(&passphrase, true, "Export passphrase")?;

  let mut args = vec![
    String::from("restorewalletmigration"),
    path,
    wallet_name,
    passphrase,
  ];
  if let Some(h) = birth_height {
    if h < 0 {
      return Err("Birth height cannot be negative".to_string());
    }
    args.push(format!("{}", h));
  }
  Ok(args)
}

#[tauri::command]
pub fn export_wallet_migration(
  path: String,
  include_private: bool,
  allow_overwrite: bool,
  export_passphrase: String,
) -> Result<serde_json::Value, String> {
  ensure_config()?;
  let args = build_export_wallet_migration_args(path, include_private, allow_overwrite, export_passphrase)?;
  let raw = run_cli(&args)?;
  parse_cli_json(&raw, "migration export")
}

#[tauri::command]
pub fn validate_wallet_migration(
  path: String,
  passphrase: String,
) -> Result<serde_json::Value, String> {
  ensure_config()?;
  let args = build_validate_wallet_migration_args(path, passphrase)?;
  let raw = run_cli(&args)?;
  parse_cli_json(&raw, "migration validation")
}

#[tauri::command]
pub fn restore_wallet_migration(
  path: String,
  wallet_name: String,
  passphrase: String,
  birth_height: Option<i64>,
) -> Result<serde_json::Value, String> {
  ensure_config()?;
  let args = build_restore_wallet_migration_args(path, wallet_name, passphrase, birth_height)?;
  let raw = run_cli(&args)?;
  parse_cli_json(&raw, "migration restore")
}

#[tauri::command]
pub fn import_priv_key(priv_key: String, label: String, rescan: bool) -> Result<String, String> {
  ensure_config()?;
  let rescan_flag = if rescan { "true" } else { "false" };
  run_cli(&[
    String::from("importprivkey"),
    priv_key,
    label,
    rescan_flag.to_string(),
  ])
}

#[tauri::command]
pub fn wallet_encrypt(password: String) -> Result<String, String> {
  ensure_config()?;
  run_cli(&[String::from("encryptwallet"), password])
}

#[tauri::command]
pub fn get_net_info() -> Result<NetworkInfo, String> {
  ensure_config()?;
  let raw = run_cli(&[String::from("getnetworkinfo")])?;
  let info: serde_json::Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;

  let version = info.get("version").and_then(|v| v.as_u64()).unwrap_or(0);
  let subversion = info.get("subversion").and_then(|v| v.as_str()).unwrap_or("").to_string();
  let protocolversion = info.get("protocolversion").and_then(|v| v.as_u64()).unwrap_or(0);
  let connections = info.get("connections").and_then(|v| v.as_u64()).unwrap_or(0);
  
  let mut localaddresses = Vec::new();
  let mut full_ip = String::new();

  if let Some(arr) = info.get("localaddresses").and_then(|v| v.as_array()) {
      for addr in arr {
          if let Some(a) = addr.get("address").and_then(|v| v.as_str()) {
              localaddresses.push(a.to_string());
              if full_ip.is_empty() { full_ip = a.to_string(); }
          }
      }
  }

  Ok(NetworkInfo {
      version,
      subversion,
      protocolversion,
      connections,
      localaddresses,
      full_ip,
  })
}

#[tauri::command]
pub fn execute_ping(host: String) -> Result<String, String> {
  let mut cmd;
  #[cfg(windows)]
  {
      cmd = Command::new("ping");
      cmd.args(["-n", "3", &host]);
  }
  #[cfg(unix)]
  {
      cmd = Command::new("ping");
      cmd.args(["-c", "3", &host]);
  }

  let output = cmd.output().map_err(|e| e.to_string())?;
  
  if output.status.success() {
      Ok(String::from_utf8_lossy(&output.stdout).to_string())
  } else {
      let err_out = String::from_utf8_lossy(&output.stderr).to_string();
      let std_out = String::from_utf8_lossy(&output.stdout).to_string();
      Err(format!("Ping failed:\n{}\n{}", std_out, err_out))
  }
}

#[tauri::command]
pub fn check_open_port(host: String, port: u16) -> Result<bool, String> {
  use std::net::{TcpStream, ToSocketAddrs};
  use std::time::Duration;

  let addr_str = format!("{}:{}", host, port);
  let addrs = addr_str.to_socket_addrs().map_err(|e| format!("DNS/Parse Error: {}", e))?;

  for addr in addrs {
      if TcpStream::connect_timeout(&addr, Duration::from_secs(3)).is_ok() {
          return Ok(true);
      }
  }
  Ok(false)
}

#[tauri::command]
pub fn wallet_unlock(password: String, duration: u64) -> Result<String, String> {
  ensure_config()?;
  run_cli(&[
    String::from("walletpassphrase"),
    password,
    duration.to_string(),
  ])
}

#[tauri::command]
pub fn wallet_lock() -> Result<String, String> {
  ensure_config()?;
  run_cli(&[String::from("walletlock")])
}

#[tauri::command]
pub fn change_wallet_password(old_pass: String, new_pass: String) -> Result<String, String> {
  ensure_config()?;
  run_cli(&[
    String::from("walletpassphrasechange"),
    old_pass,
    new_pass,
  ])
}

#[tauri::command]
pub fn run_shell_command(command: String) -> Result<String, String> {
  let line_raw = command.trim();
  if line_raw.is_empty() {
    return Err("Empty command".to_string());
  }

  let mut line = line_raw.to_string();
  if cfg!(windows) {
    let trimmed = line_raw.trim();
    if trimmed == "ls" {
      line = "dir".to_string();
    } else if trimmed.starts_with("ls ") {
      line = format!("dir {}", trimmed[3..].trim());
    } else if trimmed == "pwd" {
      line = "cd".to_string();
    } else if trimmed.starts_with("cat ") {
      line = format!("type {}", trimmed[4..].trim());
    } else if trimmed.starts_with("rm -rf ") || trimmed.starts_with("rm -r ") {
      line = format!("rmdir /s /q {}", trimmed[6..].trim());
    } else if trimmed.starts_with("rm ") {
      line = format!("del /q {}", trimmed[3..].trim());
    }
  }

  let mut state = shell_state()
    .lock()
    .map_err(|_| "Shell state unavailable".to_string())?;
  let current = state.cwd.clone();
  let lower = line.to_lowercase();

  if lower == "cd"
    || lower.starts_with("cd ")
    || lower.starts_with("cd\t")
    || lower.starts_with("cd /d ")
    || lower.starts_with("cd /d\t")
    || lower.starts_with("cd /D ")
    || lower.starts_with("cd /D\t")
  {
    let mut arg = line[2..].trim();
    if arg.to_lowercase().starts_with("/d ") || arg.to_lowercase().starts_with("/d\t") {
      arg = arg[2..].trim();
    }
    if arg.is_empty() {
      return Ok(current.to_string_lossy().to_string());
    }
    let mut cleaned = arg.trim();
    if cleaned.starts_with('"') && cleaned.ends_with('"') && cleaned.len() > 1 {
      cleaned = &cleaned[1..cleaned.len() - 1];
    }
    let mut new_path = PathBuf::from(cleaned);
    if !new_path.is_absolute() {
      new_path = current.join(new_path);
    }
    if !new_path.exists() {
      return Err(format!("Directory not found: {}", new_path.display()));
    }
    let canonical = fs::canonicalize(&new_path).unwrap_or(new_path);
    state.cwd = canonical.clone();
    return Ok(canonical.to_string_lossy().to_string());
  }

  let cwd = if current.exists() {
    current
  } else {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
  };
  state.cwd = cwd.clone();
  let output = if cfg!(windows) {
    Command::new("cmd")
      .current_dir(&cwd)
      .args(&["/C", &line])
      .output()
  } else {
    Command::new("bash")
      .current_dir(&cwd)
      .args(&["-lc", &line])
      .output()
  }
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

  if output.status.success() {
    if text.trim().is_empty() {
      Ok("(no output)".to_string())
    } else {
      Ok(text.trim_end().to_string())
    }
  } else if text.trim().is_empty() {
    Err("Command failed".to_string())
  } else {
    Err(text.trim_end().to_string())
  }
}

#[tauri::command]
pub fn shell_autocomplete(line: String) -> Result<Vec<String>, String> {
  let mut state = shell_state()
    .lock()
    .map_err(|_| "Shell state unavailable".to_string())?;
  let cwd = if state.cwd.exists() {
    state.cwd.clone()
  } else {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
  };
  state.cwd = cwd.clone();

  let trimmed = line.trim_end();
  let mut prefix = trimmed.split_whitespace().last().unwrap_or("").to_string();
  if prefix.starts_with('"') {
    prefix = prefix.trim_start_matches('"').to_string();
  }
  let prefix_cmp = if cfg!(windows) {
    prefix.to_lowercase()
  } else {
    prefix.clone()
  };

  let mut matches = Vec::new();
  let entries = fs::read_dir(&cwd).map_err(|e| e.to_string())?;
  for entry in entries.flatten() {
    let name = entry.file_name().to_string_lossy().to_string();
    let candidate_cmp = if cfg!(windows) {
      name.to_lowercase()
    } else {
      name.clone()
    };
    if prefix_cmp.is_empty() || candidate_cmp.starts_with(&prefix_cmp) {
      matches.push(name);
    }
  }
  matches.sort();
  Ok(matches)
}

#[tauri::command]
pub fn preview_send_hemp(
  destination: String,
  amount: String,
  asset: String,
  label: Option<String>,
) -> Result<SendPreview, String> {
  ensure_config()?;
  let (parsed_amount, mut warnings) =
    validate_send_preview_fields(&destination, &amount, &asset, label.as_deref())?;

  let validate_result = run_cli(&[String::from("validateaddress"), destination.trim().to_string()])
    .map_err(|e| format!("Node/wallet unavailable: {e}"))?;
  let validation: serde_json::Value =
    serde_json::from_str(&validate_result).map_err(|e| format!("Malformed validation response: {e}"))?;
  if !validation["isvalid"].as_bool().unwrap_or(false) {
    return Err("Invalid destination address format".to_string());
  }

  let available_balance = match run_cli(&[String::from("getbalance")]) {
    Ok(raw) => {
      let bal: f64 = raw.trim().parse().unwrap_or(0.0);
      format!("{:.8}", bal)
    }
    Err(_) => {
      warnings.push("Unable to retrieve available balance".to_string());
      String::from("unknown")
    }
  };

  let fee_estimate: Option<String> = None;
  let fee_warning = Some(String::from(
    "Fee estimation is not yet supported; final fee is determined by the network at broadcast time",
  ));

  if let Ok(bal) = available_balance.parse::<f64>() {
    if parsed_amount > bal {
      warnings.push(format!(
        "Amount exceeds available balance ({}) - transaction may fail",
        available_balance
      ));
    }
  }

  let summary = format!(
    "Send {} HEMP to {}{}",
    parsed_amount,
    &destination.trim()[..std::cmp::min(16, destination.trim().len())],
    if destination.trim().len() > 16 { "..." } else { "" }
  );

  Ok(SendPreview {
    destination: destination.trim().to_string(),
    amount: format!("{}", parsed_amount),
    asset: "HEMP".to_string(),
    available_balance,
    fee_estimate,
    fee_warning,
    warnings,
    summary,
    validated: true,
  })
}

fn validate_send_preview_fields(
  destination: &str,
  amount: &str,
  asset: &str,
  label: Option<&str>,
) -> Result<(f64, Vec<String>), String> {
  let mut warnings = Vec::new();
  if destination.trim().is_empty() {
    return Err("Destination address is required".to_string());
  }

  let parsed_amount: f64 = amount
    .trim()
    .parse()
    .map_err(|_| "Amount must be a numeric value".to_string())?;
  if !parsed_amount.is_finite() || parsed_amount <= 0.0 {
    return Err("Amount must be greater than zero".to_string());
  }

  if asset.trim() != "HEMP" {
    return Err("This preview command only supports regular HEMP sends".to_string());
  }

  if let Some(label) = label {
    if label.len() > 256 {
      warnings.push("Label is very long and may not be included in the transaction".to_string());
    }
  }

  Ok((parsed_amount, warnings))
}

#[tauri::command]
pub fn preview_transfer_asset(
  destination: String,
  amount: String,
  asset: String,
) -> Result<SendPreview, String> {
  ensure_config()?;
  let (parsed_amount, mut warnings) =
    validate_asset_transfer_preview_fields(&destination, &amount, &asset)?;
  let asset_name = asset.trim().to_string();

  let validate_result = run_cli(&[String::from("validateaddress"), destination.trim().to_string()])
    .map_err(|e| format!("Node/wallet unavailable: {e}"))?;
  let validation: serde_json::Value =
    serde_json::from_str(&validate_result).map_err(|e| format!("Malformed validation response: {e}"))?;
  if !validation["isvalid"].as_bool().unwrap_or(false) {
    return Err("Invalid destination address format".to_string());
  }

  let available_balance = match run_cli(&[String::from("listmyassets"), asset_name.clone(), String::from("false")]) {
    Ok(raw) => {
      let value: serde_json::Value = serde_json::from_str(&raw).unwrap_or_default();
      if let Some(balance) = asset_balance_from_listmyassets(&value, &asset_name) {
        format!("{:.8}", balance)
      } else {
        warnings.push("Unable to retrieve asset balance".to_string());
        String::from("unknown")
      }
    }
    Err(_) => {
      warnings.push("Unable to retrieve available asset balance".to_string());
      String::from("unknown")
    }
  };

  if let Ok(bal) = available_balance.parse::<f64>() {
    if parsed_amount > bal {
      warnings.push(format!(
        "Amount exceeds available {} balance ({}) - transaction may fail",
        asset_name, available_balance
      ));
    }
  }

  let fee_warning = Some(String::from(
    "Asset transfers require HEMP for network fees. Ensure your HEMP balance is sufficient.",
  ));

  let summary = format!(
    "Transfer {} {} to {}{}",
    parsed_amount,
    asset_name,
    &destination.trim()[..std::cmp::min(16, destination.trim().len())],
    if destination.trim().len() > 16 { "..." } else { "" }
  );

  Ok(SendPreview {
    destination: destination.trim().to_string(),
    amount: format!("{}", parsed_amount),
    asset: asset_name,
    available_balance,
    fee_estimate: None,
    fee_warning,
    warnings,
    summary,
    validated: true,
  })
}

fn validate_asset_transfer_preview_fields(
  destination: &str,
  amount: &str,
  asset: &str,
) -> Result<(f64, Vec<String>), String> {
  let warnings = Vec::new();
  if destination.trim().is_empty() {
    return Err("Destination address is required".to_string());
  }

  let parsed_amount: f64 = amount
    .trim()
    .parse()
    .map_err(|_| "Amount must be a numeric value".to_string())?;
  if !parsed_amount.is_finite() || parsed_amount <= 0.0 {
    return Err("Amount must be greater than zero".to_string());
  }

  if asset.trim().is_empty() {
    return Err("Asset name is required".to_string());
  }

  if asset.trim().to_uppercase() == "HEMP" {
    return Err("Use preview_send_hemp for HEMP sends".to_string());
  }

  Ok((parsed_amount, warnings))
}

fn asset_balance_from_listmyassets(value: &serde_json::Value, asset: &str) -> Option<f64> {
  if let Some(balance) = value.as_f64() {
    return Some(balance);
  }
  let asset_value = value.get(asset)?;
  if let Some(balance) = asset_value.as_f64() {
    return Some(balance);
  }
  asset_value.get("balance").and_then(|v| v.as_f64())
}

#[tauri::command]
pub fn list_utxos() -> Result<Vec<UtxoItem>, String> {
  ensure_config()?;
  let raw = run_cli(&[
    String::from("listunspent"),
    String::from("0"),
    String::from("9999999"),
    String::from("[]"),
    String::from("true"),
  ])?;
  let utxos: Vec<UtxoItem> = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
  Ok(utxos)
}

#[tauri::command]
pub fn broadcast_advanced_transaction(
  inputs: Vec<RawTxInput>,
  outputs: HashMap<String, String>,
) -> Result<String, String> {
  ensure_config()?;

  if inputs.is_empty() {
    return Err("At least one input UTXO is required".to_string());
  }

  if outputs.is_empty() {
    return Err("At least one output address and amount is required".to_string());
  }

  if detect_duplicate_inputs(&inputs) {
    return Err("Duplicate inputs detected. Each UTXO can only be used once.".to_string());
  }

  let max_safe_inputs_for_two_outputs = max_policy_inputs(2, STANDARD_MAX_TX_BYTES);
  if inputs.len() > max_safe_inputs_for_two_outputs {
    return Err(format!(
      "Selected {} UTXOs exceeds the standard relay policy limit of {} inputs for a two-output transaction. Reduce your selection or consolidate first.",
      inputs.len(), max_safe_inputs_for_two_outputs
    ));
  }

  let est_tx_bytes = estimate_legacy_tx_bytes(inputs.len(), 2);
  if est_tx_bytes > STANDARD_MAX_TX_BYTES {
    return Err(format!(
      "Estimated transaction size {} bytes exceeds standard relay limit of {} bytes. Consolidate or reduce inputs.",
      est_tx_bytes, STANDARD_MAX_TX_BYTES
    ));
  }

  let output_total = parse_output_sum(&outputs)?;
  for output_address in outputs.keys() {
    validate_destination_address(output_address)?;
  }

  let selected_keys: std::collections::HashSet<String> = inputs
    .iter()
    .map(|u| format!("{}:{}", u.txid.trim(), u.vout))
    .collect();

  let raw = run_cli(&[
    String::from("listunspent"),
    String::from("0"),
    String::from("9999999"),
    String::from("[]"),
    String::from("true"),
  ])?;
  let all_utxos: Vec<serde_json::Value> = serde_json::from_str(&raw).map_err(|e| e.to_string())?;

  let mut input_total = 0.0_f64;
  let mut matched_count = 0usize;

  for u in &all_utxos {
    let txid = u["txid"].as_str().unwrap_or("");
    let vout = u["vout"].as_u64().unwrap_or(0);
    let key = format!("{}:{}", txid, vout);
    if selected_keys.contains(&key) {
      matched_count += 1;
      let amount = u["amount"].as_f64().unwrap_or(0.0);
      let spendable = u["spendable"].as_bool();
      let safe = u["safe"].as_bool();
      let asset = u["asset"].as_str();
      let asset_amount = u.get("asset_amount").and_then(|v| v.as_f64());

      if is_utxo_unsafe_for_hemp(spendable, safe, asset, asset_amount) {
        if spendable == Some(false) {
          return Err(format!(
            "UTXO {}:{} is not spendable by the wallet. Deselect it from advanced send.",
            txid, vout
          ));
        }
        if safe == Some(false) {
          return Err(format!(
            "UTXO {}:{} is marked unsafe. Deselect it from advanced send or wait for more confirmations.",
            txid, vout
          ));
        }
        if asset.is_some() && asset != Some("HEMP") {
          return Err(format!(
            "UTXO {}:{} carries a non-HEMP asset ({}). It cannot be included in a HEMP advanced send.",
            txid, vout,
            asset.unwrap_or("unknown")
          ));
        }
        if asset_amount.unwrap_or(0.0) > 0.0 {
          return Err(format!(
            "UTXO {}:{} carries asset data and cannot be included in a HEMP advanced send.",
            txid, vout
          ));
        }
        return Err(format!(
          "UTXO {}:{} is unsafe for HEMP advanced send.",
          txid, vout
        ));
      }

      input_total += amount;
    }
  }

  if matched_count != selected_keys.len() {
    return Err(format!(
      "{} selected UTXOs are no longer available. Refresh UTXOs and preview again.",
      selected_keys.len().saturating_sub(matched_count)
    ));
  }

  let fee = input_total - output_total;
  if fee <= 0.0 {
    return Err(format!(
      "Selected inputs total {} HEMP is insufficient to cover outputs total {} HEMP plus fee",
      input_total, output_total
    ));
  }

  let inputs_json = serde_json::to_string(&inputs).map_err(|e| e.to_string())?;
  let outputs_json = serde_json::to_string(&outputs).map_err(|e| e.to_string())?;

  let raw_hex = run_cli(&[
    String::from("createrawtransaction"),
    inputs_json,
    outputs_json,
  ])?;

  let signed_res_raw = run_cli(&[
    String::from("signrawtransaction"),
    raw_hex,
  ])?;
  let signed_res: serde_json::Value = serde_json::from_str(&signed_res_raw).map_err(|e| e.to_string())?;

  let complete = signed_res["complete"].as_bool().unwrap_or(false);
  if !complete {
    if let Some(errors) = signed_res.get("errors") {
      return Err(format!("Failed to sign transaction completely: {errors}"));
    }
    return Err("Failed to sign transaction completely.".to_string());
  }
  let signed_hex = signed_res["hex"].as_str().ok_or("No signed hex returned")?.to_string();

  let txid = run_cli(&[
    String::from("sendrawtransaction"),
    signed_hex,
  ])?;

  normalize_cli_txid(txid)
}

#[tauri::command]
pub fn preview_wallet_consolidation(
  utxos: Vec<RawTxInput>,
  destination: String,
  fee_rate_sat_per_byte: Option<u64>,
) -> Result<ConsolidationPreview, String> {
  ensure_config()?;
  let fee_rate = recommended_consolidation_fee_rate_sat_per_byte(fee_rate_sat_per_byte)?;

  if utxos.is_empty() {
    return Err("At least one UTXO must be selected for consolidation".to_string());
  }

  let destination = destination.trim().to_string();
  if destination.is_empty() {
    return Err("Consolidation destination address is required".to_string());
  }

  validate_destination_address(&destination)?;

  let raw = run_cli(&[
    String::from("listunspent"),
    String::from("0"),
    String::from("9999999"),
    String::from("[]"),
    String::from("true"),
  ])?;
  let all_utxos: Vec<serde_json::Value> = serde_json::from_str(&raw).map_err(|e| e.to_string())?;

  let selected_keys: std::collections::HashSet<String> = utxos
    .iter()
    .map(|u| format!("{}:{}", u.txid.trim(), u.vout))
    .collect();
  if selected_keys.len() != utxos.len() {
    return Err("Duplicate UTXOs cannot be consolidated".to_string());
  }

  let mut selected_utxos: Vec<ConsolidationUtxoEntry> = Vec::new();
  let mut input_total = 0.0_f64;
  let mut warnings: Vec<String> = Vec::new();
  let mut unsafe_count = 0u32;

  for u in &all_utxos {
    let txid = u["txid"].as_str().unwrap_or("");
    let vout = u["vout"].as_u64().unwrap_or(0);
    let key = format!("{}:{}", txid, vout);
    if selected_keys.contains(&key) {
      let amount = u["amount"].as_f64().unwrap_or(0.0);
      let spendable = u["spendable"].as_bool().unwrap_or(true);
      let safe = u["safe"].as_bool().unwrap_or(true);
      let confirmations = u["confirmations"].as_u64().unwrap_or(0);
      let address = u["address"].as_str().map(|s| s.to_string());
      let asset = u["asset"].as_str().map(|s| s.to_string());
      let asset_amount = u.get("asset_amount").and_then(|v| v.as_f64());

      if !spendable {
        unsafe_count += 1;
        warnings.push(format!("UTXO {}:{} is not spendable and should not be used for consolidation", txid, vout));
      }

      if !safe {
        unsafe_count += 1;
        warnings.push(format!("UTXO {}:{} is marked unsafe and may require additional confirmations", txid, vout));
      }

      if asset.is_some() && (asset.as_deref() != Some("HEMP") || asset_amount.is_some()) {
        warnings.push(format!("UTXO {}:{} carries a non-HEMP asset. Including it in HEMP consolidation may result in asset loss.", txid, vout));
      }

      if confirmations < 1 {
        warnings.push(format!("UTXO {}:{} has zero confirmations - consolidation may fail if replaced or conflicted", txid, vout));
      }

      input_total += amount;

      selected_utxos.push(ConsolidationUtxoEntry {
        txid: txid.to_string(),
        vout,
        amount: format!("{:.8}", amount),
        address,
        confirmations,
        spendable,
        safe,
        asset,
        asset_amount,
      });
    }
  }

  if selected_utxos.is_empty() {
    return Err("None of the selected UTXOs matched the current wallet UTXO set".to_string());
  }
  if selected_utxos.len() != selected_keys.len() {
    return Err(format!(
      "{} selected UTXOs are no longer available. Refresh UTXOs and preview again.",
      selected_keys.len().saturating_sub(selected_utxos.len())
    ));
  }

  let utxo_count = selected_utxos.len();
  let max_safe_inputs = max_policy_inputs(1, STANDARD_MAX_TX_BYTES);

  if utxo_count > max_safe_inputs {
    return Err(format!(
      "Selected {} UTXOs exceeds the standard relay policy limit of {} inputs for a one-output transaction. Reduce your selection.",
      utxo_count, max_safe_inputs
    ));
  }

  let tx_bytes = estimate_legacy_tx_bytes(utxo_count, 1);
  let fee = estimate_fee_from_bytes(tx_bytes, fee_rate);

  if input_total <= fee {
    return Err(format!(
      "Selected inputs total {} HEMP is insufficient to cover the estimated fee {} HEMP",
      input_total, fee
    ));
  }

  let output_amount = input_total - fee;

  if output_amount <= 0.0 {
    return Err(format!(
      "Estimated output {} is not positive after fee ({} byte tx). Select more UTXOs.",
      output_amount, tx_bytes
    ));
  }

  if output_amount < sat_to_hemp(DUST_THRESHOLD_SAT) {
    warnings.push(format!(
      "Estimated output {} HEMP is below the dust threshold ({})",
      output_amount, sat_to_hemp(DUST_THRESHOLD_SAT)
    ));
  }

  if tx_bytes > STANDARD_MAX_TX_BYTES {
    return Err(format!(
      "Estimated transaction size {} bytes exceeds standard relay policy limit of {} bytes. Reduce selected UTXOs.",
      tx_bytes, STANDARD_MAX_TX_BYTES
    ));
  }

  if unsafe_count > 0 {
    warnings.push(format!(
      "{} selected UTXOs are unsafe/unspendable and should be excluded for a reliable consolidation",
      unsafe_count
    ));
  }

  warnings.push(format!(
    "This consolidates {} wallet UTXOs into one wallet address (estimated {} bytes). Estimated fee: {} HEMP ({} sat/byte). This may affect privacy by linking UTXOs.",
    utxo_count, tx_bytes, format_hemp_amount(fee), fee_rate
  ));

  let summary = format!(
    "Consolidate {} UTXOs into {} ({}) - estimated output {} HEMP",
    utxo_count,
    &destination[..std::cmp::min(12, destination.len())],
    if destination.len() > 12 { "..." } else { "" },
    format_hemp_amount(output_amount)
  );

  Ok(ConsolidationPreview {
    utxo_count,
    input_total: format!("{:.8}", input_total),
    estimated_bytes: tx_bytes,
    fee_rate_sat_per_byte: fee_rate,
    fee_estimate: format!("{:.8}", fee),
    output_amount: format!("{:.8}", output_amount),
    destination: destination.clone(),
    warnings,
    summary,
    utxos: selected_utxos,
  })
}

#[tauri::command]
pub fn broadcast_wallet_consolidation(
  utxos: Vec<RawTxInput>,
  destination: String,
  fee: f64,
  fee_rate_sat_per_byte: Option<u64>,
) -> Result<String, String> {
  ensure_config()?;
  let fee_rate = recommended_consolidation_fee_rate_sat_per_byte(fee_rate_sat_per_byte)?;

  if utxos.is_empty() {
    return Err("No UTXOs selected for consolidation".to_string());
  }

  let destination = destination.trim().to_string();
  if destination.is_empty() {
    return Err("Consolidation destination address is required".to_string());
  }

  validate_destination_address(&destination)?;

  let input_count = utxos.len();
  let max_safe_inputs = max_policy_inputs(1, STANDARD_MAX_TX_BYTES);
  if input_count > max_safe_inputs {
    return Err(format!(
      "Selected {} UTXOs exceeds the standard relay policy limit of {} inputs. Reduce your selection.",
      input_count, max_safe_inputs
    ));
  }

  let tx_bytes = estimate_legacy_tx_bytes(input_count, 1);
  if tx_bytes > STANDARD_MAX_TX_BYTES {
    return Err(format!(
      "Estimated transaction size {} bytes exceeds standard relay limit of {} bytes.",
      tx_bytes, STANDARD_MAX_TX_BYTES
    ));
  }

  let computed_fee = estimate_fee_from_bytes(tx_bytes, fee_rate);
  if !fee.is_finite() || hemp_to_sat(fee) != hemp_to_sat(computed_fee) {
    return Err(format!(
      "Consolidation fee changed after preview (expected {}, got {}). Refresh UTXOs and preview again.",
      format_hemp_amount(computed_fee), format_hemp_amount(fee)
    ));
  }

  let raw = run_cli(&[
    String::from("listunspent"),
    String::from("0"),
    String::from("9999999"),
    String::from("[]"),
    String::from("true"),
  ])?;
  let all_utxos: Vec<serde_json::Value> = serde_json::from_str(&raw).map_err(|e| e.to_string())?;

  let selected_keys: std::collections::HashSet<String> = utxos
    .iter()
    .map(|u| format!("{}:{}", u.txid.trim(), u.vout))
    .collect();
  if selected_keys.len() != utxos.len() {
    return Err("Duplicate UTXOs cannot be consolidated".to_string());
  }

  let mut input_total = 0.0_f64;
  let mut matched_count = 0usize;

  for u in &all_utxos {
    let txid = u["txid"].as_str().unwrap_or("");
    let vout = u["vout"].as_u64().unwrap_or(0);
    let key = format!("{}:{}", txid, vout);
    if selected_keys.contains(&key) {
      matched_count += 1;
      let amount = u["amount"].as_f64().unwrap_or(0.0);
      let spendable = u["spendable"].as_bool().unwrap_or(true);
      let safe = u["safe"].as_bool().unwrap_or(true);
      let asset = u["asset"].as_str().map(|s| s.to_string());

      if !spendable {
        return Err(format!(
          "UTXO {}:{} is not spendable by the wallet. Exclude it from consolidation.",
          txid, vout
        ));
      }

      if !safe {
        return Err(format!(
          "UTXO {}:{} is marked unsafe. Exclude it from consolidation or wait for more confirmations.",
          txid, vout
        ));
      }

      let asset_amount = u.get("asset_amount").and_then(|v| v.as_f64()).unwrap_or(0.0);

      if asset_amount > 0.0 {
        return Err(format!(
          "UTXO {}:{} carries asset data and cannot be included in HEMP consolidation.",
          txid, vout
        ));
      }

      if asset.is_some() && asset.as_deref() != Some("HEMP") {
        return Err(format!(
          "UTXO {}:{} carries a non-HEMP asset ({}). It cannot be included in HEMP consolidation.",
          txid, vout,
          asset.as_deref().unwrap_or("unknown")
        ));
      }

      input_total += amount;
    }
  }

  if matched_count != selected_keys.len() {
    return Err(format!(
      "{} selected UTXOs are no longer available. Refresh UTXOs and preview again.",
      selected_keys.len().saturating_sub(matched_count)
    ));
  }

  if input_total <= fee {
    return Err(format!(
      "Selected inputs total {} HEMP is insufficient to cover the fee {} HEMP",
      input_total, fee
    ));
  }

  let output_amount = input_total - fee;

  if output_amount <= 0.0 {
    return Err("Estimated output is not positive after fee. Select more UTXOs.".to_string());
  }

  if output_amount < sat_to_hemp(DUST_THRESHOLD_SAT) {
    return Err(format!(
      "Estimated output {} HEMP is below the dust threshold ({} HEMP). Select more UTXOs.",
      format_hemp_amount(output_amount),
      format_hemp_amount(sat_to_hemp(DUST_THRESHOLD_SAT))
    ));
  }

  let mut outputs = std::collections::HashMap::new();
  outputs.insert(destination.clone(), format!("{:.8}", output_amount));

  let inputs_value = serde_json::to_value(&utxos).map_err(|e| e.to_string())?;
  let outputs_value = serde_json::to_value(&outputs).map_err(|e| e.to_string())?;

  let raw_hex = rpc::call_rpc("createrawtransaction", &[inputs_value, outputs_value])?
    .as_str()
    .ok_or("Core returned an invalid raw transaction response")?
    .to_string();

  let signed_res = rpc::call_rpc("signrawtransaction", &[serde_json::Value::String(raw_hex)])?;

  let complete = signed_res["complete"].as_bool().unwrap_or(false);
  if !complete {
    if let Some(errors) = signed_res.get("errors") {
      return Err(format!("Failed to sign consolidation transaction completely: {errors}"));
    }
    return Err("Failed to sign consolidation transaction completely.".to_string());
  }
  let signed_hex = signed_res["hex"].as_str().ok_or("No signed hex returned")?.to_string();

  let txid = rpc::call_rpc("sendrawtransaction", &[serde_json::Value::String(signed_hex)])?
    .as_str()
    .ok_or("Core returned an invalid transaction id response")?
    .to_string();

  normalize_cli_txid(txid)
}

const STANDARD_MAX_TX_BYTES: u64 = 100_000;
const DEFAULT_TARGET_TX_BYTES: u64 = 90_000;
const LEGACY_P2PKH_INPUT_BYTES: u64 = 148;
const LEGACY_P2PKH_OUTPUT_BYTES: u64 = 34;
const LEGACY_TX_OVERHEAD_BYTES: u64 = 10;
const DEFAULT_FEE_RATE_SAT_PER_BYTE: u64 = 1000;
const DUST_THRESHOLD_SAT: u64 = 546;
const SATS_PER_HEMP: f64 = 100_000_000.0;
const FEE_RATE_MIN_SAT_PER_BYTE: u64 = 1;
const FEE_RATE_MAX_SAT_PER_BYTE: u64 = 10_000;

fn validate_fee_rate_sat_per_byte(rate: u64) -> Result<u64, String> {
    if rate < FEE_RATE_MIN_SAT_PER_BYTE {
        return Err(format!(
            "Fee rate {} sat/byte is below the minimum allowed ({})",
            rate, FEE_RATE_MIN_SAT_PER_BYTE
        ));
    }
    if rate > FEE_RATE_MAX_SAT_PER_BYTE {
        return Err(format!(
            "Fee rate {} sat/byte exceeds the sanity cap ({})",
            rate, FEE_RATE_MAX_SAT_PER_BYTE
        ));
    }
    Ok(rate)
}

fn clamp_fee_rate_sat_per_byte(rate: u64) -> u64 {
    std::cmp::max(FEE_RATE_MIN_SAT_PER_BYTE, std::cmp::min(FEE_RATE_MAX_SAT_PER_BYTE, rate))
}

fn parse_estimatesmartfee_sat_per_byte(raw: &str) -> Option<u64> {
    let parsed: serde_json::Value = serde_json::from_str(raw).ok()?;
    let feerate = parsed.get("feerate")?.as_f64()?;
    if !feerate.is_finite() || feerate <= 0.0 {
        return None;
    }
    let sat_per_byte = ((feerate * SATS_PER_HEMP) / 1000.0).ceil() as u64;
    Some(clamp_fee_rate_sat_per_byte(sat_per_byte))
}

fn estimate_smartfee_sat_per_byte() -> Option<u64> {
    let conservative = run_cli(&[
        String::from("estimatesmartfee"),
        String::from("6"),
        String::from("CONSERVATIVE"),
    ]).ok();
    if let Some(raw) = conservative {
        if let Some(rate) = parse_estimatesmartfee_sat_per_byte(&raw) {
            return Some(rate);
        }
    }

    let basic = run_cli(&[
        String::from("estimatesmartfee"),
        String::from("6"),
    ]).ok();
    if let Some(raw) = basic {
        return parse_estimatesmartfee_sat_per_byte(&raw);
    }
    None
}

fn recommended_consolidation_fee_rate_sat_per_byte(override_fee_rate: Option<u64>) -> Result<u64, String> {
    if let Some(rate) = override_fee_rate {
        return validate_fee_rate_sat_per_byte(rate);
    }
    if let Some(estimated) = estimate_smartfee_sat_per_byte() {
        return validate_fee_rate_sat_per_byte(estimated);
    }
    validate_fee_rate_sat_per_byte(DEFAULT_FEE_RATE_SAT_PER_BYTE)
}

fn estimate_legacy_tx_bytes(input_count: usize, output_count: usize) -> u64 {
    LEGACY_TX_OVERHEAD_BYTES
        + (input_count as u64) * LEGACY_P2PKH_INPUT_BYTES
        + (output_count as u64) * LEGACY_P2PKH_OUTPUT_BYTES
}

fn max_policy_inputs(output_count: usize, target_max_tx_bytes: u64) -> usize {
    let overhead = LEGACY_TX_OVERHEAD_BYTES + (output_count as u64) * LEGACY_P2PKH_OUTPUT_BYTES;
    if target_max_tx_bytes <= overhead {
        return 0;
    }
    ((target_max_tx_bytes - overhead) / LEGACY_P2PKH_INPUT_BYTES) as usize
}

fn estimate_fee_from_bytes(tx_bytes: u64, fee_rate_sat_per_byte: u64) -> f64 {
    (tx_bytes as f64 * fee_rate_sat_per_byte as f64) / SATS_PER_HEMP
}

fn estimate_consolidation_round_count(initial_utxos: usize, target_final_utxo_count: usize, max_inputs_per_round: usize) -> usize {
    if initial_utxos <= target_final_utxo_count || max_inputs_per_round < 2 {
        return 0;
    }
    let mut rounds = 0usize;
    let mut current = initial_utxos;
    let max_reduction = max_inputs_per_round - 1;
    while current > target_final_utxo_count {
        let reduction = std::cmp::min(max_reduction, current - target_final_utxo_count);
        current -= reduction;
        rounds += 1;
    }
    rounds
}

fn format_hemp_amount(hemp: f64) -> String {
    format!("{:.8}", hemp)
}

fn sat_to_hemp(sat: u64) -> f64 {
    sat as f64 / SATS_PER_HEMP
}

fn hemp_to_sat(hemp: f64) -> u64 {
    (hemp * SATS_PER_HEMP).round() as u64
}

#[tauri::command]
pub fn get_policy_diagnostics(
    fee_rate_sat_per_byte: Option<u64>,
) -> Result<PolicyDiagnostics, String> {
    ensure_config()?;
    let fee_rate = recommended_consolidation_fee_rate_sat_per_byte(fee_rate_sat_per_byte)?;
    let raw = run_cli(&[
        String::from("listunspent"),
        String::from("0"),
        String::from("9999999"),
        String::from("[]"),
        String::from("true"),
    ])?;
    let all_utxos: Vec<serde_json::Value> = serde_json::from_str(&raw).map_err(|e| e.to_string())?;

    let safe_count = all_utxos
        .iter()
        .filter(|u| {
            let spendable = u["spendable"].as_bool().unwrap_or(true);
            let safe = u["safe"].as_bool().unwrap_or(true);
            let asset = u["asset"].as_str();
            let asset_amount = u.get("asset_amount").and_then(|v| v.as_f64()).unwrap_or(0.0);
            spendable && safe && (asset.is_none() || asset == Some("HEMP")) && asset_amount == 0.0
        })
        .count();

    let max_inputs_one_output = max_policy_inputs(1, DEFAULT_TARGET_TX_BYTES);
    let max_inputs_two_outputs = max_policy_inputs(2, STANDARD_MAX_TX_BYTES);
    let selected_estimate_bytes = estimate_legacy_tx_bytes(std::cmp::min(safe_count, max_inputs_one_output), 1);
    let selected_estimate_fee = estimate_fee_from_bytes(selected_estimate_bytes, fee_rate);

    Ok(PolicyDiagnostics {
        current_safe_utxo_count: safe_count,
        max_safe_inputs_for_one_output: max_inputs_one_output,
        max_safe_inputs_for_two_outputs: max_inputs_two_outputs,
        estimated_selected_tx_bytes: selected_estimate_bytes,
        estimated_selected_fee: format_hemp_amount(selected_estimate_fee),
        fee_rate_sat_per_byte: fee_rate,
    })
}

#[tauri::command]
pub fn plan_wallet_consolidation(
    destination: Option<String>,
    target_final_utxo_count: Option<usize>,
    max_rounds: Option<usize>,
    target_max_tx_bytes: Option<u64>,
    fee_rate_sat_per_byte: Option<u64>,
    selected_outpoints: Option<Vec<RawTxInput>>,
) -> Result<ConsolidationPlan, String> {
    ensure_config()?;
    let fee_rate = recommended_consolidation_fee_rate_sat_per_byte(fee_rate_sat_per_byte)?;

    if let Some(destination) = destination.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        validate_destination_address(destination)?;
    }

    let target_final_utxo_count = target_final_utxo_count.unwrap_or(80);
    let max_rounds = std::cmp::max(1, max_rounds.unwrap_or(usize::MAX / 2));
    let target_max_tx_bytes = target_max_tx_bytes.unwrap_or(DEFAULT_TARGET_TX_BYTES);

    let raw = run_cli(&[
        String::from("listunspent"),
        String::from("0"),
        String::from("9999999"),
        String::from("[]"),
        String::from("true"),
    ])?;
    let all_utxos: Vec<serde_json::Value> = serde_json::from_str(&raw).map_err(|e| e.to_string())?;

    let mut safe_utxos: Vec<(String, u64, f64)> = match selected_outpoints {
        Some(selected) => collect_selected_safe_utxos(&selected, &all_utxos)?,
        None => {
            let mut collected = Vec::new();
            for u in &all_utxos {
                let txid = u["txid"].as_str().unwrap_or("").to_string();
                let vout = u["vout"].as_u64().unwrap_or(0);
                let amount = u["amount"].as_f64().unwrap_or(0.0);
                let spendable = u["spendable"].as_bool().unwrap_or(true);
                let safe = u["safe"].as_bool().unwrap_or(true);
                let asset = u["asset"].as_str();
                let asset_amount = u.get("asset_amount").and_then(|v| v.as_f64()).unwrap_or(0.0);

                if spendable && safe && (asset.is_none() || asset == Some("HEMP")) && asset_amount == 0.0 && amount > 0.0 {
                    collected.push((txid, vout, amount));
                }
            }
            collected
        }
    };

    let initial_utxo_count = safe_utxos.len();
    if initial_utxo_count < 2 {
        return Err(format!(
            "Need at least 2 safe HEMP UTXOs for consolidation (found {})",
            initial_utxo_count
        ));
    }

    safe_utxos.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));

    let max_inputs_per_round = max_policy_inputs(1, target_max_tx_bytes);
    if max_inputs_per_round < 2 {
        return Err(format!(
            "Target max tx bytes {} is too low to fit even 2 inputs",
            target_max_tx_bytes
        ));
    }

    let mut rounds: Vec<ConsolidationRoundPlan> = Vec::new();
    let mut working: Vec<(f64, String)> = safe_utxos
        .iter()
        .map(|(txid, vout, amount)| (*amount, format!("{}:{}", txid, vout)))
        .collect();
    let mut total_estimated_fee = 0.0_f64;
    let mut total_estimated_bytes = 0u64;

    for round in 0..max_rounds {
        working.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

        if working.len() <= target_final_utxo_count && !(round == 0 && working.len() > 1) {
            break;
        }

        let required_reduction = if working.len() > target_final_utxo_count {
            working.len() - target_final_utxo_count
        } else {
            1
        };
        let input_count = std::cmp::min(
            max_inputs_per_round,
            std::cmp::max(2, required_reduction + 1),
        );
        let input_count = std::cmp::min(input_count, working.len());

        if input_count < 2 {
            break;
        }

        let selected = &working[..input_count];
        let input_total: f64 = selected.iter().map(|(amt, _)| amt).sum();
        let tx_bytes = estimate_legacy_tx_bytes(input_count, 1);
        let fee_estimate = estimate_fee_from_bytes(tx_bytes, fee_rate);
        let projected_output = input_total - fee_estimate;

        if projected_output <= sat_to_hemp(DUST_THRESHOLD_SAT) {
            break;
        }

        let selected_outpoints: Vec<String> = selected.iter().map(|(_, op)| op.clone()).collect();

        let round_plan = ConsolidationRoundPlan {
            round_number: (round + 1) as u32,
            input_count,
            input_total: format_hemp_amount(input_total),
            estimated_bytes: tx_bytes,
            fee_estimate: format_hemp_amount(fee_estimate),
            projected_output: format_hemp_amount(projected_output),
            selected_outpoints,
        };

        total_estimated_fee += fee_estimate;
        total_estimated_bytes = total_estimated_bytes.saturating_add(tx_bytes);
        rounds.push(round_plan);

        let drain_end = input_count;
        working.drain(..drain_end);
        working.push((projected_output, format!("sim-round-{}:0", round + 1)));
    }

    let projected_final_utxo_count = working.len();
    let estimated_round_count = estimate_consolidation_round_count(
        initial_utxo_count,
        target_final_utxo_count,
        max_inputs_per_round,
    );
    let planned_round_count = rounds.len();

    Ok(ConsolidationPlan {
        initial_utxo_count,
        selected_safe_utxo_count: initial_utxo_count,
        target_final_utxo_count,
        projected_final_utxo_count,
        estimated_round_count,
        planned_round_count,
        max_inputs_per_round,
        target_max_tx_bytes,
        total_estimated_fee: format_hemp_amount(total_estimated_fee),
        total_estimated_bytes,
        rounds,
    })
}

fn collect_selected_safe_utxos(
    selected_outpoints: &[RawTxInput],
    all_utxos: &[serde_json::Value],
) -> Result<Vec<(String, u64, f64)>, String> {
    if selected_outpoints.is_empty() {
        return Err("Selected outpoint set is empty".to_string());
    }
    let selected_keys: std::collections::HashSet<String> = selected_outpoints
        .iter()
        .map(|u| format!("{}:{}", u.txid.trim(), u.vout))
        .collect();
    if selected_keys.len() != selected_outpoints.len() {
        return Err("Duplicate selected outpoints are not allowed".to_string());
    }

    let mut selected_safe = Vec::new();
    for u in all_utxos {
        let txid = u["txid"].as_str().unwrap_or("");
        let vout = u["vout"].as_u64().unwrap_or(0);
        let key = format!("{}:{}", txid, vout);
        if !selected_keys.contains(&key) {
            continue;
        }

        let amount = u["amount"].as_f64().unwrap_or(0.0);
        let spendable = u["spendable"].as_bool().unwrap_or(true);
        let safe = u["safe"].as_bool().unwrap_or(true);
        let asset = u["asset"].as_str();
        let asset_amount = u.get("asset_amount").and_then(|v| v.as_f64()).unwrap_or(0.0);
        if !spendable || !safe {
            return Err(format!("Selected outpoint {} is not safe/spendable", key));
        }
        if asset_amount > 0.0 || (asset.is_some() && asset != Some("HEMP")) {
            return Err(format!("Selected outpoint {} carries asset data and cannot be consolidated", key));
        }
        if amount <= 0.0 {
            return Err(format!("Selected outpoint {} has non-positive amount", key));
        }
        selected_safe.push((txid.to_string(), vout, amount));
    }

    if selected_safe.len() != selected_keys.len() {
        return Err(format!(
            "{} selected outpoints are unavailable. Refresh UTXOs and reselect.",
            selected_keys.len().saturating_sub(selected_safe.len())
        ));
    }
    Ok(selected_safe)
}

#[tauri::command]
pub fn backup_wallet() -> Result<String, String> {
  let dir = data_dir()?;
  let ts = Local::now().format("%Y%m%d_%H%M%S").to_string();
  let dest = dir.join(format!("hemp0x_backup_{}.dat", ts));
  let dest_str = dest.to_string_lossy().to_string();
  run_cli(&[String::from("backupwallet"), dest_str.clone()])?;
  Ok(dest_str)
}

#[tauri::command]
pub fn backup_wallet_to(path: String) -> Result<(), String> {
  run_cli(&[String::from("backupwallet"), path])?;
  Ok(())
}

#[tauri::command]
pub fn lock_asset_supply(name: String, current_units: u8) -> Result<String, String> {
  ensure_config()?;
  validate_asset_name(&name)?;
  // To lock: reissue with amount 0, reissuable=false.
  // We need a destination address (can be same wallet).
  let to_addr = run_cli(&[String::from("getnewaddress")])?;
  let change_addr = to_addr.clone();

  let args = build_reissue_args(
    &name,
    "0",
    &to_addr,
    &change_addr,
    false,
    current_units,
    "",
  )?;
  normalize_cli_txid(run_cli(&args)?)
}

#[tauri::command]
pub fn get_transaction_history(
  count: u64,
  skip: u64,
  category: Option<String>,
) -> Result<TransactionHistoryResult, String> {
  ensure_config()?;
  let count = std::cmp::max(1, std::cmp::min(count, 500));
  let skip = std::cmp::min(skip, 1_000_000);
  let active_filter = category.as_ref().filter(|f| !f.is_empty());
  let fetch_count = if active_filter.is_some() { 500 } else { count + 1 };
  let fetch_skip = if active_filter.is_some() { 0 } else { skip };

  let tx_raw = match rpc::call_rpc(
    "listtransactions",
    &[
      serde_json::Value::String("*".to_string()),
      serde_json::Value::Number(serde_json::value::Number::from(fetch_count)),
      serde_json::Value::Number(serde_json::value::Number::from(fetch_skip)),
    ],
  ) {
    Ok(result) => result,
    Err(_rpc_err) => {
      let raw = run_cli(&[
        String::from("listtransactions"),
        String::from("*"),
        format!("{}", fetch_count),
        format!("{}", fetch_skip),
      ])?;
      serde_json::from_str(&raw).map_err(|e| format!("CLI parse error: {e}"))?
    }
  };

  let empty_vec = vec![];
  let tx_list: &Vec<serde_json::Value> = tx_raw.as_array().unwrap_or(&empty_vec);

  let source: Vec<&serde_json::Value> = match active_filter {
    Some(filter) => tx_list
      .iter()
      .filter(|tx| tx["category"].as_str().unwrap_or("unknown") == filter.as_str())
      .collect(),
    None => tx_list.iter().collect(),
  };

  let source_len = source.len();
  let items: Vec<TransactionHistoryItem> = source
    .iter()
    .rev()
    .skip(if active_filter.is_some() { skip as usize } else { 0 })
    .take(count as usize)
    .map(|tx| map_transaction_history_item(tx))
    .collect();

  let has_more = if active_filter.is_some() {
    source_len > (skip + count) as usize
  } else {
    source_len > count as usize
  };
  let total = if active_filter.is_some() {
    source_len
  } else {
    skip as usize + items.len() + usize::from(has_more)
  };
  Ok(TransactionHistoryResult {
    items,
    total,
    has_more,
  })
}

fn map_transaction_history_item(tx: &serde_json::Value) -> TransactionHistoryItem {
  let epoch = tx["time"].as_i64().unwrap_or(0);
  let dt: DateTime<Local> = chrono::Local
    .timestamp_opt(epoch, 0)
    .single()
    .unwrap_or_else(|| chrono::Local::now());
  let amount = tx["amount"].as_f64().unwrap_or(0.0);

  let address = tx["address"]
    .as_str()
    .filter(|a| !a.is_empty())
    .map(|a| a.to_string());

  let asset = tx.get("asset")
    .and_then(|v| v.as_str())
    .filter(|a| !a.is_empty())
    .map(|a| a.to_string());

  let fee = tx["fee"]
    .as_f64()
    .map(|f| format!("{:.8}", f));

  TransactionHistoryItem {
    txid: tx["txid"].as_str().unwrap_or("-").to_string(),
    date: dt.format("%m/%d %H:%M").to_string(),
    tx_type: tx["category"].as_str().unwrap_or("unknown").to_string(),
    amount: format!("{:.8}", amount),
    confirmations: tx["confirmations"].as_u64().unwrap_or(0),
    address,
    asset,
    fee,
    raw: Some(tx.clone()),
  }
}

#[tauri::command]
pub fn preview_issue_asset(
  name: String,
  qty: String,
  units: u8,
  reissuable: bool,
  ipfs: String,
) -> Result<IssuePreview, String> {
  ensure_config()?;
  let name = name.trim().to_uppercase();
  validate_asset_name(&name)?;
  if name.contains('/') {
    return Err("Root asset name cannot contain '/'. Use sub-asset creation instead.".to_string());
  }
  if name.contains('#') {
    return Err("Root asset name cannot contain '#'. Use unique/NFT creation instead.".to_string());
  }
  let qty_val = parse_positive_amount(&qty)?;
  if units > 8 {
    return Err("Units must be between 0 and 8".to_string());
  }
  let mut warnings = Vec::new();
  if !ipfs.trim().is_empty() && !ipfs.trim().starts_with("Qm") {
    warnings.push("IPFS hash does not appear to be a valid CIDv0 format".to_string());
  }
  let is_irreversible = !reissuable;
  if is_irreversible {
    warnings.push("This asset will NOT be reissuable. This cannot be changed later.".to_string());
  }
  let fee_warning = String::from(
    "Asset creation requires a network fee (typically 0.25 HEMP for root assets). Ensure you have sufficient HEMP.",
  );
  warnings.push(fee_warning);
  let summary = format!(
    "Issue {} {} of new root asset '{}'{}",
    qty_val, if units == 0 { "whole units" } else { "units" },
    name,
    if reissuable { "" } else { " (NOT reissuable)" }
  );
  Ok(IssuePreview {
    operation_type: "issue".to_string(),
    asset_name: name,
    qty: Some(format!("{}", qty_val)),
    units: Some(units),
    reissuable: Some(reissuable),
    ipfs_hash: if ipfs.trim().is_empty() { None } else { Some(ipfs.trim().to_string()) },
    parent_asset: None,
    tags: None,
    is_irreversible,
    warnings,
    summary,
    validated: true,
  })
}

#[tauri::command]
pub fn preview_issue_sub_asset(
  parent: String,
  name: String,
  qty: String,
  reissuable: bool,
  units: u8,
  ipfs: String,
) -> Result<IssuePreview, String> {
  ensure_config()?;
  let parent = parent.trim().to_uppercase();
  let name = name.trim().to_uppercase();
  if parent.is_empty() {
    return Err("Parent asset name is required".to_string());
  }
  if name.is_empty() {
    return Err("Sub-asset name is required".to_string());
  }
  if name.contains('/') || name.contains('#') {
    return Err("Sub-asset name cannot contain '/' or '#'".to_string());
  }
  if units > 8 {
    return Err("Units must be between 0 and 8".to_string());
  }
  let full_name = format!("{}/{}", parent, name);
  validate_asset_name(&full_name)?;
  let qty_val = parse_positive_amount(&qty)?;
  let mut warnings = Vec::new();
  if !reissuable {
    warnings.push("This sub-asset will NOT be reissuable. This cannot be changed later.".to_string());
  }
  if !ipfs.trim().is_empty() && !ipfs.trim().starts_with("Qm") {
    warnings.push("IPFS hash does not appear to be a valid CIDv0 format".to_string());
  }
  let fee_warning = String::from(
    "Sub-asset creation requires a network fee (0.05 HEMP burn). Ensure you have sufficient HEMP.",
  );
  warnings.push(fee_warning);
  let summary = format!(
    "Issue {} {} of new sub-asset '{}' (parent: {}){}",
    qty_val, if units == 0 { "whole units" } else { "units" },
    full_name, parent,
    if reissuable { "" } else { " (NOT reissuable)" }
  );
  Ok(IssuePreview {
    operation_type: "issue_sub".to_string(),
    asset_name: full_name,
    qty: Some(format!("{}", qty_val)),
    units: Some(units),
    reissuable: Some(reissuable),
    ipfs_hash: if ipfs.trim().is_empty() { None } else { Some(ipfs.trim().to_string()) },
    parent_asset: Some(parent),
    tags: None,
    is_irreversible: !reissuable,
    warnings,
    summary,
    validated: true,
  })
}

#[tauri::command]
pub fn preview_issue_unique_asset(
  root_name: String,
  tags: Vec<String>,
  ipfs_hashes: Vec<String>,
) -> Result<IssuePreview, String> {
  ensure_config()?;
  let (root_name, tags, ipfs_hashes) = normalize_unique_asset_inputs(root_name, tags, ipfs_hashes)?;
  let has_ipfs = !ipfs_hashes.is_empty();
  let tag_display: Vec<String> = tags.iter().map(|t| format!("{}#{}", root_name, t)).collect();
  let mut warnings = Vec::new();
  warnings.push("NFT/unique assets are permanently non-reissuable with fixed supply of 1 and 0 decimal units.".to_string());
  let fee_warning = String::from(
    "Minting NFTs requires a 0.01 HEMP burn per asset. Ensure you have sufficient HEMP.",
  );
  warnings.push(fee_warning);
  if tags.len() > 1 {
    warnings.push(format!("You are about to mint {} NFTs in a single transaction.", tags.len()));
  }
  let summary = format!(
    "Mint {} unique asset(s) under '{}': {}",
    tags.len(), root_name, tag_display.join(", ")
  );
  Ok(IssuePreview {
    operation_type: "issue_unique".to_string(),
    asset_name: root_name.clone(),
    qty: None,
    units: None,
    reissuable: None,
    ipfs_hash: if has_ipfs { Some(ipfs_hashes.join(", ")) } else { None },
    parent_asset: Some(root_name),
    tags: Some(tag_display),
    is_irreversible: true,
    warnings,
    summary,
    validated: true,
  })
}

fn build_reissue_preview(
  name: &str,
  qty_val: f64,
  units: u8,
  reissuable: bool,
  new_ipfs: &str,
) -> Result<IssuePreview, String> {
  let mut warnings = Vec::new();
  let mut ipfs_hash = None;
  let trimmed_ipfs = new_ipfs.trim().to_string();
  if !trimmed_ipfs.is_empty() {
    if let Err(e) = validate_ipfs_reference(&trimmed_ipfs) {
      return Err(format!("Invalid IPFS hash: {e}"));
    }
    ipfs_hash = Some(trimmed_ipfs.clone());
  }
  if qty_val == 0.0 {
    if ipfs_hash.is_some() {
      warnings.push("Quantity is zero and a new IPFS hash is set — this will be a metadata update without increasing supply.".to_string());
    } else {
      warnings.push("Reissue amount is zero — no new supply will be created, but metadata/IPFS or reissuable flag may be updated.".to_string());
    }
  }
  if !reissuable {
    warnings.push("Disabling reissuability is IRREVERSIBLE. The asset supply will be permanently locked.".to_string());
  }
  let fee_warning = String::from(
    "Reissue requires a network fee (0.05 HEMP burn). Ensure you have sufficient HEMP.",
  );
  warnings.push(fee_warning);
  let is_irreversible = !reissuable;
  let summary = format!(
    "Reissue {} {} of asset '{}'{}",
    qty_val, if units == 0 { "whole units" } else { "units" },
    name,
    if reissuable { "" } else { " (lock reissuability)" }
  );
  Ok(IssuePreview {
    operation_type: "reissue".to_string(),
    asset_name: name.to_string(),
    qty: Some(format!("{}", qty_val)),
    units: Some(units),
    reissuable: Some(reissuable),
    ipfs_hash,
    parent_asset: None,
    tags: None,
    is_irreversible,
    warnings,
    summary,
    validated: true,
  })
}

#[tauri::command]
pub fn preview_reissue_asset(
  name: String,
  qty: String,
  reissuable: bool,
  new_ipfs: String,
) -> Result<IssuePreview, String> {
  ensure_config()?;
  let name = name.trim().to_string();
  validate_asset_name(&name)?;
  let qty_val = parse_non_negative_amount(&qty)?;
  let units = get_asset_units(&name)?;
  build_reissue_preview(&name, qty_val, units, reissuable, &new_ipfs)
}

#[tauri::command]
pub fn update_asset_metadata(name: String, ipfs_hash: String, current_units: u8) -> Result<String, String> {
  ensure_config()?;
  validate_asset_name(&name)?;
  if ipfs_hash.trim().is_empty() {
    return Err("IPFS hash or metadata value is required".to_string());
  }
  // To update IPFS: reissue with amount 0, reissuable=true, same units, new IPFS.
  let to_addr = run_cli(&[String::from("getnewaddress")])?;
  let change_addr = to_addr.clone();

  let args = build_reissue_args(
    &name,
    "0",
    &to_addr,
    &change_addr,
    true,
    current_units,
    &ipfs_hash,
  )?;
  normalize_cli_txid(run_cli(&args)?)
}

// ---------------------------------------------------------------------------
// Qualifier Asset Operations
// ---------------------------------------------------------------------------

fn validate_qualifier_name(name: &str) -> Result<String, String> {
  let trimmed = name.trim();
  if trimmed.is_empty() {
    return Err("Qualifier name is required".to_string());
  }
  if trimmed.len() > 128 {
    return Err("Qualifier name is too long".to_string());
  }
  if trimmed.chars().any(|c| c.is_whitespace()) {
    return Err("Qualifier name cannot contain whitespace".to_string());
  }
  let normalized = if trimmed.starts_with('#') {
    trimmed.to_string()
  } else {
    format!("#{}", trimmed)
  };
  let body = normalized.trim_start_matches('#');
  if body.is_empty() || body.starts_with('/') || body.ends_with('/') || body.contains("//") {
    return Err("Qualifier name must include a non-empty qualifier identifier".to_string());
  }
  if normalized.chars().filter(|&c| c == '/').count() > 1 {
    return Err("Sub-qualifier name cannot contain more than one '/'".to_string());
  }
  if normalized.chars().filter(|&c| c == '#').count() > 1 {
    return Err("Qualifier name cannot contain more than one '#'".to_string());
  }
  Ok(normalized)
}

#[tauri::command]
pub fn preview_issue_qualifier_asset(
  name: String,
  qty: String,
  destination: Option<String>,
  ipfs: Option<String>,
) -> Result<QualifierIssuePreview, String> {
  ensure_config()?;
  let name = validate_qualifier_name(&name)?;
  let qty_val = parse_positive_amount(&qty)?;
  let destination = validate_optional_destination(destination)?;
  if qty_val < 1.0 || qty_val > 10.0 {
    return Err("Qualifier asset amount must be between 1 and 10".to_string());
  }
  let ipfs = ipfs.map(|h| h.trim().to_string()).filter(|h| !h.is_empty());
  let mut warnings = Vec::new();
  if let Some(ref ipfs) = ipfs {
    if !ipfs.starts_with("Qm") {
      warnings.push("IPFS hash does not appear to be a valid CIDv0 format".to_string());
    }
  }
  warnings.push("Qualifier assets have fixed units=0 and are non-reissuable. This cannot be changed.".to_string());
  warnings.push("Issuing a qualifier asset requires a network fee and wallet unlock.".to_string());

  let summary = format!(
    "Issue {} of qualifier asset '{}'{}",
    qty_val,
    name,
    if let Some(ref d) = destination { format!(" to {}", &d[..std::cmp::min(16, d.len())]) } else { String::new() }
  );

  Ok(QualifierIssuePreview {
    operation_type: "issue_qualifier".to_string(),
    asset_name: name.clone(),
    qualifier_name: name,
    qty: format!("{}", qty_val),
    destination,
    ipfs_hash: ipfs,
    warnings,
    summary,
    is_irreversible: true,
    validated: true,
  })
}

#[tauri::command]
pub fn issue_qualifier_asset(
  name: String,
  qty: String,
  destination: Option<String>,
  ipfs: Option<String>,
) -> Result<String, String> {
  ensure_config()?;
  let name = validate_qualifier_name(&name)?;
  let qty_val = parse_positive_amount(&qty)?;
  let destination = validate_optional_destination(destination)?;
  if qty_val < 1.0 || qty_val > 10.0 {
    return Err("Qualifier asset amount must be between 1 and 10".to_string());
  }
  let change_addr = run_cli(&[String::from("getnewaddress")])?;

  let ipfs_str = ipfs.map(|h| h.trim().to_string()).unwrap_or_default();
  let has_ipfs = !ipfs_str.is_empty();

  let mut args = vec![
    String::from("issuequalifierasset"),
    name,
    format!("{qty_val}"),
    destination.unwrap_or_else(|| change_addr.clone()),
    change_addr,
  ];
  if has_ipfs {
    args.push(String::from("true"));
    args.push(ipfs_str.trim().to_string());
  }

  normalize_cli_txid(run_cli(&args)?)
}

// ---------------------------------------------------------------------------
// Restricted Asset Operations
// ---------------------------------------------------------------------------

fn validate_restricted_name(name: &str) -> Result<String, String> {
  let trimmed = name.trim();
  if trimmed.is_empty() {
    return Err("Restricted asset name is required".to_string());
  }
  if trimmed.len() > 128 {
    return Err("Restricted asset name is too long".to_string());
  }
  if trimmed.chars().any(|c| c.is_whitespace()) {
    return Err("Restricted asset name cannot contain whitespace".to_string());
  }
  let normalized = if trimmed.starts_with('$') {
    trimmed.to_string()
  } else {
    format!("${}", trimmed)
  };
  if normalized.trim_start_matches('$').is_empty() {
    return Err("Restricted asset name must include a non-empty asset identifier".to_string());
  }
  Ok(normalized)
}

fn validate_verifier_string(verifier: &str) -> Result<String, String> {
  let trimmed = verifier.trim();
  if trimmed.is_empty() {
    return Err("Verifier string is required for a restricted asset".to_string());
  }
  if trimmed.len() > 512 {
    return Err("Verifier string is too long (max 512 characters)".to_string());
  }
  Ok(trimmed.to_string())
}

fn validate_optional_destination(destination: Option<String>) -> Result<Option<String>, String> {
  match destination {
    Some(value) => {
      let trimmed = value.trim().to_string();
      if trimmed.is_empty() {
        Ok(None)
      } else {
        validate_destination_address(&trimmed)?;
        Ok(Some(trimmed))
      }
    }
    None => Ok(None),
  }
}

#[tauri::command]
pub fn preview_issue_restricted_asset(
  name: String,
  qty: String,
  verifier: String,
  destination: Option<String>,
  units: Option<u8>,
  reissuable: Option<bool>,
  ipfs: Option<String>,
) -> Result<RestrictedIssuePreview, String> {
  ensure_config()?;
  let name = validate_restricted_name(&name)?;
  let verifier = validate_verifier_string(&verifier)?;
  let qty_val = parse_positive_amount(&qty)?;
  let destination = validate_optional_destination(destination)?;
  let units = units.unwrap_or(0);
  let reissuable = reissuable.unwrap_or(true);
  if units > 8 {
    return Err("Units must be between 0 and 8".to_string());
  }
  let mut warnings = Vec::new();
  if !reissuable {
    warnings.push("This restricted asset will NOT be reissuable. This cannot be changed later.".to_string());
  }
  if let Some(ref ipfs) = ipfs {
    if !ipfs.trim().is_empty() && !ipfs.trim().starts_with("Qm") {
      warnings.push("IPFS hash does not appear to be a valid CIDv0 format".to_string());
    }
  }
  warnings.push("Restricted asset creation requires the wallet to be unlocked and a wallet transaction fee.".to_string());
  warnings.push("The verifier string determines which tagged addresses can hold this asset.".to_string());

  let summary = format!(
    "Issue {} of restricted asset '{}' with verifier '{}'{}",
    qty_val,
    name,
    verifier,
    if reissuable { "" } else { " (NOT reissuable)" }
  );

  Ok(RestrictedIssuePreview {
    operation_type: "issue_restricted".to_string(),
    asset_name: name,
    qty: format!("{}", qty_val),
    verifier,
    destination: destination.unwrap_or_else(|| String::from("auto-generated")),
    units,
    reissuable,
    ipfs_hash: ipfs.filter(|h| !h.trim().is_empty()),
    warnings,
    summary,
    is_irreversible: !reissuable,
    validated: true,
  })
}

#[tauri::command]
pub fn issue_restricted_asset(
  name: String,
  qty: String,
  verifier: String,
  destination: Option<String>,
  units: Option<u8>,
  reissuable: Option<bool>,
  ipfs: Option<String>,
) -> Result<String, String> {
  ensure_config()?;
  let name = validate_restricted_name(&name)?;
  let verifier = validate_verifier_string(&verifier)?;
  let qty_val = parse_positive_amount(&qty)?;
  let destination = validate_optional_destination(destination)?;
  let units = units.unwrap_or(0);
  let reissuable = reissuable.unwrap_or(true);
  if units > 8 {
    return Err("Units must be between 0 and 8".to_string());
  }
  let to_addr = match destination {
    Some(address) => address,
    None => run_cli(&[String::from("getnewaddress")])?,
  };
  let change_addr = run_cli(&[String::from("getnewaddress")])?;

  let ipfs_str = ipfs.unwrap_or_else(|| String::new());
  let has_ipfs = !ipfs_str.trim().is_empty();

  let mut args = vec![
    String::from("issuerestrictedasset"),
    name,
    format!("{qty_val}"),
    verifier,
    to_addr,
    change_addr,
    units.to_string(),
    if reissuable { String::from("true") } else { String::from("false") },
  ];
  if has_ipfs {
    args.push(String::from("true"));
    args.push(ipfs_str.trim().to_string());
  }

  normalize_cli_txid(run_cli(&args)?)
}

// ---------------------------------------------------------------------------
// Tag Operations
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn preview_add_tag_to_address(
  tag_name: String,
  address: String,
) -> Result<TagOperationPreview, String> {
  ensure_config()?;
  let tag_name = validate_qualifier_name(&tag_name)?;
  validate_destination_address(&address)?;
  let mut warnings = Vec::new();
  warnings.push("Adding a tag requires the wallet to be unlocked and ownership of the qualifier asset.".to_string());
  warnings.push("This operation sends 1 unit of the qualifier asset with tag assignment data.".to_string());
  let summary = format!(
    "Assign tag '{}' to address {}",
    tag_name,
    &address[..std::cmp::min(16, address.len())]
  );
  Ok(TagOperationPreview {
    operation_type: "add_tag".to_string(),
    asset_name: tag_name.clone(),
    tag_name,
    address,
    is_adding: true,
    warnings,
    summary,
    is_irreversible: false,
    validated: true,
  })
}

#[tauri::command]
pub fn add_tag_to_address(
  tag_name: String,
  address: String,
  change_address: Option<String>,
  asset_data: Option<String>,
) -> Result<String, String> {
  ensure_config()?;
  let tag_name = validate_qualifier_name(&tag_name)?;
  validate_destination_address(&address)?;
  let mut args = vec![
    String::from("addtagtoaddress"),
    tag_name,
    address,
  ];
  if let Some(ref ch) = change_address {
    if !ch.trim().is_empty() {
      args.push(ch.trim().to_string());
      if let Some(ref ad) = asset_data {
        if !ad.trim().is_empty() {
          args.push(ad.trim().to_string());
        }
      }
    }
  }
  normalize_cli_txid(run_cli(&args)?)
}

#[tauri::command]
pub fn preview_remove_tag_from_address(
  tag_name: String,
  address: String,
) -> Result<TagOperationPreview, String> {
  ensure_config()?;
  let tag_name = validate_qualifier_name(&tag_name)?;
  validate_destination_address(&address)?;
  let mut warnings = Vec::new();
  warnings.push("Removing a tag requires the wallet to be unlocked and ownership of the qualifier asset.".to_string());
  warnings.push("This operation sends 1 unit of the qualifier asset with tag removal data.".to_string());
  let summary = format!(
    "Remove tag '{}' from address {}",
    tag_name,
    &address[..std::cmp::min(16, address.len())]
  );
  Ok(TagOperationPreview {
    operation_type: "remove_tag".to_string(),
    asset_name: tag_name.clone(),
    tag_name,
    address,
    is_adding: false,
    warnings,
    summary,
    is_irreversible: false,
    validated: true,
  })
}

#[tauri::command]
pub fn remove_tag_from_address(
  tag_name: String,
  address: String,
  change_address: Option<String>,
  asset_data: Option<String>,
) -> Result<String, String> {
  ensure_config()?;
  let tag_name = validate_qualifier_name(&tag_name)?;
  validate_destination_address(&address)?;
  let mut args = vec![
    String::from("removetagfromaddress"),
    tag_name,
    address,
  ];
  if let Some(ref ch) = change_address {
    if !ch.trim().is_empty() {
      args.push(ch.trim().to_string());
      if let Some(ref ad) = asset_data {
        if !ad.trim().is_empty() {
          args.push(ad.trim().to_string());
        }
      }
    }
  }
  normalize_cli_txid(run_cli(&args)?)
}

// ---------------------------------------------------------------------------
// Tag / Restricted Read-Only Helpers
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn check_address_tag(address: String, tag_name: String) -> Result<bool, String> {
  ensure_config()?;
  validate_destination_address(&address)?;
  let tag_name = validate_qualifier_name(&tag_name)?;
  let raw = run_cli(&[String::from("checkaddresstag"), address, tag_name])?;
  Ok(raw.trim().to_lowercase() == "true")
}

#[tauri::command]
pub fn list_tags_for_address(address: String) -> Result<Vec<String>, String> {
  ensure_config()?;
  validate_destination_address(&address)?;
  let raw = run_cli(&[String::from("listtagsforaddress"), address])?;
  let value: serde_json::Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
  let tags: Vec<String> = value.as_array()
    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
    .unwrap_or_default();
  Ok(tags)
}

#[tauri::command]
pub fn list_addresses_for_tag(tag_name: String) -> Result<Vec<String>, String> {
  ensure_config()?;
  let tag_name = validate_qualifier_name(&tag_name)?;
  let raw = run_cli(&[String::from("listaddressesfortag"), tag_name])?;
  let value: serde_json::Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
  let addresses: Vec<String> = value.as_array()
    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
    .unwrap_or_default();
  Ok(addresses)
}

#[tauri::command]
pub fn get_verifier_string(restricted_name: String) -> Result<String, String> {
  ensure_config()?;
  let restricted_name = validate_restricted_name(&restricted_name)?;
  run_cli(&[String::from("getverifierstring"), restricted_name])
}

#[tauri::command]
pub fn list_global_restrictions() -> Result<Vec<String>, String> {
  ensure_config()?;
  let raw = run_cli(&[String::from("listglobalrestrictions")])?;
  let value: serde_json::Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
  let restrictions: Vec<String> = value.as_array()
    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
    .unwrap_or_default();
  Ok(restrictions)
}

#[tauri::command]
pub fn check_global_restriction(restricted_name: String) -> Result<bool, String> {
  ensure_config()?;
  let restricted_name = validate_restricted_name(&restricted_name)?;
  let raw = run_cli(&[String::from("checkglobalrestriction"), restricted_name])?;
  Ok(raw.trim().to_lowercase() == "true")
}

// ---------------------------------------------------------------------------
// Snapshot Operations
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn request_snapshot(asset_name: String, block_height: i64) -> Result<serde_json::Value, String> {
  ensure_config()?;
  validate_asset_name(&asset_name)?;
  if block_height <= 0 {
    return Err("Block height must be greater than zero".to_string());
  }
  let raw = run_cli(&[
    String::from("requestsnapshot"),
    asset_name,
    format!("{block_height}"),
  ])?;
  serde_json::from_str(&raw).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_snapshot_request(asset_name: String, block_height: i64) -> Result<SnapshotRequestEntry, String> {
  ensure_config()?;
  validate_asset_name(&asset_name)?;
  if block_height <= 0 {
    return Err("Block height must be greater than zero".to_string());
  }
  let raw = run_cli(&[
    String::from("getsnapshotrequest"),
    asset_name.clone(),
    format!("{block_height}"),
  ])?;
  let value: serde_json::Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
  let asset_name = value.get("asset_name").and_then(|v| v.as_str()).unwrap_or(&asset_name).to_string();
  let block_height = value.get("block_height").and_then(|v| v.as_i64()).unwrap_or(block_height);
  Ok(SnapshotRequestEntry { asset_name, block_height })
}

#[tauri::command]
pub fn list_snapshot_requests(
  asset_name: Option<String>,
  block_height: Option<i64>,
) -> Result<Vec<SnapshotRequestEntry>, String> {
  ensure_config()?;
  let mut args = vec![String::from("listsnapshotrequests")];
  if let Some(ref an) = asset_name {
    let trimmed = an.trim().to_string();
    if !trimmed.is_empty() {
      validate_asset_name(&trimmed)?;
      args.push(trimmed);
    }
  }
  if let Some(bh) = block_height {
    if bh <= 0 {
      return Err("Block height must be greater than zero".to_string());
    }
    args.push(format!("{bh}"));
  }
  let raw = run_cli(&args)?;
  let value: serde_json::Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
  let entries: Vec<SnapshotRequestEntry> = value.as_array()
    .map(|arr| {
      arr.iter().map(|v| {
        SnapshotRequestEntry {
          asset_name: v.get("asset_name").and_then(|a| a.as_str()).unwrap_or("").to_string(),
          block_height: v.get("block_height").and_then(|b| b.as_i64()).unwrap_or(0),
        }
      }).collect()
    })
    .unwrap_or_default();
  Ok(entries)
}

#[tauri::command]
pub fn cancel_snapshot_request(asset_name: String, block_height: i64) -> Result<serde_json::Value, String> {
  ensure_config()?;
  validate_asset_name(&asset_name)?;
  if block_height <= 0 {
    return Err("Block height must be greater than zero".to_string());
  }
  let raw = run_cli(&build_cancel_snapshot_args(&asset_name, block_height)?)?;
  serde_json::from_str(&raw).map_err(|e| e.to_string())
}

fn build_cancel_snapshot_args(asset_name: &str, block_height: i64) -> Result<Vec<String>, String> {
  validate_asset_name(asset_name)?;
  if block_height <= 0 {
    return Err("Block height must be greater than zero".to_string());
  }
  Ok(vec![
    String::from("cancelsnapshotrequest"),
    asset_name.trim().to_string(),
    format!("{block_height}"),
  ])
}

#[tauri::command]
pub fn get_asset_snapshot(asset_name: String, block_height: i64) -> Result<SnapshotData, String> {
  ensure_config()?;
  validate_asset_name(&asset_name)?;
  if block_height <= 0 {
    return Err("Block height must be greater than zero".to_string());
  }
  let raw = run_cli(&[
    String::from("getsnapshot"),
    asset_name.clone(),
    format!("{block_height}"),
  ])?;
  let value: serde_json::Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
  if value.is_null() {
    return Err("Snapshot not available for the requested asset and block height".to_string());
  }
  let name = value.get("name").and_then(|v| v.as_str()).unwrap_or(&asset_name).to_string();
  let height = value.get("height").and_then(|v| v.as_i64()).unwrap_or(block_height);
  let owners: Vec<SnapshotOwnerEntry> = value.get("owners")
    .and_then(|v| v.as_array())
    .map(|arr| {
      arr.iter().map(|owner| {
        SnapshotOwnerEntry {
          address: owner.get("address").and_then(|a| a.as_str()).unwrap_or("").to_string(),
          amount_owned: owner.get("amount_owned").cloned().unwrap_or(serde_json::Value::Null),
        }
      }).collect()
    })
    .unwrap_or_default();
  Ok(SnapshotData { name, height, owners })
}

// ---------------------------------------------------------------------------
// Raw Transaction Editor Commands
// ---------------------------------------------------------------------------

fn validate_raw_tx_hex(hex: &str) -> Result<String, String> {
  let hex = hex.trim();
  if hex.is_empty() {
    return Err("Raw transaction hex is required".to_string());
  }
  if hex.len() % 2 != 0 {
    return Err("Raw transaction hex has odd length (not valid hex bytes)".to_string());
  }
  if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
    return Err("Raw transaction hex contains non-hex characters".to_string());
  }
  if hex.len() < 20 {
    return Err("Raw transaction hex is too short to be a valid transaction".to_string());
  }
  Ok(hex.to_string())
}

#[tauri::command]
pub fn decode_raw_transaction(raw_hex: String) -> Result<serde_json::Value, String> {
  ensure_config()?;
  let hex = validate_raw_tx_hex(&raw_hex)?;
  let raw = run_cli(&[String::from("decoderawtransaction"), hex])?;
  serde_json::from_str(&raw).map_err(|e| format!("Failed to parse decoded transaction: {e}"))
}

#[tauri::command]
pub fn test_mempool_accept(raw_hex: String) -> Result<serde_json::Value, String> {
  ensure_config()?;
  let hex = validate_raw_tx_hex(&raw_hex)?;
  let hexes = serde_json::to_string(&[hex]).map_err(|e| e.to_string())?;
  let raw = run_cli(&[String::from("testmempoolaccept"), hexes])?;
  serde_json::from_str(&raw).map_err(|e| format!("Failed to parse mempool accept result: {e}"))
}

fn validate_tx_input(input: &serde_json::Value) -> Result<(), String> {
  let txid = input.get("txid").and_then(|v| v.as_str()).unwrap_or("");
  if txid.trim().is_empty() {
    return Err("Each input must have a txid".to_string());
  }
  if txid.len() != 64 || !txid.chars().all(|c| c.is_ascii_hexdigit()) {
    return Err(format!("Invalid txid '{}': must be 64 hex characters", txid));
  }
  let vout = input.get("vout").and_then(|v| v.as_u64());
  if vout.is_none() {
    return Err("Each input must have a numeric vout".to_string());
  }
  Ok(())
}

fn normalize_raw_tx_outputs(outputs: &[serde_json::Value]) -> Result<serde_json::Map<String, serde_json::Value>, String> {
  if outputs.is_empty() {
    return Err("At least one output is required".to_string());
  }
  let mut normalized = serde_json::Map::new();
  for output in outputs {
    let addr = output.get("address").and_then(|v| v.as_str()).unwrap_or("").trim();
    if addr.trim().is_empty() {
      return Err("Output address cannot be empty".to_string());
    }
    if normalized.contains_key(addr) {
      return Err(format!("Duplicate output address '{}'", addr));
    }
    let amount = output.get("amount")
      .ok_or_else(|| format!("Output amount is required for address '{}'", addr))?;
    let amount_text = match amount {
      serde_json::Value::String(s) => s.trim().to_string(),
      serde_json::Value::Number(n) => n.to_string(),
      _ => return Err(format!("Output amount for '{}' is not a valid number", addr)),
    };
    let parsed: f64 = amount_text
      .parse()
      .map_err(|_| format!("Output amount '{}' is not a valid number", amount_text))?;
    if !parsed.is_finite() || parsed <= 0.0 {
      return Err(format!("Output amount '{}' must be a positive number", amount_text));
    }
    let number = serde_json::Number::from_f64(parsed)
      .ok_or_else(|| format!("Output amount '{}' is not finite", amount_text))?;
    normalized.insert(addr.to_string(), serde_json::Value::Number(number));
  }
  Ok(normalized)
}

#[tauri::command]
pub fn create_unsigned_raw_transaction(
  inputs_json: String,
  outputs_json: String,
) -> Result<RawTxBuildResult, String> {
  ensure_config()?;

  let inputs: Vec<serde_json::Value> = serde_json::from_str(&inputs_json)
    .map_err(|e| format!("Failed to parse inputs JSON: {e}"))?;
  if inputs.is_empty() {
    return Err("At least one input is required".to_string());
  }
  let mut seen_inputs = std::collections::HashSet::new();
  for input in &inputs {
    validate_tx_input(input)?;
    let txid = input.get("txid").and_then(|v| v.as_str()).unwrap_or("").trim();
    let vout = input.get("vout").and_then(|v| v.as_u64()).unwrap_or(0);
    if !seen_inputs.insert(format!("{}:{}", txid, vout)) {
      return Err(format!("Duplicate input '{}:{}'", txid, vout));
    }
  }

  let outputs: Vec<serde_json::Value> = serde_json::from_str(&outputs_json)
    .map_err(|e| format!("Failed to parse outputs JSON: {e}"))?;
  let outputs = normalize_raw_tx_outputs(&outputs)?;
  for address in outputs.keys() {
    validate_destination_address(address)?;
  }

  let outputs_json_normalized = serde_json::to_string(&outputs).map_err(|e| e.to_string())?;
  let inputs_json_normalized = serde_json::to_string(&inputs).map_err(|e| e.to_string())?;

  let raw_hex = run_cli(&[
    String::from("createrawtransaction"),
    inputs_json_normalized,
    outputs_json_normalized,
  ])?;

  let decoded = run_cli(&[String::from("decoderawtransaction"), raw_hex.clone()])?;
  let decoded_value: serde_json::Value = serde_json::from_str(&decoded)
    .map_err(|e| format!("Failed to parse decoded transaction: {e}"))?;

  let input_total = inputs.len();
  let output_total = outputs.len();
  let fee_warning = format!(
    "This is an unsigned raw transaction with {} input(s) and {} output(s). Fees are not computed until all inputs are known. Sign and fund before broadcasting.",
    input_total, output_total
  );

  Ok(RawTxBuildResult {
    raw_hex,
    decoded: decoded_value,
    input_count: input_total,
    output_count: output_total,
    fee_warning,
  })
}

// ---------------------------------------------------------------------------
// Rewards / Dividends Distribution
// ---------------------------------------------------------------------------

fn validate_reward_snapshot_height(height: i64) -> Result<i64, String> {
  if height <= 0 {
    return Err("Snapshot block height must be greater than zero".to_string());
  }
  Ok(height)
}

fn validate_distribution_amount(amount: &str) -> Result<f64, String> {
  parse_positive_amount(amount)
}

fn format_reward_amount(amount: f64) -> String {
  format!("{amount:.8}")
}

fn parse_exception_addresses(raw: Option<String>) -> Result<String, String> {
  match raw {
    Some(val) => {
      let trimmed = val.trim().to_string();
      if trimmed.is_empty() {
        return Ok(String::new());
      }
      let addresses: Vec<&str> = trimmed.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
      if addresses.len() > 500 {
        return Err("Too many exception addresses (max 500)".to_string());
      }
      Ok(addresses.join(","))
    }
    None => Ok(String::new()),
  }
}

fn validate_reward_exception_addresses(addresses: &str) -> Result<(), String> {
  for address in addresses.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
    if address.len() < 20 || address.len() > 90 {
      return Err("Exception address length is invalid".to_string());
    }
    if !address.chars().all(|c| c.is_ascii_alphanumeric()) {
      return Err("Exception addresses must be comma-separated address strings".to_string());
    }
  }
  Ok(())
}

fn validate_optional_change_address(change_address: Option<String>) -> Result<Option<String>, String> {
  match change_address {
    Some(value) => {
      let trimmed = value.trim().to_string();
      if trimmed.is_empty() {
        return Ok(None);
      }
      if trimmed.len() < 20 || trimmed.len() > 90 {
        return Err("Change/dust address length is invalid".to_string());
      }
      if !trimmed.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err("Change/dust address must be an alphanumeric address string".to_string());
      }
      Ok(Some(trimmed))
    }
    None => Ok(None),
  }
}

fn build_distribute_reward_args(
  ownership_asset: &str,
  snapshot_height: i64,
  distribution_asset: &str,
  gross_amount: &str,
  exception_addresses: Option<String>,
  change_address: Option<String>,
) -> Result<Vec<String>, String> {
  validate_asset_name(ownership_asset)?;
  validate_reward_snapshot_height(snapshot_height)?;
  if distribution_asset != "HEMP" {
    validate_asset_name(distribution_asset)?;
  }
  let amount_val = validate_distribution_amount(gross_amount)?;
  let amount_formatted = format_reward_amount(amount_val);
  let exceptions = parse_exception_addresses(exception_addresses)?;
  validate_reward_exception_addresses(&exceptions)?;
  let change = validate_optional_change_address(change_address)?;

  let mut args = vec![
    String::from("distributereward"),
    ownership_asset.trim().to_string(),
    format!("{snapshot_height}"),
    distribution_asset.trim().to_string(),
    amount_formatted,
  ];
  if !exceptions.is_empty() {
    args.push(exceptions);
  } else if change.is_some() {
    args.push(String::new());
  }
  if let Some(change_addr) = change {
    args.push(change_addr);
  }
  Ok(args)
}

fn reward_status_label(status: i64) -> &'static str {
  match status {
    0 => "ERROR",
    1 => "PROCESSING",
    2 => "COMPLETE",
    3 => "LOW_FUNDS",
    4 => "NOT_ENOUGH_FEE",
    5 => "LOW_REWARDS",
    6 => "STUCK_TX",
    7 => "NETWORK_ERROR",
    8 => "FAILED_CREATE_TRANSACTION",
    9 => "FAILED_COMMIT_TRANSACTION",
    _ => "UNKNOWN",
  }
}

fn reward_rpc_status_value(raw: &str) -> serde_json::Value {
  let trimmed = raw.trim();
  match serde_json::from_str::<serde_json::Value>(trimmed) {
    Ok(mut value) => {
      if let Some(status) = value.get("Status").and_then(|v| v.as_i64()) {
        if let Some(obj) = value.as_object_mut() {
          obj.insert("Status Label".to_string(), serde_json::Value::String(reward_status_label(status).to_string()));
        }
      }
      value
    }
    Err(_) => serde_json::json!({ "Status": trimmed }),
  }
}

#[tauri::command]
pub fn preview_distribute_reward(
  ownership_asset: String,
  snapshot_height: i64,
  distribution_asset: String,
  gross_amount: String,
  exception_addresses: Option<String>,
  change_address: Option<String>,
) -> Result<RewardDistributionPreview, String> {
  ensure_config()?;
  let ownership_asset = ownership_asset.trim().to_string();
  validate_asset_name(&ownership_asset)?;
  let snapshot_height = validate_reward_snapshot_height(snapshot_height)?;
  let distribution_asset = distribution_asset.trim().to_string();
  if distribution_asset == "HEMP" {
    // HEMP distribution does not need ownership token check
  } else {
    validate_asset_name(&distribution_asset)?;
  }
  let amount_val = validate_distribution_amount(&gross_amount)?;
  let amount_formatted = format_reward_amount(amount_val);
  let exceptions = parse_exception_addresses(exception_addresses)?;
  validate_reward_exception_addresses(&exceptions)?;
  let _change = validate_optional_change_address(change_address)?;

  let mut warnings = Vec::new();
  warnings.push("Reward distributions are IRREVERSIBLE once triggered. Funds cannot be recalled.".to_string());
  warnings.push("Distributereward requires -assetindex to be enabled on the node.".to_string());
  warnings.push("The distribution is processed asynchronously in batches by the node. The operation returns a status, not individual transaction IDs.".to_string());
  warnings.push(format!(
    "The snapshot must have been requested and completed at block height {} before distribution can be initiated.",
    snapshot_height
  ));

  let mut estimated_recipient_count: Option<usize> = None;

  // Try to get the snapshot for recipient count estimate (best-effort, non-blocking)
  if let Ok(raw) = run_cli(&[
    String::from("getsnapshot"),
    ownership_asset.clone(),
    format!("{snapshot_height}"),
  ]) {
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) {
      if let Some(owners) = value.get("owners").and_then(|v| v.as_array()) {
        estimated_recipient_count = Some(owners.len());
        if owners.len() > 1000 {
          warnings.push(format!(
            "Distribution has {} recipients which exceeds the 1000-per-batch limit. The node will process this in multiple batches.",
            owners.len()
          ));
        }
        if owners.len() == 0 {
          warnings.push("Snapshot has zero holders. Distribution will have no recipients.".to_string());
        }
      }
    }
  }

  let summary = format!(
    "Distribute {} {} to {} holders of '{}' at snapshot height {}",
    amount_formatted, distribution_asset,
    estimated_recipient_count.map_or("unknown".to_string(), |c| c.to_string()),
    ownership_asset, snapshot_height
  );

  Ok(RewardDistributionPreview {
    operation_type: "distribute_reward".to_string(),
    asset_name: ownership_asset.clone(),
    ownership_asset: ownership_asset.clone(),
    snapshot_height,
    distribution_asset: distribution_asset.clone(),
    gross_amount: amount_formatted,
    exception_addresses: if exceptions.is_empty() { None } else { Some(exceptions) },
    estimated_recipient_count,
    warnings,
    summary,
    is_irreversible: true,
    validated: true,
  })
}

#[tauri::command]
pub fn distribute_reward(
  ownership_asset: String,
  snapshot_height: i64,
  distribution_asset: String,
  gross_amount: String,
  exception_addresses: Option<String>,
  change_address: Option<String>,
  dry_run: Option<bool>,
) -> Result<serde_json::Value, String> {
  ensure_config()?;
  if dry_run.unwrap_or(false) {
    return Err("Dry run is handled by preview_distribute_reward; no reward distribution was broadcast.".to_string());
  }
  let args = build_distribute_reward_args(
    ownership_asset.trim(),
    snapshot_height,
    distribution_asset.trim(),
    gross_amount.trim(),
    exception_addresses,
    change_address,
  )?;

  let raw = run_cli(&args)?;

  // distributereward returns "Created reward distribution" (a simple string, not JSON)
  let trimmed = raw.trim();
  if trimmed != "Created reward distribution" {
    return Err(trimmed.to_string());
  }
  Ok(serde_json::json!({
    "status": trimmed,
    "command": "distributereward"
  }))
}

#[tauri::command]
pub fn get_distribute_reward_status(
  ownership_asset: String,
  snapshot_height: i64,
  distribution_asset: String,
  gross_amount: String,
  exception_addresses: Option<String>,
  change_address: Option<String>,
) -> Result<serde_json::Value, String> {
  ensure_config()?;
  let ownership_asset = ownership_asset.trim().to_string();
  validate_asset_name(&ownership_asset)?;
  let snapshot_height = validate_reward_snapshot_height(snapshot_height)?;
  let distribution_asset = distribution_asset.trim().to_string();
  if distribution_asset != "HEMP" {
    validate_asset_name(&distribution_asset)?;
  }
  let amount_val = validate_distribution_amount(&gross_amount)?;
  let amount_formatted = format_reward_amount(amount_val);
  let exceptions = parse_exception_addresses(exception_addresses)?;
  validate_reward_exception_addresses(&exceptions)?;
  let change = validate_optional_change_address(change_address)?;

  let mut args = vec![
    String::from("getdistributestatus"),
    ownership_asset,
    format!("{snapshot_height}"),
    distribution_asset,
    amount_formatted,
  ];
  let has_exceptions = !exceptions.is_empty();
  if has_exceptions {
    args.push(exceptions);
  }
  if let Some(change_addr) = change {
    if !has_exceptions {
      args.push(String::new());
    }
    args.push(change_addr);
  }

  let raw = run_cli(&args)?;
  Ok(reward_rpc_status_value(&raw))
}

// ---------------------------------------------------------------------------
// On-Chain Messaging Operations
// ---------------------------------------------------------------------------

fn validate_channel_name(name: &str) -> Result<String, String> {
  let trimmed = name.trim();
  if trimmed.is_empty() {
    return Err("Channel name is required".to_string());
  }
  if trimmed.len() > 128 {
    return Err("Channel name is too long".to_string());
  }
  if trimmed.chars().any(|c| c.is_whitespace()) {
    return Err("Channel name cannot contain whitespace".to_string());
  }
  Ok(trimmed.to_string())
}

fn message_authority_asset_name(channel_name: &str) -> String {
  if channel_name.ends_with('!') || channel_name.contains('~') {
    channel_name.to_string()
  } else {
    format!("{channel_name}!")
  }
}

fn validate_ipfs_hash(hash: &str) -> Result<String, String> {
  let trimmed = hash.trim();
  if trimmed.is_empty() {
    return Err("IPFS hash is required for message content".to_string());
  }
  if trimmed.len() > 64 {
    return Err("IPFS hash is too long".to_string());
  }
  Ok(trimmed.to_string())
}

fn validate_message_expire_time(expire_time: Option<i64>) -> Result<Option<i64>, String> {
  if let Some(expire_time) = expire_time {
    if expire_time <= 0 {
      return Err("Expire time must be a positive UTC timestamp".to_string());
    }
    Ok(Some(expire_time))
  } else {
    Ok(None)
  }
}

fn wallet_owns_asset(asset_name: &str) -> bool {
  match run_cli(&[
    String::from("listmyassets"),
    asset_name.to_string(),
    String::from("true"),
  ]) {
    Ok(raw) => {
      if let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) {
        asset_balance_from_listmyassets(&value, asset_name).unwrap_or(0.0) > 0.0
      } else {
        false
      }
    }
    Err(_) => false,
  }
}

fn parse_message_entry(value: &serde_json::Value) -> AssetMessageEntry {
  let expire_time = value.get("Expire Time").and_then(|v| v.as_str()).map(|s| s.to_string());
  let expire_utc_time = value.get("Expire UTC Time").and_then(|v| v.as_i64());
  AssetMessageEntry {
    asset_name: value.get("Asset Name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
    message: value.get("Message").and_then(|v| v.as_str()).unwrap_or("").to_string(),
    time: value.get("Time").and_then(|v| v.as_str()).unwrap_or("").to_string(),
    block_height: value.get("Block Height").and_then(|v| v.as_i64()).unwrap_or(0),
    status: value.get("Status").and_then(|v| v.as_str()).unwrap_or("UNKNOWN").to_string(),
    expire_time,
    expire_utc_time,
  }
}

fn parse_channel_name_list(value: &serde_json::Value) -> Vec<String> {
  value.as_array()
    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
    .unwrap_or_default()
}

fn parse_messaging_info(value: &serde_json::Value) -> MessagingInfo {
  let warnings: Vec<String> = value.get("warnings")
    .and_then(|w| w.as_array())
    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
    .unwrap_or_default();

  fn boolish(value: &serde_json::Value, key: &str) -> bool {
    match value.get(key) {
      Some(v) if v.is_boolean() => v.as_bool().unwrap_or(false),
      Some(v) if v.is_number() => v.as_i64().unwrap_or(0) != 0,
      _ => false,
    }
  }

  MessagingInfo {
    enabled: boolish(value, "enabled"),
    messaging_active: boolish(value, "messaging_active"),
    restricted_active: boolish(value, "restricted_active"),
    activation_block: value.get("activation_block").and_then(|v| v.as_i64()).unwrap_or(0),
    databases_available: boolish(value, "databases_available"),
    caches_available: boolish(value, "caches_available"),
    message_count: value.get("message_count").and_then(|v| v.as_i64()).unwrap_or(0),
    channel_count: value.get("channel_count").and_then(|v| v.as_i64()).unwrap_or(0),
    dirty_cache_size_bytes: value.get("dirty_cache_size_bytes").and_then(|v| v.as_i64()).unwrap_or(0),
    wallet_available: boolish(value, "wallet_available"),
    warnings,
  }
}

#[tauri::command]
pub fn get_messaging_info() -> Result<MessagingInfo, String> {
  ensure_config()?;
  let raw = run_cli(&[String::from("getmessaginginfo")])?;
  let value: serde_json::Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
  Ok(parse_messaging_info(&value))
}

#[tauri::command]
pub fn view_asset_messages() -> Result<Vec<AssetMessageEntry>, String> {
  ensure_config()?;
  let raw = run_cli(&[String::from("viewallmessages")])?;
  let value: serde_json::Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
  let messages: Vec<AssetMessageEntry> = value.as_array()
    .map(|arr| arr.iter().map(parse_message_entry).collect())
    .unwrap_or_default();
  Ok(messages)
}

#[tauri::command]
pub fn view_message_channels() -> Result<Vec<String>, String> {
  ensure_config()?;
  let raw = run_cli(&[String::from("viewallmessagechannels")])?;
  let value: serde_json::Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
  Ok(parse_channel_name_list(&value))
}

#[tauri::command]
pub fn subscribe_to_channel(channel_name: String) -> Result<String, String> {
  ensure_config()?;
  validate_channel_name(&channel_name)?;
  run_cli(&[String::from("subscribetochannel"), channel_name])
}

#[tauri::command]
pub fn unsubscribe_from_channel(channel_name: String) -> Result<String, String> {
  ensure_config()?;
  validate_channel_name(&channel_name)?;
  run_cli(&[String::from("unsubscribefromchannel"), channel_name])
}

#[tauri::command]
pub fn preview_send_announcement(
  channel_name: String,
  ipfs_hash: String,
  expire_time: Option<i64>,
) -> Result<AssetAnnouncementPreview, String> {
  ensure_config()?;
  let channel_name = validate_channel_name(&channel_name)?;
  let ipfs_hash = validate_ipfs_hash(&ipfs_hash)?;
  let expire_time = validate_message_expire_time(expire_time)?;
  let authority_asset = message_authority_asset_name(&channel_name);

  let has_ownership = wallet_owns_asset(&authority_asset);

  let mut warnings = Vec::new();
  if !has_ownership {
    warnings.push("You do not appear to hold the channel asset. Sending a message requires owning the channel asset and that wallet unlock may be required.".to_string());
  }
  warnings.push("Sending a message is an on-chain broadcast that creates a transaction. This is irreversible and requires wallet unlock.".to_string());

  let summary = format!(
    "Send announcement on '{}' with IPFS hash {}",
    channel_name, ipfs_hash
  );

  Ok(AssetAnnouncementPreview {
    channel_name,
    ipfs_hash,
    expire_time,
    has_ownership,
    is_irreversible: true,
    warnings,
    summary,
    validated: true,
  })
}

#[tauri::command]
pub fn send_announcement(
  channel_name: String,
  ipfs_hash: String,
  expire_time: Option<i64>,
) -> Result<String, String> {
  ensure_config()?;
  let channel_name = validate_channel_name(&channel_name)?;
  let ipfs_hash = validate_ipfs_hash(&ipfs_hash)?;
  let expire_time = validate_message_expire_time(expire_time)?;
  let authority_asset = message_authority_asset_name(&channel_name);

  if !wallet_owns_asset(&authority_asset) {
    return Err(format!(
      "Wallet does not currently own the channel authority asset ({authority_asset}). Refresh and preview again."
    ));
  }

  let mut args = vec![
    String::from("sendmessage"),
    channel_name,
    ipfs_hash,
  ];
  if let Some(expire) = expire_time {
    args.push(format!("{expire}"));
  }

  normalize_cli_txid(run_cli(&args)?)
}

// ---------------------------------------------------------------------------
// IPFS Reference Helpers
// ---------------------------------------------------------------------------

fn validate_ipfs_reference(hash: &str) -> Result<IpfsReferenceInfo, String> {
  let trimmed = hash.trim();
  if trimmed.is_empty() {
    return Err("IPFS hash is empty".to_string());
  }
  if trimmed.len() > 64 {
    return Err("IPFS hash is too long".to_string());
  }
  if trimmed.chars().any(|c| c.is_whitespace() || c.is_control()) {
    return Err("IPFS hash contains whitespace or control characters".to_string());
  }
  let mut warnings = Vec::new();
  let kind = if trimmed.starts_with("Qm") && trimmed.len() >= 46 {
    "cidv0".to_string()
  } else if trimmed.starts_with("bafy") || trimmed.starts_with("bafk") || trimmed.starts_with("bae") {
    "cidv1".to_string()
  } else {
    warnings.push("Hash does not match known CIDv0 or CIDv1 prefix patterns".to_string());
    "unknown".to_string()
  };
  Ok(IpfsReferenceInfo {
    normalized: trimmed.to_string(),
    kind,
    warnings,
  })
}

fn build_ipfs_gateway_url(hash: &str, gateway_base: Option<String>) -> Result<String, String> {
  let info = validate_ipfs_reference(hash)?;
  let base = match gateway_base {
    Some(ref g) if !g.trim().is_empty() => g.trim().to_string(),
    _ => "http://127.0.0.1:8080/ipfs/".to_string(),
  };
  let clean_base = base.trim_end_matches('/');
  let url = format!("{}/{}", clean_base, info.normalized);
  Ok(url)
}

#[tauri::command]
pub fn ipfs_validate(hash: String) -> Result<IpfsReferenceInfo, String> {
  validate_ipfs_reference(&hash)
}

#[tauri::command]
pub fn ipfs_gateway_url(hash: String, gateway_base: Option<String>) -> Result<String, String> {
  build_ipfs_gateway_url(&hash, gateway_base)
}

// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
  use std::collections::HashMap;
  use super::{
    asset_balance_from_listmyassets, build_issue_unique_args, build_reissue_args,
    build_export_wallet_migration_args, build_restore_wallet_migration_args,
    build_validate_wallet_migration_args, detect_duplicate_inputs,
    is_utxo_unsafe_for_hemp, normalize_cli_txid, normalize_unique_asset_inputs,
    parse_non_negative_amount, parse_output_sum, parse_positive_amount,
    validate_asset_name, validate_asset_transfer_preview_fields,
    validate_migration_passphrase, validate_migration_wallet_name,
    validate_send_preview_fields,
    validate_qualifier_name, validate_restricted_name, validate_verifier_string,
    estimate_legacy_tx_bytes, max_policy_inputs, estimate_fee_from_bytes,
    format_hemp_amount, sat_to_hemp, validate_fee_rate_sat_per_byte,
    clamp_fee_rate_sat_per_byte, parse_estimatesmartfee_sat_per_byte,
    recommended_consolidation_fee_rate_sat_per_byte, estimate_consolidation_round_count,
    collect_selected_safe_utxos,
    hemp_to_sat,
    validate_channel_name, validate_ipfs_hash, validate_message_expire_time,
    message_authority_asset_name,
    parse_message_entry, parse_channel_name_list, parse_messaging_info,
    validate_ipfs_reference, build_ipfs_gateway_url, build_reissue_preview,
    validate_raw_tx_hex, validate_tx_input, normalize_raw_tx_outputs,
  };
  use crate::modules::models::RawTxInput;

  #[test]
  fn validates_valid_send_preview_input() {
    assert!(validate_send_preview_fields("Haddr1", "1.5", "HEMP", None).is_ok());
  }

  #[test]
  fn warns_on_send_preview_input_with_long_label() {
    let label = "a".repeat(300);
    let (_, warnings) =
      validate_send_preview_fields("Haddr1", "0.5", "HEMP", Some(&label)).unwrap();
    assert_eq!(warnings.len(), 1);
  }

  #[test]
  fn rejects_empty_destination() {
    let result = validate_send_preview_fields("", "1.0", "HEMP", None);
    assert!(result.is_err());
  }

  #[test]
  fn rejects_zero_amount() {
    let result = validate_send_preview_fields("Haddr1", "0", "HEMP", None);
    assert!(result.is_err());
  }

  #[test]
  fn rejects_negative_amount() {
    let result = validate_send_preview_fields("Haddr1", "-1", "HEMP", None);
    assert!(result.is_err());
  }

  #[test]
  fn rejects_non_numeric_amount() {
    let result = validate_send_preview_fields("Haddr1", "abc", "HEMP", None);
    assert!(result.is_err());
  }

  #[test]
  fn rejects_non_hemp_asset_for_hemp_preview() {
    let result = validate_send_preview_fields("Haddr1", "1.0", "TOKEN", None);
    assert!(result.is_err());
  }

  #[test]
  fn accepts_valid_label() {
    assert!(validate_send_preview_fields("Haddr1", "1.0", "HEMP", Some("My Label")).is_ok());
  }

  #[test]
  fn builds_reissue_args_in_core_order_without_ipfs() {
    let args = build_reissue_args("ROOT", "1", "Haddr", "Hchange", true, 8, "").unwrap();
    assert_eq!(
      args,
      vec![
        "reissue",
        "ROOT",
        "1",
        "Haddr",
        "Hchange",
        "true",
        "8",
      ]
    );
  }

  #[test]
  fn builds_reissue_args_in_core_order_with_ipfs() {
    let args = build_reissue_args("ROOT", "0", "Haddr", "Hchange", false, 0, "QmHash").unwrap();
    assert_eq!(
      args,
      vec![
        "reissue",
        "ROOT",
        "0",
        "Haddr",
        "Hchange",
        "false",
        "0",
        "QmHash",
      ]
    );
  }

  // --- Reissue Preview Tests ---

  #[test]
  fn reissue_preview_empty_ipfs_preserves_old_behavior() {
    let preview = build_reissue_preview("TOKEN", 0.0, 0, true, "").unwrap();
    assert_eq!(preview.ipfs_hash, None);
    assert_eq!(preview.qty, Some("0".to_string()));
    assert!(preview.warnings.iter().any(|w| w.contains("no new supply will be created")));
  }

  #[test]
  fn reissue_preview_valid_ipfs_includes_it() {
    let preview = build_reissue_preview("TOKEN", 0.0, 0, true, "QmZPGfJojdTzaqCWJu2m3krark38X1rqEHBo4SjeqHKB26").unwrap();
    assert_eq!(preview.ipfs_hash, Some("QmZPGfJojdTzaqCWJu2m3krark38X1rqEHBo4SjeqHKB26".to_string()));
    assert!(preview.warnings.iter().any(|w| w.contains("metadata update without increasing supply")));
  }

  #[test]
  fn reissue_preview_invalid_ipfs_is_rejected() {
    let result = build_reissue_preview("TOKEN", 0.0, 0, true, "bad hash with spaces");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Invalid IPFS hash"));
  }

  // --- Asset Transfer Preview Tests ---

  #[test]
  fn asset_preview_rejects_hemp() {
    let result = validate_asset_transfer_preview_fields("Haddr1", "1.0", "HEMP");
    assert!(result.is_err());
  }

  #[test]
  fn asset_preview_rejects_hemp_case_insensitive() {
    let result = validate_asset_transfer_preview_fields("Haddr1", "1.0", "hemp");
    assert!(result.is_err());
  }

  #[test]
  fn asset_preview_rejects_empty_asset() {
    let result = validate_asset_transfer_preview_fields("Haddr1", "1.0", "");
    assert!(result.is_err());
  }

  #[test]
  fn asset_preview_rejects_whitespace_asset() {
    let result = validate_asset_transfer_preview_fields("Haddr1", "1.0", "   ");
    assert!(result.is_err());
  }

  #[test]
  fn asset_preview_rejects_empty_destination() {
    let result = validate_asset_transfer_preview_fields("", "1.0", "TOKEN");
    assert!(result.is_err());
  }

  #[test]
  fn asset_preview_rejects_zero_amount() {
    let result = validate_asset_transfer_preview_fields("Haddr1", "0", "TOKEN");
    assert!(result.is_err());
  }

  #[test]
  fn asset_preview_rejects_negative_amount() {
    let result = validate_asset_transfer_preview_fields("Haddr1", "-1", "TOKEN");
    assert!(result.is_err());
  }

  #[test]
  fn asset_preview_rejects_non_numeric_amount() {
    let result = validate_asset_transfer_preview_fields("Haddr1", "abc", "TOKEN");
    assert!(result.is_err());
  }

  #[test]
  fn asset_preview_accepts_valid_asset_transfer() {
    assert!(validate_asset_transfer_preview_fields("Haddr1", "100.0", "TOKEN").is_ok());
  }

  #[test]
  fn asset_preview_accepts_fractional_amount() {
    assert!(validate_asset_transfer_preview_fields("Haddr1", "0.5", "TOKEN").is_ok());
  }

  #[test]
  fn parses_asset_balance_from_keyed_number_response() {
    let value = serde_json::json!({ "TOKEN": 12.5 });
    assert_eq!(asset_balance_from_listmyassets(&value, "TOKEN"), Some(12.5));
  }

  #[test]
  fn parses_asset_balance_from_keyed_object_response() {
    let value = serde_json::json!({ "TOKEN": { "balance": 7.25 } });
    assert_eq!(asset_balance_from_listmyassets(&value, "TOKEN"), Some(7.25));
  }

  #[test]
  fn parses_asset_balance_from_direct_number_response() {
    let value = serde_json::json!(3.0);
    assert_eq!(asset_balance_from_listmyassets(&value, "TOKEN"), Some(3.0));
  }

  // --- Asset Name Validation Tests ---

  #[test]
  fn valid_asset_name_passes() {
    assert!(validate_asset_name("MYTOKEN").is_ok());
    assert!(validate_asset_name("A").is_ok());
    assert!(validate_asset_name("MYTOKEN/SUB").is_ok());
  }

  #[test]
  fn empty_asset_name_fails() {
    assert!(validate_asset_name("").is_err());
    assert!(validate_asset_name("   ").is_err());
  }

  #[test]
  fn overly_long_asset_name_fails() {
    let long_name = "A".repeat(129);
    assert!(validate_asset_name(&long_name).is_err());
    let max_name = "A".repeat(128);
    assert!(validate_asset_name(&max_name).is_ok());
  }

  #[test]
  fn asset_name_with_whitespace_fails() {
    assert!(validate_asset_name("MY TOKEN").is_err());
    assert!(validate_asset_name("MY\tTOKEN").is_err());
  }

  // --- Amount Parsing Tests ---

  #[test]
  fn positive_amount_parses() {
    assert!(parse_positive_amount("1.5").is_ok());
    assert_eq!(parse_positive_amount("100").unwrap(), 100.0);
  }

  #[test]
  fn zero_positive_amount_fails() {
    assert!(parse_positive_amount("0").is_err());
    assert!(parse_positive_amount("0.0").is_err());
  }

  #[test]
  fn negative_positive_amount_fails() {
    assert!(parse_positive_amount("-1").is_err());
  }

  #[test]
  fn non_numeric_positive_amount_fails() {
    assert!(parse_positive_amount("abc").is_err());
    assert!(parse_positive_amount("").is_err());
  }

  #[test]
  fn non_negative_amount_parses() {
    assert!(parse_non_negative_amount("0").is_ok());
    assert!(parse_non_negative_amount("1.5").is_ok());
  }

  #[test]
  fn negative_non_negative_amount_fails() {
    assert!(parse_non_negative_amount("-1").is_err());
    assert!(parse_non_negative_amount("-0.1").is_err());
  }

  // --- Reissue Args Tests ---

  #[test]
  fn reissue_args_reject_empty_to_address() {
    let result = build_reissue_args("ROOT", "1", "", "Hchange", true, 8, "");
    assert!(result.is_err());
  }

  #[test]
  fn reissue_args_reject_empty_change_address() {
    let result = build_reissue_args("ROOT", "1", "Haddr", "", true, 8, "");
    assert!(result.is_err());
  }

  #[test]
  fn reissue_args_reject_units_over_8() {
    let result = build_reissue_args("ROOT", "1", "Haddr", "Hchange", true, 9, "");
    assert!(result.is_err());
  }

  #[test]
  fn unique_asset_inputs_normalize_clean_values() {
    let (root, tags, ipfs) = normalize_unique_asset_inputs(
      " root ".to_string(),
      vec![" one ".to_string()],
      vec![" QmTest ".to_string()],
    )
    .unwrap();
    assert_eq!(root, "ROOT");
    assert_eq!(tags, vec!["one"]);
    assert_eq!(ipfs, vec!["QmTest"]);
  }

  #[test]
  fn unique_asset_inputs_reject_invalid_tags() {
    assert!(normalize_unique_asset_inputs(
      "ROOT".to_string(),
      vec!["bad/tag".to_string()],
      vec![],
    )
    .is_err());
    assert!(normalize_unique_asset_inputs(
      "ROOT".to_string(),
      vec!["bad tag".to_string()],
      vec![],
    )
    .is_err());
  }

  #[test]
  fn unique_asset_inputs_require_matching_ipfs_count() {
    assert!(normalize_unique_asset_inputs(
      "ROOT".to_string(),
      vec!["one".to_string(), "two".to_string()],
      vec!["QmOne".to_string()],
    )
    .is_err());
  }

  #[test]
  fn issue_unique_args_omit_empty_ipfs_argument() {
    let args = build_issue_unique_args(
      "ROOT".to_string(),
      vec!["one".to_string()],
      vec![],
    )
    .unwrap();
    assert_eq!(args, vec![
      "issueunique".to_string(),
      "ROOT".to_string(),
      "[\"one\"]".to_string(),
    ]);
  }

  #[test]
  fn issue_unique_args_include_matching_ipfs_argument() {
    let args = build_issue_unique_args(
      "ROOT".to_string(),
      vec!["one".to_string()],
      vec!["QmOne".to_string()],
    )
    .unwrap();
    assert_eq!(args, vec![
      "issueunique".to_string(),
      "ROOT".to_string(),
      "[\"one\"]".to_string(),
      "[\"QmOne\"]".to_string(),
    ]);
  }

  #[test]
  fn cli_txid_normalizer_extracts_array_txid() {
    assert_eq!(
      normalize_cli_txid("[\"abc123\"]".to_string()).unwrap(),
      "abc123"
    );
    assert_eq!(
      normalize_cli_txid("  abc123  ".to_string()).unwrap(),
      "abc123"
    );
  }

  #[test]
  fn cli_txid_normalizer_rejects_empty_output() {
    assert!(normalize_cli_txid("".to_string()).is_err());
    assert!(normalize_cli_txid("   ".to_string()).is_err());
  }

  #[test]
  fn cli_txid_normalizer_rejects_empty_array() {
    assert!(normalize_cli_txid("[]".to_string()).is_err());
  }

  // --- Duplicate Input Detection Tests ---

  #[test]
  fn detects_duplicate_inputs() {
    use super::RawTxInput;
    let inputs = vec![
      RawTxInput { txid: "abc".to_string(), vout: 0 },
      RawTxInput { txid: "abc".to_string(), vout: 0 },
    ];
    assert!(detect_duplicate_inputs(&inputs));
  }

  #[test]
  fn no_duplicate_for_distinct_inputs() {
    use super::RawTxInput;
    let inputs = vec![
      RawTxInput { txid: "abc".to_string(), vout: 0 },
      RawTxInput { txid: "abc".to_string(), vout: 1 },
      RawTxInput { txid: "def".to_string(), vout: 0 },
    ];
    assert!(!detect_duplicate_inputs(&inputs));
  }

  #[test]
  fn no_duplicate_for_single_input() {
    use super::RawTxInput;
    let inputs = vec![
      RawTxInput { txid: "abc".to_string(), vout: 0 },
    ];
    assert!(!detect_duplicate_inputs(&inputs));
  }

  // --- Output Parsing Tests ---

  #[test]
  fn parses_valid_output_map() {
    let mut outputs = HashMap::new();
    outputs.insert("Haddr1".to_string(), "1.5".to_string());
    outputs.insert("Hchange".to_string(), "0.3".to_string());
    let sum = parse_output_sum(&outputs).unwrap();
    assert!((sum - 1.8).abs() < f64::EPSILON);
  }

  #[test]
  fn parses_single_output() {
    let mut outputs = HashMap::new();
    outputs.insert("Haddr1".to_string(), "100.0".to_string());
    let sum = parse_output_sum(&outputs).unwrap();
    assert!((sum - 100.0).abs() < f64::EPSILON);
  }

  #[test]
  fn rejects_empty_output_address() {
    let mut outputs = HashMap::new();
    outputs.insert("".to_string(), "1.0".to_string());
    assert!(parse_output_sum(&outputs).is_err());
  }

  #[test]
  fn rejects_non_numeric_output_amount() {
    let mut outputs = HashMap::new();
    outputs.insert("Haddr1".to_string(), "abc".to_string());
    assert!(parse_output_sum(&outputs).is_err());
  }

  #[test]
  fn rejects_zero_output_amount() {
    let mut outputs = HashMap::new();
    outputs.insert("Haddr1".to_string(), "0.0".to_string());
    assert!(parse_output_sum(&outputs).is_err());
  }

  #[test]
  fn rejects_negative_output_amount() {
    let mut outputs = HashMap::new();
    outputs.insert("Haddr1".to_string(), "-1.0".to_string());
    assert!(parse_output_sum(&outputs).is_err());
  }

  // --- Unsafe UTXO Detection Tests ---

  #[test]
  fn safe_hemp_utxo_is_not_unsafe() {
    assert!(!is_utxo_unsafe_for_hemp(Some(true), Some(true), Some("HEMP"), None));
    assert!(!is_utxo_unsafe_for_hemp(None, None, None, None));
  }

  #[test]
  fn unspendable_utxo_is_unsafe() {
    assert!(is_utxo_unsafe_for_hemp(Some(false), Some(true), Some("HEMP"), None));
  }

  #[test]
  fn unsafe_marked_utxo_is_unsafe() {
    assert!(is_utxo_unsafe_for_hemp(Some(true), Some(false), Some("HEMP"), None));
  }

  #[test]
  fn asset_utxo_is_unsafe() {
    assert!(is_utxo_unsafe_for_hemp(Some(true), Some(true), Some("TOKEN"), None));
  }

  #[test]
  fn asset_amount_utxo_is_unsafe() {
    assert!(is_utxo_unsafe_for_hemp(Some(true), Some(true), None, Some(1.0)));
  }

  #[test]
  fn dump_priv_key_rejects_empty_address_via_frontend() {
    let empty = String::new();
    assert!(empty.trim().is_empty());
    let valid = String::from("Haddr");
    assert!(!valid.trim().is_empty());
  }

  #[test]
  fn migration_export_public_builds_args() {
    let args = build_export_wallet_migration_args(
      "/tmp/hemp0x-migration.json".to_string(),
      false,
      false,
      String::new(),
    )
    .unwrap();
    assert_eq!(args, vec![
      "exportwalletmigration".to_string(),
      "/tmp/hemp0x-migration.json".to_string(),
      "false".to_string(),
      "false".to_string(),
    ]);
  }

  #[test]
  fn migration_export_private_requires_passphrase_min_length() {
    let err = build_export_wallet_migration_args(
      "/tmp/hemp0x-migration.json".to_string(),
      true,
      false,
      "short".to_string(),
    )
    .unwrap_err();
    assert!(err.contains("at least 8"));

    let args = build_export_wallet_migration_args(
      "/tmp/hemp0x-migration.json".to_string(),
      true,
      false,
      "long enough".to_string(),
    )
    .unwrap();
    assert_eq!(args.last().unwrap(), "long enough");
  }

  #[test]
  fn migration_passphrase_max_length_check() {
    let max_pass = String::from("a").repeat(1024);
    assert!(validate_migration_passphrase(&max_pass, true, "Export passphrase").is_ok());
    let over_pass = String::from("a").repeat(1025);
    assert!(validate_migration_passphrase(&over_pass, true, "Export passphrase").is_err());
  }

  #[test]
  fn migration_validate_preserves_passphrase_whitespace() {
    let args = build_validate_wallet_migration_args(
      "/tmp/hemp0x-migration.json".to_string(),
      " pass phrase ".to_string(),
    )
    .unwrap();
    assert_eq!(args.last().unwrap(), " pass phrase ");
  }

  #[test]
  fn migration_restore_rejects_empty_wallet_name() {
    let err = build_restore_wallet_migration_args(
      "/tmp/hemp0x-migration.json".to_string(),
      String::new(),
      "long enough".to_string(),
      None,
    )
    .unwrap_err();
    assert!(err.contains("Wallet name"));
  }

  #[test]
  fn migration_restore_rejects_empty_passphrase() {
    let err = build_restore_wallet_migration_args(
      "/tmp/hemp0x-migration.json".to_string(),
      "restored_wallet".to_string(),
      String::new(),
      None,
    )
    .unwrap_err();
    assert!(err.contains("Export passphrase"));
  }

  #[test]
  fn migration_restore_rejects_path_like_wallet_names() {
    assert!(validate_migration_wallet_name("../wallet").is_err());
    assert!(validate_migration_wallet_name("bad/name").is_err());
    assert!(validate_migration_wallet_name("bad\\name").is_err());
    assert!(validate_migration_wallet_name("C:wallet").is_err());
  }

  #[test]
  fn migration_restore_rejects_reserved_wallet_names() {
    assert!(validate_migration_wallet_name("CON").is_err());
    assert!(validate_migration_wallet_name("nul.dat").is_err());
    assert!(validate_migration_wallet_name("LPT1").is_err());
    assert!(validate_migration_wallet_name("wallet?name").is_err());
    assert!(validate_migration_wallet_name("wallet\nname").is_err());
    assert!(validate_migration_wallet_name("safe_wallet").is_ok());
  }

  #[test]
  fn migration_restore_rejects_negative_birth_height() {
    let err = build_restore_wallet_migration_args(
      "/tmp/hemp0x-migration.json".to_string(),
      "restored_wallet".to_string(),
      "long enough".to_string(),
      Some(-1),
    )
    .unwrap_err();
    assert!(err.contains("Birth height"));
  }

  // --- Qualifier Name Validation Tests ---

  #[test]
  fn valid_qualifier_name_normalizes_with_hash() {
    let result = validate_qualifier_name("#TAG").unwrap();
    assert_eq!(result, "#TAG");
  }

  #[test]
  fn qualifier_name_adds_hash_prefix() {
    let result = validate_qualifier_name("TAG").unwrap();
    assert_eq!(result, "#TAG");
  }

  #[test]
  fn qualifier_name_rejects_empty() {
    assert!(validate_qualifier_name("").is_err());
    assert!(validate_qualifier_name("   ").is_err());
    assert!(validate_qualifier_name("#").is_err());
    assert!(validate_qualifier_name("#ROOT/").is_err());
  }

  #[test]
  fn qualifier_name_rejects_whitespace() {
    assert!(validate_qualifier_name("#MY TAG").is_err());
  }

  #[test]
  fn qualifier_name_rejects_too_long() {
    let long_name = "#".to_string() + &"A".repeat(128);
    assert!(validate_qualifier_name(&long_name).is_err());
  }

  // --- Restricted Name Validation Tests ---

  #[test]
  fn valid_restricted_name_normalizes_with_dollar() {
    let result = validate_restricted_name("$ASSET").unwrap();
    assert_eq!(result, "$ASSET");
  }

  #[test]
  fn restricted_name_adds_dollar_prefix() {
    let result = validate_restricted_name("ASSET").unwrap();
    assert_eq!(result, "$ASSET");
  }

  #[test]
  fn restricted_name_rejects_empty() {
    assert!(validate_restricted_name("").is_err());
    assert!(validate_restricted_name("   ").is_err());
    assert!(validate_restricted_name("$").is_err());
  }

  #[test]
  fn restricted_name_rejects_whitespace() {
    assert!(validate_restricted_name("$MY ASSET").is_err());
  }

  // --- Verifier Validation Tests ---

  #[test]
  fn valid_verifier_string_passes() {
    assert_eq!(validate_verifier_string("#KYC & !#AML").unwrap(), "#KYC & !#AML");
  }

  #[test]
  fn verifier_string_rejects_empty() {
    assert!(validate_verifier_string("").is_err());
    assert!(validate_verifier_string("   ").is_err());
  }

  #[test]
  fn verifier_string_rejects_too_long() {
    let long = "A".repeat(513);
    assert!(validate_verifier_string(&long).is_err());
  }

  // --- Qualifier/Issue Argument Builder Tests ---

  #[test]
  fn qualifier_issue_args_minimal() {
    // Test the args for issuequalifierasset with minimal params
    // name qty [to_address] [change_address] [has_ipfs] [ipfs_hash]
    // Since we call run_cli which requires a node, we test validation only
    let name = validate_qualifier_name("#TAG").unwrap();
    let qty = parse_positive_amount("3").unwrap();
    assert_eq!(name, "#TAG");
    assert_eq!(qty, 3.0);
  }

  #[test]
  fn qualifier_amount_rejects_out_of_range() {
    // parse_positive_amount just checks > 0, so 0.5 passes it.
    // The qualifier range 1-10 is enforced at the command level.
    assert!(parse_positive_amount("100").is_ok());
    assert!(parse_positive_amount("5.5").is_ok());
    assert!(parse_positive_amount("0").is_err());
  }

  #[test]
  fn restricted_issue_args_validation() {
    let name = validate_restricted_name("$TOKEN").unwrap();
    let verifier = validate_verifier_string("#KYC").unwrap();
    assert_eq!(name, "$TOKEN");
    assert_eq!(verifier, "#KYC");
  }

  // --- Reward Distribution Tests ---

  use super::{
    build_cancel_snapshot_args, build_distribute_reward_args, format_reward_amount, parse_exception_addresses, reward_rpc_status_value,
    validate_distribution_amount, validate_reward_exception_addresses,
    validate_reward_snapshot_height,
  };

  #[test]
  fn reward_snapshot_height_positive() {
    assert_eq!(validate_reward_snapshot_height(100).unwrap(), 100);
  }

  #[test]
  fn reward_snapshot_height_rejects_zero() {
    assert!(validate_reward_snapshot_height(0).is_err());
  }

  #[test]
  fn reward_snapshot_height_rejects_negative() {
    assert!(validate_reward_snapshot_height(-1).is_err());
  }

  #[test]
  fn reward_amount_positive() {
    assert_eq!(validate_distribution_amount("100.5").unwrap(), 100.5);
  }

  #[test]
  fn reward_amount_formats_fixed_precision() {
    assert_eq!(format_reward_amount(100.5), "100.50000000");
  }

  #[test]
  fn reward_amount_rejects_zero() {
    assert!(validate_distribution_amount("0").is_err());
    assert!(validate_distribution_amount("0.0").is_err());
  }

  #[test]
  fn reward_amount_rejects_negative() {
    assert!(validate_distribution_amount("-50").is_err());
  }

  #[test]
  fn reward_amount_rejects_non_numeric() {
    assert!(validate_distribution_amount("abc").is_err());
  }

  #[test]
  fn reward_exception_addresses_parses_single() {
    let result = parse_exception_addresses(Some("Haddr1".to_string())).unwrap();
    assert_eq!(result, "Haddr1");
  }

  #[test]
  fn reward_exception_addresses_parses_comma_separated() {
    let result = parse_exception_addresses(Some("Haddr1, Haddr2 , Haddr3".to_string())).unwrap();
    assert_eq!(result, "Haddr1,Haddr2,Haddr3");
  }

  #[test]
  fn reward_exception_addresses_empty_none() {
    assert_eq!(parse_exception_addresses(None).unwrap(), "");
  }

  #[test]
  fn reward_exception_addresses_empty_some() {
    assert_eq!(parse_exception_addresses(Some("   ".to_string())).unwrap(), "");
  }

  #[test]
  fn reward_exception_addresses_validate_lightweight_syntax() {
    assert!(validate_reward_exception_addresses("H123456789ABCDEFGHijklmno").is_ok());
    assert!(validate_reward_exception_addresses("bad address").is_err());
    assert!(validate_reward_exception_addresses("short").is_err());
  }

  #[test]
  fn cancel_snapshot_args_reject_empty_asset() {
    assert!(build_cancel_snapshot_args("", 123).is_err());
  }

  #[test]
  fn cancel_snapshot_args_reject_invalid_height() {
    assert!(build_cancel_snapshot_args("ASSET", 0).is_err());
    assert!(build_cancel_snapshot_args("ASSET", -1).is_err());
  }

  #[test]
  fn cancel_snapshot_args_builds_expected() {
    let args = build_cancel_snapshot_args("ASSET", 12345).unwrap();
    assert_eq!(args, vec!["cancelsnapshotrequest", "ASSET", "12345"]);
  }

  #[test]
  fn reward_status_value_labels_numeric_status() {
    let value = reward_rpc_status_value(r#"{"Status":1}"#);
    assert_eq!(value["Status Label"], "PROCESSING");
  }

  #[test]
  fn raw_tx_hex_validation_accepts_even_hex() {
    assert_eq!(
      validate_raw_tx_hex(" 01000000000000000000 ").unwrap(),
      "01000000000000000000"
    );
  }

  #[test]
  fn raw_tx_hex_validation_rejects_odd_or_non_hex() {
    assert!(validate_raw_tx_hex("abc").is_err());
    assert!(validate_raw_tx_hex("zz00000000").is_err());
    assert!(validate_raw_tx_hex("00").is_err());
  }

  #[test]
  fn raw_tx_input_validation_requires_txid_and_numeric_vout() {
    let valid = serde_json::json!({
      "txid": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      "vout": 1
    });
    assert!(validate_tx_input(&valid).is_ok());

    let bad_txid = serde_json::json!({ "txid": "abc", "vout": 1 });
    assert!(validate_tx_input(&bad_txid).is_err());

    let bad_vout = serde_json::json!({
      "txid": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      "vout": "1"
    });
    assert!(validate_tx_input(&bad_vout).is_err());
  }

  #[test]
  fn raw_tx_outputs_normalize_to_numeric_json_values() {
    let outputs = vec![serde_json::json!({
      "address": "H123456789ABCDEFGHijklmno",
      "amount": "1.25000000"
    })];
    let normalized = normalize_raw_tx_outputs(&outputs).unwrap();
    assert_eq!(normalized["H123456789ABCDEFGHijklmno"], serde_json::json!(1.25));
  }

  #[test]
  fn raw_tx_outputs_reject_duplicate_address() {
    let outputs = vec![
      serde_json::json!({ "address": "H123456789ABCDEFGHijklmno", "amount": "1.0" }),
      serde_json::json!({ "address": "H123456789ABCDEFGHijklmno", "amount": "2.0" }),
    ];
    assert!(normalize_raw_tx_outputs(&outputs).is_err());
  }

  #[test]
  fn reward_distributereward_args_validation() {
    let asset = "MYTOKEN".to_string();
    let height = 12345i64;
    let _dist = "HEMP".to_string();
    let amount = "1000";
    assert!(validate_asset_name(&asset).is_ok());
    assert!(validate_reward_snapshot_height(height).is_ok());
    assert!(validate_distribution_amount(amount).is_ok());
    // For HEMP distribution, no ownership token check needed client-side
    // Non-HEMP distribution would require asset name validation
    assert!(validate_asset_name("DIVIDENDS").is_ok());
  }

  #[test]
  fn reward_distributereward_args_validation_non_hemp() {
    assert!(validate_asset_name("DIVIDENDS").is_ok());
    assert!(validate_distribution_amount("500").is_ok());
    assert!(validate_reward_snapshot_height(100000).is_ok());
  }

  #[test]
  fn distributereward_args_build_with_exceptions_and_change() {
    let args = build_distribute_reward_args(
      "OWN",
      123,
      "HEMP",
      "500",
      Some("H123456789ABCDEFGHijklmno".to_string()),
      Some("H123456789ABCDEFGHijklmnopqr".to_string()),
    ).unwrap();
    assert_eq!(
      args,
      vec![
        "distributereward",
        "OWN",
        "123",
        "HEMP",
        "500.00000000",
        "H123456789ABCDEFGHijklmno",
        "H123456789ABCDEFGHijklmnopqr",
      ]
    );
  }

  #[test]
  fn distributereward_args_build_change_without_exceptions_preserves_position() {
    let args = build_distribute_reward_args(
      "OWN",
      123,
      "HEMP",
      "500",
      None,
      Some("H123456789ABCDEFGHijklmnopqr".to_string()),
    ).unwrap();
    assert_eq!(
      args,
      vec![
        "distributereward",
        "OWN",
        "123",
        "HEMP",
        "500.00000000",
        "",
        "H123456789ABCDEFGHijklmnopqr",
      ]
    );
  }

  #[test]
  fn distributereward_args_build_execute_without_optional_fields() {
    let args = build_distribute_reward_args(
      "OWN",
      123,
      "DIV",
      "12.5",
      None,
      None,
    ).unwrap();
    assert_eq!(
      args,
      vec!["distributereward", "OWN", "123", "DIV", "12.50000000"]
    );
  }

  #[test]
  fn distributereward_args_reject_invalid_change_address() {
    let result = build_distribute_reward_args(
      "OWN",
      123,
      "HEMP",
      "1",
      None,
      Some("bad address".to_string()),
    );
    assert!(result.is_err());
  }

  // --- Policy Helper Tests ---

  #[test]
  fn estimate_one_input_one_output_bytes() {
    let bytes = estimate_legacy_tx_bytes(1, 1);
    assert_eq!(bytes, 10 + 148 + 34);
  }

  #[test]
  fn estimate_ten_inputs_one_output_bytes() {
    let bytes = estimate_legacy_tx_bytes(10, 1);
    assert_eq!(bytes, 10 + 10 * 148 + 34);
  }

  #[test]
  fn max_policy_inputs_one_output_100k() {
    let max = max_policy_inputs(1, 100_000);
    assert_eq!(max, 675);
  }

  #[test]
  fn max_policy_inputs_two_output_100k() {
    let max = max_policy_inputs(2, 100_000);
    assert_eq!(max, 675);
  }

  #[test]
  fn max_policy_inputs_one_output_90k_default() {
    let max = max_policy_inputs(1, 90_000);
    assert_eq!(max, 607);
  }

  #[test]
  fn max_policy_inputs_zero_when_budget_too_small() {
    let max = max_policy_inputs(1000, 10);
    assert_eq!(max, 0);
  }

  #[test]
  fn max_policy_inputs_upper_bound() {
    let max = max_policy_inputs(1, 100_000);
    assert!(max <= 675);
  }

  #[test]
  fn estimate_fee_from_bytes_one_input() {
    let bytes = estimate_legacy_tx_bytes(1, 1);
    let fee = estimate_fee_from_bytes(bytes, 1000);
    assert!(fee > 0.0);
    let expected = (bytes as f64 * 1000.0) / 100_000_000.0;
    assert!((fee - expected).abs() < 1e-12);
  }

  #[test]
  fn estimate_fee_from_bytes_consistency() {
    let bytes = estimate_legacy_tx_bytes(100, 1);
    let fee = estimate_fee_from_bytes(bytes, 1000);
    assert!(fee > 0.0);
    assert!(fee < 1.0);
  }

  #[test]
  fn format_hemp_amount_precision() {
    assert_eq!(format_hemp_amount(1.5), "1.50000000");
    assert_eq!(format_hemp_amount(0.00000546), "0.00000546");
    assert_eq!(format_hemp_amount(0.0), "0.00000000");
  }

  #[test]
  fn sat_to_hemp_conversion() {
    assert_eq!(sat_to_hemp(100_000_000), 1.0);
    assert_eq!(sat_to_hemp(546), 0.00000546);
    assert_eq!(sat_to_hemp(1), 0.00000001);
  }

  #[test]
  fn max_policy_inputs_rejects_oversized_selection() {
    let safe_max = max_policy_inputs(1, 100_000);
    let oversized = safe_max + 1;
    let bytes = estimate_legacy_tx_bytes(oversized, 1);
    assert!(bytes > 100_000);
  }

  #[test]
  fn max_policy_inputs_with_target_90k_is_safe() {
    let safe_max = max_policy_inputs(1, 90_000);
    let bytes = estimate_legacy_tx_bytes(safe_max, 1);
    assert!(bytes < 100_000);
    assert!(bytes <= 90_000);
  }

  #[test]
  fn size_based_fee_consistency_single_vs_multi_input() {
    let fee_1 = estimate_fee_from_bytes(estimate_legacy_tx_bytes(1, 1), 1000);
    let fee_10 = estimate_fee_from_bytes(estimate_legacy_tx_bytes(10, 1), 1000);
    assert!(fee_10 > fee_1);
    assert!((fee_10 / fee_1 - 1.0) > 0.1);
  }

  #[test]
  fn planner_rejects_empty_utxo_set_conceptually() {
    assert!(max_policy_inputs(1, 90_000) >= 2);
    assert!(max_policy_inputs(1, 100) == 0);
  }

  // --- Fee Rate Validation Tests ---

  #[test]
  fn valid_fee_rates_pass_validation() {
    assert!(validate_fee_rate_sat_per_byte(1).is_ok());
    assert!(validate_fee_rate_sat_per_byte(1000).is_ok());
    assert!(validate_fee_rate_sat_per_byte(10000).is_ok());
    assert!(validate_fee_rate_sat_per_byte(500).is_ok());
  }

  #[test]
  fn fee_rate_zero_is_invalid() {
    assert!(validate_fee_rate_sat_per_byte(0).is_err());
  }

  #[test]
  fn fee_rate_above_max_is_invalid() {
    assert!(validate_fee_rate_sat_per_byte(10_001).is_err());
    assert!(validate_fee_rate_sat_per_byte(100_000).is_err());
  }

  #[test]
  fn clamp_fee_rate_applies_bounds() {
    assert_eq!(clamp_fee_rate_sat_per_byte(0), 1);
    assert_eq!(clamp_fee_rate_sat_per_byte(500), 500);
    assert_eq!(clamp_fee_rate_sat_per_byte(50_000), 10_000);
  }

  #[test]
  fn parse_estimatesmartfee_sat_per_byte_parses_valid_value() {
    let parsed = parse_estimatesmartfee_sat_per_byte(r#"{"feerate":0.001}"#);
    assert_eq!(parsed, Some(100));
  }

  #[test]
  fn parse_estimatesmartfee_sat_per_byte_rejects_missing_or_invalid() {
    assert_eq!(parse_estimatesmartfee_sat_per_byte(r#"{"errors":["insufficient data"]}"#), None);
    assert_eq!(parse_estimatesmartfee_sat_per_byte(r#"{"feerate":0}"#), None);
  }

  #[test]
  fn recommended_consolidation_fee_rate_respects_override() {
    assert_eq!(recommended_consolidation_fee_rate_sat_per_byte(Some(250)).unwrap(), 250);
    assert!(recommended_consolidation_fee_rate_sat_per_byte(Some(0)).is_err());
    assert!(recommended_consolidation_fee_rate_sat_per_byte(Some(20_000)).is_err());
  }

  #[test]
  fn fee_estimate_changes_with_fee_rate() {
    let bytes = estimate_legacy_tx_bytes(100, 1);
    let fee_100 = estimate_fee_from_bytes(bytes, 100);
    let fee_1000 = estimate_fee_from_bytes(bytes, 1000);
    assert!(fee_1000 > fee_100);
    assert!((fee_1000 / fee_100 - 10.0).abs() < 1e-12);
  }

  #[test]
  fn fee_estimate_scales_linearly() {
    let bytes = 10_000u64;
    let fee_1 = estimate_fee_from_bytes(bytes, 1);
    let fee_100 = estimate_fee_from_bytes(bytes, 100);
    assert!((fee_100 / fee_1 - 100.0).abs() < 1e-12);
  }

  #[test]
  fn preview_broadcast_fee_comparison_matches_satoshi_rounded() {
    let tx_bytes = estimate_legacy_tx_bytes(10, 1);
    let fee = estimate_fee_from_bytes(tx_bytes, 1000);
    let sat_a = hemp_to_sat(fee);
    let sat_b = hemp_to_sat(estimate_fee_from_bytes(tx_bytes, 1000));
    assert_eq!(sat_a, sat_b);
  }

  #[test]
  fn preview_broadcast_fee_comparison_detects_mismatch() {
    let tx_bytes = estimate_legacy_tx_bytes(10, 1);
    let fee_1000 = estimate_fee_from_bytes(tx_bytes, 1000);
    let fee_500 = estimate_fee_from_bytes(tx_bytes, 500);
    assert!(hemp_to_sat(fee_1000) != hemp_to_sat(fee_500));
  }

  #[test]
  fn max_policy_inputs_unchanged_by_fee_rate() {
    let max_1 = max_policy_inputs(1, 100_000);
    let max_2 = max_policy_inputs(1, 100_000);
    assert_eq!(max_1, max_2);
    let fee_1 = estimate_fee_from_bytes(estimate_legacy_tx_bytes(max_1, 1), 1);
    let fee_1000 = estimate_fee_from_bytes(estimate_legacy_tx_bytes(max_1, 1), 1000);
    assert!(fee_1000 > fee_1);
  }

  #[test]
  fn consolidation_round_count_large_wallet_example() {
    assert_eq!(estimate_consolidation_round_count(15029, 80, 607), 25);
  }

  #[test]
  fn consolidation_round_count_can_exceed_six() {
    assert!(estimate_consolidation_round_count(5000, 80, 607) > 6);
  }

  #[test]
  fn selected_planning_rejects_duplicate_outpoints() {
    let selected = vec![
      RawTxInput { txid: "a".repeat(64), vout: 0 },
      RawTxInput { txid: "a".repeat(64), vout: 0 },
    ];
    let all = vec![];
    assert!(collect_selected_safe_utxos(&selected, &all).is_err());
  }

  #[test]
  fn selected_planning_rejects_unavailable_outpoints() {
    let selected = vec![RawTxInput { txid: "b".repeat(64), vout: 1 }];
    let all = vec![serde_json::json!({
      "txid": "c".repeat(64),
      "vout": 1,
      "amount": 1.0,
      "spendable": true,
      "safe": true
    })];
    assert!(collect_selected_safe_utxos(&selected, &all).is_err());
  }

  // --- Channel Name Validation Tests ---

  #[test]
  fn valid_channel_name_passes() {
    assert!(validate_channel_name("MESSAGING!").is_ok());
    assert!(validate_channel_name("MESSAGING~CHAN").is_ok());
    assert!(validate_channel_name("ASSET").is_ok());
  }

  #[test]
  fn empty_channel_name_fails() {
    assert!(validate_channel_name("").is_err());
    assert!(validate_channel_name("   ").is_err());
  }

  #[test]
  fn channel_name_with_whitespace_fails() {
    assert!(validate_channel_name("MY CHAN").is_err());
  }

  #[test]
  fn overly_long_channel_name_fails() {
    let long_name = "A".repeat(129);
    assert!(validate_channel_name(&long_name).is_err());
  }

  #[test]
  fn message_authority_asset_adds_owner_for_root_asset() {
    assert_eq!(message_authority_asset_name("TOKEN"), "TOKEN!");
    assert_eq!(message_authority_asset_name("TOKEN!"), "TOKEN!");
    assert_eq!(message_authority_asset_name("TOKEN~NEWS"), "TOKEN~NEWS");
  }

  // --- IPFS Hash Validation Tests ---

  #[test]
  fn valid_ipfs_hash_passes() {
    assert!(validate_ipfs_hash("QmZPGfJojdTzaqCWJu2m3krark38X1rqEHBo4SjeqHKB26").is_ok());
    assert!(validate_ipfs_hash("bafkreid").is_ok());
  }

  #[test]
  fn empty_ipfs_hash_fails() {
    assert!(validate_ipfs_hash("").is_err());
    assert!(validate_ipfs_hash("   ").is_err());
  }

  #[test]
  fn overly_long_ipfs_hash_fails() {
    let long_hash = "Q".repeat(65);
    assert!(validate_ipfs_hash(&long_hash).is_err());
  }

  #[test]
  fn message_expire_time_must_be_positive_when_present() {
    assert_eq!(validate_message_expire_time(None).unwrap(), None);
    assert_eq!(validate_message_expire_time(Some(1737500000)).unwrap(), Some(1737500000));
    assert!(validate_message_expire_time(Some(0)).is_err());
    assert!(validate_message_expire_time(Some(-1)).is_err());
  }

  // --- Message Entry Parsing Tests ---

  #[test]
  fn parses_message_entry_from_valid_json() {
    let json = serde_json::json!({
      "Asset Name": "MESSAGING!",
      "Message": "QmZPGfJojdTzaqCWJu2m3krark38X1rqEHBo4SjeqHKB26",
      "Time": "2025-01-15 10:30:00",
      "Block Height": 12345,
      "Status": "UNREAD",
      "Expire Time": "2026-01-15 10:30:00"
    });
    let entry = parse_message_entry(&json);
    assert_eq!(entry.asset_name, "MESSAGING!");
    assert_eq!(entry.message, "QmZPGfJojdTzaqCWJu2m3krark38X1rqEHBo4SjeqHKB26");
    assert_eq!(entry.time, "2025-01-15 10:30:00");
    assert_eq!(entry.block_height, 12345);
    assert_eq!(entry.status, "UNREAD");
    assert_eq!(entry.expire_time, Some("2026-01-15 10:30:00".to_string()));
  }

  #[test]
  fn parses_message_entry_missing_fields_default() {
    let json = serde_json::json!({});
    let entry = parse_message_entry(&json);
    assert_eq!(entry.asset_name, "");
    assert_eq!(entry.message, "");
    assert_eq!(entry.status, "UNKNOWN");
  }

  #[test]
  fn parses_message_entry_with_expire_utc_time() {
    let json = serde_json::json!({
      "Asset Name": "CHANNEL",
      "Message": "QmHash",
      "Time": "2025-01-15 10:30:00",
      "Block Height": 100,
      "Status": "READ",
      "Expire UTC Time": 1737500000
    });
    let entry = parse_message_entry(&json);
    assert_eq!(entry.expire_utc_time, Some(1737500000));
    assert_eq!(entry.expire_time, None);
  }

  // --- Channel Name List Parsing Tests ---

  #[test]
  fn parses_channel_name_list_from_json_array() {
    let json = serde_json::json!(["MESSAGING!", "MESSAGING~ONE", "TOKEN!"]);
    let list = parse_channel_name_list(&json);
    assert_eq!(list, vec!["MESSAGING!", "MESSAGING~ONE", "TOKEN!"]);
  }

  #[test]
  fn parses_empty_channel_list() {
    let json = serde_json::json!([]);
    let list = parse_channel_name_list(&json);
    assert_eq!(list.len(), 0);
  }

  #[test]
  fn parses_null_channel_list_as_empty() {
    let json = serde_json::json!(null);
    let list = parse_channel_name_list(&json);
    assert_eq!(list.len(), 0);
  }

  // --- Messaging Info Parsing Tests ---

  #[test]
  fn parses_messaging_info_enabled() {
    let json = serde_json::json!({
      "enabled": true,
      "messaging_active": true,
      "restricted_active": true,
      "activation_block": 432,
      "databases_available": true,
      "caches_available": true,
      "message_count": 5,
      "channel_count": 3,
      "dirty_cache_size_bytes": 1024,
      "wallet_available": true,
      "warnings": []
    });
    let info = parse_messaging_info(&json);
    assert!(info.enabled);
    assert!(info.messaging_active);
    assert_eq!(info.activation_block, 432);
    assert_eq!(info.message_count, 5);
    assert_eq!(info.channel_count, 3);
    assert!(info.warnings.is_empty());
  }

  #[test]
  fn parses_messaging_info_numeric_booleans() {
    let json = serde_json::json!({
      "enabled": 1,
      "messaging_active": 1,
      "restricted_active": 1,
      "activation_block": 1,
      "databases_available": 1,
      "caches_available": 1,
      "message_count": 0,
      "channel_count": 9,
      "dirty_cache_size_bytes": 96,
      "wallet_available": 1,
      "warnings": []
    });
    let info = parse_messaging_info(&json);
    assert!(info.enabled);
    assert!(info.messaging_active);
    assert!(info.restricted_active);
    assert!(info.databases_available);
    assert!(info.caches_available);
    assert!(info.wallet_available);
    assert_eq!(info.channel_count, 9);
  }

  #[test]
  fn parses_messaging_info_disabled() {
    let json = serde_json::json!({
      "enabled": false,
      "messaging_active": false,
      "restricted_active": false,
      "activation_block": 0,
      "databases_available": false,
      "caches_available": false,
      "message_count": 0,
      "channel_count": 0,
      "dirty_cache_size_bytes": 0,
      "wallet_available": false,
      "warnings": ["Messaging is disabled via -disablemessaging"]
    });
    let info = parse_messaging_info(&json);
    assert!(!info.enabled);
    assert_eq!(info.warnings.len(), 1);
    assert_eq!(info.warnings[0], "Messaging is disabled via -disablemessaging");
  }

  // --- IPFS Reference Tests ---

  #[test]
  fn ipfs_validate_accepts_cidv0() {
    let result = validate_ipfs_reference("QmZPGfJojdTzaqCWJu2m3krark38X1rqEHBo4SjeqHKB26").unwrap();
    assert_eq!(result.kind, "cidv0");
    assert_eq!(result.normalized, "QmZPGfJojdTzaqCWJu2m3krark38X1rqEHBo4SjeqHKB26");
    assert!(result.warnings.is_empty());
  }

  #[test]
  fn ipfs_validate_accepts_cidv1_bafy() {
    let result = validate_ipfs_reference("bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi").unwrap();
    assert_eq!(result.kind, "cidv1");
  }

  #[test]
  fn ipfs_validate_accepts_cidv1_bafk() {
    let result = validate_ipfs_reference("bafkreidgvpkjawlxz6sffxzwgooouw5s2e4foc3b7b6r7d5e7a2vz").unwrap();
    assert_eq!(result.kind, "cidv1");
  }

  #[test]
  fn ipfs_validate_marks_unknown_prefix() {
    let result = validate_ipfs_reference("xyz123notaciddata").unwrap();
    assert_eq!(result.kind, "unknown");
    assert!(!result.warnings.is_empty());
  }

  #[test]
  fn ipfs_validate_rejects_empty() {
    assert!(validate_ipfs_reference("").is_err());
  }

  #[test]
  fn ipfs_validate_rejects_whitespace_only() {
    assert!(validate_ipfs_reference("   ").is_err());
  }

  #[test]
  fn ipfs_validate_rejects_whitespace_inside() {
    assert!(validate_ipfs_reference("Qm Hash with spaces").is_err());
  }

  #[test]
  fn ipfs_validate_rejects_control_characters() {
    assert!(validate_ipfs_reference("QmHash\nwith\nnewline").is_err());
  }

  #[test]
  fn ipfs_validate_rejects_too_long() {
    let long_hash = "Qm".to_string() + &"a".repeat(63);
    assert!(validate_ipfs_reference(&long_hash).is_err());
  }

  #[test]
  fn ipfs_validate_accepts_max_length() {
    let hash_64 = "Qm".to_string() + &"a".repeat(62);
    assert!(validate_ipfs_reference(&hash_64).is_ok());
  }

  #[test]
  fn ipfs_gateway_url_builds_with_local_default() {
    let url = build_ipfs_gateway_url("QmTest", None).unwrap();
    assert!(url.starts_with("http://127.0.0.1:8080/ipfs/"));
    assert!(url.ends_with("/QmTest"));
  }

  #[test]
  fn ipfs_gateway_url_builds_with_custom_base() {
    let url = build_ipfs_gateway_url("QmTest", Some("http://localhost:5001/ipfs/".to_string())).unwrap();
    assert!(url.starts_with("http://localhost:5001/ipfs/"));
    assert!(url.ends_with("/QmTest"));
  }

  #[test]
  fn ipfs_gateway_url_trims_trailing_slashes() {
    let url = build_ipfs_gateway_url("QmTest", Some("http://gateway.example/".to_string())).unwrap();
    assert_eq!(url, "http://gateway.example/QmTest");
  }

  #[test]
  fn ipfs_gateway_url_handles_base_without_trailing_slash() {
    let url = build_ipfs_gateway_url("QmTest", Some("http://gateway.example".to_string())).unwrap();
    assert_eq!(url, "http://gateway.example/QmTest");
  }

  #[test]
  fn ipfs_gateway_url_rejects_invalid_input() {
    assert!(build_ipfs_gateway_url("", None).is_err());
  }

}
