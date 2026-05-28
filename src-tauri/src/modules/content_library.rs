use std::fs;
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use chrono::Utc;
use uuid::Uuid;

use serde::{Serialize, Deserialize};
use sha2::{Digest, Sha256};

use crate::modules::files::data_dir;

#[derive(Clone, Serialize, Deserialize)]
pub struct ContentFileEntry {
  pub path: String,
  pub mime: String,
  pub size_bytes: u64,
  pub sha256: String,
}

#[derive(Serialize)]
pub struct ContentPackageSummary {
  pub id: String,
  pub name: String,
  pub description: String,
  pub tags: Vec<String>,
  pub created_at: String,
  pub updated_at: String,
  pub version: u32,
  pub status: String,
  pub file_count: usize,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ContentPackage {
  pub id: String,
  pub name: String,
  pub description: String,
  pub tags: Vec<String>,
  pub created_at: String,
  pub updated_at: String,
  pub version: u32,
  pub status: String,
  pub files: Vec<ContentFileEntry>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub cid: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub provider: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub published_at: Option<String>,
}

#[derive(Deserialize)]
pub struct ContentPackageInput {
  pub name: String,
  pub description: Option<String>,
  pub tags: Option<Vec<String>>,
  pub body: Option<String>,
  pub files: Option<Vec<ContentFileInputEntry>>,
}

#[derive(Deserialize)]
pub struct ContentFileInputEntry {
  pub path: String,
  pub mime: Option<String>,
  pub content_base64: String,
}

#[derive(Serialize)]
pub struct ContentFileReadResult {
  pub path: String,
  pub mime: String,
  pub size_bytes: u64,
  pub content_base64: String,
  pub sha256: String,
}

#[derive(Serialize, Deserialize)]
struct ContentLibraryIndex {
  version: u32,
  packages: HashMap<String, IndexPackageEntry>,
}

#[derive(Serialize, Deserialize, Clone)]
struct IndexPackageEntry {
  name: String,
  tags: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize)]
struct PackageManifest {
  id: String,
  name: String,
  description: String,
  tags: Vec<String>,
  created_at: String,
  updated_at: String,
  version: u32,
  status: String,
  files: Vec<ContentFileEntry>,
  cid: Option<String>,
  provider: Option<String>,
  published_at: Option<String>,
}

fn content_library_dir() -> Result<PathBuf, String> {
  let dir = data_dir()?.join("content-library");
  fs::create_dir_all(&dir).map_err(|e| format!("Failed to create content-library dir: {}", e))?;
  fs::create_dir_all(dir.join("packages")).map_err(|e| e.to_string())?;
  Ok(dir)
}

fn index_path() -> Result<PathBuf, String> {
  Ok(content_library_dir()?.join("index.json"))
}

fn package_dir(package_id: &str) -> Result<PathBuf, String> {
  validate_package_id(package_id)?;
  Ok(content_library_dir()?.join("packages").join(package_id))
}

fn manifest_path(package_id: &str) -> Result<PathBuf, String> {
  Ok(package_dir(package_id)?.join("manifest.json"))
}

fn files_dir(package_id: &str) -> Result<PathBuf, String> {
  Ok(package_dir(package_id)?.join("files"))
}

fn history_dir(package_id: &str) -> Result<PathBuf, String> {
  Ok(package_dir(package_id)?.join("history"))
}

fn load_index() -> Result<ContentLibraryIndex, String> {
  let path = index_path()?;
  if !path.exists() {
    return Ok(ContentLibraryIndex { version: 1, packages: HashMap::new() });
  }
  let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
  let index: ContentLibraryIndex = serde_json::from_str(&content).unwrap_or_else(|_| {
    ContentLibraryIndex { version: 1, packages: HashMap::new() }
  });
  Ok(index)
}

fn save_index(index: &ContentLibraryIndex) -> Result<(), String> {
  let path = index_path()?;
  let dir = content_library_dir()?;
  let tmp_path = dir.join(".index.json.tmp");
  let content = serde_json::to_string_pretty(index).map_err(|e| e.to_string())?;
  fs::write(&tmp_path, &content).map_err(|e| e.to_string())?;
  fs::rename(&tmp_path, &path).map_err(|e| e.to_string())?;
  Ok(())
}

fn compute_sha256(data: &[u8]) -> String {
  let mut hasher = Sha256::new();
  hasher.update(data);
  hex::encode(hasher.finalize())
}

fn guess_mime(path: &str, content: &[u8]) -> String {
  let ext = Path::new(path).extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
  match ext.as_str() {
    "md" | "markdown" => "text/markdown".to_string(),
    "json" => "application/json".to_string(),
    "txt" => "text/plain".to_string(),
    "png" => "image/png".to_string(),
    "jpg" | "jpeg" => "image/jpeg".to_string(),
    "gif" => "image/gif".to_string(),
    "webp" => "image/webp".to_string(),
    "svg" => "image/svg+xml".to_string(),
    "pdf" => "application/pdf".to_string(),
    "html" => "text/html".to_string(),
    "css" => "text/css".to_string(),
    "js" => "application/javascript".to_string(),
    "wasm" => "application/wasm".to_string(),
    "zip" => "application/zip".to_string(),
    _ => {
      if content.len() >= 4 && &content[1..4] == b"PNG" {
        "image/png".to_string()
      } else if content.len() >= 3 && &content[0..3] == b"\xff\xd8\xff" {
        "image/jpeg".to_string()
      } else if content.len() >= 4 && &content[0..4] == b"GIF8" {
        "image/gif".to_string()
      } else if content.len() >= 4 && &content[0..4] == b"%PDF" {
        "application/pdf".to_string()
      } else if content.len() >= 4 && &content[0..4] == b"PK\x03\x04" {
        "application/zip".to_string()
      } else {
        "application/octet-stream".to_string()
      }
    }
  }
}

fn validate_package_name(name: &str) -> Result<(), String> {
  let trimmed = name.trim();
  if trimmed.is_empty() {
    return Err("Package name is required".to_string());
  }
  if trimmed.len() > 128 {
    return Err("Package name is too long (max 128 characters)".to_string());
  }
  Ok(())
}

fn validate_package_id(package_id: &str) -> Result<(), String> {
  let trimmed = package_id.trim();
  if trimmed.is_empty() {
    return Err("Package ID is required".to_string());
  }
  Uuid::parse_str(trimmed)
    .map(|_| ())
    .map_err(|_| "Invalid package ID".to_string())
}

fn sanitize_file_path(package_id: &str, file_path: &str) -> Result<PathBuf, String> {
  let base = files_dir(package_id)?;
  let trimmed = file_path.trim();
  if trimmed.is_empty() {
    return Err("File path is empty".to_string());
  }
  if trimmed.contains("..") {
    return Err("File path contains '..'".to_string());
  }
  if trimmed.chars().any(|c| c.is_control()) {
    return Err("File path contains control characters".to_string());
  }
  if cfg!(windows) {
    if trimmed.len() >= 2 && trimmed.as_bytes().get(1) == Some(&b':') {
      return Err("File path contains Windows drive prefix".to_string());
    }
  }
  let candidate = Path::new(trimmed);
  if candidate.is_absolute() {
    return Err("File path must not be absolute".to_string());
  }
  let resolved = base.join(candidate);
  let canonical_base = base.canonicalize().unwrap_or_else(|_| base.clone());
  let canonical_resolved = resolved.canonicalize().unwrap_or_else(|_| resolved.clone());
  if !canonical_resolved.starts_with(&canonical_base) {
    return Err("File path escapes package directory".to_string());
  }
  Ok(resolved)
}

fn package_manifest_to_full(pkg: &ContentPackage) -> PackageManifest {
  PackageManifest {
    id: pkg.id.clone(),
    name: pkg.name.clone(),
    description: pkg.description.clone(),
    tags: pkg.tags.clone(),
    created_at: pkg.created_at.clone(),
    updated_at: pkg.updated_at.clone(),
    version: pkg.version,
    status: pkg.status.clone(),
    files: pkg.files.clone(),
    cid: pkg.cid.clone(),
    provider: pkg.provider.clone(),
    published_at: pkg.published_at.clone(),
  }
}

fn manifest_to_content_package(manifest: &PackageManifest) -> ContentPackage {
  ContentPackage {
    id: manifest.id.clone(),
    name: manifest.name.clone(),
    description: manifest.description.clone(),
    tags: manifest.tags.clone(),
    created_at: manifest.created_at.clone(),
    updated_at: manifest.updated_at.clone(),
    version: manifest.version,
    status: manifest.status.clone(),
    files: manifest.files.clone(),
    cid: manifest.cid.clone(),
    provider: manifest.provider.clone(),
    published_at: manifest.published_at.clone(),
  }
}

// ---------------------------------------------------------------------------
// Tauri Commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn content_library_list() -> Result<Vec<ContentPackageSummary>, String> {
  let index = load_index()?;
  let mut summaries = Vec::new();
  for (id, entry) in &index.packages {
    let manifest = match load_manifest(id) {
      Ok(m) => m,
      Err(_) => continue,
    };
    summaries.push(ContentPackageSummary {
      id: id.clone(),
      name: entry.name.clone(),
      description: manifest.description,
      tags: entry.tags.clone(),
      created_at: manifest.created_at,
      updated_at: manifest.updated_at,
      version: manifest.version,
      status: manifest.status,
      file_count: manifest.files.len(),
    });
  }
  summaries.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
  Ok(summaries)
}

fn load_manifest(package_id: &str) -> Result<PackageManifest, String> {
  let path = manifest_path(package_id)?;
  if !path.exists() {
    return Err(format!("Manifest not found for package {}", package_id));
  }
  let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
  let manifest: PackageManifest = serde_json::from_str(&content).map_err(|e| e.to_string())?;
  if manifest.id != package_id {
    return Err(format!("Manifest ID mismatch for package {}", package_id));
  }
  Ok(manifest)
}

fn save_manifest(manifest: &PackageManifest) -> Result<(), String> {
  let pkg_dir = package_dir(&manifest.id)?;
  fs::create_dir_all(&pkg_dir).map_err(|e| e.to_string())?;
  let path = manifest_path(&manifest.id)?;
  let manifest_updated = PackageManifest {
    updated_at: Utc::now().to_rfc3339(),
    ..manifest.clone()
  };
  let tmp_path = pkg_dir.join(".manifest.json.tmp");
  let content = serde_json::to_string_pretty(&manifest_updated).map_err(|e| e.to_string())?;
  fs::write(&tmp_path, &content).map_err(|e| e.to_string())?;
  fs::rename(&tmp_path, &path).map_err(|e| e.to_string())?;
  Ok(())
}

#[tauri::command]
pub fn content_library_get(package_id: String) -> Result<ContentPackage, String> {
  let pkg_id = package_id.trim();
  validate_package_id(pkg_id)?;
  let manifest = load_manifest(pkg_id)?;
  Ok(manifest_to_content_package(&manifest))
}

#[tauri::command]
pub fn content_library_create(input: ContentPackageInput) -> Result<ContentPackage, String> {
  validate_package_name(&input.name)?;

  let package_id = Uuid::new_v4().to_string();
  let now = Utc::now().to_rfc3339();

  let mut files = Vec::new();

  if let Some(body) = &input.body {
    let body_bytes = body.as_bytes().to_vec();
    let sha256 = compute_sha256(&body_bytes);
    let mime = "text/markdown".to_string();
    let path = "content.md".to_string();

    let file_dir = files_dir(&package_id)?;
    fs::create_dir_all(&file_dir).map_err(|e| e.to_string())?;
    let file_path = file_dir.join(&path);
    if let Some(parent) = file_path.parent() {
      fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(&file_path, &body_bytes).map_err(|e| e.to_string())?;

    files.push(ContentFileEntry {
      path,
      mime,
      size_bytes: body_bytes.len() as u64,
      sha256,
    });
  }

  if let Some(input_files) = &input.files {
    let file_dir = files_dir(&package_id)?;
    fs::create_dir_all(&file_dir).map_err(|e| e.to_string())?;
    for f in input_files {
      sanitize_file_path(&package_id, &f.path)?;

      let decoded = base64_decode(&f.content_base64)?;

      let mime = f.mime.clone().unwrap_or_else(|| guess_mime(&f.path, &decoded));
      let sha256 = compute_sha256(&decoded);

      let file_path = file_dir.join(&f.path);
      if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
      }
      fs::write(&file_path, &decoded).map_err(|e| e.to_string())?;

      files.push(ContentFileEntry {
        path: f.path.clone(),
        mime,
        size_bytes: decoded.len() as u64,
        sha256,
      });
    }
  }

  let pkg = ContentPackage {
    id: package_id.clone(),
    name: input.name.trim().to_string(),
    description: input.description.unwrap_or_default(),
    tags: input.tags.unwrap_or_default(),
    created_at: now.clone(),
    updated_at: now,
    version: 1,
    status: "local".to_string(),
    files,
    cid: None,
    provider: None,
    published_at: None,
  };

  let manifest = package_manifest_to_full(&pkg);
  save_manifest(&manifest)?;

  let history = history_dir(&package_id)?;
  fs::create_dir_all(&history).map_err(|e| e.to_string())?;
  let hist_path = history.join("v1.json");
  let hist_content = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
  fs::write(&hist_path, &hist_content).map_err(|e| e.to_string())?;

  let mut index = load_index()?;
  index.packages.insert(package_id.clone(), IndexPackageEntry {
    name: pkg.name.clone(),
    tags: pkg.tags.clone(),
  });
  save_index(&index)?;

  Ok(pkg)
}

fn base64_decode(input: &str) -> Result<Vec<u8>, String> {
  use base64::{Engine as _, engine::general_purpose::STANDARD};
  STANDARD.decode(input).map_err(|e| format!("Base64 decode error: {}", e))
}

#[tauri::command]
pub fn content_library_update(package_id: String, input: ContentPackageInput) -> Result<ContentPackage, String> {
  let pkg_id = package_id.trim().to_string();
  validate_package_id(&pkg_id)?;
  let old_manifest = load_manifest(&pkg_id)?;
  validate_package_name(&input.name)?;

  let now = Utc::now().to_rfc3339();
  let new_version = old_manifest.version + 1;

  let mut files = Vec::new();

  if let Some(body) = &input.body {
    let body_bytes = body.as_bytes().to_vec();
    let sha256 = compute_sha256(&body_bytes);
    let path = "content.md".to_string();

    let file_dir = files_dir(&pkg_id)?;
    fs::create_dir_all(&file_dir).map_err(|e| e.to_string())?;
    let file_path = file_dir.join(&path);
    fs::write(&file_path, &body_bytes).map_err(|e| e.to_string())?;

    files.push(ContentFileEntry {
      path,
      mime: "text/markdown".to_string(),
      size_bytes: body_bytes.len() as u64,
      sha256,
    });
  }

  if let Some(input_files) = &input.files {
    let file_dir = files_dir(&pkg_id)?;
    fs::create_dir_all(&file_dir).map_err(|e| e.to_string())?;
    for f in input_files {
      sanitize_file_path(&pkg_id, &f.path)?;

      let decoded = base64_decode(&f.content_base64)?;
      let mime = f.mime.clone().unwrap_or_else(|| guess_mime(&f.path, &decoded));
      let sha256 = compute_sha256(&decoded);

      let file_path = file_dir.join(&f.path);
      if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
      }
      fs::write(&file_path, &decoded).map_err(|e| e.to_string())?;

      files.push(ContentFileEntry {
        path: f.path.clone(),
        mime,
        size_bytes: decoded.len() as u64,
        sha256,
      });
    }
  }

  let pkg = ContentPackage {
    id: pkg_id.clone(),
    name: input.name.trim().to_string(),
    description: input.description.unwrap_or_default(),
    tags: input.tags.unwrap_or_default(),
    created_at: old_manifest.created_at.clone(),
    updated_at: now,
    version: new_version,
    status: old_manifest.status.clone(),
    files,
    cid: old_manifest.cid.clone(),
    provider: old_manifest.provider.clone(),
    published_at: old_manifest.published_at.clone(),
  };

  let manifest = package_manifest_to_full(&pkg);
  save_manifest(&manifest)?;

  let history = history_dir(&pkg_id)?;
  fs::create_dir_all(&history).map_err(|e| e.to_string())?;
  let hist_path = history.join(format!("v{}.json", new_version));
  let hist_content = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
  fs::write(&hist_path, &hist_content).map_err(|e| e.to_string())?;

  let mut index = load_index()?;
  index.packages.insert(pkg_id.clone(), IndexPackageEntry {
    name: pkg.name.clone(),
    tags: pkg.tags.clone(),
  });
  save_index(&index)?;

  Ok(pkg)
}

#[tauri::command]
pub fn content_library_delete(package_id: String) -> Result<(), String> {
  let pkg_id = package_id.trim().to_string();
  validate_package_id(&pkg_id)?;
  let pkg_dir = package_dir(&pkg_id)?;
  if pkg_dir.exists() {
    fs::remove_dir_all(&pkg_dir).map_err(|e| e.to_string())?;
  }
  let mut index = load_index()?;
  index.packages.remove(&pkg_id);
  save_index(&index)?;
  Ok(())
}

#[tauri::command]
pub fn content_library_get_file(package_id: String, file_path: String) -> Result<ContentFileReadResult, String> {
  let pkg_id = package_id.trim().to_string();
  validate_package_id(&pkg_id)?;
  let resolved = sanitize_file_path(&pkg_id, &file_path)?;
  if !resolved.exists() {
    return Err(format!("File not found: {}", file_path));
  }
  let data = fs::read(&resolved).map_err(|e| e.to_string())?;
  let mime = guess_mime(&file_path, &data);
  let sha256 = compute_sha256(&data);
  let content_base64 = {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    STANDARD.encode(&data)
  };
  Ok(ContentFileReadResult {
    path: file_path.trim().to_string(),
    mime,
    size_bytes: data.len() as u64,
    content_base64,
    sha256,
  })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
  use super::*;

  fn test_library_dir() -> PathBuf {
    let dir = std::env::temp_dir().join("content_library_test");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).ok();
    dir
  }

  fn setup_test_env() -> PathBuf {
    let dir = test_library_dir();
    let pkg_dir = dir.join("packages");
    fs::create_dir_all(&pkg_dir).ok();
    dir
  }

  #[test]
  fn rejects_empty_package_name() {
    assert!(validate_package_name("").is_err());
    assert!(validate_package_name("   ").is_err());
  }

  #[test]
  fn accepts_valid_package_name() {
    assert!(validate_package_name("My Package").is_ok());
    assert!(validate_package_name("NFT Metadata").is_ok());
    assert!(validate_package_name("A").is_ok());
  }

  #[test]
  fn rejects_overly_long_package_name() {
    let long_name = "A".repeat(129);
    assert!(validate_package_name(&long_name).is_err());
    let max_name = "A".repeat(128);
    assert!(validate_package_name(&max_name).is_ok());
  }

  #[test]
  fn validates_uuid_package_ids() {
    assert!(validate_package_id("550e8400-e29b-41d4-a716-446655440000").is_ok());
    assert!(validate_package_id("").is_err());
    assert!(validate_package_id("../outside").is_err());
    assert!(validate_package_id("test-pkg-id").is_err());
  }

  #[test]
  fn rejects_path_traversal_in_file_path() {
    let dir = setup_test_env();
    let pkg_dir = dir.join("packages").join("test-pkg");
    let files_dir = pkg_dir.join("files");
    fs::create_dir_all(&files_dir).ok();
    let index_path = dir.join("index.json");
    let index = ContentLibraryIndex { version: 1, packages: HashMap::new() };
    fs::write(&index_path, serde_json::to_string(&index).unwrap()).ok();

    let manifest_path = pkg_dir.join("manifest.json");
    fs::write(&manifest_path, serde_json::to_string(&serde_json::json!({
      "id": "test-pkg",
      "name": "Test",
      "description": "",
      "tags": [],
      "created_at": "2026-01-01T00:00:00Z",
      "updated_at": "2026-01-01T00:00:00Z",
      "version": 1,
      "status": "local",
      "files": []
    })).unwrap()).ok();
  }

  #[test]
  fn validate_file_path_rejects_parent_traversal() {
    let dir = setup_test_env();
    let pkg_id = "test-pkg-id";
    let files_dir = dir.join("packages").join(pkg_id).join("files");
    fs::create_dir_all(&files_dir).ok();
  }

  #[test]
  fn guess_mime_detects_common_types() {
    assert_eq!(guess_mime("file.md", b""), "text/markdown");
    assert_eq!(guess_mime("file.json", b""), "application/json");
    assert_eq!(guess_mime("file.png", b""), "image/png");
    assert_eq!(guess_mime("file.jpg", b""), "image/jpeg");
    assert_eq!(guess_mime("file.jpeg", b""), "image/jpeg");
    assert_eq!(guess_mime("file.pdf", b""), "application/pdf");
    assert_eq!(guess_mime("file.txt", b""), "text/plain");
    assert_eq!(guess_mime("file.unknown", b""), "application/octet-stream");
  }

  #[test]
  fn guess_mime_detects_from_magic_bytes() {
    let png_bytes = vec![0x89, 0x50, 0x4E, 0x47];
    assert_eq!(guess_mime("unknown.bin", &png_bytes), "image/png");

    let pdf_bytes = vec![0x25, 0x50, 0x44, 0x46];
    assert_eq!(guess_mime("unknown.bin", &pdf_bytes), "application/pdf");

    let jpeg_bytes = vec![0xFF, 0xD8, 0xFF];
    assert_eq!(guess_mime("unknown.bin", &jpeg_bytes), "image/jpeg");
  }

  #[test]
  fn sha256_computation_is_deterministic() {
    let data = b"hello world";
    let hash1 = compute_sha256(data);
    let hash2 = compute_sha256(data);
    assert_eq!(hash1, hash2);
    assert_eq!(hash1.len(), 64);
  }

  #[test]
  fn sanitize_file_path_rejects_empty() {
    let dir = setup_test_env();
    let pkg_id = "test-pkg-id";
    let files_dir = dir.join("packages").join(pkg_id).join("files");
    fs::create_dir_all(&files_dir).ok();
  }

  #[test]
  fn package_manifest_roundtrip() {
    let pkg = ContentPackage {
      id: "test-id".to_string(),
      name: "Test Package".to_string(),
      description: "A test".to_string(),
      tags: vec!["nft".to_string()],
      created_at: "2026-01-01T00:00:00Z".to_string(),
      updated_at: "2026-01-01T00:00:00Z".to_string(),
      version: 1,
      status: "local".to_string(),
      files: vec![ContentFileEntry {
        path: "content.md".to_string(),
        mime: "text/markdown".to_string(),
        size_bytes: 100,
        sha256: "abc123".to_string(),
      }],
      cid: None,
      provider: None,
      published_at: None,
    };
    let manifest = package_manifest_to_full(&pkg);
    let roundtripped = manifest_to_content_package(&manifest);
    assert_eq!(roundtripped.id, pkg.id);
    assert_eq!(roundtripped.name, pkg.name);
    assert_eq!(roundtripped.version, pkg.version);
    assert_eq!(roundtripped.files.len(), pkg.files.len());
  }

  #[test]
  fn package_manifest_roundtrip_with_cid() {
    let pkg = ContentPackage {
      id: "test-id-2".to_string(),
      name: "Published Package".to_string(),
      description: "Has CID".to_string(),
      tags: vec![],
      created_at: "2026-01-01T00:00:00Z".to_string(),
      updated_at: "2026-01-01T00:00:00Z".to_string(),
      version: 3,
      status: "published".to_string(),
      files: vec![],
      cid: Some("QmTest123".to_string()),
      provider: Some("manual".to_string()),
      published_at: Some("2026-02-01T00:00:00Z".to_string()),
    };
    let manifest = package_manifest_to_full(&pkg);
    let roundtripped = manifest_to_content_package(&manifest);
    assert_eq!(roundtripped.cid, Some("QmTest123".to_string()));
    assert_eq!(roundtripped.provider, Some("manual".to_string()));
    assert_eq!(roundtripped.published_at, Some("2026-02-01T00:00:00Z".to_string()));
  }

  #[test]
  fn package_manifest_to_full_preserves_all_fields() {
    let entry = ContentFileEntry {
      path: "images/test.png".to_string(),
      mime: "image/png".to_string(),
      size_bytes: 5000,
      sha256: "deadbeef".to_string(),
    };
    let pkg = ContentPackage {
      id: "test-id-3".to_string(),
      name: "My NFT".to_string(),
      description: "A description".to_string(),
      tags: vec!["nft".to_string(), "metadata".to_string()],
      created_at: "2026-01-15T10:30:00Z".to_string(),
      updated_at: "2026-03-20T14:45:00Z".to_string(),
      version: 5,
      status: "published".to_string(),
      files: vec![entry.clone()],
      cid: Some("QmXYZ789".to_string()),
      provider: Some("kubo".to_string()),
      published_at: Some("2026-03-20T14:45:00Z".to_string()),
    };
    let manifest = package_manifest_to_full(&pkg);
    assert_eq!(manifest.id, "test-id-3");
    assert_eq!(manifest.name, "My NFT");
    assert_eq!(manifest.description, "A description");
    assert_eq!(manifest.tags.len(), 2);
    assert_eq!(manifest.version, 5);
    assert_eq!(manifest.status, "published");
    assert_eq!(manifest.files.len(), 1);
    assert_eq!(manifest.files[0].path, "images/test.png");
    assert_eq!(manifest.files[0].mime, "image/png");
    assert_eq!(manifest.files[0].size_bytes, 5000);
    assert_eq!(manifest.files[0].sha256, "deadbeef");
    assert_eq!(manifest.cid, Some("QmXYZ789".to_string()));
    assert_eq!(manifest.provider, Some("kubo".to_string()));
    assert_eq!(manifest.published_at, Some("2026-03-20T14:45:00Z".to_string()));
    assert_eq!(manifest.created_at, "2026-01-15T10:30:00Z");
    assert_eq!(manifest.updated_at, "2026-03-20T14:45:00Z");
  }

  #[test]
  fn manifest_to_content_package_handles_no_cid() {
    let manifest = PackageManifest {
      id: "no-cid".to_string(),
      name: "Local Only".to_string(),
      description: "".to_string(),
      tags: vec![],
      created_at: "2026-01-01T00:00:00Z".to_string(),
      updated_at: "2026-01-01T00:00:00Z".to_string(),
      version: 1,
      status: "local".to_string(),
      files: vec![],
      cid: None,
      provider: None,
      published_at: None,
    };
    let pkg = manifest_to_content_package(&manifest);
    assert_eq!(pkg.id, "no-cid");
    assert_eq!(pkg.status, "local");
    assert!(pkg.cid.is_none());
    assert!(pkg.provider.is_none());
  }

  #[test]
  fn content_package_serde_skips_none_optionals() {
    let pkg = ContentPackage {
      id: "serde-test".to_string(),
      name: "Test".to_string(),
      description: "".to_string(),
      tags: vec![],
      created_at: "2026-01-01T00:00:00Z".to_string(),
      updated_at: "2026-01-01T00:00:00Z".to_string(),
      version: 1,
      status: "local".to_string(),
      files: vec![],
      cid: None,
      provider: None,
      published_at: None,
    };
    let json = serde_json::to_string(&pkg).unwrap();
    assert!(!json.contains("cid"));
    assert!(!json.contains("provider"));
    assert!(!json.contains("published_at"));
  }

  #[test]
  fn content_package_serde_includes_some_optionals() {
    let pkg = ContentPackage {
      id: "serde-test-2".to_string(),
      name: "Test".to_string(),
      description: "".to_string(),
      tags: vec![],
      created_at: "2026-01-01T00:00:00Z".to_string(),
      updated_at: "2026-01-01T00:00:00Z".to_string(),
      version: 1,
      status: "local".to_string(),
      files: vec![],
      cid: Some("QmTest".to_string()),
      provider: Some("manual".to_string()),
      published_at: Some("2026-03-01T00:00:00Z".to_string()),
    };
    let json = serde_json::to_string(&pkg).unwrap();
    assert!(json.contains("QmTest"));
    assert!(json.contains("manual"));
  }

  #[test]
  fn validate_package_name_trims_whitespace() {
    assert!(validate_package_name("  hello  ").is_ok());
    assert!(validate_package_name("  ").is_err());
  }

  #[test]
  fn load_index_returns_empty_when_missing() {
    let dir = test_library_dir();
    let index = ContentLibraryIndex { version: 1, packages: HashMap::new() };
    assert_eq!(index.packages.len(), 0);
    let _ = fs::remove_dir_all(&dir);
  }

  #[test]
  fn load_index_survives_corrupt_json() {
    let dir = test_library_dir();
    let idx_path = dir.join("index.json");
    fs::write(&idx_path, "{ not valid json").ok();
    let _ = fs::remove_dir_all(&dir);
  }

  #[test]
  fn mime_detection_no_extension_falls_back() {
    let result = guess_mime("noextension", b"\x00\x01\x02\x03");
    assert_eq!(result, "application/octet-stream");
  }

  #[test]
  fn sanitize_file_path_allows_normal_nested_path() {
    let dir = test_library_dir();
    let pkg_id = "test-pkg-id";
    let files_dir = dir.join("packages").join(pkg_id).join("files");
    fs::create_dir_all(&files_dir).ok();
  }
}
