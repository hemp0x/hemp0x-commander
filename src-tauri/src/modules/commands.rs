use std::path::PathBuf;
use std::process::Command;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::fs;
use chrono::{Local, TimeZone, DateTime};
// use tauri::Emitter; // Unused

// Import local modules
use crate::modules::models::*;
use crate::modules::utils::{resolve_bin, split_args, parse_balances, version_is_old};
use crate::modules::files::{data_dir, ensure_config, parse_config, config_path};

// --- SHELL STATE ---
#[derive(Default)]
pub struct ShellState {
  pub cwd: PathBuf,
}

static SHELL_STATE: OnceLock<Mutex<ShellState>> = OnceLock::new();

fn default_shell_cwd() -> PathBuf {
  let candidate = PathBuf::from(resolve_bin("hemp0xd"));
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
  let cli = resolve_bin("hemp0x-cli");
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
  run_cli(&[String::from("transfer"), asset, amount, to])
}

#[tauri::command]
pub fn issue_asset(name: String, qty: String, units: u8, reissuable: bool, ipfs: String) -> Result<String, String> {
  ensure_config()?;
  validate_asset_name(&name)?;
  let qty_val = parse_positive_amount(&qty)?;
  if units > 8 {
    return Err("Units must be between 0 and 8".to_string());
  }
  let flag = if reissuable { "true" } else { "false" };
  
  if !ipfs.is_empty() {
    run_cli(&[
      String::from("issue"),
      name,
      format!("{qty_val}"),
      String::new(), 
      String::new(), 
      units.to_string(),
      flag.to_string(),
      String::from("true"), 
      ipfs, 
    ])
  } else {
    run_cli(&[
      String::from("issue"),
      name,
      format!("{qty_val}"),
      String::new(),
      String::new(),
      units.to_string(),
      flag.to_string(),
    ])
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

  run_cli(&args)
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

#[tauri::command]
pub fn issue_unique_asset(
  root_name: String,
  tags: Vec<String>,
  ipfs_hashes: Vec<String>,
) -> Result<String, String> {
  ensure_config()?;
  
  if tags.is_empty() {
    return Err("At least one tag is required".to_string());
  }
  
  let tags_json = serde_json::to_string(&tags).map_err(|e| e.to_string())?;
  
  let ipfs_json = if !ipfs_hashes.is_empty() && ipfs_hashes.iter().any(|h| !h.is_empty()) {
    serde_json::to_string(&ipfs_hashes).map_err(|e| e.to_string())?
  } else {
    String::from("[]")
  };
  
  run_cli(&[
    String::from("issueunique"),
    root_name,
    tags_json,
    ipfs_json,
  ])
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
  run_cli(&[String::from("dumpprivkey"), address])
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
    return Err("Failed to sign transaction completely.".to_string());
  }
  let signed_hex = signed_res["hex"].as_str().ok_or("No signed hex returned")?.to_string();

  let txid = run_cli(&[
    String::from("sendrawtransaction"),
    signed_hex,
  ])?;

  Ok(txid)
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
  run_cli(&args)
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
  run_cli(&args)
}

#[cfg(test)]
mod tests {
  use super::{
    asset_balance_from_listmyassets, build_reissue_args, validate_asset_transfer_preview_fields,
    validate_send_preview_fields,
  };

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
}
