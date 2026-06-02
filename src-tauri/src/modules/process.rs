use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;
use std::fs;

// Import local modules
use crate::modules::files::{data_dir, ensure_config, config_path, load_app_settings, save_app_settings};
use crate::modules::utils::{resolve_bin, resolve_bin_with_override};
use crate::modules::commands::run_cli;

#[tauri::command]
pub fn start_node() -> Result<(), String> {
  let cfg = ensure_config()?;
  let dir = data_dir()?;

  let settings: crate::modules::models::AppSettings = load_app_settings()?;
  let custom_bin_dir = settings.custom_core_binary_dir.clone();

  let daemon = if let Some(ref d) = custom_bin_dir {
    resolve_bin_with_override("hemp0xd", Some(d))
  } else {
    resolve_bin("hemp0xd")
  };

  let settings: crate::modules::models::AppSettings = load_app_settings()?;
  let repair_flag: Option<String> = match settings.pending_repair_mode.as_deref() {
    Some("reindex") | Some("reindex-chainstate") => settings.pending_repair_mode.clone(),
    _ => None,
  };

  let daemon_path = PathBuf::from(&daemon);
  if !daemon_path.exists() {
    let hint = if custom_bin_dir.is_some() {
      "Daemon not found in custom Core binary folder. Check the folder path or reset to bundled."
    } else {
      &format!("Daemon not found at {}", daemon)
    };
    return Err(hint.to_string());
  }
  #[cfg(unix)]
  let mut cmd = Command::new("sh");
  
  #[cfg(windows)]
  let mut cmd = Command::new(&daemon);

  #[cfg(windows)]
  {
    use std::os::windows::process::CommandExt;
    cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    cmd.arg(format!("-conf={}", cfg.to_string_lossy()))
       .arg(format!("-datadir={}", dir.to_string_lossy()));
    if let Some(ref flag) = repair_flag {
      cmd.arg(format!("-{}", flag));
    }
  }

  #[cfg(unix)]
  {
     let mut full_cmd = format!(
         "\"{}\" -conf=\"{}\" -datadir=\"{}\" -daemon",
         daemon,
         cfg.to_string_lossy(),
         dir.to_string_lossy()
     );
     if let Some(ref flag) = repair_flag {
       full_cmd.push_str(&format!(" -{}", flag));
     }
     
     cmd.arg("-c").arg(full_cmd);
  }

  cmd
    .spawn()
    .map_err(|e| {
        e.to_string()
    })?;

  if repair_flag.is_some() {
    let mut updated = settings.clone();
    updated.pending_repair_mode = None;
    save_app_settings(updated)?;
  }

  Ok(())
}

#[tauri::command]
pub fn stop_node() -> Result<(), String> {
  let _ = run_cli(&[String::from("stop")])?;
  Ok(())
}

#[tauri::command]
pub fn set_network_mode(mode: String) -> Result<String, String> {
  // Attempt to stop the running node BEFORE changing config
  let _ = stop_node(); 
  
  // Give it a moment to shutdown gracefully
  thread::sleep(Duration::from_secs(2));

  let cfg_path = config_path()?;
  ensure_config()?; // Ensure it exists

  let content = fs::read_to_string(&cfg_path).map_err(|e| e.to_string())?;
  let mut new_lines: Vec<String> = Vec::new();

  // Filter out existing network flags
  for line in content.lines() {
    if !line.trim().starts_with("testnet=") && !line.trim().starts_with("regtest=") {
      new_lines.push(line.to_string());
    }
  }

  // Add new mode
  match mode.as_str() {
    "testnet" => new_lines.push("testnet=1".to_string()),
    "regtest" => new_lines.push("regtest=1".to_string()),
    "mainnet" => {}, // distinct absence of flags
    _ => return Err("Invalid network mode".to_string()),
  }

  // Write back
  fs::write(&cfg_path, new_lines.join("\n")).map_err(|e| e.to_string())?;
  Ok("Network mode updated. Please restart the node.".to_string())
}

#[tauri::command]
pub fn restart_app(app_handle: tauri::AppHandle) {
  app_handle.restart();
}

// Helper for restore_wallet and create_new_wallet
pub fn wait_for_lock_release(dir: &Path) {
  let lock_path = dir.join(".lock");
  for _ in 0..20 {
    if !lock_path.exists() {
      break;
    }
    thread::sleep(Duration::from_millis(500));
  }
}

pub fn stop_node_internal() {
    let _ = run_cli(&[String::from("stop")]);
    thread::sleep(Duration::from_secs(2));
}

#[tauri::command]
pub fn restore_wallet(path: String, backup_existing: bool, restart_node: bool) -> Result<(), String> {
  let dir = data_dir()?;
    let wallet = dir.join("wallet.dat");
    if !Path::new(&path).exists() {
      return Err("Restore file not found.".to_string());
    }
    
    // Stop node logic internal
    stop_node_internal();
    
    if wallet.exists() && backup_existing {
      let ts = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
      let backup_dir = dir.join("wallet_backups");
      let _ = fs::create_dir_all(&backup_dir);
      let backup = backup_dir.join(format!("wallet_{}.bak", ts));
      fs::rename(&wallet, backup).map_err(|e| e.to_string())?;
    } else if wallet.exists() {
      fs::remove_file(&wallet).map_err(|e| e.to_string())?;
    }
    fs::copy(path, wallet).map_err(|e| e.to_string())?;
  if restart_node {
    wait_for_lock_release(&dir);
    let _ = start_node();
  }
  Ok(())
}

#[tauri::command]
pub fn create_new_wallet(backup_existing: bool, restart_node: bool) -> Result<(), String> {
  let dir = data_dir()?;
    let wallet = dir.join("wallet.dat");
    
    stop_node_internal();
    
    if wallet.exists() && backup_existing {
      let ts = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
      let backup_dir = dir.join("wallet_backups");
      let _ = fs::create_dir_all(&backup_dir);
      let backup = backup_dir.join(format!("wallet_{}.bak", ts));
      fs::rename(&wallet, backup).map_err(|e| e.to_string())?;
    } else if wallet.exists() {
      fs::remove_file(&wallet).map_err(|e| e.to_string())?;
    }
  if restart_node {
    wait_for_lock_release(&dir);
    let _ = start_node();
  }
  Ok(())
}

// Commands from commands.rs that also need start/stop access, i.e., backup_wallet is fine in commands.rs
// restore/create_wallet involve restarting logic so they moved here.
