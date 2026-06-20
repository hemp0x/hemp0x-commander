use chrono::Utc;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::modules::files::{commander_content_library_dir, data_dir};

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    pub folder: String,
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
    pub folder: String,
}

#[derive(Deserialize)]
pub struct ContentPackageInput {
    pub name: String,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
    pub body: Option<String>,
    pub files: Option<Vec<ContentFileInputEntry>>,
    pub folder: Option<String>,
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
pub(crate) struct ContentLibraryIndex {
    pub(crate) version: u32,
    pub(crate) packages: HashMap<String, IndexPackageEntry>,
    #[serde(default)]
    pub(crate) folders: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct IndexPackageEntry {
    pub(crate) name: String,
    pub(crate) tags: Vec<String>,
    pub(crate) folder: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct PackageManifest {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) tags: Vec<String>,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
    pub(crate) version: u32,
    pub(crate) status: String,
    pub(crate) files: Vec<ContentFileEntry>,
    pub(crate) cid: Option<String>,
    pub(crate) provider: Option<String>,
    pub(crate) published_at: Option<String>,
    #[serde(default)]
    pub(crate) folder: String,
}

pub(crate) fn content_library_dir() -> Result<PathBuf, String> {
    let dir = commander_content_library_dir()?;
    let legacy = data_dir()?.join("content-library");
    if !dir.exists() && legacy.exists() {
        if let Some(parent) = dir.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::rename(&legacy, &dir)
            .or_else(|_| copy_dir_all(&legacy, &dir))
            .map_err(|e| format!("Failed to migrate content library: {}", e))?;
    }
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create content-library dir: {}", e))?;
    fs::create_dir_all(dir.join("packages")).map_err(|e| e.to_string())?;
    Ok(dir)
}

pub(crate) fn index_path() -> Result<PathBuf, String> {
    Ok(content_library_dir()?.join("index.json"))
}

pub(crate) fn package_dir(package_id: &str) -> Result<PathBuf, String> {
    validate_package_id(package_id)?;
    Ok(content_library_dir()?.join("packages").join(package_id))
}

pub(crate) fn manifest_path(package_id: &str) -> Result<PathBuf, String> {
    Ok(package_dir(package_id)?.join("manifest.json"))
}

pub(crate) fn files_dir(package_id: &str) -> Result<PathBuf, String> {
    Ok(package_dir(package_id)?.join("files"))
}

pub(crate) fn history_dir(package_id: &str) -> Result<PathBuf, String> {
    Ok(package_dir(package_id)?.join("history"))
}

pub(crate) fn load_index() -> Result<ContentLibraryIndex, String> {
    let path = index_path()?;
    if !path.exists() {
        return Ok(ContentLibraryIndex {
            version: 1,
            packages: HashMap::new(),
            folders: Vec::new(),
        });
    }
    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut index: ContentLibraryIndex =
        serde_json::from_str(&content).unwrap_or_else(|_| ContentLibraryIndex {
            version: 1,
            packages: HashMap::new(),
            folders: Vec::new(),
        });
    // Ensure folders list is deduped and sorted, and includes all folders from packages
    let mut folder_set: std::collections::HashSet<String> = index.folders.iter().cloned().collect();
    for entry in index.packages.values() {
        if !entry.folder.is_empty() {
            folder_set.insert(entry.folder.clone());
        }
    }
    let mut folders: Vec<String> = folder_set.into_iter().collect();
    folders.sort();
    index.folders = folders;
    Ok(index)
}

pub(crate) fn ensure_folder_in_index(index: &mut ContentLibraryIndex, folder: &str) {
    let clean = folder.trim();
    if clean.is_empty() {
        return;
    }
    if !index.folders.contains(&clean.to_string()) {
        index.folders.push(clean.to_string());
        index.folders.sort();
    }
}

pub(crate) fn save_index(index: &ContentLibraryIndex) -> Result<(), String> {
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
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
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

fn validate_folder(folder: &str) -> Result<String, String> {
    let trimmed = folder.trim();
    if trimmed.len() > 64 {
        return Err("Folder name is too long (max 64 characters)".to_string());
    }
    if trimmed.chars().any(|c| c.is_control()) {
        return Err("Folder name contains control characters".to_string());
    }
    if trimmed.contains("..") {
        return Err("Folder name contains '..'".to_string());
    }
    if trimmed.contains('/') || trimmed.contains('\\') {
        return Err("Folder name cannot contain path separators".to_string());
    }
    Ok(trimmed.to_string())
}

pub(crate) fn validate_package_id(package_id: &str) -> Result<(), String> {
    let trimmed = package_id.trim();
    if trimmed.is_empty() {
        return Err("Package ID is required".to_string());
    }
    Uuid::parse_str(trimmed)
        .map(|_| ())
        .map_err(|_| "Invalid package ID".to_string())
}

pub fn validate_import_cid(cid: &str) -> Result<(), String> {
    let trimmed = cid.trim();
    if trimmed.is_empty() {
        return Err("CID is required".to_string());
    }
    if trimmed.len() > 128 {
        return Err("CID is too long".to_string());
    }
    if trimmed.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err("CID contains whitespace or control characters".to_string());
    }

    let is_hex_64 = trimmed.len() == 64 && trimmed.chars().all(|c| c.is_ascii_hexdigit());
    let looks_like_cidv0 = trimmed.len() == 46 && trimmed.starts_with("Qm");
    let looks_like_cidv1 =
        trimmed.starts_with("bafy") || trimmed.starts_with("bafk") || trimmed.starts_with("bae");

    if is_hex_64 || looks_like_cidv0 || looks_like_cidv1 {
        Ok(())
    } else {
        Err(
            "CID must be a CIDv0 Qm..., CIDv1 bafy/bafk/bae..., or 64-character hex reference"
                .to_string(),
        )
    }
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

pub(crate) fn package_manifest_to_full(pkg: &ContentPackage) -> PackageManifest {
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
        folder: pkg.folder.clone(),
    }
}

pub(crate) fn manifest_to_content_package(manifest: &PackageManifest) -> ContentPackage {
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
        folder: manifest.folder.clone(),
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
            cid: manifest.cid.clone(),
            provider: manifest.provider.clone(),
            folder: manifest.folder.clone(),
        });
    }
    summaries.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(summaries)
}

pub(crate) fn load_manifest(package_id: &str) -> Result<PackageManifest, String> {
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

pub(crate) fn save_manifest(manifest: &PackageManifest) -> Result<(), String> {
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

            let mime = f
                .mime
                .clone()
                .unwrap_or_else(|| guess_mime(&f.path, &decoded));
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

    let folder = input
        .folder
        .as_ref()
        .map(|f| validate_folder(f))
        .transpose()?
        .unwrap_or_default();

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
        folder,
    };

    let manifest = package_manifest_to_full(&pkg);
    save_manifest(&manifest)?;

    let history = history_dir(&package_id)?;
    fs::create_dir_all(&history).map_err(|e| e.to_string())?;
    let hist_path = history.join("v1.json");
    let hist_content = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
    fs::write(&hist_path, &hist_content).map_err(|e| e.to_string())?;

    let mut index = load_index()?;
    index.packages.insert(
        package_id.clone(),
        IndexPackageEntry {
            name: pkg.name.clone(),
            tags: pkg.tags.clone(),
            folder: pkg.folder.clone(),
        },
    );
    ensure_folder_in_index(&mut index, &pkg.folder);
    save_index(&index)?;

    Ok(pkg)
}

fn base64_decode(input: &str) -> Result<Vec<u8>, String> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    STANDARD
        .decode(input)
        .map_err(|e| format!("Base64 decode error: {}", e))
}

#[tauri::command]
pub fn content_library_update(
    package_id: String,
    input: ContentPackageInput,
) -> Result<ContentPackage, String> {
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
            let mime = f
                .mime
                .clone()
                .unwrap_or_else(|| guess_mime(&f.path, &decoded));
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

    // Preserve files from old manifest that were not in the new input.
    // If neither body nor files are provided, keep all old files intact.
    let has_new_content_md = files.iter().any(|f| f.path == "content.md");
    let has_new_files = !files.is_empty();
    for old_file in &old_manifest.files {
        if old_file.path == "content.md" && has_new_content_md {
            continue;
        }
        if has_new_files && files.iter().any(|f| f.path == old_file.path) {
            continue;
        }
        if has_new_files && old_file.path != "content.md" {
            // new file list explicitly provided, exclude old files not in it
            continue;
        }
        files.push(old_file.clone());
    }

    let folder = input
        .folder
        .as_ref()
        .map(|f| validate_folder(f))
        .transpose()?
        .unwrap_or(old_manifest.folder.clone());

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
        folder,
    };

    let manifest = package_manifest_to_full(&pkg);
    save_manifest(&manifest)?;

    let history = history_dir(&pkg_id)?;
    fs::create_dir_all(&history).map_err(|e| e.to_string())?;
    let hist_path = history.join(format!("v{}.json", new_version));
    let hist_content = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
    fs::write(&hist_path, &hist_content).map_err(|e| e.to_string())?;

    let mut index = load_index()?;
    index.packages.insert(
        pkg_id.clone(),
        IndexPackageEntry {
            name: pkg.name.clone(),
            tags: pkg.tags.clone(),
            folder: pkg.folder.clone(),
        },
    );
    ensure_folder_in_index(&mut index, &pkg.folder);
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
pub fn content_library_get_file(
    package_id: String,
    file_path: String,
) -> Result<ContentFileReadResult, String> {
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
        use base64::{engine::general_purpose::STANDARD, Engine as _};
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
// CID Import
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct CidImportInput {
    pub cid: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
}

#[tauri::command]
pub fn content_library_import_cid(input: CidImportInput) -> Result<ContentPackage, String> {
    let cid = input.cid.trim().to_string();
    validate_import_cid(&cid)?;

    // check for duplicate CID
    let index = load_index()?;
    for (existing_id, _entry) in &index.packages {
        let manifest = match load_manifest(existing_id) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if let Some(ref existing_cid) = manifest.cid {
            if existing_cid == &cid {
                return Err(format!("CID already imported as package {}", existing_id));
            }
        }
    }

    let name = input
        .name
        .unwrap_or_else(|| format!("CID: {}", &cid[..cid.len().min(24)]));
    validate_package_name(&name)?;

    let package_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    let pkg = ContentPackage {
        id: package_id.clone(),
        name: name.trim().to_string(),
        description: input.description.unwrap_or_default(),
        tags: input.tags.unwrap_or_default(),
        created_at: now.clone(),
        updated_at: now,
        version: 1,
        status: "external".to_string(),
        files: vec![],
        cid: Some(cid.clone()),
        provider: Some("manual".to_string()),
        published_at: None,
        folder: "".to_string(),
    };

    let manifest = package_manifest_to_full(&pkg);
    save_manifest(&manifest)?;

    let history = history_dir(&package_id)?;
    fs::create_dir_all(&history).map_err(|e| e.to_string())?;
    let hist_path = history.join("v1.json");
    let hist_content = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
    fs::write(&hist_path, &hist_content).map_err(|e| e.to_string())?;

    let mut index = load_index()?;
    index.packages.insert(
        package_id.clone(),
        IndexPackageEntry {
            name: pkg.name.clone(),
            tags: pkg.tags.clone(),
            folder: pkg.folder.clone(),
        },
    );
    save_index(&index)?;

    Ok(pkg)
}

// ---------------------------------------------------------------------------
// Manual CID Link
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct CidLinkInput {
    pub package_id: String,
    pub cid: String,
    pub provider: Option<String>,
}

#[tauri::command]
pub fn content_library_link_cid(input: CidLinkInput) -> Result<ContentPackage, String> {
    let pkg_id = input.package_id.trim().to_string();
    validate_package_id(&pkg_id)?;
    let cid = input.cid.trim().to_string();
    validate_import_cid(&cid)?;

    let old_manifest = load_manifest(&pkg_id)?;

    let provider = input.provider.unwrap_or_else(|| "manual".to_string());
    let valid_providers = ["manual", "pinata", "installed_kubo", "filebase"];
    if !valid_providers.contains(&provider.as_str()) {
        return Err(format!(
            "Invalid provider: {}. Must be one of: {}",
            provider,
            valid_providers.join(", ")
        ));
    }

    // Check CID not already used by another package
    let index = load_index()?;
    for (existing_id, _entry) in &index.packages {
        if existing_id == &pkg_id {
            continue;
        }
        let manifest = match load_manifest(existing_id) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if let Some(ref existing_cid) = manifest.cid {
            if existing_cid == &cid {
                return Err(format!("CID already linked to package {}", existing_id));
            }
        }
    }

    let now = Utc::now().to_rfc3339();
    let new_version = old_manifest.version + 1;

    let pkg = ContentPackage {
        id: pkg_id.clone(),
        name: old_manifest.name.clone(),
        description: old_manifest.description.clone(),
        tags: old_manifest.tags.clone(),
        created_at: old_manifest.created_at.clone(),
        updated_at: now.clone(),
        version: new_version,
        status: "published".to_string(),
        files: old_manifest.files.clone(),
        cid: Some(cid.clone()),
        provider: Some(provider),
        published_at: Some(now),
        folder: old_manifest.folder.clone(),
    };

    let manifest = package_manifest_to_full(&pkg);
    save_manifest(&manifest)?;

    let history = history_dir(&pkg_id)?;
    fs::create_dir_all(&history).map_err(|e| e.to_string())?;
    let hist_path = history.join(format!("v{}.json", new_version));
    let hist_content = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
    fs::write(&hist_path, &hist_content).map_err(|e| e.to_string())?;

    let mut index = load_index()?;
    index.packages.insert(
        pkg_id.clone(),
        IndexPackageEntry {
            name: pkg.name.clone(),
            tags: pkg.tags.clone(),
            folder: pkg.folder.clone(),
        },
    );
    save_index(&index)?;

    Ok(pkg)
}

#[tauri::command]
pub fn content_library_set_folder(
    package_id: String,
    folder: String,
) -> Result<ContentPackage, String> {
    let pkg_id = package_id.trim().to_string();
    validate_package_id(&pkg_id)?;
    let folder_clean = validate_folder(&folder)?;

    let mut manifest = load_manifest(&pkg_id)?;
    manifest.folder = folder_clean.clone();
    save_manifest(&manifest)?;

    let mut index = load_index()?;
    if let Some(entry) = index.packages.get_mut(&pkg_id) {
        entry.folder = folder_clean.clone();
    }
    ensure_folder_in_index(&mut index, &folder_clean);
    save_index(&index)?;

    Ok(manifest_to_content_package(&manifest))
}

#[tauri::command]
pub fn content_library_open_root_folder() -> Result<String, String> {
    use std::process::Command;
    let dir = content_library_dir()?;
    let dir_str = dir.to_string_lossy().to_string();

    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .arg(&dir)
            .spawn()
            .map_err(|e| format!("Failed to open folder: {}", e))?;
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(&dir)
            .spawn()
            .map_err(|e| format!("Failed to open folder: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open")
            .arg(&dir_str)
            .spawn()
            .map_err(|e| format!("Failed to open folder: {}", e))?;
    }

    Ok(dir_str)
}

#[tauri::command]
pub fn content_library_open_package_folder(package_id: String) -> Result<String, String> {
    use std::process::Command;
    let pkg_id = package_id.trim().to_string();
    validate_package_id(&pkg_id)?;
    let pkg_dir = package_dir(&pkg_id)?;
    if !pkg_dir.exists() {
        return Err(format!("Package folder not found: {}", pkg_id));
    }
    let dir_str = pkg_dir.to_string_lossy().to_string();

    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .arg(&pkg_dir)
            .spawn()
            .map_err(|e| format!("Failed to open folder: {}", e))?;
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(&pkg_dir)
            .spawn()
            .map_err(|e| format!("Failed to open folder: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open")
            .arg(&dir_str)
            .spawn()
            .map_err(|e| format!("Failed to open folder: {}", e))?;
    }

    Ok(dir_str)
}

#[tauri::command]
pub fn content_library_create_folder(name: String) -> Result<Vec<String>, String> {
    let folder_clean = validate_folder(&name)?;
    if folder_clean.is_empty() {
        return Err("Folder name cannot be empty".to_string());
    }
    let mut index = load_index()?;
    ensure_folder_in_index(&mut index, &folder_clean);
    save_index(&index)?;
    Ok(index.folders.clone())
}

#[tauri::command]
pub fn content_library_delete_folder(name: String) -> Result<(), String> {
    let folder_clean = validate_folder(&name)?;
    if folder_clean.is_empty() {
        return Err("Cannot delete the default folder".to_string());
    }
    let mut index = load_index()?;

    // Find all packages in this folder
    let pkg_ids_to_delete: Vec<String> = index
        .packages
        .iter()
        .filter(|(_, entry)| entry.folder.trim() == folder_clean)
        .map(|(id, _)| id.clone())
        .collect();

    // Delete package directories and remove from index
    for pkg_id in &pkg_ids_to_delete {
        let pkg_dir = package_dir(pkg_id)?;
        if pkg_dir.exists() {
            fs::remove_dir_all(&pkg_dir)
                .map_err(|e| format!("Failed to delete package {}: {}", pkg_id, e))?;
        }
        index.packages.remove(pkg_id);
    }

    // Remove folder from index
    index.folders.retain(|f| f.trim() != folder_clean);
    save_index(&index)?;
    Ok(())
}

#[tauri::command]
pub fn content_library_list_folders() -> Result<Vec<String>, String> {
    let index = load_index()?;
    Ok(index.folders.clone())
}

#[tauri::command]
pub fn content_library_duplicate(package_id: String) -> Result<ContentPackage, String> {
    let pkg_id = package_id.trim().to_string();
    validate_package_id(&pkg_id)?;

    let manifest = load_manifest(&pkg_id)?;
    let new_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    // Copy files directory
    let old_dir = package_dir(&pkg_id)?;
    let new_dir = package_dir(&new_id)?;
    if old_dir.exists() {
        copy_dir_all(&old_dir, &new_dir)
            .map_err(|e| format!("Failed to copy package files: {}", e))?;
    }
    let old_history_copy = new_dir.join("history");
    if old_history_copy.exists() {
        fs::remove_dir_all(&old_history_copy)
            .map_err(|e| format!("Failed to reset copied history: {}", e))?;
    }

    // Update manifest with new ID and timestamps
    let mut new_manifest = manifest.clone();
    new_manifest.id = new_id.clone();
    new_manifest.created_at = now.clone();
    new_manifest.updated_at = now.clone();
    new_manifest.version = 1;
    new_manifest.cid = None;
    new_manifest.provider = None;
    new_manifest.published_at = None;
    new_manifest.status = "local".to_string();
    // Append " (Copy)" to name, capped at 128 chars
    let copy_suffix = " (Copy)";
    let new_name = if manifest.name.len() + copy_suffix.len() > 128 {
        format!(
            "{}…{}",
            &manifest.name[..(128 - copy_suffix.len() - 1)],
            copy_suffix
        )
    } else {
        format!("{}{}", manifest.name, copy_suffix)
    };
    new_manifest.name = new_name;

    // Update manifest.json inside new dir
    let new_manifest_path = new_dir.join("manifest.json");
    let new_manifest_content =
        serde_json::to_string_pretty(&new_manifest).map_err(|e| e.to_string())?;
    fs::write(&new_manifest_path, &new_manifest_content).map_err(|e| e.to_string())?;

    // Create empty history dir for the new package
    let new_history = new_dir.join("history");
    fs::create_dir_all(&new_history).map_err(|e| e.to_string())?;

    // Update index
    let mut index = load_index()?;
    index.packages.insert(
        new_id.clone(),
        IndexPackageEntry {
            name: new_manifest.name.clone(),
            tags: new_manifest.tags.clone(),
            folder: new_manifest.folder.clone(),
        },
    );
    ensure_folder_in_index(&mut index, &new_manifest.folder);
    save_index(&index)?;

    Ok(manifest_to_content_package(&new_manifest))
}

fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn content_library_move_packages(
    package_ids: Vec<String>,
    folder: String,
) -> Result<(), String> {
    let folder_clean = validate_folder(&folder)?;
    let mut index = load_index()?;

    for pkg_id in &package_ids {
        let pid = pkg_id.trim().to_string();
        validate_package_id(&pid)?;

        // Update manifest
        let mut manifest = load_manifest(&pid)?;
        manifest.folder = folder_clean.clone();
        save_manifest(&manifest)?;

        // Update index
        if let Some(entry) = index.packages.get_mut(&pid) {
            entry.folder = folder_clean.clone();
        }
    }

    ensure_folder_in_index(&mut index, &folder_clean);
    save_index(&index)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::files;

    fn test_library_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("content_library_test_{}", Uuid::new_v4()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).ok();
        dir
    }

    struct ContentLibraryTestDir {
        dir: PathBuf,
    }

    impl ContentLibraryTestDir {
        fn new() -> Self {
            let dir = test_library_dir();
            files::TEST_COMMANDER_DIR.with(|cell| *cell.borrow_mut() = Some(dir.clone()));
            files::TEST_DATA_DIR.with(|cell| *cell.borrow_mut() = Some(dir.clone()));
            ContentLibraryTestDir { dir }
        }
    }

    impl Drop for ContentLibraryTestDir {
        fn drop(&mut self) {
            files::TEST_COMMANDER_DIR.with(|cell| *cell.borrow_mut() = None);
            files::TEST_DATA_DIR.with(|cell| *cell.borrow_mut() = None);
            let _ = fs::remove_dir_all(&self.dir);
        }
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
    fn validates_import_cid_shapes() {
        assert!(validate_import_cid("QmZPGfJojdTzaqCWJu2m3krark38X1rqEHBo4SjeqHKB26").is_ok());
        assert!(
            validate_import_cid("bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi")
                .is_ok()
        );
        assert!(validate_import_cid(
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        )
        .is_ok());
        assert!(validate_import_cid("not-a-cid").is_err());
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
            folder: "".to_string(),
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
            folder: "".to_string(),
        };
        let manifest = package_manifest_to_full(&pkg);
        let roundtripped = manifest_to_content_package(&manifest);
        assert_eq!(roundtripped.cid, Some("QmTest123".to_string()));
        assert_eq!(roundtripped.provider, Some("manual".to_string()));
        assert_eq!(
            roundtripped.published_at,
            Some("2026-02-01T00:00:00Z".to_string())
        );
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
            folder: "nfts".to_string(),
        };
        let manifest = package_manifest_to_full(&pkg);
        assert_eq!(manifest.folder, "nfts");
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
        assert_eq!(
            manifest.published_at,
            Some("2026-03-20T14:45:00Z".to_string())
        );
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
            folder: "".to_string(),
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
            folder: "".to_string(),
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
            folder: "test-folder".to_string(),
        };
        let json = serde_json::to_string(&pkg).unwrap();
        assert!(json.contains("QmTest"));
        assert!(json.contains("manual"));
        assert!(json.contains("test-folder"));
    }

    #[test]
    fn validate_package_name_trims_whitespace() {
        assert!(validate_package_name("  hello  ").is_ok());
        assert!(validate_package_name("  ").is_err());
    }

    #[test]
    fn load_index_returns_empty_when_missing() {
        let dir = test_library_dir();
        let index = ContentLibraryIndex {
            version: 1,
            packages: HashMap::new(),
            folders: Vec::new(),
        };
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
    fn cid_import_rejects_empty_cid() {
        let input = CidImportInput {
            cid: "".to_string(),
            name: None,
            description: None,
            tags: None,
        };
        let result = content_library_import_cid(input);
        assert!(result.is_err());
    }

    #[test]
    fn cid_import_rejects_whitespace_cid() {
        let input = CidImportInput {
            cid: "   ".to_string(),
            name: None,
            description: None,
            tags: None,
        };
        let result = content_library_import_cid(input);
        assert!(result.is_err());
    }

    #[test]
    fn cid_import_rejects_control_chars() {
        let input = CidImportInput {
            cid: "QmTest\n123".to_string(),
            name: None,
            description: None,
            tags: None,
        };
        let result = content_library_import_cid(input);
        assert!(result.is_err());
    }

    #[test]
    fn cid_import_creates_external_package() {
        let _test_dir = ContentLibraryTestDir::new();

        let input = CidImportInput {
            cid: "QmZPGfJojdTzaqCWJu2m3krark38X1rqEHBo4SjeqHKB26".to_string(),
            name: Some("Test CID Import".to_string()),
            description: Some("A test".to_string()),
            tags: Some(vec!["test".to_string()]),
        };
        let result = content_library_import_cid(input).unwrap();
        assert_eq!(result.status, "external");
        assert_eq!(
            result.cid,
            Some("QmZPGfJojdTzaqCWJu2m3krark38X1rqEHBo4SjeqHKB26".to_string())
        );
        assert_eq!(result.provider, Some("manual".to_string()));
        assert_eq!(result.version, 1);
        assert_eq!(result.name, "Test CID Import");

        // Verify it shows up in list
        let list = content_library_list().unwrap();
        assert!(list.iter().any(|s| s.id == result.id
            && s.cid == Some("QmZPGfJojdTzaqCWJu2m3krark38X1rqEHBo4SjeqHKB26".to_string())));
    }

    #[test]
    fn cid_import_uses_auto_name_when_none_provided() {
        let _test_dir = ContentLibraryTestDir::new();

        let input = CidImportInput {
            cid: "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi".to_string(),
            name: None,
            description: None,
            tags: None,
        };
        let result = content_library_import_cid(input).unwrap();
        assert!(result.name.starts_with("CID: "));
    }

    #[test]
    fn content_package_summary_includes_cid_and_provider() {
        let summary = ContentPackageSummary {
            id: "test-id".to_string(),
            name: "Test".to_string(),
            description: "Desc".to_string(),
            tags: vec![],
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
            version: 1,
            status: "local".to_string(),
            file_count: 0,
            cid: Some("QmTest".to_string()),
            provider: Some("manual".to_string()),
            folder: "my-folder".to_string(),
        };
        let json = serde_json::to_string(&summary).unwrap();
        assert!(json.contains("QmTest"));
        assert!(json.contains("manual"));
        assert!(json.contains("my-folder"));
    }

    #[test]
    fn content_package_summary_omits_optionals_when_none() {
        let summary = ContentPackageSummary {
            id: "test-id".to_string(),
            name: "Test".to_string(),
            description: "Desc".to_string(),
            tags: vec![],
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
            version: 1,
            status: "local".to_string(),
            file_count: 0,
            cid: None,
            provider: None,
            folder: "".to_string(),
        };
        let json = serde_json::to_string(&summary).unwrap();
        assert!(!json.contains("cid"));
        assert!(!json.contains("provider"));
    }

    #[test]
    fn base64_decode_rejects_invalid_input() {
        assert!(base64_decode("!!!not%valid%base64!!!").is_err());
        assert!(base64_decode("").is_ok());
        assert!(base64_decode("aGVsbG8gd29ybGQ=").is_ok());
    }

    #[test]
    fn sanitize_file_path_rejects_absolute() {
        let dir = test_library_dir();
        let pkg_id = "test-pkg-id";
        let files_dir = dir.join("packages").join(pkg_id).join("files");
        fs::create_dir_all(&files_dir).ok();
        let result = sanitize_file_path(pkg_id, "/etc/passwd");
        assert!(result.is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn sanitize_file_path_rejects_dot_dot() {
        let dir = test_library_dir();
        let pkg_id = "test-pkg-id";
        let files_dir = dir.join("packages").join(pkg_id).join("files");
        fs::create_dir_all(&files_dir).ok();
        let result = sanitize_file_path(pkg_id, "../outside");
        assert!(result.is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_package_id_rejects_invalid_for_public_commands() {
        assert!(validate_package_id("").is_err());
        assert!(validate_package_id("   ").is_err());
        assert!(validate_package_id("not-a-uuid").is_err());
        assert!(validate_package_id("../../etc/passwd").is_err());
    }

    #[test]
    fn cid_link_rejects_empty_cid() {
        let input = CidLinkInput {
            package_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            cid: "".to_string(),
            provider: None,
        };
        let result = content_library_link_cid(input);
        assert!(result.is_err());
    }

    #[test]
    fn cid_link_rejects_invalid_cid() {
        let input = CidLinkInput {
            package_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            cid: "not-a-cid".to_string(),
            provider: None,
        };
        let result = content_library_link_cid(input);
        assert!(result.is_err());
    }

    #[test]
    fn cid_link_rejects_invalid_provider() {
        let input = CidLinkInput {
            package_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            cid: "QmZPGfJojdTzaqCWJu2m3krark38X1rqEHBo4SjeqHKB26".to_string(),
            provider: Some("evil_provider".to_string()),
        };
        let result = content_library_link_cid(input);
        assert!(result.is_err());
    }

    #[test]
    fn cid_link_accepts_valid_providers() {
        let input = CidLinkInput {
            package_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            cid: "QmZPGfJojdTzaqCWJu2m3krark38X1rqEHBo4SjeqHKB26".to_string(),
            provider: Some("pinata".to_string()),
        };
        let result = content_library_link_cid(CidLinkInput {
            package_id: input.package_id,
            cid: input.cid,
            provider: Some("pinata".to_string()),
        });
        assert!(result.is_err());

        let result2 = CidLinkInput {
            package_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            cid: "QmZPGfJojdTzaqCWJu2m3krark38X1rqEHBo4SjeqHKB26".to_string(),
            provider: Some("filebase".to_string()),
        };
        // This will fail because package doesn't exist, not because of provider
        let r2 = content_library_link_cid(result2);
        assert!(r2.is_err());

        let result3 = CidLinkInput {
            package_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            cid: "QmZPGfJojdTzaqCWJu2m3krark38X1rqEHBo4SjeqHKB26".to_string(),
            provider: Some("manual".to_string()),
        };
        let r3 = content_library_link_cid(result3);
        assert!(r3.is_err());
    }

    #[test]
    fn cid_link_rejects_empty_package_id() {
        let input = CidLinkInput {
            package_id: "".to_string(),
            cid: "QmZPGfJojdTzaqCWJu2m3krark38X1rqEHBo4SjeqHKB26".to_string(),
            provider: None,
        };
        let result = content_library_link_cid(input);
        assert!(result.is_err());
    }

    #[test]
    fn old_manifest_without_folder_deserializes() {
        let json = r#"{"id":"old","name":"Old","description":"","tags":[],"created_at":"2026-01-01T00:00:00Z","updated_at":"2026-01-01T00:00:00Z","version":1,"status":"local","files":[],"cid":null,"provider":null,"published_at":null}"#;
        let manifest: PackageManifest = serde_json::from_str(json).unwrap();
        assert_eq!(manifest.folder, "");
    }

    #[test]
    fn folder_validation_accepts_normal_names() {
        assert_eq!(validate_folder("NFTs").unwrap(), "NFTs");
        assert_eq!(validate_folder("  My Folder  ").unwrap(), "My Folder");
        assert_eq!(validate_folder("").unwrap(), "");
        assert_eq!(validate_folder("a-b_c").unwrap(), "a-b_c");
        assert!(validate_folder("a".repeat(64).as_str()).is_ok());
    }

    #[test]
    fn folder_validation_rejects_traversal_and_separators_and_control() {
        assert!(validate_folder("../escape").is_err());
        assert!(validate_folder("folder/sub").is_err());
        assert!(validate_folder("folder\\sub").is_err());
        assert!(validate_folder("folder\x00null").is_err());
        assert!(validate_folder("folder\nline").is_err());
        assert!(validate_folder("a".repeat(65).as_str()).is_err());
    }

    #[test]
    fn create_preserves_folder_from_input() {
        let _test_dir = ContentLibraryTestDir::new();

        let input = ContentPackageInput {
            name: "Foldered Package".to_string(),
            description: None,
            tags: None,
            body: None,
            files: None,
            folder: Some("  My Folder  ".to_string()),
        };
        let result = content_library_create(input).unwrap();
        assert_eq!(result.folder, "My Folder");

        let list = content_library_list().unwrap();
        let found = list.iter().find(|s| s.id == result.id).unwrap();
        assert_eq!(found.folder, "My Folder");
    }

    #[test]
    fn update_preserves_existing_folder_when_not_provided() {
        let _test_dir = ContentLibraryTestDir::new();

        let create_input = ContentPackageInput {
            name: "Original".to_string(),
            description: None,
            tags: None,
            body: Some("body".to_string()),
            files: None,
            folder: Some("Original Folder".to_string()),
        };
        let created = content_library_create(create_input).unwrap();

        let update_input = ContentPackageInput {
            name: "Renamed".to_string(),
            description: None,
            tags: None,
            body: Some("new body".to_string()),
            files: None,
            folder: None,
        };
        let updated = content_library_update(created.id.clone(), update_input).unwrap();
        assert_eq!(updated.folder, "Original Folder");
    }

    #[test]
    fn update_changes_folder_when_provided() {
        let _test_dir = ContentLibraryTestDir::new();

        let create_input = ContentPackageInput {
            name: "Original".to_string(),
            description: None,
            tags: None,
            body: Some("body".to_string()),
            files: None,
            folder: Some("Old Folder".to_string()),
        };
        let created = content_library_create(create_input).unwrap();

        let update_input = ContentPackageInput {
            name: "Renamed".to_string(),
            description: None,
            tags: None,
            body: Some("new body".to_string()),
            files: None,
            folder: Some("New Folder".to_string()),
        };
        let updated = content_library_update(created.id.clone(), update_input).unwrap();
        assert_eq!(updated.folder, "New Folder");
    }

    #[test]
    fn set_folder_rejects_invalid_package_id() {
        let result = content_library_set_folder("not-a-uuid".to_string(), "test".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn set_folder_rejects_invalid_folder_name() {
        let result = content_library_set_folder(
            "550e8400-e29b-41d4-a716-446655440000".to_string(),
            "bad/folder".to_string(),
        );
        assert!(result.is_err());
    }
}
