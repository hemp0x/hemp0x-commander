use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::modules::files::{commander_dir, load_app_settings_impl};

const JOURNAL_SCHEMA: &str = "hemp0x.commander.tx_journal";
const JOURNAL_SCHEMA_VERSION: u32 = 1;
const ARCHIVES_DIR_NAME: &str = "journal_archives";
static JOURNAL_LOCK: Mutex<()> = Mutex::new(());

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum JournalStatus {
    Draft,
    Previewed,
    Signed,
    Broadcasted,
    Confirmed,
    Failed,
    Abandoned,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct JournalEntry {
    pub id: String,
    pub status: JournalStatus,
    pub operation_type: String,
    pub summary: String,
    pub txid: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub details: serde_json::Value,
    #[serde(default)]
    pub network: Option<String>,
    #[serde(default)]
    pub core_wallet_name: Option<String>,
    #[serde(default)]
    pub vault_display_name: Option<String>,
    #[serde(default)]
    pub vault_fingerprint: Option<String>,
    #[serde(default)]
    pub wallet_record_id: Option<String>,
    #[serde(default)]
    pub alignment_id: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct TxJournal {
    pub schema: String,
    pub schema_version: u32,
    pub entries: Vec<JournalEntry>,
}

#[derive(Deserialize)]
pub struct JournalEntryInput {
    pub status: JournalStatus,
    pub operation_type: String,
    pub summary: String,
    pub txid: Option<String>,
    pub details: Option<serde_json::Value>,
    #[serde(default)]
    pub network: Option<String>,
    #[serde(default)]
    pub core_wallet_name: Option<String>,
    #[serde(default)]
    pub vault_display_name: Option<String>,
    #[serde(default)]
    pub vault_fingerprint: Option<String>,
    #[serde(default)]
    pub wallet_record_id: Option<String>,
    #[serde(default)]
    pub alignment_id: Option<String>,
}

#[derive(Clone, Serialize)]
pub struct JournalArchiveDescriptor {
    pub filename: String,
    pub created_at: String,
    pub entry_count: usize,
    pub size_bytes: u64,
}

fn journal_path() -> Result<PathBuf, String> {
    Ok(commander_dir()?.join("tx_journal.json"))
}

fn archives_dir_path() -> Result<PathBuf, String> {
    Ok(commander_dir()?.join(ARCHIVES_DIR_NAME))
}

fn archive_path_for_filename(filename: &str) -> Result<PathBuf, String> {
    if filename.trim().is_empty()
        || filename.contains('/')
        || filename.contains('\\')
        || !filename.ends_with(".json")
    {
        return Err("Invalid journal archive filename.".to_string());
    }
    let path = PathBuf::from(filename);
    if path.file_name().and_then(|name| name.to_str()) != Some(filename) {
        return Err("Invalid journal archive filename.".to_string());
    }
    Ok(archives_dir_path()?.join(filename))
}

fn empty_journal() -> TxJournal {
    TxJournal {
        schema: JOURNAL_SCHEMA.to_string(),
        schema_version: JOURNAL_SCHEMA_VERSION,
        entries: Vec::new(),
    }
}

fn read_inner(path: &PathBuf) -> Result<TxJournal, String> {
    if !path.exists() {
        return Ok(empty_journal());
    }
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    #[derive(Deserialize)]
    struct VersionProbe {
        schema_version: u32,
    }
    let probe: VersionProbe = serde_json::from_str(&text)
        .map_err(|e| format!("Transaction journal is unreadable: {e}"))?;
    if probe.schema_version != JOURNAL_SCHEMA_VERSION {
        return Err(format!(
            "Unsupported transaction journal schema version {} (supported: v1)",
            probe.schema_version
        ));
    }
    let journal: TxJournal = serde_json::from_str(&text)
        .map_err(|e| format!("Transaction journal is unreadable. Original file preserved: {e}"))?;
    if journal.schema != JOURNAL_SCHEMA {
        return Err("Unrecognized transaction journal schema identifier".to_string());
    }
    Ok(journal)
}

fn read_journal() -> Result<TxJournal, String> {
    read_inner(&journal_path()?)
}

fn write_journal_at(journal: &TxJournal, path: &PathBuf) -> Result<(), String> {
    let tmp = path.with_extension("json.tmp");
    let text = serde_json::to_string_pretty(journal).map_err(|e| e.to_string())?;
    {
        let mut file = fs::File::create(&tmp).map_err(|e| e.to_string())?;
        file.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
    }
    #[cfg(windows)]
    {
        if path.exists() {
            fs::remove_file(path).map_err(|e| e.to_string())?;
        }
    }
    fs::rename(&tmp, path).map_err(|e| e.to_string())?;
    Ok(())
}

fn write_journal(journal: &TxJournal) -> Result<(), String> {
    let path = journal_path()?;
    write_journal_at(journal, &path)
}

#[tauri::command]
pub fn get_tx_journal_path() -> Result<String, String> {
    Ok(journal_path()?.to_string_lossy().to_string())
}

#[tauri::command]
pub fn list_tx_journal_entries() -> Result<Vec<JournalEntry>, String> {
    let _guard = JOURNAL_LOCK
        .lock()
        .map_err(|_| "Transaction journal lock is poisoned".to_string())?;
    Ok(read_journal()?.entries)
}

#[tauri::command]
pub fn add_tx_journal_entry(input: JournalEntryInput) -> Result<JournalEntry, String> {
    let _guard = JOURNAL_LOCK
        .lock()
        .map_err(|_| "Transaction journal lock is poisoned".to_string())?;
    let mut journal = read_journal()?;
    let now = Utc::now().to_rfc3339();
    let core_wallet_name = input.core_wallet_name.or_else(|| {
        load_app_settings_impl()
            .ok()
            .and_then(|s| s.active_vault_wallet_name)
    });
    let entry = JournalEntry {
        id: Uuid::new_v4().to_string(),
        status: input.status,
        operation_type: input.operation_type,
        summary: input.summary,
        txid: input.txid,
        created_at: now.clone(),
        updated_at: now,
        details: input.details.unwrap_or(serde_json::Value::Null),
        network: input.network,
        core_wallet_name,
        vault_display_name: input.vault_display_name,
        vault_fingerprint: input.vault_fingerprint,
        wallet_record_id: input.wallet_record_id,
        alignment_id: input.alignment_id,
    };
    journal.entries.push(entry.clone());
    write_journal(&journal)?;
    Ok(entry)
}

#[tauri::command]
pub fn update_tx_journal_entry(
    id: String,
    status: JournalStatus,
    txid: Option<String>,
    details: Option<serde_json::Value>,
) -> Result<JournalEntry, String> {
    let _guard = JOURNAL_LOCK
        .lock()
        .map_err(|_| "Transaction journal lock is poisoned".to_string())?;
    let mut journal = read_journal()?;
    let now = Utc::now().to_rfc3339();
    let mut updated = None;

    for entry in &mut journal.entries {
        if entry.id == id {
            entry.status = status.clone();
            entry.txid = txid.clone().or_else(|| entry.txid.clone());
            if let Some(details) = details.clone() {
                entry.details = details;
            }
            entry.updated_at = now.clone();
            updated = Some(entry.clone());
            break;
        }
    }

    let entry = updated.ok_or_else(|| "Transaction journal entry not found".to_string())?;
    write_journal(&journal)?;
    Ok(entry)
}

#[tauri::command]
pub fn delete_tx_journal_entry(id: String) -> Result<(), String> {
    let _guard = JOURNAL_LOCK
        .lock()
        .map_err(|_| "Transaction journal lock is poisoned".to_string())?;
    let mut journal = read_journal()?;
    let before = journal.entries.len();
    journal.entries.retain(|entry| entry.id != id);
    if journal.entries.len() == before {
        return Err("Transaction journal entry not found".to_string());
    }
    write_journal(&journal)
}

#[tauri::command]
pub fn delete_tx_journal_entries(ids: Vec<String>) -> Result<String, String> {
    let _guard = JOURNAL_LOCK
        .lock()
        .map_err(|_| "Transaction journal lock is poisoned".to_string())?;
    let mut journal = read_journal()?;
    let before = journal.entries.len();
    let id_set: std::collections::HashSet<&str> = ids.iter().map(|s| s.as_str()).collect();
    journal
        .entries
        .retain(|entry| !id_set.contains(entry.id.as_str()));
    let removed = before - journal.entries.len();
    write_journal(&journal)?;
    Ok(format!("{} entries deleted.", removed))
}

#[tauri::command]
pub fn export_tx_journal(path: String) -> Result<(), String> {
    let _guard = JOURNAL_LOCK
        .lock()
        .map_err(|_| "Transaction journal lock is poisoned".to_string())?;
    let journal = read_journal()?;
    let text = serde_json::to_string_pretty(&journal).map_err(|e| e.to_string())?;
    fs::write(path, text).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn import_merge_tx_journal(path: String) -> Result<String, String> {
    let _guard = JOURNAL_LOCK
        .lock()
        .map_err(|_| "Transaction journal lock is poisoned".to_string())?;
    let import_path = PathBuf::from(&path);
    let imported = read_inner(&import_path)?;
    let mut current = read_journal()?;
    let existing_ids: std::collections::HashSet<String> =
        current.entries.iter().map(|e| e.id.clone()).collect();
    let mut added = 0u32;
    let mut skipped = 0u32;
    for entry in &imported.entries {
        if existing_ids.contains(&entry.id) {
            skipped += 1;
        } else {
            current.entries.push(entry.clone());
            added += 1;
        }
    }
    write_journal(&current)?;
    Ok(format!(
        "Import complete: {} entries added, {} duplicates skipped.",
        added, skipped
    ))
}

#[tauri::command]
pub fn archive_tx_journal() -> Result<String, String> {
    let _guard = JOURNAL_LOCK
        .lock()
        .map_err(|_| "Transaction journal lock is poisoned".to_string())?;
    let current_path = journal_path()?;
    if !current_path.exists() || read_inner(&current_path)?.entries.is_empty() {
        return Err("No journal entries to archive.".to_string());
    }
    let archives_dir = archives_dir_path()?;
    fs::create_dir_all(&archives_dir).map_err(|e| e.to_string())?;
    let ts = Utc::now().format("%Y%m%d_%H%M%S_%f");
    let archive_name = format!("tx_journal_{}.json", ts);
    let archive_path = archives_dir.join(&archive_name);
    fs::copy(&current_path, &archive_path).map_err(|e| e.to_string())?;
    let fresh = empty_journal();
    write_journal_at(&fresh, &current_path)?;
    Ok(format!("Journal archived as {} and reset.", archive_name))
}

#[tauri::command]
pub fn list_tx_journal_archives() -> Result<Vec<JournalArchiveDescriptor>, String> {
    let _guard = JOURNAL_LOCK
        .lock()
        .map_err(|_| "Transaction journal lock is poisoned".to_string())?;
    let archives_dir = archives_dir_path()?;
    if !archives_dir.exists() {
        return Ok(Vec::new());
    }
    let mut result = Vec::new();
    let dir = fs::read_dir(&archives_dir).map_err(|e| e.to_string())?;
    for entry in dir {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.extension().map_or(false, |e| e == "json") {
            let meta = entry.metadata().map_err(|e| e.to_string())?;
            let journal = match read_inner(&path) {
                Ok(j) => j,
                Err(_) => continue,
            };
            let created_at = meta
                .modified()
                .ok()
                .or_else(|| meta.created().ok())
                .map(|t| {
                    chrono::DateTime::<chrono::Utc>::from(t)
                        .format("%Y-%m-%dT%H:%M:%SZ")
                        .to_string()
                })
                .unwrap_or_else(|| "unknown".to_string());
            result.push(JournalArchiveDescriptor {
                filename: path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string(),
                created_at,
                entry_count: journal.entries.len(),
                size_bytes: meta.len(),
            });
        }
    }
    result.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(result)
}

#[tauri::command]
pub fn delete_tx_journal_archive(filename: String) -> Result<String, String> {
    let _guard = JOURNAL_LOCK
        .lock()
        .map_err(|_| "Transaction journal lock is poisoned".to_string())?;
    let archive_path = archive_path_for_filename(&filename)?;
    if !archive_path.exists() {
        return Err(format!("Archive file '{}' not found.", filename));
    }
    fs::remove_file(&archive_path).map_err(|e| e.to_string())?;
    Ok(format!("Archive '{}' deleted.", filename))
}

#[tauri::command]
pub fn restore_tx_journal_archive(filename: String) -> Result<String, String> {
    let _guard = JOURNAL_LOCK
        .lock()
        .map_err(|_| "Transaction journal lock is poisoned".to_string())?;
    let archives_dir = archives_dir_path()?;
    let archive_path = archive_path_for_filename(&filename)?;
    if !archive_path.exists() {
        return Err(format!("Archive file '{}' not found.", filename));
    }
    let _journal = read_inner(&archive_path)?;
    let current_path = journal_path()?;
    if current_path.exists() {
        let ts = Utc::now().format("%Y%m%d_%H%M%S_%f");
        let backup_name = format!("tx_journal_pre_restore_{}.json", ts);
        let backup_path = archives_dir.join(&backup_name);
        fs::copy(&current_path, &backup_path).map_err(|e| e.to_string())?;
    }
    fs::copy(&archive_path, &current_path).map_err(|e| e.to_string())?;
    Ok(format!("Journal restored from archive '{}'.", filename))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::files;
    use std::fs;

    struct JournalTestEnv {
        path: PathBuf,
    }

    impl JournalTestEnv {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("journal_test_{}", Uuid::new_v4()));
            fs::create_dir_all(&path).expect("create journal test dir");
            files::TEST_COMMANDER_DIR.with(|cell| {
                cell.borrow_mut().replace(path.clone());
            });
            JournalTestEnv { path }
        }
    }

    impl Drop for JournalTestEnv {
        fn drop(&mut self) {
            files::TEST_COMMANDER_DIR.with(|cell| {
                cell.borrow_mut().take();
            });
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn setup_test_env() -> JournalTestEnv {
        JournalTestEnv::new()
    }

    fn cleanup_test_env() {
        files::TEST_COMMANDER_DIR.with(|cell| {
            cell.borrow_mut().take();
        });
    }

    #[test]
    fn archive_filename_rejects_path_traversal() {
        let _tmp = setup_test_env();
        assert!(archive_path_for_filename("../tx_journal_bad.json").is_err());
        assert!(archive_path_for_filename("nested/tx_journal_bad.json").is_err());
        assert!(archive_path_for_filename("nested\\tx_journal_bad.json").is_err());
        assert!(archive_path_for_filename("tx_journal_bad.txt").is_err());
    }

    fn make_journal(entries: Vec<JournalEntry>) -> TxJournal {
        TxJournal {
            schema: JOURNAL_SCHEMA.to_string(),
            schema_version: JOURNAL_SCHEMA_VERSION,
            entries,
        }
    }

    fn make_entry(id: &str, summary: &str) -> JournalEntry {
        JournalEntry {
            id: id.to_string(),
            status: JournalStatus::Draft,
            operation_type: "test_op".to_string(),
            summary: summary.to_string(),
            txid: None,
            created_at: Utc::now().to_rfc3339(),
            updated_at: Utc::now().to_rfc3339(),
            details: serde_json::Value::Null,
            network: None,
            core_wallet_name: None,
            vault_display_name: None,
            vault_fingerprint: None,
            wallet_record_id: None,
            alignment_id: None,
        }
    }

    #[test]
    fn v1_journal_loads_with_new_context_fields() {
        let _tmp = setup_test_env();
        let v1 = serde_json::json!({
            "schema": JOURNAL_SCHEMA,
            "schema_version": 1,
            "entries": [{
                "id": "abc-1",
                "status": "Draft",
                "operation_type": "test_op",
                "summary": "entry one",
                "txid": null,
                "created_at": "2026-01-01T00:00:00Z",
                "updated_at": "2026-01-01T00:00:00Z",
                "details": null
            }],
        });
        let journal_path = journal_path().unwrap();
        fs::write(&journal_path, serde_json::to_string_pretty(&v1).unwrap()).unwrap();

        let journal = read_journal().unwrap();
        assert_eq!(journal.schema_version, JOURNAL_SCHEMA_VERSION);
        assert_eq!(journal.entries.len(), 1);
        assert_eq!(journal.entries[0].id, "abc-1");
        assert_eq!(journal.entries[0].network, None);
        assert_eq!(journal.entries[0].core_wallet_name, None);
        cleanup_test_env();
    }

    #[test]
    fn journal_loads_with_context_fields() {
        let _tmp = setup_test_env();
        let v1 = serde_json::json!({
            "schema": JOURNAL_SCHEMA,
            "schema_version": 1,
            "entries": [{
                "id": "ctx-1",
                "status": "Draft",
                "operation_type": "test_op",
                "summary": "with context",
                "txid": null,
                "created_at": "2026-01-01T00:00:00Z",
                "updated_at": "2026-01-01T00:00:00Z",
                "details": null,
                "network": "mainnet",
                "core_wallet_name": "test-wallet",
                "vault_display_name": "My Vault",
                "vault_fingerprint": null,
                "wallet_record_id": null,
                "alignment_id": null
            }],
        });
        let journal_path = journal_path().unwrap();
        fs::write(&journal_path, serde_json::to_string_pretty(&v1).unwrap()).unwrap();

        let journal = read_journal().unwrap();
        assert_eq!(journal.entries.len(), 1);
        assert_eq!(journal.entries[0].network.as_deref(), Some("mainnet"));
        assert_eq!(
            journal.entries[0].core_wallet_name.as_deref(),
            Some("test-wallet")
        );
        assert_eq!(
            journal.entries[0].vault_display_name.as_deref(),
            Some("My Vault")
        );
        cleanup_test_env();
    }

    #[test]
    fn merge_dedup_by_entry_id() {
        let _tmp = setup_test_env();
        let e1 = make_entry("id-1", "alpha");
        let e2 = make_entry("id-2", "beta");
        write_journal(&make_journal(vec![e1.clone(), e2.clone()])).unwrap();

        let e3 = make_entry("id-2", "beta from import");
        let e4 = make_entry("id-3", "gamma");
        let import_path = commander_dir().unwrap().join("import_test.json");
        let imported = make_journal(vec![e3, e4]);
        fs::write(
            &import_path,
            serde_json::to_string_pretty(&imported).unwrap(),
        )
        .unwrap();

        let result = import_merge_tx_journal(import_path.to_string_lossy().to_string()).unwrap();
        assert!(result.contains("1 entries added"));
        assert!(result.contains("1 duplicates skipped"));

        let merged = read_journal().unwrap();
        assert_eq!(merged.entries.len(), 3);
        assert_eq!(
            merged
                .entries
                .iter()
                .find(|e| e.id == "id-2")
                .unwrap()
                .summary,
            "beta"
        );
        assert!(merged.entries.iter().any(|e| e.id == "id-3"));
        cleanup_test_env();
    }

    #[test]
    fn merge_preserves_existing_on_conflict() {
        let _tmp = setup_test_env();
        write_journal(&make_journal(vec![make_entry("conflict-1", "original")])).unwrap();

        let import_path = commander_dir().unwrap().join("import_conflict.json");
        let imported = make_journal(vec![make_entry("conflict-1", "imported value")]);
        fs::write(
            &import_path,
            serde_json::to_string_pretty(&imported).unwrap(),
        )
        .unwrap();

        import_merge_tx_journal(import_path.to_string_lossy().to_string()).unwrap();
        let merged = read_journal().unwrap();
        assert_eq!(merged.entries.len(), 1);
        assert_eq!(merged.entries[0].summary, "original");
        cleanup_test_env();
    }

    #[test]
    fn archive_creates_timestamped_backup_and_resets() {
        let _tmp = setup_test_env();
        write_journal(&make_journal(vec![make_entry("arch-1", "entry")])).unwrap();

        let result = archive_tx_journal().unwrap();
        assert!(result.contains("archived"));

        let archives = list_tx_journal_archives().unwrap();
        assert_eq!(archives.len(), 1);
        assert!(archives[0].filename.starts_with("tx_journal_"));
        assert_eq!(archives[0].entry_count, 1);

        let fresh = read_journal().unwrap();
        assert_eq!(fresh.entries.len(), 0);
        cleanup_test_env();
    }

    #[test]
    fn archive_errors_on_empty_journal() {
        let _tmp = setup_test_env();
        write_journal(&empty_journal()).unwrap();
        assert!(archive_tx_journal().is_err());
        cleanup_test_env();
    }

    #[test]
    fn restore_from_archive() {
        let _tmp = setup_test_env();
        write_journal(&make_journal(vec![make_entry("rest-1", "restorable")])).unwrap();
        archive_tx_journal().unwrap();

        let archives = list_tx_journal_archives().unwrap();
        let filename = archives[0].filename.clone();

        assert_eq!(read_journal().unwrap().entries.len(), 0);

        restore_tx_journal_archive(filename).unwrap();
        let restored = read_journal().unwrap();
        assert_eq!(restored.entries.len(), 1);
        assert_eq!(restored.entries[0].id, "rest-1");
        cleanup_test_env();
    }

    #[test]
    fn restore_creates_pre_restore_backup() {
        let _tmp = setup_test_env();
        write_journal(&make_journal(vec![make_entry("rb-1", "first")])).unwrap();
        archive_tx_journal().unwrap();
        write_journal(&make_journal(vec![make_entry("rb-2", "second")])).unwrap();

        let archives_before = list_tx_journal_archives().unwrap();
        let file_count_before = archives_before.len();

        let archive_filename = archives_before
            .iter()
            .find(|a| a.entry_count == 1)
            .unwrap()
            .filename
            .clone();
        restore_tx_journal_archive(archive_filename).unwrap();

        let archives_after = list_tx_journal_archives().unwrap();
        assert_eq!(archives_after.len(), file_count_before + 1);
        assert!(archives_after
            .iter()
            .any(|a| a.filename.contains("pre_restore")));
        cleanup_test_env();
    }

    #[test]
    fn bulk_delete_removes_multiple_entries() {
        let _tmp = setup_test_env();
        write_journal(&make_journal(vec![
            make_entry("bd-1", "keep"),
            make_entry("bd-2", "remove"),
            make_entry("bd-3", "remove"),
            make_entry("bd-4", "keep"),
        ]))
        .unwrap();

        let result =
            delete_tx_journal_entries(vec!["bd-2".to_string(), "bd-3".to_string()]).unwrap();
        assert!(result.contains("2 entries deleted"));

        let journal = read_journal().unwrap();
        assert_eq!(journal.entries.len(), 2);
        assert!(journal.entries.iter().any(|e| e.id == "bd-1"));
        assert!(journal.entries.iter().any(|e| e.id == "bd-4"));
        cleanup_test_env();
    }

    #[test]
    fn new_entry_populates_context_fields() {
        let _tmp = setup_test_env();
        let input = JournalEntryInput {
            status: JournalStatus::Draft,
            operation_type: "test".to_string(),
            summary: "context test".to_string(),
            txid: None,
            details: None,
            network: Some("mainnet".to_string()),
            core_wallet_name: Some("test-wallet".to_string()),
            vault_display_name: Some("My Vault".to_string()),
            vault_fingerprint: Some("fp-123".to_string()),
            wallet_record_id: Some("wallet.webcom.hemp.primary".to_string()),
            alignment_id: None,
        };
        let entry = add_tx_journal_entry(input).unwrap();
        assert_eq!(entry.network.as_deref(), Some("mainnet"));
        assert_eq!(entry.core_wallet_name.as_deref(), Some("test-wallet"));
        assert_eq!(entry.vault_display_name.as_deref(), Some("My Vault"));
        assert_eq!(entry.vault_fingerprint.as_deref(), Some("fp-123"));
        assert_eq!(
            entry.wallet_record_id.as_deref(),
            Some("wallet.webcom.hemp.primary")
        );
        assert_eq!(entry.alignment_id, None);
        cleanup_test_env();
    }

    #[test]
    fn new_entry_auto_populates_core_wallet_name_from_settings() {
        let _tmp = setup_test_env();
        let mut settings = files::load_app_settings_impl().unwrap_or_default();
        settings.active_vault_wallet_name = Some("auto-wallet".to_string());
        files::save_app_settings_impl(&settings).unwrap();

        let input = JournalEntryInput {
            status: JournalStatus::Draft,
            operation_type: "test".to_string(),
            summary: "auto wallet name".to_string(),
            txid: None,
            details: None,
            network: None,
            core_wallet_name: None,
            vault_display_name: None,
            vault_fingerprint: None,
            wallet_record_id: None,
            alignment_id: None,
        };
        let entry = add_tx_journal_entry(input).unwrap();
        assert_eq!(entry.core_wallet_name.as_deref(), Some("auto-wallet"));
        cleanup_test_env();
    }
}
