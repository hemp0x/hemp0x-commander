use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::modules::content_library;
use crate::modules::files::data_dir;
use crate::modules::provider_settings;

#[derive(Serialize)]
pub struct ProviderTestResult {
    pub success: bool,
    pub message: String,
}

#[derive(Serialize)]
pub struct PublishResult {
    pub package_id: String,
    pub cid: String,
    pub provider: String,
    pub published_at: String,
    pub version: u32,
    pub status: String,
}

#[derive(Serialize)]
pub struct PackagePublishPreview {
    pub package_id: String,
    pub name: String,
    pub file_count: usize,
    pub total_size_bytes: u64,
    pub has_body: bool,
    pub files: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct PackageMetadata {
    package_id: String,
    name: String,
    description: String,
    tags: Vec<String>,
    version: u32,
    created_at: String,
    updated_at: String,
    files: Vec<MetadataFileEntry>,
    main_content: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct MetadataFileEntry {
    path: String,
    mime: String,
    size_bytes: u64,
    sha256: String,
}

fn create_staging_dir(package_id: &str) -> Result<PathBuf, String> {
    let dir = data_dir()?.join("content-library").join(".staging").join(package_id);
    if dir.exists() {
        fs::remove_dir_all(&dir).map_err(|e| format!("Failed to clean staging dir: {}", e))?;
    }
    fs::create_dir_all(dir.join("files")).map_err(|e| format!("Failed to create staging dir: {}", e))?;
    Ok(dir)
}

fn validate_package_relative_path(name: &str) -> Result<PathBuf, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("File path is empty".to_string());
    }
    if trimmed.chars().any(|c| c.is_control()) {
        return Err(format!("File path contains control characters: {}", trimmed));
    }
    let candidate = Path::new(trimmed);
    if candidate.is_absolute() {
        return Err(format!("File path must be relative: {}", trimmed));
    }
    let mut clean = PathBuf::new();
    for component in candidate.components() {
        match component {
            Component::Normal(part) => clean.push(part),
            _ => return Err(format!("Unsafe file path: {}", trimmed)),
        }
    }
    if clean.as_os_str().is_empty() {
        return Err("File path is empty".to_string());
    }
    Ok(clean)
}

fn build_staging(package_id: &str) -> Result<(PathBuf, PackageMetadata, Vec<(String, Vec<u8>)>), String> {
    let manifest = content_library::load_manifest(package_id)?;
    let staging_dir = create_staging_dir(package_id)?;
    let mut staged_files: Vec<(String, Vec<u8>)> = Vec::new();
    let mut metadata_files: Vec<MetadataFileEntry> = Vec::new();

    let files_dir = content_library::files_dir(package_id)?;
    let mut main_content_path: Option<String> = None;

    for file_entry in &manifest.files {
        let safe_rel = validate_package_relative_path(&file_entry.path)?;
        let file_path = files_dir.join(&safe_rel);
        if !file_path.exists() {
            continue;
        }

        let data = fs::read(&file_path)
            .map_err(|e| format!("Failed to read file {}: {}", file_entry.path, e))?;

        let rel_path = safe_rel.to_string_lossy().replace('\\', "/");

        if rel_path == "content.md" {
            main_content_path = Some("content.md".to_string());
        }

        let dest = staging_dir.join(&rel_path);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(&dest, &data).map_err(|e| e.to_string())?;

        staged_files.push((rel_path.clone(), data.clone()));

        metadata_files.push(MetadataFileEntry {
            path: rel_path,
            mime: file_entry.mime.clone(),
            size_bytes: file_entry.size_bytes,
            sha256: file_entry.sha256.clone(),
        });
    }

    let metadata = PackageMetadata {
        package_id: manifest.id.clone(),
        name: manifest.name.clone(),
        description: manifest.description.clone(),
        tags: manifest.tags.clone(),
        version: manifest.version,
        created_at: manifest.created_at.clone(),
        updated_at: manifest.updated_at.clone(),
        files: metadata_files,
        main_content: main_content_path,
    };

    let metadata_json = serde_json::to_string_pretty(&metadata)
        .map_err(|e| format!("Failed to serialize metadata: {}", e))?;

    let metadata_path = staging_dir.join("metadata.json");
    fs::write(&metadata_path, metadata_json.as_bytes()).map_err(|e| e.to_string())?;
    staged_files.push(("metadata.json".to_string(), metadata_json.as_bytes().to_vec()));

    Ok((staging_dir, metadata, staged_files))
}

fn build_multipart_body(files: &[(String, Vec<u8>)], boundary: &str) -> Vec<u8> {
    let mut body = Vec::new();
    let b = format!("--{}\r\n", boundary);
    let end = format!("--{}--\r\n", boundary);

    for (file_path, data) in files {
        body.extend_from_slice(b.as_bytes());
        body.extend_from_slice(
            format!(
                "Content-Disposition: form-data; name=\"file\"; filename=\"{}\"\r\n",
                file_path.replace('"', "\\\"")
            ).as_bytes(),
        );
        body.extend_from_slice(b"Content-Type: application/octet-stream\r\n");
        body.extend_from_slice(b"\r\n");
        body.extend_from_slice(data);
        body.extend_from_slice(b"\r\n");
    }

    body.extend_from_slice(end.as_bytes());
    body
}

fn random_boundary() -> String {
    format!("----CommanderBoundary{:x}", rand::random::<u64>())
}

fn pinata_api_url(settings: &provider_settings::ProviderSettings) -> String {
    if settings.pinata_api_url.is_empty() {
        "https://api.pinata.cloud".to_string()
    } else {
        settings.pinata_api_url.trim_end_matches('/').to_string()
    }
}

fn pinata_test_connection(settings: &provider_settings::ProviderSettings) -> ProviderTestResult {
    let token = settings.pinata_api_token.trim();
    if token.is_empty() {
        return ProviderTestResult {
            success: false,
            message: "Pinata API token is not configured. Add your JWT in Settings.".to_string(),
        };
    }

    let url = format!("{}/data/testAuthentication", pinata_api_url(settings));

    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(15))
        .build();

    match agent.get(&url)
        .set("Authorization", &format!("Bearer {}", token))
        .set("User-Agent", "hemp0x-commander/2.0")
        .call()
    {
        Ok(_response) => ProviderTestResult {
            success: true,
            message: "Pinata authentication successful.".to_string(),
        },
        Err(ureq::Error::Status(401, _)) => ProviderTestResult {
            success: false,
            message: "Pinata API returned 401 Unauthorized. Check your JWT token.".to_string(),
        },
        Err(ureq::Error::Status(code, _)) => ProviderTestResult {
            success: false,
            message: format!("Pinata API returned HTTP {}.", code),
        },
        Err(e) => ProviderTestResult {
            success: false,
            message: format!("Pinata connection failed: {}", e),
        },
    }
}

fn pinata_upload(
    _package_name: &str,
    files: &[(String, Vec<u8>)],
    settings: &provider_settings::ProviderSettings,
) -> Result<String, String> {
    let token = settings.pinata_api_token.trim().to_string();
    if token.is_empty() {
        return Err("Pinata API token is not configured. Add your JWT in IPFS Settings.".to_string());
    }

    let boundary = random_boundary();
    let body = build_multipart_body(files, &boundary);
    let content_type = format!("multipart/form-data; boundary={}", boundary);

    let url = format!("{}/pinning/pinFileToIPFS", pinata_api_url(settings));

    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(15))
        .timeout_read(Duration::from_secs(120))
        .timeout_write(Duration::from_secs(120))
        .build();

    let response = agent.post(&url)
        .set("Authorization", &format!("Bearer {}", token))
        .set("Content-Type", &content_type)
        .send_bytes(&body)
        .map_err(|e| match e {
            ureq::Error::Status(401, _) => {
                "Pinata returned 401 Unauthorized. Your API token may be invalid or expired.".to_string()
            }
            ureq::Error::Status(403, _) => {
                "Pinata returned 403 Forbidden. Check your account permissions.".to_string()
            }
            ureq::Error::Status(413, _) => {
                "Pinata returned 413 Payload Too Large. Package exceeds the upload limit.".to_string()
            }
            ureq::Error::Status(429, _) => {
                "Pinata returned 429 Rate Limited. Please wait and try again.".to_string()
            }
            ureq::Error::Status(code, _) => {
                format!("Pinata API returned HTTP {}: upload failed.", code)
            }
            ureq::Error::Transport(t) => {
                format!("Pinata network/transport error: {}", t)
            }
        })?;

    let response_body: serde_json::Value = response.into_json()
        .map_err(|e| format!("Failed to parse Pinata response: {}", e))?;

    let cid = response_body
        .get("IpfsHash")
        .and_then(|v| v.as_str())
        .or_else(|| response_body.get("cid").and_then(|v| v.as_str()))
        .ok_or_else(|| {
            let err = response_body.to_string();
            if err.len() > 200 {
                "Pinata response did not contain a CID.".to_string()
            } else {
                format!("Pinata response did not contain a CID: {}", err)
            }
        })?;

    if let Some(err_val) = response_body.get("error") {
        if let Some(err_msg) = err_val.as_str() {
            if !err_msg.is_empty() {
                return Err(format!("Pinata reported an error: {}", err_msg));
            }
        }
    }

    Ok(cid.to_string())
}

fn kubo_endpoint(settings: &provider_settings::ProviderSettings) -> String {
    if settings.kubo_endpoint.is_empty() {
        "http://127.0.0.1:5001".to_string()
    } else {
        settings.kubo_endpoint.trim_end_matches('/').to_string()
    }
}

fn filebase_endpoint(settings: &provider_settings::ProviderSettings) -> String {
    if settings.filebase_endpoint.is_empty() {
        "https://api.filebase.io/v1/ipfs".to_string()
    } else {
        settings.filebase_endpoint.trim_end_matches('/').to_string()
    }
}

fn ipfs_rpc_test(api_endpoint: &str, bearer_token: Option<&str>) -> ProviderTestResult {
    let url = format!("{}/api/v0/version", api_endpoint);

    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(15))
        .build();

    let mut req = agent.post(&url)
        .set("User-Agent", "hemp0x-commander/2.0");
    if let Some(token) = bearer_token {
        req = req.set("Authorization", &format!("Bearer {}", token));
    }

    match req.call() {
        Ok(response) => {
            let body: Result<serde_json::Value, _> = response.into_json();
            match body {
                Ok(json) => {
                    let version = json
                        .get("Version")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    ProviderTestResult {
                        success: true,
                        message: format!("IPFS RPC v{} is running at {}.", version, api_endpoint),
                    }
                }
                Err(_) => ProviderTestResult {
                    success: true,
                    message: format!("IPFS RPC responded at {} but version info could not be parsed.", api_endpoint),
                },
            }
        }
        Err(ureq::Error::Status(401, _)) => ProviderTestResult {
            success: false,
            message: "IPFS RPC returned 401 Unauthorized. Check your API token.".to_string(),
        },
        Err(ureq::Error::Status(code, _)) => ProviderTestResult {
            success: false,
            message: format!("IPFS RPC returned HTTP {} at {}.", code, api_endpoint),
        },
        Err(e) => ProviderTestResult {
            success: false,
            message: format!("Cannot connect at {}: {}", api_endpoint, e),
        },
    }
}

fn ipfs_rpc_add(
    files: &[(String, Vec<u8>)],
    api_endpoint: &str,
    bearer_token: Option<&str>,
) -> Result<String, String> {
    let boundary = random_boundary();
    let body = build_multipart_body(files, &boundary);
    let content_type = format!("multipart/form-data; boundary={}", boundary);

    let url = format!("{}/api/v0/add?pin=true&wrap-with-directory=true", api_endpoint);

    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(15))
        .timeout_read(Duration::from_secs(120))
        .timeout_write(Duration::from_secs(120))
        .build();

    let mut req = agent.post(&url)
        .set("Content-Type", &content_type)
        .set("User-Agent", "hemp0x-commander/2.0");
    if let Some(token) = bearer_token {
        req = req.set("Authorization", &format!("Bearer {}", token));
    }

    let response = req.send_bytes(&body)
        .map_err(|e| match e {
            ureq::Error::Status(401, _) => {
                "IPFS RPC returned 401 Unauthorized. Check your API token.".to_string()
            }
            ureq::Error::Status(403, _) => {
                "IPFS RPC returned 403 Forbidden. Check your account permissions.".to_string()
            }
            ureq::Error::Status(code, _) => {
                format!("IPFS RPC returned HTTP {}: upload failed.", code)
            }
            ureq::Error::Transport(t) => {
                format!("Cannot connect to IPFS RPC: {}. Verify the endpoint is correct.", t)
            }
        })?;

    let response_text = response.into_string()
        .map_err(|e| format!("Failed to read IPFS RPC response: {}", e))?;

    let mut root_cid: Option<String> = None;
    for line in response_text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) {
            let name = entry.get("Name").and_then(|v| v.as_str()).unwrap_or("");
            if name.is_empty() || name == "." || name == "/" {
                if let Some(hash) = entry.get("Hash").and_then(|v| v.as_str()) {
                    root_cid = Some(hash.to_string());
                }
            }
            if let Some(hash) = entry.get("Hash").and_then(|v| v.as_str()) {
                root_cid = Some(hash.to_string());
            }
        }
    }

    root_cid.ok_or_else(|| {
        format!(
            "IPFS RPC did not return a directory CID. Response: {}",
            if response_text.len() > 500 { &response_text[..500] } else { &response_text }
        )
    })
}

fn kubo_test_connection(settings: &provider_settings::ProviderSettings) -> ProviderTestResult {
    ipfs_rpc_test(&kubo_endpoint(settings), None)
}

fn kubo_upload(
    files: &[(String, Vec<u8>)],
    settings: &provider_settings::ProviderSettings,
) -> Result<String, String> {
    ipfs_rpc_add(files, &kubo_endpoint(settings), None)
}

fn filebase_test_connection(settings: &provider_settings::ProviderSettings) -> ProviderTestResult {
    let token = settings.filebase_token.trim();
    if token.is_empty() {
        return ProviderTestResult {
            success: false,
            message: "Filebase access token is not configured. Generate one in the Filebase console under Access Keys.".to_string(),
        };
    }
    ipfs_rpc_test(&filebase_endpoint(settings), Some(token))
}

fn filebase_upload(
    files: &[(String, Vec<u8>)],
    settings: &provider_settings::ProviderSettings,
) -> Result<String, String> {
    let token = settings.filebase_token.trim().to_string();
    if token.is_empty() {
        return Err("Filebase access token is not configured. Generate one in the Filebase console under Access Keys and enter it in IPFS Settings.".to_string());
    }
    ipfs_rpc_add(files, &filebase_endpoint(settings), Some(&token))
}

#[tauri::command]
pub fn ipfs_test_publish_provider(provider: String) -> Result<ProviderTestResult, String> {
    let settings = provider_settings::load_provider_settings()?;
    match provider.as_str() {
        "pinata" => Ok(pinata_test_connection(&settings)),
        "installed_kubo" => Ok(kubo_test_connection(&settings)),
        "filebase" => Ok(filebase_test_connection(&settings)),
        "manual" => Ok(ProviderTestResult {
            success: true,
            message: "Manual CID linking is always available.".to_string(),
        }),
        _ => Err(format!("Unknown provider: {}. Valid providers: manual, pinata, installed_kubo, filebase", provider)),
    }
}

#[tauri::command]
pub fn content_library_publish_package(
    package_id: String,
    provider: String,
) -> Result<PublishResult, String> {
    let pkg_id = package_id.trim().to_string();
    content_library::validate_package_id(&pkg_id)?;

    let settings = provider_settings::load_provider_settings()?;
    let provider = provider.trim().to_string();

    match provider.as_str() {
        "manual" => {
            return Err("Use the Link CID command for manual CID linking. Publishing requires a configured provider.".to_string());
        }
        "pinata" | "installed_kubo" | "filebase" => {}
        _ => {
            return Err(format!(
                "Unknown provider: {}. Valid providers: pinata, installed_kubo, filebase, manual",
                provider
            ));
        }
    }

    let manifest = content_library::load_manifest(&pkg_id)?;

    if manifest.files.is_empty() {
        return Err("Package has no files to publish. Add content or attachments first.".to_string());
    }

    let (_staging_dir, _metadata, staged_files) = build_staging(&pkg_id)?;

    let total_size: u64 = staged_files.iter().map(|(_, d)| d.len() as u64).sum();
    if total_size > 500 * 1024 * 1024 {
        return Err("Package exceeds 500 MB upload limit.".to_string());
    }

    let cid = match provider.as_str() {
        "pinata" => pinata_upload(&manifest.name, &staged_files, &settings)?,
        "installed_kubo" => kubo_upload(&staged_files, &settings)?,
        "filebase" => filebase_upload(&staged_files, &settings)?,
        _ => unreachable!(),
    };

    content_library::validate_import_cid(&cid)?;

    let now = Utc::now().to_rfc3339();
    let new_version = manifest.version + 1;

    let pkg = content_library::ContentPackage {
        id: pkg_id.clone(),
        name: manifest.name.clone(),
        description: manifest.description.clone(),
        tags: manifest.tags.clone(),
        created_at: manifest.created_at.clone(),
        updated_at: now.clone(),
        version: new_version,
        status: "published".to_string(),
        files: manifest.files.clone(),
        cid: Some(cid.clone()),
        provider: Some(provider.clone()),
        published_at: Some(now.clone()),
        folder: manifest.folder.clone(),
    };

    let new_manifest = content_library::package_manifest_to_full(&pkg);
    content_library::save_manifest(&new_manifest)?;

    let history_dir = content_library::history_dir(&pkg_id)?;
    fs::create_dir_all(&history_dir).map_err(|e| e.to_string())?;
    let hist_path = history_dir.join(format!("v{}.json", new_version));
    let hist_content = serde_json::to_string_pretty(&new_manifest).map_err(|e| e.to_string())?;
    fs::write(&hist_path, hist_content).map_err(|e| e.to_string())?;

    let mut index = content_library::load_index()?;
    index.packages.insert(pkg_id.clone(), content_library::IndexPackageEntry {
        name: pkg.name.clone(),
        tags: pkg.tags.clone(),
        folder: pkg.folder.clone(),
    });
    content_library::ensure_folder_in_index(&mut index, &pkg.folder);
    content_library::save_index(&index)?;

    let staging_dir = data_dir()?.join("content-library").join(".staging").join(&pkg_id);
    let _ = fs::remove_dir_all(&staging_dir);

    Ok(PublishResult {
        package_id: pkg_id,
        cid,
        provider,
        published_at: now,
        version: new_version,
        status: "published".to_string(),
    })
}

#[tauri::command]
pub fn content_library_prepare_publish_package(
    package_id: String,
) -> Result<PackagePublishPreview, String> {
    let pkg_id = package_id.trim().to_string();
    content_library::validate_package_id(&pkg_id)?;

    let (_staging_dir, metadata, staged_files) = build_staging(&pkg_id)?;

    let total_size: u64 = staged_files.iter().map(|(_, d)| d.len() as u64).sum();
    let has_body = metadata.main_content.is_some();
    let file_names: Vec<String> = staged_files.iter().map(|(name, _)| name.clone()).collect();

    Ok(PackagePublishPreview {
        package_id: pkg_id,
        name: metadata.name,
        file_count: staged_files.len(),
        total_size_bytes: total_size,
        has_body,
        files: file_names,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_multipart_body_produces_valid_format() {
        let files = vec![
            ("content.md".to_string(), b"# Hello\n".to_vec()),
            ("metadata.json".to_string(), b"{}".to_vec()),
        ];
        let boundary = "test-boundary-123";
        let body = build_multipart_body(&files, boundary);

        let body_str = String::from_utf8_lossy(&body);
        assert!(body_str.starts_with("--test-boundary-123\r\n"));
        assert!(body_str.contains("Content-Disposition: form-data; name=\"file\"; filename=\"content.md\""));
        assert!(body_str.ends_with("--test-boundary-123--\r\n"));
    }

    #[test]
    fn random_boundary_is_long_enough() {
        let b = random_boundary();
        assert!(b.len() > 20);
        assert!(b.starts_with("----CommanderBoundary"));
    }

    #[test]
    fn reject_unsafe_filename_blocks_traversal() {
        assert!(validate_package_relative_path("../etc/passwd").is_err());
        assert!(validate_package_relative_path("normal-file.txt").is_ok());
        assert!(validate_package_relative_path("images/file.png").is_ok());
        assert!(validate_package_relative_path("").is_err());
    }

    #[test]
    fn provider_test_result_serializes() {
        let result = ProviderTestResult {
            success: true,
            message: "Connected".to_string(),
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("success"));
        assert!(json.contains("Connected"));
    }

    #[test]
    fn publish_result_serializes() {
        let result = PublishResult {
            package_id: "test-id".to_string(),
            cid: "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi".to_string(),
            provider: "filebase".to_string(),
            published_at: "2026-01-01T00:00:00Z".to_string(),
            version: 2,
            status: "published".to_string(),
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("filebase"));
    }

    #[test]
    fn package_publish_preview_serializes() {
        let preview = PackagePublishPreview {
            package_id: "test-id".to_string(),
            name: "Test".to_string(),
            file_count: 2,
            total_size_bytes: 1000,
            has_body: true,
            files: vec!["content.md".to_string(), "metadata.json".to_string()],
        };
        let json = serde_json::to_string(&preview).unwrap();
        assert!(json.contains("content.md"));
    }
}
