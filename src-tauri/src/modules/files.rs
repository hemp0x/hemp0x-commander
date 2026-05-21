use std::fs;
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::process::Command;
use uuid::Uuid;
use chrono::Local;
use std::io::{Read, Seek, SeekFrom};
use rand::Rng;
use rand::distributions::Alphanumeric;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

// Import local modules
use crate::modules::models::{ConfigPaths, DataFolderInfo, BinaryStatus, AddressBookEntry, AppSettings};
use crate::modules::utils::{resolve_bin, bin_name, calculate_dir_size, format_size};

pub fn data_dir() -> Result<PathBuf, String> {
  if cfg!(windows) {
    let appdata = std::env::var("APPDATA").map_err(|_| "APPDATA not set".to_string())?;
    Ok(PathBuf::from(appdata).join("Hemp0x"))
  } else {
    let home = dirs::home_dir().ok_or("HOME not set")?;
    Ok(home.join(".hemp0x"))
  }
}

pub fn config_path() -> Result<PathBuf, String> {
  Ok(data_dir()?.join("hemp.conf"))
}

pub fn ensure_config() -> Result<PathBuf, String> {
  let dir = data_dir()?;
  let cfg = config_path()?;
  if !dir.exists() {
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
  }
  if !cfg.exists() {
    let rpc_user = format!("u{}", rand::thread_rng().gen_range(10000..99999));
    let rpc_pass = Uuid::new_v4();
    let daemon_flag = "0";
    let content = format!(
      "rpcuser={}\nrpcpassword={}\nserver=1\ndaemon={}\naddnode=154.38.164.123:42069\naddnode=147.93.185.184:42069\n",
      rpc_user, rpc_pass, daemon_flag
    );
    fs::write(&cfg, content).map_err(|e| e.to_string())?;
  }
  Ok(cfg)
}

pub fn parse_config(path: &Path) -> Result<HashMap<String, String>, String> {
  let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
  let mut map = HashMap::new();
  for line in content.lines() {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
      continue;
    }
    if let Some((k, v)) = line.split_once('=') {
      map.insert(k.trim().to_string(), v.trim().to_string());
    }
  }
  Ok(map)
}

fn address_book_path() -> Result<PathBuf, String> {
  Ok(data_dir()?.join("address_book.json"))
}

#[tauri::command]
pub fn load_address_book() -> Result<Vec<AddressBookEntry>, String> {
  let path = address_book_path()?;
  if !path.exists() {
    return Ok(Vec::new());
  }
  let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
  let entries: Vec<AddressBookEntry> = serde_json::from_str(&content).unwrap_or_default();
  Ok(entries)
}

#[tauri::command]
pub fn save_address_book(entries: Vec<AddressBookEntry>) -> Result<(), String> {
  let path = address_book_path()?;
  let content = serde_json::to_string_pretty(&entries).map_err(|e| e.to_string())?;
  fs::write(&path, content).map_err(|e| e.to_string())?;
  Ok(())
}

fn app_settings_path() -> Result<PathBuf, String> {
  Ok(data_dir()?.join("app_settings.json"))
}

#[tauri::command]
pub fn load_app_settings() -> Result<AppSettings, String> {
  let path = app_settings_path()?;
  if !path.exists() {
    return Ok(AppSettings::default());
  }
  let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
  let settings: AppSettings = serde_json::from_str(&content).unwrap_or_default();
  Ok(settings)
}

#[tauri::command]
pub fn save_app_settings(settings: AppSettings) -> Result<(), String> {
  let path = app_settings_path()?;
  let content = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
  fs::write(&path, content).map_err(|e| e.to_string())?;
  Ok(())
}

#[tauri::command]
pub fn read_text_file(path: String) -> Result<String, String> {
  fs::read_to_string(path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn write_text_file(path: String, content: String) -> Result<(), String> {
  fs::write(path, content).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn init_config() -> Result<ConfigPaths, String> {
  let cfg = ensure_config()?;
  let dir = data_dir()?;
  Ok(ConfigPaths {
    data_dir: dir.to_string_lossy().to_string(),
    config_path: cfg.to_string_lossy().to_string(),
    daemon_path: resolve_bin("hemp0xd"),
    cli_path: resolve_bin("hemp0x-cli"),
  })
}

#[tauri::command]
pub fn read_config() -> Result<String, String> {
  let cfg = ensure_config()?;
  fs::read_to_string(cfg).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn write_config(contents: String) -> Result<(), String> {
  let cfg = ensure_config()?;
  fs::write(cfg, contents).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn check_config_exists() -> Result<bool, String> {
  let cfg = config_path()?;
  Ok(cfg.exists())
}

#[tauri::command]
pub fn create_default_config() -> Result<(), String> {
  let cfg = config_path()?;
  let dir = data_dir()?;
  
  if !dir.exists() {
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
  }
  
  // Generate random RPC credentials for security
  let rpc_user: String = rand::thread_rng()
    .sample_iter(&Alphanumeric)
    .take(12)
    .map(char::from)
    .collect();
  let rpc_pass: String = rand::thread_rng()
    .sample_iter(&Alphanumeric)
    .take(24)
    .map(char::from)
    .collect();
  
  let default_config = format!(r#"# Hemp0x Configuration File
rpcuser={}
rpcpassword={}
server=1
daemon=0
listen=1
txindex=1
assetindex=1
port=42069
rpcport=42068
"#, rpc_user, rpc_pass);
  
  fs::write(&cfg, default_config).map_err(|e| e.to_string())?;
  Ok(())
}

#[tauri::command]
pub fn get_data_folder_info() -> Result<DataFolderInfo, String> {
  let dir = data_dir()?;
  let folder_exists = dir.exists();
  let config_exists = dir.join("hemp.conf").exists();
  let wallet_exists = dir.join("wallet.dat").exists();
  
  let size_bytes = if folder_exists { calculate_dir_size(&dir) } else { 0 };
  let size_display = format_size(size_bytes);
  
  Ok(DataFolderInfo {
    path: dir.to_string_lossy().to_string(),
    size_bytes,
    size_display,
    config_exists,
    wallet_exists,
    folder_exists,
  })
}

#[tauri::command]
pub fn open_data_dir() -> Result<(), String> {
  let dir = data_dir()?;
  
  if cfg!(windows) {
    Command::new("explorer")
      .arg(&dir)
      .spawn()
      .map_err(|e| e.to_string())?;
      Ok(())
  } else if cfg!(target_os = "macos") {
    Command::new("open")
      .arg(&dir)
      .spawn()
      .map_err(|e| e.to_string())?;
      Ok(())
  } else {
    // Try dolphin directly (KDE)
    if Command::new("dolphin")
      .arg("--new-window")
      .arg(&dir)
      .spawn()
      .is_ok() {
      return Ok(());
    }
    
    // Try nautilus (GNOME)
    if Command::new("nautilus")
      .arg(&dir)
      .spawn()
      .is_ok()
    {
      return Ok(());
    }

    Ok(())
  }
}

fn read_log_tail(path: &Path, max_lines: usize) -> Result<String, String> {
  if !path.exists() {
    return Ok(String::from("Log file not found."));
  }
  let mut file = fs::File::open(path).map_err(|e| e.to_string())?;
  let size = file.metadata().map_err(|e| e.to_string())?.len();
  let read_size = std::cmp::min(size, 2 * 1024 * 1024);
  file
    .seek(SeekFrom::End(-(read_size as i64)))
    .map_err(|e| e.to_string())?;
  let mut buf = String::new();
  file.read_to_string(&mut buf).map_err(|e| e.to_string())?;
  let lines: Vec<&str> = buf.lines().collect();
  let start = lines.len().saturating_sub(max_lines);
  Ok(lines[start..].join("\n"))
}

#[tauri::command]
pub fn read_log(lines: Option<u32>) -> Result<String, String> {
  let dir = data_dir()?;
  let log_path = dir.join("debug.log");
  read_log_tail(&log_path, lines.unwrap_or(200) as usize)
}

#[tauri::command]
pub fn truncate_log() -> Result<(), String> {
  let dir = data_dir()?;
  let log_path = dir.join("debug.log");
  if log_path.exists() {
    fs::write(&log_path, "").map_err(|e| e.to_string())?;
  }
  Ok(())
}

#[tauri::command]
pub fn backup_data_folder() -> Result<String, String> {
  let dir = data_dir()?;
  if !dir.exists() {
    return Err("Data folder does not exist".to_string());
  }
  
  let ts = Local::now().format("%Y%m%d_%H%M%S").to_string();
  let backup_name = format!("hemp0x_data_backup_{}", ts);
  
  let backup_base = dirs::desktop_dir()
    .or_else(dirs::home_dir)
    .ok_or("Could not determine backup location")?;
  let backup_path = backup_base.join(&backup_name);
  
  fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    for entry in fs::read_dir(src).map_err(|e| e.to_string())? {
      let entry = entry.map_err(|e| e.to_string())?;
      let src_path = entry.path();
      let dst_path = dst.join(entry.file_name());
      if src_path.is_dir() {
        copy_dir_recursive(&src_path, &dst_path)?;
      } else {
        fs::copy(&src_path, &dst_path).map_err(|e| e.to_string())?;
      }
    }
    Ok(())
  }
  
  copy_dir_recursive(&dir, &backup_path)?;
  Ok(backup_path.to_string_lossy().to_string())
}

#[tauri::command]
pub fn backup_data_folder_to(path: String) -> Result<(), String> {
  let dir = data_dir()?;
  if !dir.exists() {
    return Err("Data folder does not exist".to_string());
  }
  
  fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    for entry in fs::read_dir(src).map_err(|e| e.to_string())? {
      let entry = entry.map_err(|e| e.to_string())?;
      let src_path = entry.path();
      let dst_path = dst.join(entry.file_name());
      if src_path.is_dir() {
        copy_dir_recursive(&src_path, &dst_path)?;
      } else {
        fs::copy(&src_path, &dst_path).map_err(|e| e.to_string())?;
      }
    }
    Ok(())
  }
  
  copy_dir_recursive(&dir, Path::new(&path))?;
  Ok(())
}

#[tauri::command]
pub fn extract_binaries(target_dir: String) -> Result<String, String> {
  let target_path = PathBuf::from(target_dir);
  if !target_path.exists() {
    return Err("Target directory does not exist".to_string());
  }

  let bins = ["hemp0xd", "hemp0x-cli", "hemp0x-tx"];
  let mut extracted = Vec::new();

  for bin in bins {
    let src_str = resolve_bin(bin);
    let src = PathBuf::from(&src_str);
    if !src.exists() {
      // Try resolving without extension if on Windows and failed? No, resolve_bin handles it.
      return Err(format!("Source binary not found: {}", src_str));
    }

    let dest = target_path.join(bin_name(bin));
    fs::copy(&src, &dest).map_err(|e| format!("Failed to copy {}: {}", bin, e))?;

    // Set executable permissions on Linux/Mac
    #[cfg(unix)]
    {
       if let Ok(metadata) = fs::metadata(&dest) {
           let mut perms = metadata.permissions();
           perms.set_mode(0o755);
           let _ = fs::set_permissions(&dest, perms);
       }
    }
    extracted.push(bin);
  }

  Ok(format!("Successfully extracted: {}", extracted.join(", ")))
}

#[tauri::command]
pub fn get_binary_status() -> Result<BinaryStatus, String> {
  let daemon_path = PathBuf::from(resolve_bin("hemp0xd"));
  let cli_path = PathBuf::from(resolve_bin("hemp0x-cli"));
  let tx_path = PathBuf::from(resolve_bin("hemp0x-tx"));
  Ok(BinaryStatus {
    daemon_exists: daemon_path.exists(),
    cli_exists: cli_path.exists(),
    tx_exists: tx_path.exists(),
  })
}

#[tauri::command]
pub fn extract_snapshot(archive_path: String) -> Result<String, String> {
  let archive = Path::new(&archive_path);
  if !archive.exists() {
    return Err("Snapshot file not found".to_string());
  }
  
  let dest_dir = data_dir()?;
  
  // Create a temp extraction folder
  let temp_extract = dest_dir.join("_snapshot_temp");
  if temp_extract.exists() {
    fs::remove_dir_all(&temp_extract).map_err(|e| format!("Failed to clean temp: {}", e))?;
  }
  fs::create_dir_all(&temp_extract).map_err(|e| format!("Failed to create temp dir: {}", e))?;
  
  // Extract to temp folder
  sevenz_rust::decompress_file(&archive, &temp_extract)
    .map_err(|e| format!("7z extraction failed: {}", e))?;
  
  // Find blocks and chainstate folders - check root level first, then one level deep
  fn find_chain_folders(base: &Path) -> Option<(Option<PathBuf>, Option<PathBuf>)> {
    let mut blocks_path: Option<PathBuf> = None;
    let mut chainstate_path: Option<PathBuf> = None;
    
    // Check root level
    let root_blocks = base.join("blocks");
    let root_chainstate = base.join("chainstate");
    
    if root_blocks.exists() && root_blocks.is_dir() {
      blocks_path = Some(root_blocks);
    }
    if root_chainstate.exists() && root_chainstate.is_dir() {
      chainstate_path = Some(root_chainstate);
    }
    
    // If found at root, return
    if blocks_path.is_some() || chainstate_path.is_some() {
      return Some((blocks_path, chainstate_path));
    }
    
    // Check one level deep (e.g., snapshot_folder/blocks)
    if let Ok(entries) = fs::read_dir(base) {
      for entry in entries.flatten() {
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
          let sub_dir = entry.path();
          let nested_blocks = sub_dir.join("blocks");
          let nested_chainstate = sub_dir.join("chainstate");
          
          if nested_blocks.exists() && nested_blocks.is_dir() {
            blocks_path = Some(nested_blocks);
          }
          if nested_chainstate.exists() && nested_chainstate.is_dir() {
            chainstate_path = Some(nested_chainstate);
          }
          
          if blocks_path.is_some() || chainstate_path.is_some() {
            return Some((blocks_path, chainstate_path));
          }
        }
      }
    }
    
    None
  }
  
  let (blocks_path, chainstate_path) = find_chain_folders(&temp_extract)
    .ok_or_else(|| "Invalid snapshot: could not find 'blocks' or 'chainstate' folder".to_string())?;
  
  let has_blocks = blocks_path.is_some();
  let has_chainstate = chainstate_path.is_some();
  
  // Remove existing folders and move new ones
  if let Some(blocks_src) = blocks_path {
    let old_blocks = dest_dir.join("blocks");
    if old_blocks.exists() {
      fs::remove_dir_all(&old_blocks).map_err(|e| format!("Failed to remove old blocks: {}", e))?;
    }
    fs::rename(&blocks_src, &old_blocks)
      .map_err(|e| format!("Failed to install blocks: {}", e))?;
  }
  
  if let Some(chainstate_src) = chainstate_path {
    let old_chainstate = dest_dir.join("chainstate");
    if old_chainstate.exists() {
      fs::remove_dir_all(&old_chainstate).map_err(|e| format!("Failed to remove old chainstate: {}", e))?;
    }
    fs::rename(&chainstate_src, &old_chainstate)
      .map_err(|e| format!("Failed to install chainstate: {}", e))?;
  }
  
  // Clean up temp
  let _ = fs::remove_dir_all(&temp_extract);
  
  let msg = format!(
    "Snapshot installed! blocks: {}, chainstate: {}",
    if has_blocks { "OK" } else { "-" },
    if has_chainstate { "OK" } else { "-" }
  );
  Ok(msg)
}
