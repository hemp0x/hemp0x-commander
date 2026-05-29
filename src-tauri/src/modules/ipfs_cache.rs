use std::fs;
use std::path::PathBuf;
use std::io::Read;
use std::time::Duration;

use chrono::Utc;
use serde::{Serialize, Deserialize};

use crate::modules::files::data_dir;
use crate::modules::content_library::validate_import_cid;

const DEFAULT_GATEWAYS: &[&str] = &[
    "https://dweb.link/ipfs/",
    "https://ipfs.io/ipfs/",
];

const FETCH_TIMEOUT_SECS: u64 = 30;
const MAX_DOWNLOAD_SIZE: u64 = 16 * 1024 * 1024;

// ---------------------------------------------------------------------------
// Data structures
// ---------------------------------------------------------------------------

#[derive(Clone, Serialize, Deserialize)]
pub struct IpfsFetchResult {
    pub cid: String,
    pub gateway_used: String,
    pub content_type: String,
    pub size_bytes: u64,
    pub fetched_at: String,
    pub local_path: String,
    pub content_base64: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub cid: String,
    pub gateway_used: String,
    pub content_type: String,
    pub size_bytes: u64,
    pub fetched_at: String,
    pub local_path: String,
}

#[derive(Serialize)]
pub struct CacheStatus {
    pub cache_dir: String,
    pub entry_count: usize,
    pub total_size_bytes: u64,
    pub entries: Vec<CacheEntry>,
}

// ---------------------------------------------------------------------------
// Paths
// ---------------------------------------------------------------------------

fn cache_dir() -> Result<PathBuf, String> {
    let dir = data_dir()?.join("content-library").join("cache");
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create cache dir: {}", e))?;
    Ok(dir)
}

fn cid_cache_dir(cid: &str) -> Result<PathBuf, String> {
    let safe = safe_cid_dirname(cid);
    Ok(cache_dir()?.join(safe))
}

fn cid_content_path(cid: &str) -> Result<PathBuf, String> {
    Ok(cid_cache_dir(cid)?.join("content"))
}

fn cid_meta_path(cid: &str) -> Result<PathBuf, String> {
    Ok(cid_cache_dir(cid)?.join("meta.json"))
}

fn safe_cid_dirname(cid: &str) -> String {
    cid.chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect::<String>()
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn load_cache_meta(cid: &str) -> Result<CacheEntry, String> {
    let path = cid_meta_path(cid)?;
    if !path.exists() {
        return Err("Cache entry not found".to_string());
    }
    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let entry: CacheEntry = serde_json::from_str(&content).map_err(|e| e.to_string())?;
    Ok(entry)
}

fn save_cache_meta(cid: &str, entry: &CacheEntry) -> Result<(), String> {
    let dir = cid_cache_dir(cid)?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = cid_meta_path(cid)?;
    let content = serde_json::to_string_pretty(entry).map_err(|e| e.to_string())?;
    fs::write(&path, &content).map_err(|e| e.to_string())?;
    Ok(())
}

fn save_cache_content(cid: &str, data: &[u8]) -> Result<PathBuf, String> {
    let dir = cid_cache_dir(cid)?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = cid_content_path(cid)?;
    fs::write(&path, data).map_err(|e| e.to_string())?;
    Ok(path)
}

/// Fetches content from a single gateway URL with timeout and size limit.
/// Returns (content_type, data).
fn fetch_from_gateway(cid: &str, gateway_base: &str) -> Result<(String, Vec<u8>), String> {
    let clean_base = gateway_base.trim_end_matches('/');
    let url = format!("{}/{}", clean_base, cid);

    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(FETCH_TIMEOUT_SECS))
        .timeout_write(Duration::from_secs(15))
        .build();

    let response = agent.get(&url)
        .set("User-Agent", "hemp0x-commander/2.0")
        .call()
        .map_err(|e| match e {
            ureq::Error::Transport(t) => format!("Gateway unreachable: {}", t),
            ureq::Error::Status(code, _) => format!("Gateway returned HTTP {}", code),
        })?;

    let content_type = response.content_type().to_string();

    let reader = response.into_reader();
    let mut buf = Vec::new();

    let mut limited = reader.take(MAX_DOWNLOAD_SIZE + 1);
    let bytes_read = limited.read_to_end(&mut buf).map_err(|e| format!("Read error: {}", e))?;

    if bytes_read as u64 > MAX_DOWNLOAD_SIZE {
        return Err(format!(
            "Content exceeds max download size of {} MB",
            MAX_DOWNLOAD_SIZE / (1024 * 1024)
        ));
    }

    Ok((content_type, buf))
}

fn guess_content_type_priority(
    content_type_header: &str,
    data: &[u8],
    _cid: &str,
) -> String {
    let ct = content_type_header.to_lowercase();
    if ct.contains("text/plain") || ct.is_empty() {
        if serde_json::from_slice::<serde_json::Value>(data).is_ok() {
            return "application/json".to_string();
        }
        if data.len() >= 4 && &data[1..4] == b"PNG" {
            return "image/png".to_string();
        }
        if data.len() >= 4 && &data[0..4] == b"%PDF" {
            return "application/pdf".to_string();
        }
        if data.len() > 0 && (data[0] == b'#' || data[0] == b'-' || data[0] == b'>') {
            return "text/markdown".to_string();
        }
        return "text/plain".to_string();
    }
    if ct.contains("octet-stream") || ct.contains("binary") || ct == "unknown/unknown" {
        if data.len() >= 4 && &data[1..4] == b"PNG" {
            return "image/png".to_string();
        } else if data.len() >= 3 && &data[0..3] == b"\xff\xd8\xff" {
            return "image/jpeg".to_string();
        } else if data.len() >= 4 && &data[0..4] == b"GIF8" {
            return "image/gif".to_string();
        } else if data.len() >= 4 && &data[0..4] == b"%PDF" {
            return "application/pdf".to_string();
        } else if data.len() >= 4 && &data[0..4] == b"PK\x03\x04" {
            return "application/zip".to_string();
        } else if is_likely_markdown(data) {
            return "text/markdown".to_string();
        } else if is_utf8_text(data) && data.len() > 0 && data[0] == b'{' {
            return "application/json".to_string();
        } else if is_utf8_text(data) {
            return "text/plain".to_string();
        }
    }
    ct
}

fn is_likely_markdown(data: &[u8]) -> bool {
    if !is_utf8_text(data) { return false; }
    let s = std::str::from_utf8(data).unwrap_or("");
    let lines: Vec<&str> = s.lines().take(10).collect();
    for line in &lines {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.starts_with("## ") || trimmed.starts_with("### ")
            || trimmed.starts_with('-') || trimmed.starts_with('*')
            || trimmed.starts_with('>') || trimmed.starts_with("```")
            || trimmed.starts_with('|')
        {
            return true;
        }
    }
    false
}

fn is_utf8_text(data: &[u8]) -> bool {
    std::str::from_utf8(data).is_ok()
}

// ---------------------------------------------------------------------------
// Tauri Commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn content_library_fetch_cid(cid: String, gateway_index: Option<usize>) -> Result<IpfsFetchResult, String> {
    let cid = cid.trim().to_string();
    validate_import_cid(&cid)?;

    let cache_meta_path = cid_meta_path(&cid)?;
    if cache_meta_path.exists() {
        return Err("Content is already cached. Use refresh to re-fetch.".to_string());
    }

    let idx = gateway_index.unwrap_or(0);
    let gateways = DEFAULT_GATEWAYS;
    if idx >= gateways.len() {
        return Err(format!("Invalid gateway index (max {})", gateways.len().saturating_sub(1)));
    }

    let mut last_error = String::new();
    for offset in 0..gateways.len() {
        let gidx = (idx + offset) % gateways.len();
        let gw = gateways[gidx];
        match fetch_from_gateway(&cid, gw) {
            Ok((content_type, data)) => {
                let final_type = guess_content_type_priority(&content_type, &data, &cid);

                let local_path = save_cache_content(&cid, &data)?;
                let local_path_str = local_path.to_string_lossy().to_string();

                let now = Utc::now().to_rfc3339();

                let entry = CacheEntry {
                    cid: cid.clone(),
                    gateway_used: gw.to_string(),
                    content_type: final_type.clone(),
                    size_bytes: data.len() as u64,
                    fetched_at: now.clone(),
                    local_path: local_path_str,
                };
                save_cache_meta(&cid, &entry)?;

                let content_base64 = {
                    use base64::{Engine as _, engine::general_purpose::STANDARD};
                    STANDARD.encode(&data)
                };

                return Ok(IpfsFetchResult {
                    cid: cid.clone(),
                    gateway_used: gw.to_string(),
                    content_type: final_type,
                    size_bytes: data.len() as u64,
                    fetched_at: now,
                    local_path: entry.local_path.clone(),
                    content_base64,
                });
            }
            Err(e) => {
                last_error = e;
            }
        }
    }

    Err(format!("All gateways failed. Last error: {}", last_error))
}

#[tauri::command]
pub fn content_library_get_cached(cid: String) -> Result<IpfsFetchResult, String> {
    let cid = cid.trim().to_string();
    validate_import_cid(&cid)?;

    let entry = load_cache_meta(&cid)?;
    let content_path = cid_content_path(&cid)?;
    if !content_path.exists() {
        return Err("Cache content file missing. Re-fetch required.".to_string());
    }

    let data = fs::read(&content_path).map_err(|e| e.to_string())?;

    let content_base64 = {
        use base64::{Engine as _, engine::general_purpose::STANDARD};
        STANDARD.encode(&data)
    };

    Ok(IpfsFetchResult {
        cid,
        gateway_used: entry.gateway_used,
        content_type: entry.content_type,
        size_bytes: entry.size_bytes,
        fetched_at: entry.fetched_at,
        local_path: entry.local_path,
        content_base64,
    })
}

#[tauri::command]
pub fn content_library_refresh_cached(cid: String, gateway_index: Option<usize>) -> Result<IpfsFetchResult, String> {
    let cid = cid.trim().to_string();
    validate_import_cid(&cid)?;

    let cache_entry_dir = cid_cache_dir(&cid)?;
    if cache_entry_dir.exists() {
        fs::remove_dir_all(&cache_entry_dir).map_err(|e| e.to_string())?;
    }

    content_library_fetch_cid(cid, gateway_index)
}

#[tauri::command]
pub fn content_library_has_cache(cid: String) -> Result<bool, String> {
    let cid = cid.trim().to_string();
    validate_import_cid(&cid)?;
    Ok(cid_meta_path(&cid)?.exists())
}

#[tauri::command]
pub fn content_library_cache_status() -> Result<CacheStatus, String> {
    let dir = cache_dir()?;
    let mut entries = Vec::new();
    let mut total_size = 0u64;

    if dir.exists() {
        for entry in fs::read_dir(&dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                let meta_path = entry.path().join("meta.json");
                if meta_path.exists() {
                    if let Ok(content) = fs::read_to_string(&meta_path) {
                        if let Ok(ce) = serde_json::from_str::<CacheEntry>(&content) {
                            total_size += ce.size_bytes;
                            entries.push(ce);
                        }
                    }
                }
            }
        }
    }

    entries.sort_by(|a, b| b.fetched_at.cmp(&a.fetched_at));

    Ok(CacheStatus {
        cache_dir: dir.to_string_lossy().to_string(),
        entry_count: entries.len(),
        total_size_bytes: total_size,
        entries,
    })
}

#[tauri::command]
pub fn content_library_clear_cache() -> Result<(), String> {
    let dir = cache_dir()?;
    if dir.exists() {
        fs::remove_dir_all(&dir).map_err(|e| format!("Failed to clear cache: {}", e))?;
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn content_library_get_cache_dir() -> Result<String, String> {
    cache_dir().map(|p| p.to_string_lossy().to_string())
}

#[tauri::command]
pub fn content_library_default_gateways() -> Result<Vec<String>, String> {
    Ok(DEFAULT_GATEWAYS.iter().map(|s| s.to_string()).collect())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(dead_code)]
    fn test_cache_dir() -> PathBuf {
        let dir = std::env::temp_dir().join("ipfs_cache_test");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).ok();
        dir
    }

    #[test]
    fn safe_cid_dirname_sanitizes_special_chars() {
        let result = safe_cid_dirname("Qmabc/..\\test");
        assert!(!result.contains('/'));
        assert!(!result.contains('\\'));
        assert!(!result.contains(".."));
    }

    #[test]
    fn safe_cid_dirname_preserves_alphanumeric() {
        let result = safe_cid_dirname("QmTest123");
        assert_eq!(result, "QmTest123");
    }

    #[test]
    fn guess_content_type_detects_png_magic() {
        let data = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        let ct = guess_content_type_priority("application/octet-stream", &data, "");
        assert_eq!(ct, "image/png");
    }

    #[test]
    fn guess_content_type_detects_jpeg_magic() {
        let data = vec![0xFF, 0xD8, 0xFF, 0xE0];
        let ct = guess_content_type_priority("application/octet-stream", &data, "");
        assert_eq!(ct, "image/jpeg");
    }

    #[test]
    fn guess_content_type_detects_pdf_magic() {
        let data = vec![0x25, 0x50, 0x44, 0x46];
        let ct = guess_content_type_priority("application/octet-stream", &data, "");
        assert_eq!(ct, "application/pdf");
    }

    #[test]
    fn guess_content_type_detects_json_from_content() {
        let data = br#"{"key": "value"}"#.to_vec();
        let ct = guess_content_type_priority("text/plain", &data, "");
        assert_eq!(ct, "application/json");
    }

    #[test]
    fn guess_content_type_detects_markdown() {
        let data = b"# Heading\n\nSome text\n- list item".to_vec();
        let ct = guess_content_type_priority("text/plain", &data, "");
        assert_eq!(ct, "text/markdown");
    }

    #[test]
    fn guess_content_type_passes_through_known_mime() {
        let data = vec![0u8; 10];
        let ct = guess_content_type_priority("image/gif", &data, "");
        assert_eq!(ct, "image/gif");
    }

    #[test]
    fn is_likely_markdown_detects_headers() {
        assert!(is_likely_markdown(b"# Title"));
        assert!(is_likely_markdown(b"## Section"));
        assert!(is_likely_markdown(b"- list item"));
        assert!(is_likely_markdown(b"* bullet"));
        assert!(is_likely_markdown(b"> quote"));
    }

    #[test]
    fn is_likely_markdown_rejects_binary() {
        assert!(!is_likely_markdown(&[0x00, 0x01, 0xFF]));
    }

    #[test]
    fn is_utf8_text_validates_encoding() {
        assert!(is_utf8_text(b"hello world"));
        assert!(is_utf8_text("こんにちは".as_bytes()));
        assert!(!is_utf8_text(&[0xFF, 0xFE, 0x00]));
    }

    #[test]
    fn cache_entry_serialization_roundtrip() {
        let entry = CacheEntry {
            cid: "QmTest123".to_string(),
            gateway_used: "https://dweb.link/ipfs/".to_string(),
            content_type: "image/png".to_string(),
            size_bytes: 5000,
            fetched_at: "2026-05-28T00:00:00Z".to_string(),
            local_path: "/some/path".to_string(),
        };
        let json = serde_json::to_string(&entry).unwrap();
        let parsed: CacheEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.cid, entry.cid);
        assert_eq!(parsed.content_type, entry.content_type);
        assert_eq!(parsed.size_bytes, entry.size_bytes);
    }

    #[test]
    fn ipfs_fetch_result_serialization() {
        let result = IpfsFetchResult {
            cid: "QmTest".to_string(),
            gateway_used: "https://ipfs.io/ipfs/".to_string(),
            content_type: "text/markdown".to_string(),
            size_bytes: 100,
            fetched_at: "2026-05-28T00:00:00Z".to_string(),
            local_path: "/tmp/test".to_string(),
            content_base64: "aGVsbG8=".to_string(),
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("QmTest"));
        assert!(json.contains("text/markdown"));
        assert!(json.contains("aGVsbG8="));
    }

    #[test]
    fn cache_status_empty() {
        let status = CacheStatus {
            cache_dir: "/tmp".to_string(),
            entry_count: 0,
            total_size_bytes: 0,
            entries: vec![],
        };
        assert_eq!(status.entry_count, 0);
        assert_eq!(status.total_size_bytes, 0);
    }

    #[test]
    fn cache_status_with_entries() {
        let entries = vec![
            CacheEntry {
                cid: "cid1".to_string(),
                gateway_used: "gw1".to_string(),
                content_type: "text/plain".to_string(),
                size_bytes: 100,
                fetched_at: "2026-01-01T00:00:00Z".to_string(),
                local_path: "/p1".to_string(),
            },
        ];
        let status = CacheStatus {
            cache_dir: "/tmp".to_string(),
            entry_count: 1,
            total_size_bytes: 100,
            entries,
        };
        assert_eq!(status.entry_count, 1);
        assert_eq!(status.total_size_bytes, 100);
    }
}
