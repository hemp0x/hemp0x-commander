use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::modules::files::data_dir;

const JOURNAL_SCHEMA: &str = "hemp0x.commander.tx_journal";
const JOURNAL_SCHEMA_VERSION: u32 = 1;
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
}

fn commander_dir() -> Result<PathBuf, String> {
    let dir = data_dir()?.join("commander");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn journal_path() -> Result<PathBuf, String> {
    Ok(commander_dir()?.join("tx_journal.json"))
}

fn empty_journal() -> TxJournal {
    TxJournal {
        schema: JOURNAL_SCHEMA.to_string(),
        schema_version: JOURNAL_SCHEMA_VERSION,
        entries: Vec::new(),
    }
}

fn read_journal() -> Result<TxJournal, String> {
    let path = journal_path()?;
    if !path.exists() {
        return Ok(empty_journal());
    }
    let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let journal: TxJournal = serde_json::from_str(&text)
        .map_err(|e| format!("Transaction journal is unreadable. Original file preserved: {e}"))?;
    if journal.schema != JOURNAL_SCHEMA || journal.schema_version != JOURNAL_SCHEMA_VERSION {
        return Err("Unsupported transaction journal schema version".to_string());
    }
    Ok(journal)
}

fn write_journal(journal: &TxJournal) -> Result<(), String> {
    let path = journal_path()?;
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
            fs::remove_file(&path).map_err(|e| e.to_string())?;
        }
    }
    fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
    Ok(())
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
    let entry = JournalEntry {
        id: Uuid::new_v4().to_string(),
        status: input.status,
        operation_type: input.operation_type,
        summary: input.summary,
        txid: input.txid,
        created_at: now.clone(),
        updated_at: now,
        details: input.details.unwrap_or(serde_json::Value::Null),
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
pub fn export_tx_journal(path: String) -> Result<(), String> {
    let _guard = JOURNAL_LOCK
        .lock()
        .map_err(|_| "Transaction journal lock is poisoned".to_string())?;
    let journal = read_journal()?;
    let text = serde_json::to_string_pretty(&journal).map_err(|e| e.to_string())?;
    fs::write(path, text).map_err(|e| e.to_string())
}
