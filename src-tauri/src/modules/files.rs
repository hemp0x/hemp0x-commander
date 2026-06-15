use std::fs;
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::process::Command;
use chrono::Local;
use std::io::{Read, Seek, SeekFrom};
use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

// Import local modules
use crate::modules::models::{ConfigPaths, DataFolderInfo, DataMovePreview, DataMoveResult, BinaryStatus, AddressBookEntry, AppSettings, RepairStatus};
use crate::modules::utils::{resolve_bin, resolve_bin_with_override, bin_name, calculate_dir_size, format_size};

pub fn default_core_data_dir() -> Result<PathBuf, String> {
  if cfg!(windows) {
    let appdata = std::env::var("APPDATA").map_err(|_| "APPDATA not set".to_string())?;
    Ok(PathBuf::from(appdata).join("Hemp0x"))
  } else {
    let home = dirs::home_dir().ok_or("HOME not set")?;
    Ok(home.join(".hemp0x"))
  }
}

// Bootstrap dir is always under default_core_data_dir — never under active data dir
fn bootstrap_dir() -> Result<PathBuf, String> {
  Ok(default_core_data_dir()?.join("commander"))
}

fn bootstrap_path() -> Result<PathBuf, String> {
  Ok(bootstrap_dir()?.join("bootstrap.json"))
}

fn load_bootstrap() -> Result<serde_json::Value, String> {
  let path = bootstrap_path()?;
  if path.exists() {
    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let val: serde_json::Value = serde_json::from_str(&content).unwrap_or_else(|_| {
      let mut map = serde_json::Map::new();
      map.insert("custom_data_dir".to_string(), serde_json::Value::Null);
      serde_json::Value::Object(map)
    });
    return Ok(val);
  }
  let mut map = serde_json::Map::new();
  map.insert("custom_data_dir".to_string(), serde_json::Value::Null);
  Ok(serde_json::Value::Object(map))
}

fn save_bootstrap(custom_data_dir: Option<String>) -> Result<(), String> {
  let path = bootstrap_path()?;
  let dir = path.parent().ok_or("Could not determine bootstrap parent directory")?;
  if !dir.exists() {
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
  }
  let mut map = serde_json::Map::new();
  map.insert("custom_data_dir".to_string(), match custom_data_dir {
    Some(s) => serde_json::Value::String(s),
    None => serde_json::Value::Null,
  });
  let content = serde_json::to_string_pretty(&serde_json::Value::Object(map)).map_err(|e| e.to_string())?;
  fs::write(&path, content).map_err(|e| e.to_string())?;
  Ok(())
}

// Active data dir follows the bootstrap pointer
pub fn active_data_dir() -> Result<PathBuf, String> {
  let bootstrap = load_bootstrap()?;
  if let Some(serde_json::Value::String(ref custom)) = bootstrap.get("custom_data_dir") {
    let p = PathBuf::from(custom);
    if !p.is_absolute() {
      return Err("Custom data directory must be an absolute path. Use Settings to fix or reset.".to_string());
    }
    if p.exists() && p.is_file() {
      return Err("Custom data directory points to a file, not a directory. Use Settings to fix or reset.".to_string());
    }
    return Ok(p);
  }
  default_core_data_dir()
}

// Legacy alias: data_dir() is the active data directory
pub fn data_dir() -> Result<PathBuf, String> {
  active_data_dir()
}

// Active commander settings live under the active data dir
pub fn commander_settings_path() -> Result<PathBuf, String> {
  Ok(active_data_dir()?.join("commander").join("app_settings.json"))
}

#[cfg(test)]
thread_local! {
    pub static TEST_COMMANDER_DIR: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
}

pub fn commander_dir() -> Result<PathBuf, String> {
  #[cfg(test)]
  {
    let override_path = TEST_COMMANDER_DIR.with(|cell| cell.borrow().clone());
    if let Some(dir) = override_path {
      fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
      return Ok(dir);
    }
  }
  let dir = active_data_dir()?.join("commander");
  fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
  Ok(dir)
}

pub fn commander_content_library_dir() -> Result<PathBuf, String> {
  Ok(commander_dir()?.join("content-library"))
}

fn external_slice17_settings_path() -> Result<PathBuf, String> {
  if cfg!(windows) {
    let appdata = std::env::var("APPDATA").map_err(|_| "APPDATA not set".to_string())?;
    Ok(PathBuf::from(appdata).join("Hemp0xCommander").join("app_settings.json"))
  } else if cfg!(target_os = "macos") {
    let home = dirs::home_dir().ok_or("HOME not set")?;
    Ok(home.join("Library").join("Application Support").join("Hemp0xCommander").join("app_settings.json"))
  } else {
    let config = dirs::config_dir().ok_or("Could not determine config directory")?;
    Ok(config.join("hemp0x-commander").join("app_settings.json"))
  }
}

fn fallback_prior_17b_settings_path() -> Result<PathBuf, String> {
  // Slice 17b stored settings under default dir regardless of custom dir
  Ok(default_core_data_dir()?.join("commander").join("app_settings.json"))
}

pub fn load_app_settings_impl() -> Result<AppSettings, String> {
  let active_path = commander_settings_path()?;

  if active_path.exists() {
    let content = fs::read_to_string(&active_path).map_err(|e| e.to_string())?;
    return Ok(serde_json::from_str(&content).unwrap_or_default());
  }

  // Fallback 1: prior 17b path (default dir /commander/app_settings.json)
  if let Ok(prior17b) = fallback_prior_17b_settings_path() {
    if prior17b.exists() {
      let content = fs::read_to_string(&prior17b).map_err(|e| e.to_string())?;
      let settings: AppSettings = serde_json::from_str(&content).unwrap_or_default();
      save_app_settings_impl(&settings)?;
      return Ok(settings);
    }
  }

  // Fallback 2: external Slice 17 path
  if let Ok(external) = external_slice17_settings_path() {
    if external.exists() {
      let content = fs::read_to_string(&external).map_err(|e| e.to_string())?;
      let settings: AppSettings = serde_json::from_str(&content).unwrap_or_default();
      save_app_settings_impl(&settings)?;
      return Ok(settings);
    }
  }

  // Fallback 3: legacy root path
  let legacy_path = default_core_data_dir()?.join("app_settings.json");
  if legacy_path.exists() {
    let content = fs::read_to_string(&legacy_path).map_err(|e| e.to_string())?;
    let settings: AppSettings = serde_json::from_str(&content).unwrap_or_default();
    save_app_settings_impl(&settings)?;
    return Ok(settings);
  }

  Ok(AppSettings::default())
}

pub fn save_app_settings_impl(settings: &AppSettings) -> Result<(), String> {
  let path = commander_settings_path()?;
  let cfg_dir = path.parent().ok_or("Could not determine settings parent directory")?;
  if !cfg_dir.exists() {
    fs::create_dir_all(cfg_dir).map_err(|e| e.to_string())?;
  }
  let content = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
  fs::write(&path, content).map_err(|e| e.to_string())?;
  Ok(())
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
    let daemon_flag = "0";
    let content = format!(
      "# Hemp0x Configuration\n\
       # Core cookie auth is used by default; rpcuser/rpcpassword are not required.\n\
       server=1\n\
       daemon={}\n\
       addnode=154.38.164.123:42069\n\
       addnode=147.93.185.184:42069\n",
      daemon_flag
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
  Ok(commander_dir()?.join("address_book.json"))
}

fn legacy_address_book_path() -> Result<PathBuf, String> {
  Ok(data_dir()?.join("address_book.json"))
}

fn migrate_legacy_address_book() -> Result<PathBuf, String> {
  let path = address_book_path()?;
  if path.exists() {
    return Ok(path);
  }
  let legacy = legacy_address_book_path()?;
  if legacy.exists() {
    if let Some(parent) = path.parent() {
      fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::copy(&legacy, &path).map_err(|e| format!("Failed to migrate address book: {e}"))?;
  }
  Ok(path)
}

#[tauri::command]
pub fn load_address_book() -> Result<Vec<AddressBookEntry>, String> {
  let path = migrate_legacy_address_book()?;
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

#[tauri::command]
pub fn load_app_settings() -> Result<AppSettings, String> {
  load_app_settings_impl()
}

#[tauri::command]
pub fn save_app_settings(settings: AppSettings) -> Result<(), String> {
  save_app_settings_impl(&settings)
}

#[tauri::command]
pub fn read_text_file(path: String) -> Result<String, String> {
  let resolved = validate_read_path(&path)?;
  validate_path_in_allowed_roots(&resolved)?;
  fs::read_to_string(&resolved).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn write_text_file(path: String, content: String) -> Result<(), String> {
  let resolved = validate_write_path(&path)?;
  validate_path_in_allowed_roots(&resolved)?;
  fs::write(&resolved, content).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn dialog_read_text_file(
    app: AppHandle,
    title: Option<String>,
    filters: Vec<Vec<String>>,
) -> Result<String, String> {
    let mut builder = app.dialog().file();
    if let Some(ref t) = title {
        builder = builder.set_title(t);
    }
    for filter in &filters {
        if filter.len() >= 2 {
            let name = &filter[0];
            let exts: Vec<&str> = filter[1..].iter().map(|s| s.as_str()).collect();
            builder = builder.add_filter(name, &exts);
        }
    }
    let file_path = builder.blocking_pick_file()
        .ok_or("No file selected")?;
    let path = file_path.as_path().ok_or("Invalid file path")?;
    let resolved = validate_read_path(&path.to_string_lossy())?;
    fs::read_to_string(&resolved).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn dialog_write_text_file(
    app: AppHandle,
    content: String,
    default_path: Option<String>,
    title: Option<String>,
    filters: Vec<Vec<String>>,
) -> Result<String, String> {
    let mut builder = app.dialog().file();
    if let Some(ref t) = title {
        builder = builder.set_title(t);
    }
    if let Some(ref dp) = default_path {
        builder = builder.set_file_name(dp);
    }
    for filter in &filters {
        if filter.len() >= 2 {
            let name = &filter[0];
            let exts: Vec<&str> = filter[1..].iter().map(|s| s.as_str()).collect();
            builder = builder.add_filter(name, &exts);
        }
    }
    let file_path = builder.blocking_save_file()
        .ok_or("No file selected")?;
    let path = file_path.as_path().ok_or("Invalid file path")?;
    let resolved = validate_write_path(&path.to_string_lossy())?;
    fs::write(&resolved, content).map_err(|e| e.to_string())?;
    Ok(resolved.to_string_lossy().to_string())
}

fn validate_read_path(path: &str) -> Result<PathBuf, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("Path is empty".to_string());
    }
    let p = PathBuf::from(trimmed);
    if !p.is_absolute() {
        return Err("Path must be absolute".to_string());
    }
    if !p.exists() {
        return Err("File does not exist".to_string());
    }
    if p.is_dir() {
        return Err("Path is a directory, not a file".to_string());
    }
    let canonical = p.canonicalize().map_err(|e| format!("Cannot resolve path: {e}"))?;
    Ok(canonical)
}

fn validate_write_path(path: &str) -> Result<PathBuf, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("Path is empty".to_string());
    }
    let p = PathBuf::from(trimmed);
    if !p.is_absolute() {
        return Err("Path must be absolute".to_string());
    }
    if p.is_dir() {
        return Err("Path is a directory, not a file".to_string());
    }
    let parent = p.parent().ok_or("Path has no parent directory")?;
    if !parent.exists() {
        return Err("Parent directory does not exist".to_string());
    }
    let canonical_parent = parent.canonicalize().map_err(|e| format!("Cannot resolve parent directory: {e}"))?;
    let file_name = p.file_name().ok_or("Path has no filename")?;
    let resolved = canonical_parent.join(file_name);
    Ok(resolved)
}

fn validate_path_in_allowed_roots(path: &Path) -> Result<(), String> {
    let canonical = if path.exists() {
        path.canonicalize().map_err(|e| format!("Cannot resolve path: {e}"))?
    } else {
        let parent = path.parent().ok_or("Path has no parent directory")?;
        let canonical_parent = parent.canonicalize().map_err(|e| format!("Cannot resolve parent directory: {e}"))?;
        let file_name = path.file_name().ok_or("Path has no filename")?;
        canonical_parent.join(file_name)
    };

    let data_dir = data_dir()?;
    let data_canonical = data_dir.canonicalize().unwrap_or_else(|_| data_dir.clone());
    if canonical.starts_with(&data_canonical) {
        return Ok(());
    }

    let content_lib_dir = commander_content_library_dir()?;
    let content_lib_canonical = content_lib_dir.canonicalize().unwrap_or_else(|_| content_lib_dir.clone());
    if canonical.starts_with(&content_lib_canonical) {
        return Ok(());
    }

    Err(format!(
        "Path is outside allowed directories. Allowed: data directory ({}) and content library.",
        data_dir.to_string_lossy()
    ))
}

#[tauri::command]
pub fn init_config() -> Result<ConfigPaths, String> {
  let cfg = ensure_config()?;
  let dir = data_dir()?;
  let custom_bin_dir = load_app_settings_impl().ok().and_then(|s| s.custom_core_binary_dir);
  let resolve = |name: &str| -> String {
    if let Some(ref d) = custom_bin_dir {
      resolve_bin_with_override(name, Some(d))
    } else {
      resolve_bin(name)
    }
  };
  Ok(ConfigPaths {
    data_dir: dir.to_string_lossy().to_string(),
    config_path: cfg.to_string_lossy().to_string(),
    daemon_path: resolve("hemp0xd"),
    cli_path: resolve("hemp0x-cli"),
  })
}

#[tauri::command]
pub fn get_commander_settings_path() -> Result<String, String> {
  commander_settings_path().map(|p| p.to_string_lossy().to_string())
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
  
  let default_config = r#"# Hemp0x Configuration File
# Core cookie auth is used by default; rpcuser/rpcpassword are not required.
server=1
daemon=0
listen=1
txindex=1
assetindex=1
port=42069
rpcport=42068
"#;
  
  fs::write(&cfg, default_config).map_err(|e| e.to_string())?;
  Ok(())
}

#[tauri::command]
pub fn get_data_folder_info() -> Result<DataFolderInfo, String> {
  let default_dir = default_core_data_dir()?;
  let settings_path = commander_settings_path().unwrap_or_else(|_| PathBuf::from(""));
  let bootstrap = bootstrap_path().unwrap_or_else(|_| PathBuf::from(""));

  // Try to resolve the active data dir; fall back to default if bootstrap is invalid
  let (dir, bootstrap_error) = match active_data_dir() {
    Ok(d) => (d, None),
    Err(e) => (default_dir.clone(), Some(e)),
  };

  let folder_exists = dir.exists();
  let config_exists = dir.join("hemp.conf").exists();
  let wallet_exists = dir.join("wallet.dat").exists();
  let blocks_exists = dir.join("blocks").exists();
  let chainstate_exists = dir.join("chainstate").exists();
  let debug_log_exists = dir.join("debug.log").exists();
  let lock_exists = dir.join(".lock").exists();

  let size_bytes = if folder_exists { calculate_dir_size(&dir) } else { 0 };
  let size_display = format_size(size_bytes);

  Ok(DataFolderInfo {
    path: dir.to_string_lossy().to_string(),
    default_path: default_dir.to_string_lossy().to_string(),
    using_custom_path: bootstrap_error.is_none() && dir != default_dir,
    commander_settings_path: settings_path.to_string_lossy().to_string(),
    bootstrap_path: bootstrap.to_string_lossy().to_string(),
    size_bytes,
    size_display,
    config_exists,
    wallet_exists,
    folder_exists,
    blocks_exists,
    chainstate_exists,
    debug_log_exists,
    lock_exists,
    bootstrap_error,
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

  safe_copy_dir_recursive(&dir, &backup_path)?;
  Ok(backup_path.to_string_lossy().to_string())
}

#[tauri::command]
pub fn backup_data_folder_to(path: String) -> Result<(), String> {
  let dir = data_dir()?;
  if !dir.exists() {
    return Err("Data folder does not exist".to_string());
  }

  safe_copy_dir_recursive(&dir, Path::new(&path))?;
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
  let custom_bin_dir = load_app_settings_impl().ok().and_then(|s| s.custom_core_binary_dir);
  get_binary_status_with_override(custom_bin_dir.as_deref())
}

// SAFETY: replace_binaries removed in Slice 17d.
// External Core folder selection (non-destructive) is deferred to a future slice.
// See SystemHub.svelte for the placeholder UI.

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

#[tauri::command]
pub fn set_core_data_dir(path: String) -> Result<DataFolderInfo, String> {
  let p = PathBuf::from(&path);
  if path.trim().is_empty() {
    return Err("Path cannot be empty".to_string());
  }
  if !p.is_absolute() {
    return Err("Path must be absolute".to_string());
  }
  if p.exists() && p.is_file() {
    return Err("Path points to a file, not a directory".to_string());
  }
  if !p.exists() {
    fs::create_dir_all(&p).map_err(|e| e.to_string())?;
  }
  let current_dir = data_dir()?;
  if p == current_dir {
    return get_data_folder_info();
  }

  // 1. Load current settings from wherever they are now
  let mut settings = load_app_settings_impl()?;

  // 2. Write bootstrap pointer to default data dir
  save_bootstrap(Some(path.clone()))?;

  // 3. Update settings for backwards compatibility
  settings.custom_data_dir = Some(path.clone());

  // 4. Now active_data_dir() resolves to the new path — save settings there
  save_app_settings_impl(&settings)?;

  // 5. Ensure Core config exists in the custom data dir
  ensure_config()?;

  get_data_folder_info()
}

#[tauri::command]
pub fn reset_core_data_dir() -> Result<DataFolderInfo, String> {
  // 1. Clear bootstrap pointer FIRST so an invalid bootstrap does not block reset
  save_bootstrap(None)?;

  // 2. Load current settings from active location (now default, since bootstrap is cleared)
  let mut settings = load_app_settings_impl().unwrap_or_default();

  // 3. Update settings for backwards compatibility
  settings.custom_data_dir = None;

  // 4. active_data_dir() now resolves to default — save settings there
  save_app_settings_impl(&settings)?;

  // 5. Ensure default data dir exists
  let default_dir = default_core_data_dir()?;
  if !default_dir.exists() {
    fs::create_dir_all(&default_dir).map_err(|e| e.to_string())?;
  }

  // 6. Ensure Core config exists in default dir
  ensure_config()?;

  get_data_folder_info()
}

#[tauri::command]
pub fn prepare_core_data_dir_move(target_path: String) -> Result<DataMovePreview, String> {
  let source = data_dir()?;
  let target = PathBuf::from(&target_path);
  if target_path.trim().is_empty() {
    return Err("Target path cannot be empty".to_string());
  }
  if !target.is_absolute() {
    return Err("Target path must be absolute".to_string());
  }
  if target.exists() && target.is_file() {
    return Err("Target path points to a file, not a directory".to_string());
  }

  if path_contains(&source, &target) {
    return Err("Target directory is inside the source directory".to_string());
  }
  if target.exists() && path_contains(&target, &source) {
    return Err("Source directory is inside the target directory".to_string());
  }
  if let Some(normalized_target) = nearest_existing_absolute_ancestor(&target) {
    if path_contains(&source, &normalized_target) {
      return Err("Target path would be inside the source directory".to_string());
    }
  }

  let source_size = calculate_dir_size(&source);
  let target_exists = target.exists();
  let target_is_empty = target_exists && fs::read_dir(&target).map(|mut r| r.next().is_none()).unwrap_or(true);
  let target_has_files = target_exists && !target_is_empty;

  let mut warnings: Vec<String> = Vec::new();
  if target_has_files {
    warnings.push("Target directory exists and contains files. Copy will be refused for safety.".to_string());
  }
  if source.join(".lock").exists() {
    warnings.push("Source has a .lock file — daemon may be running. Stop it before move.".to_string());
  }

  Ok(DataMovePreview {
    source_path: source.to_string_lossy().to_string(),
    target_path: target.to_string_lossy().to_string(),
    source_size_bytes: source_size,
    source_size_display: format_size(source_size),
    target_exists,
    target_is_empty,
    target_has_files,
    wallet_present: source.join("wallet.dat").exists(),
    config_present: source.join("hemp.conf").exists(),
    blocks_present: source.join("blocks").exists(),
    chainstate_present: source.join("chainstate").exists(),
    warnings,
  })
}

#[tauri::command]
pub fn copy_core_data_dir_to(target_path: String) -> Result<DataMoveResult, String> {
  let source = data_dir()?;
  let target = PathBuf::from(&target_path);
  if !source.exists() {
    return Err("Source data directory does not exist".to_string());
  }
  if source.join(".lock").exists() {
    return Err("Cannot copy while daemon .lock file exists. Stop the daemon first.".to_string());
  }

  if path_contains(&source, &target) {
    return Err("Target directory is inside the source directory".to_string());
  }
  if target.exists() && path_contains(&target, &source) {
    return Err("Source directory is inside the target directory".to_string());
  }
  if let Some(normalized_target) = nearest_existing_absolute_ancestor(&target) {
    if path_contains(&source, &normalized_target) {
      return Err("Target path would be inside the source directory".to_string());
    }
  }

  if target.exists() {
    let has_content = fs::read_dir(&target).map(|mut r| r.next().is_some()).unwrap_or(false);
    if has_content {
      return Err("Target directory is not empty. Clear it or choose a different target.".to_string());
    }
  }
  if !target.exists() {
    fs::create_dir_all(&target).map_err(|e| e.to_string())?;
  }

  let (file_count, byte_count) = safe_copy_dir_recursive(&source, &target)?;
  Ok(DataMoveResult {
    success: true,
    message: format!("Copied {} files ({}) to {}", file_count, format_size(byte_count), target.to_string_lossy()),
    files_copied: file_count,
    bytes_copied: byte_count,
  })
}

fn safe_absolute_path(p: &Path) -> PathBuf {
  if p.is_absolute() {
    let mut normalized = PathBuf::new();
    for c in p.components() {
      match c {
        std::path::Component::Prefix(_) | std::path::Component::RootDir => {
          normalized.push(c.as_os_str());
        }
        std::path::Component::Normal(name) => {
          normalized.push(name);
        }
        std::path::Component::CurDir => {}
        std::path::Component::ParentDir => {
          normalized.pop();
        }
      }
    }
    normalized
  } else {
    p.to_path_buf()
  }
}

fn safe_copy_dir_recursive(src: &Path, dst: &Path) -> Result<(u64, u64), String> {
  fs::create_dir_all(dst).map_err(|e| e.to_string())?;
  let mut file_count: u64 = 0;
  let mut byte_count: u64 = 0;
  for entry in fs::read_dir(src).map_err(|e| e.to_string())? {
    let entry = entry.map_err(|e| e.to_string())?;
    let src_path = entry.path();
    let dst_path = dst.join(entry.file_name());
    let ft = entry.file_type().map_err(|e| e.to_string())?;
    if ft.is_symlink() {
      continue;
    }
    if ft.is_dir() {
      let (fc, bc) = safe_copy_dir_recursive(&src_path, &dst_path)?;
      file_count += fc;
      byte_count += bc;
    } else {
      let meta = fs::metadata(&src_path).map_err(|e| e.to_string())?;
      fs::copy(&src_path, &dst_path).map_err(|e| e.to_string())?;
      file_count += 1;
      byte_count += meta.len();
    }
  }
  Ok((file_count, byte_count))
}

fn path_contains(a: &Path, b: &Path) -> bool {
  let a_abs = safe_absolute_path(a);
  let b_abs = safe_absolute_path(b);
  if let (Ok(canon_a), Ok(canon_b)) = (a.canonicalize(), b.canonicalize()) {
    return canon_b.starts_with(&canon_a);
  }
  b_abs.starts_with(&a_abs)
}

fn nearest_existing_absolute_ancestor(p: &Path) -> Option<PathBuf> {
  if p.is_absolute() && p.exists() {
    return Some(p.to_path_buf());
  }
  let mut current = p.to_path_buf();
  loop {
    if let Some(parent) = current.parent() {
      if parent.as_os_str().is_empty() {
        return None;
      }
      if !parent.is_absolute() {
        continue;
      }
      if parent.exists() {
        if let Ok(canon) = parent.canonicalize() {
          let rel = current.strip_prefix(parent).unwrap_or_else(|_| Path::new(""));
          return Some(canon.join(rel));
        }
        return Some(current.to_path_buf());
      }
      current = parent.to_path_buf();
    } else {
      return None;
    }
  }
}

#[tauri::command]
pub fn set_daemon_repair_mode(mode: String) -> Result<AppSettings, String> {
  match mode.as_str() {
    "none" | "reindex" | "reindex-chainstate" => {},
    _ => return Err("Invalid repair mode. Use 'none', 'reindex', or 'reindex-chainstate'.".to_string()),
  }
  let mut settings = load_app_settings_impl()?;
  if mode == "none" {
    settings.pending_repair_mode = None;
  } else {
    settings.pending_repair_mode = Some(mode.clone());
    settings.active_repair_mode = Some(mode);
    settings.active_repair_started_at = Some(std::time::SystemTime::now()
      .duration_since(std::time::UNIX_EPOCH)
      .map(|d| d.as_secs())
      .unwrap_or(0));
  }
  save_app_settings_impl(&settings)?;
  Ok(settings)
}

#[tauri::command]
pub fn clear_daemon_repair_mode() -> Result<AppSettings, String> {
  let mut settings = load_app_settings_impl()?;
  settings.pending_repair_mode = None;
  settings.active_repair_mode = None;
  settings.active_repair_started_at = None;
  save_app_settings_impl(&settings)?;
  Ok(settings)
}

#[tauri::command]
pub fn get_daemon_repair_mode() -> Result<String, String> {
  let settings = load_app_settings_impl()?;
  Ok(settings.pending_repair_mode.unwrap_or_else(|| "none".to_string()))
}

const REQUIRED_CORE_BINS: &[&str] = &["hemp0xd", "hemp0x-cli"];
const OPTIONAL_CORE_BINS: &[&str] = &["hemp0x-tx"];

#[tauri::command]
pub fn set_core_binary_dir(path: String) -> Result<BinaryStatus, String> {
  let p = PathBuf::from(&path);
  if path.trim().is_empty() {
    return Err("Path cannot be empty".to_string());
  }
  if !p.is_absolute() {
    return Err("Path must be absolute".to_string());
  }
  if !p.exists() {
    return Err("Directory does not exist".to_string());
  }
  if !p.is_dir() {
    return Err("Path is not a directory".to_string());
  }

  for bin in REQUIRED_CORE_BINS {
    let bin_path = p.join(bin_name(bin));
    if !bin_path.exists() {
      return Err(format!("Missing required binary: {} (not found at {})", bin, bin_path.display()));
    }
  }

  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    for bin in REQUIRED_CORE_BINS.iter().chain(OPTIONAL_CORE_BINS.iter()) {
      let bin_path = p.join(bin_name(bin));
      if let Ok(meta) = fs::metadata(&bin_path) {
        let mut perms = meta.permissions();
        let mode = perms.mode();
        if mode & 0o111 == 0 {
          perms.set_mode(0o755);
          fs::set_permissions(&bin_path, perms)
            .map_err(|e| format!("Failed to set executable permission on {}: {}", bin, e))?;
        }
      }
    }
  }

  let mut settings = load_app_settings_impl()?;
  settings.custom_core_binary_dir = Some(path.clone());
  save_app_settings_impl(&settings)?;

  get_binary_status_with_override(Some(&path))
}

#[tauri::command]
pub fn reset_core_binary_dir() -> Result<BinaryStatus, String> {
  let mut settings = load_app_settings_impl()?;
  settings.custom_core_binary_dir = None;
  save_app_settings_impl(&settings)?;
  get_binary_status_with_override(None)
}

#[tauri::command]
pub fn get_core_binary_dir() -> Result<Option<String>, String> {
  let settings = load_app_settings_impl()?;
  Ok(settings.custom_core_binary_dir)
}

fn get_binary_status_with_override(override_dir: Option<&str>) -> Result<BinaryStatus, String> {
  let daemon_path = resolve_bin_with_override("hemp0xd", override_dir);
  let cli_path = resolve_bin_with_override("hemp0x-cli", override_dir);
  let tx_path = resolve_bin_with_override("hemp0x-tx", override_dir);
  Ok(BinaryStatus {
    daemon_exists: PathBuf::from(&daemon_path).exists(),
    cli_exists: PathBuf::from(&cli_path).exists(),
    tx_exists: PathBuf::from(&tx_path).exists(),
    daemon_path,
    cli_path,
    tx_path,
  })
}

fn read_log_recent_lines(path: &Path, max_lines: usize) -> Vec<String> {
  if !path.exists() {
    return Vec::new();
  }
  if let Ok(content) = fs::read_to_string(path) {
    let lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    let start = lines.len().saturating_sub(max_lines);
    return lines[start..].to_vec();
  }
  Vec::new()
}

fn parse_log_hint(lines: &[String]) -> Option<String> {
  for line in lines.iter().rev() {
    let lower = line.to_lowercase();
    if lower.contains("reindexing") {
      return Some("Reindexing block data".to_string());
    }
    if lower.contains("loadblockindex") {
      return Some("Loading block index".to_string());
    }
    if lower.contains("loaded block index") {
      return Some("Block index loaded".to_string());
    }
    if lower.contains("verifying blocks") {
      return Some("Verifying chainstate".to_string());
    }
    if lower.contains("updatetip") {
      return Some("RPC online, syncing".to_string());
    }
    if lower.contains("progress=") {
      return Some("Syncing in progress".to_string());
    }
    if lower.contains("shutdown") {
      return Some("Daemon shutting down".to_string());
    }
  }
  None
}

#[tauri::command]
pub fn get_daemon_repair_status() -> Result<RepairStatus, String> {
  let mut settings = load_app_settings_impl()?;
  let pending_mode = settings.pending_repair_mode.clone();
  let active_mode = settings.active_repair_mode.clone();
  let dir = data_dir().ok();
  let lock_exists = dir.as_ref().map(|d| d.join(".lock").exists()).unwrap_or(false);
  let debug_log_path = dir.as_ref().map(|d| d.join("debug.log"));

  let rpc_online = {
    use std::net::TcpStream;
    use std::time::Duration;
    TcpStream::connect_timeout(&"127.0.0.1:42068".parse().unwrap(), Duration::from_millis(500)).is_ok()
  };

  let (blocks, headers, verification_progress) = if rpc_online {
    use crate::modules::rpc::rpc_context;
    if let Ok(ctx) = rpc_context() {
      if let Ok(data) = ctx.call("getblockchaininfo", &[]) {
        let b = data["blocks"].as_u64();
        let h = data["headers"].as_u64();
        let vp = data["verificationprogress"].as_f64();
        (b, h, vp)
      } else {
        (None, None, None)
      }
    } else {
      (None, None, None)
    }
  } else {
    (None, None, None)
  };

  let log_lines = debug_log_path.as_ref().map(|p| read_log_recent_lines(p, 50)).unwrap_or_default();
  let log_hint = parse_log_hint(&log_lines);
  let latest_log_line = log_lines.last().cloned();

  let has_pending = pending_mode.as_deref() == Some("reindex") || pending_mode.as_deref() == Some("reindex-chainstate");
  let has_active = active_mode.as_deref() == Some("reindex") || active_mode.as_deref() == Some("reindex-chainstate");
  let repair_mode = if has_active { active_mode.clone() } else { pending_mode.clone() };

  let repair_work_ongoing = log_hint.as_ref().map(|h| {
    let l = h.to_lowercase();
    l.contains("reindexing") || l.contains("loading block index") || l.contains("verifying")
  }).unwrap_or(false);
  let sync_ongoing = log_hint.as_ref().map(|h| h.to_lowercase().contains("syncing")).unwrap_or(false);

  let rpc_healthy = rpc_online && blocks.is_some() && headers.is_some();
  let repair_complete = rpc_healthy && !repair_work_ongoing;

  if repair_complete && has_active {
    settings.active_repair_mode = None;
    settings.active_repair_started_at = None;
    save_app_settings_impl(&settings)?;
  }

  let is_repair_active = has_active || has_pending;
  let is_startup_repair = is_repair_active && !rpc_online;

  let phase = if is_startup_repair {
    if lock_exists {
      "Starting daemon with repair flag".to_string()
    } else {
      log_hint.clone().unwrap_or_else(|| "Reindexing block data".to_string())
    }
  } else if repair_complete {
    "Repair complete / node online".to_string()
  } else if rpc_online && (repair_work_ongoing || sync_ongoing) {
    "RPC online, syncing".to_string()
  } else if rpc_online && !repair_work_ongoing && has_active {
    "Node online; repair/sync status available".to_string()
  } else if is_repair_active {
    log_hint.clone().unwrap_or_else(|| "Daemon starting".to_string())
  } else {
    "Idle".to_string()
  };

  Ok(RepairStatus {
    active: is_repair_active && (!repair_complete || repair_work_ongoing),
    mode: repair_mode,
    phase,
    rpc_online,
    lock_exists,
    blocks,
    headers,
    verification_progress,
    latest_log_line,
    log_hint,
  })
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::fs;

  #[test]
  fn path_contains_target_inside_source() {
    let dir = std::env::temp_dir().join("hemp17d_contains_test_1");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let source = dir.join("source");
    let target = dir.join("source").join("child");
    fs::create_dir_all(&source).unwrap();
    fs::create_dir_all(&target).unwrap();
    assert!(path_contains(&source, &target), "child should be inside source");
    let _ = fs::remove_dir_all(&dir);
  }

  #[test]
  fn path_contains_source_inside_target() {
    let dir = std::env::temp_dir().join("hemp17d_contains_test_2");
    let _ = fs::remove_dir_all(&dir);
    let source = dir.join("child");
    let target = dir;
    fs::create_dir_all(&source).unwrap();
    fs::create_dir_all(&target).unwrap();
    assert!(path_contains(&target, &source), "source(child) should be inside target(dir)");
    let _ = fs::remove_dir_all(&target);
  }

  #[test]
  fn path_contains_unrelated_paths() {
    let dir = std::env::temp_dir().join("hemp17d_contains_test_3");
    let _ = fs::remove_dir_all(&dir);
    let a = dir.join("a");
    let b = dir.join("b");
    fs::create_dir_all(&a).unwrap();
    fs::create_dir_all(&b).unwrap();
    assert!(!path_contains(&a, &b), "sibling dirs should not contain each other");
    assert!(!path_contains(&b, &a), "sibling dirs should not contain each other");
    let _ = fs::remove_dir_all(&dir);
  }

  #[test]
  fn path_contains_non_existing_target() {
    let dir = std::env::temp_dir().join("hemp17d_contains_test_4");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let a = dir.clone();
    let b = dir.join("nonexistent_child");
    assert!(path_contains(&a, &b), "non-existing child should still be detected as inside parent");
    let _ = fs::remove_dir_all(&a);
  }

  #[test]
  fn validate_read_path_accepts_existing_file() {
    let dir = std::env::temp_dir().join("hemp61b_read_test");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let file = dir.join("test.txt");
    fs::write(&file, "hello").unwrap();
    let result = validate_read_path(&file.to_string_lossy());
    assert!(result.is_ok());
    let _ = fs::remove_dir_all(&dir);
  }

  #[test]
  fn validate_read_path_rejects_nonexistent() {
    let dir = std::env::temp_dir().join("hemp61b_read_nonexistent");
    let file = dir.join("missing.txt");
    let result = validate_read_path(&file.to_string_lossy());
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("does not exist"));
  }

  #[test]
  fn validate_read_path_rejects_directory() {
    let dir = std::env::temp_dir().join("hemp61b_read_dir");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let result = validate_read_path(&dir.to_string_lossy());
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("directory"));
    let _ = fs::remove_dir_all(&dir);
  }

  #[test]
  fn validate_read_path_rejects_empty() {
    assert!(validate_read_path("").is_err());
    assert!(validate_read_path("   ").is_err());
  }

  #[test]
  fn validate_read_path_rejects_relative() {
    assert!(validate_read_path("relative/path.txt").is_err());
  }

  #[test]
  fn validate_write_path_accepts_new_file_under_existing_parent() {
    let dir = std::env::temp_dir().join("hemp61b_write_test");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let file = dir.join("new_output.txt");
    let result = validate_write_path(&file.to_string_lossy());
    assert!(result.is_ok());
    let resolved = result.unwrap();
    assert_eq!(resolved.file_name().unwrap(), "new_output.txt");
    assert!(resolved.parent().unwrap().exists());
    let _ = fs::remove_dir_all(&dir);
  }

  #[test]
  fn validate_write_path_rejects_missing_parent() {
    let dir = std::env::temp_dir().join("hemp61b_write_missing_parent");
    let file = dir.join("subdir").join("output.txt");
    let result = validate_write_path(&file.to_string_lossy());
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("does not exist"));
  }

  #[test]
  fn validate_write_path_rejects_directory_target() {
    let dir = std::env::temp_dir().join("hemp61b_write_dir_target");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let result = validate_write_path(&dir.to_string_lossy());
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("directory"));
    let _ = fs::remove_dir_all(&dir);
  }

  #[test]
  fn validate_write_path_rejects_empty() {
    assert!(validate_write_path("").is_err());
    assert!(validate_write_path("   ").is_err());
  }

  #[test]
  fn validate_write_path_rejects_relative() {
    assert!(validate_write_path("relative/output.txt").is_err());
  }

  #[test]
  fn validate_write_path_resolves_symlink_parent() {
    let dir = std::env::temp_dir().join("hemp61b_symlink_test");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let real_dir = dir.join("real");
    fs::create_dir_all(&real_dir).unwrap();
    let link_dir = dir.join("link");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&real_dir, &link_dir).unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_dir(&real_dir, &link_dir).unwrap();
    let file = link_dir.join("output.txt");
    let result = validate_write_path(&file.to_string_lossy());
    assert!(result.is_ok());
    let resolved = result.unwrap();
    assert!(resolved.starts_with(&real_dir));
    let _ = fs::remove_dir_all(&dir);
  }

  #[test]
  fn validate_path_in_allowed_roots_accepts_data_dir_child() {
    let dir = std::env::temp_dir().join("hemp61b_allowed_test");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let data_dir = dir.join(".hemp0x");
    fs::create_dir_all(&data_dir).unwrap();
    let file = data_dir.join("test.txt");
    fs::write(&file, "hello").unwrap();
    std::env::set_var("HOME", dir.to_str().unwrap());
    let result = validate_path_in_allowed_roots(&file);
    assert!(result.is_ok());
    let _ = fs::remove_dir_all(&dir);
  }

  #[test]
  fn validate_path_in_allowed_roots_rejects_outside_path() {
    let dir = std::env::temp_dir().join("hemp61b_reject_test");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let data_dir = dir.join(".hemp0x");
    fs::create_dir_all(&data_dir).unwrap();
    let outside = dir.join("outside").join("test.txt");
    fs::create_dir_all(dir.join("outside")).unwrap();
    fs::write(&outside, "hello").unwrap();
    std::env::set_var("HOME", dir.to_str().unwrap());
    let result = validate_path_in_allowed_roots(&outside);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("outside allowed"));
    let _ = fs::remove_dir_all(&dir);
  }
}
