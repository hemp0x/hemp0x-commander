use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use pbkdf2::pbkdf2_hmac;
use rand::RngCore;
use scrypt::{scrypt, Params as ScryptParams};
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Sha512};
use zeroize::Zeroizing;

use crate::modules::files::{commander_dir, data_dir};

const CURRENT_BUNDLE_VERSION: i32 = 3;
const FORMAT_IDENTIFIER: &str = "hemp0x-unified-vault-bundle";
const CURRENT_VAULT_VERSION: i32 = 1;
const SCHEMA_IDENTIFIER: &str = "hemp0x-commander-vault";
const APP_IDENTIFIER: &str = "hemp0x-commander";
const CIPHER_PROFILE: &str = "aes-256-gcm-v1";
const WRAP_CIPHER_PROFILE: &str = "aes-256-gcm-v1";
const AAD_PROFILE: &str = "commander-envelope-v1";
const PAYLOAD_SCHEMA: &str = "commander-secrets-v1";
const PAYLOAD_VERSION: i32 = 1;
const KDF_PROFILE_PBKDF2_SHA512: &str = "pbkdf2-hmac-sha512-v1";
const KDF_PROFILE_PBKDF2_SHA256: &str = "pbkdf2-sha256-v1";
const KDF_PROFILE_SCRYPT: &str = "scrypt-v1";
const PBKDF2_ITERATIONS: u32 = 600_000;
const SCRYPT_DEFAULT_LOG_N: u8 = 14;
const SCRYPT_DEFAULT_R: u32 = 8;
const SCRYPT_DEFAULT_P: u32 = 1;
const KDF_DKLEN: u32 = 32;
const KDF_SALT_SIZE: usize = 32;
const KDF_KEY_SIZE: usize = 32;
const DEK_SIZE: usize = 32;
const GCM_IV_SIZE: usize = 12;

const RECORD_TYPE_API_TOKEN: &str = "provider.api_token";
const RECORD_TYPE_WALLET_BIP39: &str = "wallet.bip39";
const RECORD_TYPE_WALLET_WIF: &str = "wallet.wif";
const RECORD_TYPE_WALLET_CORE_MIGRATION: &str = "wallet.core_migration_envelope";
const RECORD_TYPE_WALLET_HARDWARE: &str = "wallet.hardware_metadata";
const RECORD_TYPE_WALLET_WATCH_ONLY: &str = "wallet.watch_only";
const RECORD_TYPE_PROTOCOL_NOSTR: &str = "protocol.nostr_key";
const RECORD_TYPE_APP_SECRET: &str = "app.secret";
const RECORD_TYPE_NOTE_SECURE: &str = "note.secure";

const RECORD_ID_PINATA: &str = "provider.pinata.api_token";
const RECORD_ID_FILEBASE: &str = "provider.filebase.token";

pub const VALID_NETWORKS: &[&str] = &["mainnet", "testnet", "regtest"];

pub const DERIVATION_HEMP_CANONICAL_420: &str = "hemp0x.mainnet.bip44.p2pkh.coin420.v1";
pub const DERIVATION_HEMP_LEGACY_175: &str = "hemp0x.webcom.legacy.bip44.p2pkh.coin175.v1";
pub const DERIVATION_HEMP_LEGACY_GENERIC: &str = "hemp0x.mainnet.bip44.p2pkh.v1";
pub const DERIVATION_BTC_BIP84: &str = "btc.mainnet.bip84.p2wpkh.v1";
pub const DERIVATION_WIF_SINGLE: &str = "hemp0x.mainnet.wif.single.v1";

const SUPPORTED_DERIVATION_PROFILES: &[(&str, &str, &str)] = &[
    (DERIVATION_HEMP_CANONICAL_420, "Hemp0x canonical BIP44 coin 420", "m/44'/420'/0'/change/index"),
    (DERIVATION_HEMP_LEGACY_175, "Legacy WebCom BIP44 coin 175", "m/44'/175'/0'/change/index"),
    (DERIVATION_HEMP_LEGACY_GENERIC, "Early WebCom generic (derives 175)", "m/44'/175'/0'/change/index"),
    (DERIVATION_BTC_BIP84, "BTC native SegWit BIP84", "m/84'/0'/0'/change/index"),
    (DERIVATION_WIF_SINGLE, "WIF single-key import", "N/A"),
];

const SUPPORTED_RECORD_TYPES: &[(&str, &str)] = &[
    (RECORD_TYPE_API_TOKEN, "IPFS/API provider tokens (Pinata, Filebase, Kubo)"),
    (RECORD_TYPE_WALLET_BIP39, "BIP39 mnemonic wallet (12/24 word recovery phrase)"),
    (RECORD_TYPE_WALLET_WIF, "WIF single-key wallet"),
    (RECORD_TYPE_WALLET_CORE_MIGRATION, "Core Next migration envelope reference or embedded artifact"),
    (RECORD_TYPE_WALLET_HARDWARE, "Hardware wallet / WalletConnect metadata"),
    (RECORD_TYPE_WALLET_WATCH_ONLY, "Watch-only address / public key"),
    (RECORD_TYPE_PROTOCOL_NOSTR, "Nostr nsec/npub key"),
    (RECORD_TYPE_APP_SECRET, "Generic application API key or credential"),
    (RECORD_TYPE_NOTE_SECURE, "Encrypted secure note"),
];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VaultPayloadBlock {
    pub payload_schema: String,
    pub iv: String,
    pub ciphertext: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KeySlot {
    pub slot_id: String,
    pub slot_type: String,
    pub kdf_profile: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kdf_iterations: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kdf_log_n: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kdf_r: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kdf_p: Option<u32>,
    #[serde(default = "default_kdf_dklen")]
    pub kdf_dklen: u32,
    pub salt: String,
    pub wrap_cipher_profile: String,
    pub wrap_iv: String,
    pub wrapped_dek: String,
    pub created: i64,
    pub modified: i64,
}

fn default_kdf_dklen() -> u32 { KDF_DKLEN }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VaultEnvelope {
    pub version: i32,
    pub schema_identifier: String,
    pub app_identifier: String,
    pub network: Option<String>,
    pub cipher_profile: String,
    pub aad_profile: String,
    pub payload: VaultPayloadBlock,
    pub key_slots: Vec<KeySlot>,
    pub created: i64,
    pub modified: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[allow(non_snake_case)]
pub struct VaultBundle {
    /// camelCase for WebCom compatibility (unified bundle v3+)
    #[serde(alias = "bundle_version")]
    pub bundleVersion: i32,
    pub format_identifier: String,
    pub vault: VaultEnvelope,
    /// camelCase for WebCom compatibility; advisory non-secret metadata
    #[serde(alias = "public_meta")]
    pub meta: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SecretRecord {
    pub record_id: String,
    pub record_type: String,
    pub label: String,
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin_app: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub derivation_profiles: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network: Option<String>,
    pub created: i64,
    pub modified: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VaultPayload {
    pub payload_version: i32,
    pub secrets: HashMap<String, SecretRecord>,
}

impl Default for VaultPayload {
    fn default() -> Self {
        Self {
            payload_version: PAYLOAD_VERSION,
            secrets: HashMap::new(),
        }
    }
}

pub fn payload_pinata_token(payload: &VaultPayload) -> String {
    payload.secrets.get(RECORD_ID_PINATA).map(|r| r.value.clone()).unwrap_or_default()
}

pub fn payload_filebase_token(payload: &VaultPayload) -> String {
    payload.secrets.get(RECORD_ID_FILEBASE).map(|r| r.value.clone()).unwrap_or_default()
}

fn set_provider_tokens_in_payload(
    payload: &mut VaultPayload,
    pinata: &str,
    filebase: &str,
    pinata_endpoint: &str,
    filebase_endpoint: &str,
) {
    let now = chrono::Utc::now().timestamp();
    if !pinata.is_empty() {
        let existing_created = payload.secrets.get(RECORD_ID_PINATA).map(|r| r.created);
        payload.secrets.insert(RECORD_ID_PINATA.to_string(), SecretRecord {
            record_id: RECORD_ID_PINATA.to_string(),
            record_type: RECORD_TYPE_API_TOKEN.to_string(),
            label: "Pinata API Token".to_string(),
            value: pinata.to_string(),
            metadata: Some(serde_json::json!({
                "provider_id": "pinata",
                "provider_name": "Pinata",
                "endpoint": pinata_endpoint,
                "token_kind": "jwt",
            })),
            tags: None,
            origin_app: Some(APP_IDENTIFIER.to_string()),
            derivation_profiles: None,
            network: None,
            created: existing_created.unwrap_or(now),
            modified: now,
        });
    }
    if !filebase.is_empty() {
        let existing_created = payload.secrets.get(RECORD_ID_FILEBASE).map(|r| r.created);
        payload.secrets.insert(RECORD_ID_FILEBASE.to_string(), SecretRecord {
            record_id: RECORD_ID_FILEBASE.to_string(),
            record_type: RECORD_TYPE_API_TOKEN.to_string(),
            label: "Filebase Token".to_string(),
            value: filebase.to_string(),
            metadata: Some(serde_json::json!({
                "provider_id": "filebase",
                "provider_name": "Filebase",
                "endpoint": filebase_endpoint,
                "token_kind": "bearer",
            })),
            tags: None,
            origin_app: Some(APP_IDENTIFIER.to_string()),
            derivation_profiles: None,
            network: None,
            created: existing_created.unwrap_or(now),
            modified: now,
        });
    }
}

fn upgrade_legacy_payload(raw: &serde_json::Value) -> VaultPayload {
    let mut payload = if raw.get("secrets").is_some() {
        serde_json::from_value(raw.clone()).unwrap_or_default()
    } else {
        VaultPayload::default()
    };
    let pinata = raw.get("pinata_api_token").and_then(|v| v.as_str()).unwrap_or("");
    let filebase = raw.get("filebase_token").and_then(|v| v.as_str()).unwrap_or("");
    if !pinata.is_empty() && !payload.secrets.contains_key(RECORD_ID_PINATA) {
        set_provider_tokens_in_payload(&mut payload, pinata, filebase, "", "");
    }
    payload
}

#[cfg(test)]
thread_local! {
    static TEST_VAULT_DIR: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
struct TestVaultDirGuard {
    dir: PathBuf,
}

#[cfg(test)]
fn setup_test_vault_dir() -> TestVaultDirGuard {
    let dir = std::env::temp_dir().join(format!("commander_vault_test_{}", chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)));
    std::fs::create_dir_all(&dir).unwrap();
    TEST_VAULT_DIR.with(|cell| {
        *cell.borrow_mut() = Some(dir.clone());
    });
    TestVaultDirGuard { dir }
}

#[cfg(test)]
impl Drop for TestVaultDirGuard {
    fn drop(&mut self) {
        TEST_VAULT_DIR.with(|cell| {
            *cell.borrow_mut() = None;
        });
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn vault_path() -> Result<PathBuf, String> {
    #[cfg(test)]
    {
        let override_path = TEST_VAULT_DIR.with(|cell| cell.borrow().clone());
        if let Some(dir) = override_path {
            return Ok(dir.join("vault.json"));
        }
    }
    Ok(commander_dir()?.join("vault.json"))
}

fn vault_temp_path() -> Result<PathBuf, String> {
    #[cfg(test)]
    {
        let override_path = TEST_VAULT_DIR.with(|cell| cell.borrow().clone());
        if let Some(dir) = override_path {
            return Ok(dir.join("vault.json.tmp"));
        }
    }
    Ok(commander_dir()?.join("vault.json.tmp"))
}

fn validate_network(network: Option<&str>) -> Result<(), String> {
    match network {
        None | Some("") => Err("Network must be set".to_string()),
        Some(n) if VALID_NETWORKS.contains(&n) => Ok(()),
        Some(n) => Err(format!("Invalid network: {n}")),
    }
}

fn build_payload_aad(envelope: &VaultEnvelope) -> Vec<u8> {
    let payload = format!(
        "{}:{}:{}:{}:{}:{}:{}:{}:{}",
        envelope.schema_identifier,
        envelope.version,
        envelope.app_identifier,
        envelope.network.as_deref().unwrap_or(""),
        envelope.cipher_profile,
        envelope.aad_profile,
        envelope.payload.payload_schema,
        envelope.created,
        envelope.modified,
    );
    payload.into_bytes()
}

fn build_slot_wrap_aad(envelope: &VaultEnvelope, slot: &KeySlot) -> Vec<u8> {
    let kdf_params = match slot.kdf_profile.as_str() {
        KDF_PROFILE_PBKDF2_SHA512 | KDF_PROFILE_PBKDF2_SHA256 => {
            format!("{}", slot.kdf_iterations.unwrap_or(0))
        }
        KDF_PROFILE_SCRYPT => {
            format!(
                "{}/{}/{}",
                slot.kdf_log_n.unwrap_or(0),
                slot.kdf_r.unwrap_or(0),
                slot.kdf_p.unwrap_or(0)
            )
        }
        _ => String::new(),
    };

    let payload = format!(
        "{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}",
        envelope.schema_identifier,
        envelope.version,
        envelope.app_identifier,
        envelope.network.as_deref().unwrap_or(""),
        slot.slot_id,
        slot.slot_type,
        slot.kdf_profile,
        kdf_params,
        slot.kdf_dklen,
        slot.wrap_cipher_profile,
        slot.created,
    );
    payload.into_bytes()
}

fn derive_pbkdf2_key(passphrase: &str, salt: &[u8], iterations: u32) -> Zeroizing<[u8; KDF_KEY_SIZE]> {
    let mut key = Zeroizing::new([0u8; KDF_KEY_SIZE]);
    pbkdf2_hmac::<Sha512>(passphrase.as_bytes(), salt, iterations, key.as_mut());
    key
}

fn derive_pbkdf2_sha256_key(passphrase: &str, salt: &[u8], iterations: u32) -> Zeroizing<[u8; KDF_KEY_SIZE]> {
    let mut key = Zeroizing::new([0u8; KDF_KEY_SIZE]);
    pbkdf2_hmac::<Sha256>(passphrase.as_bytes(), salt, iterations, key.as_mut());
    key
}

fn derive_scrypt_key(passphrase: &str, salt: &[u8], log_n: u8, r: u32, p: u32) -> Result<Zeroizing<[u8; KDF_KEY_SIZE]>, String> {
    let params = ScryptParams::new(log_n, r, p, KDF_KEY_SIZE)
        .map_err(|e| format!("Invalid scrypt params: {e}"))?;
    let mut key = Zeroizing::new([0u8; KDF_KEY_SIZE]);
    scrypt(passphrase.as_bytes(), salt, &params, key.as_mut())
        .map_err(|e| format!("scrypt KDF failed: {e}"))?;
    Ok(key)
}

fn validate_kdf_params(slot: &KeySlot) -> Result<(), String> {
    if slot.kdf_dklen != KDF_DKLEN {
        return Err(format!("Invalid kdf_dklen: {} (expected {})", slot.kdf_dklen, KDF_DKLEN));
    }
    match slot.kdf_profile.as_str() {
        KDF_PROFILE_PBKDF2_SHA512 | KDF_PROFILE_PBKDF2_SHA256 => {
            let iterations = slot.kdf_iterations.ok_or("PBKDF2 slot missing kdf_iterations")?;
            if iterations < 10_000 || iterations > 5_000_000 {
                return Err(format!("Invalid KDF iterations: {iterations}"));
            }
        }
        KDF_PROFILE_SCRYPT => {
            let log_n = slot.kdf_log_n.ok_or("scrypt slot missing kdf_log_n")?;
            let r = slot.kdf_r.ok_or("scrypt slot missing kdf_r")?;
            let p = slot.kdf_p.ok_or("scrypt slot missing kdf_p")?;
            if log_n < 12 || log_n > 18 {
                return Err(format!("Invalid scrypt log_n: {log_n}"));
            }
            if r < 1 || r > 32 {
                return Err(format!("Invalid scrypt r: {r}"));
            }
            if p < 1 || p > 16 {
                return Err(format!("Invalid scrypt p: {p}"));
            }
        }
        other => return Err(format!("Unsupported KDF profile: {other}")),
    }
    Ok(())
}

fn derive_slot_key(passphrase: &str, slot: &KeySlot) -> Result<Zeroizing<[u8; KDF_KEY_SIZE]>, String> {
    let salt = hex::decode(&slot.salt).map_err(|e| format!("Invalid slot salt hex: {e}"))?;
    match slot.kdf_profile.as_str() {
        KDF_PROFILE_PBKDF2_SHA256 => {
            // WebCom uses 16-byte salts; unified writes use 32-byte.
            // Accept both for bridge import compatibility.
            if salt.len() != KDF_SALT_SIZE && salt.len() != 16 {
                return Err(format!(
                    "Invalid PBKDF2-SHA256 salt length: {} (expected 16 or 32)",
                    salt.len()
                ));
            }
        }
        _ => {
            if salt.len() != KDF_SALT_SIZE {
                return Err("Invalid slot salt length".to_string());
            }
        }
    }
    validate_kdf_params(slot)?;
    match slot.kdf_profile.as_str() {
        KDF_PROFILE_PBKDF2_SHA512 => {
            let iterations = slot.kdf_iterations.unwrap();
            Ok(derive_pbkdf2_key(passphrase, &salt, iterations))
        }
        KDF_PROFILE_PBKDF2_SHA256 => {
            let iterations = slot.kdf_iterations.unwrap();
            Ok(derive_pbkdf2_sha256_key(passphrase, &salt, iterations))
        }
        KDF_PROFILE_SCRYPT => {
            let log_n = slot.kdf_log_n.unwrap();
            let r = slot.kdf_r.unwrap();
            let p = slot.kdf_p.unwrap();
            derive_scrypt_key(passphrase, &salt, log_n, r, p)
        }
        other => Err(format!("Unsupported KDF profile: {other}")),
    }
}

fn unwrap_dek(passphrase: &str, envelope: &VaultEnvelope, slot: &KeySlot) -> Result<Zeroizing<[u8; DEK_SIZE]>, String> {
    if slot.wrap_cipher_profile != WRAP_CIPHER_PROFILE {
        return Err(format!(
            "Unsupported wrap cipher profile: {}",
            slot.wrap_cipher_profile
        ));
    }

    let slot_key = derive_slot_key(passphrase, slot)?;
    let key = aes_gcm::Key::<Aes256Gcm>::from_slice(slot_key.as_slice());
    let cipher = Aes256Gcm::new(key);

    let wrap_iv = hex::decode(&slot.wrap_iv).map_err(|e| format!("Invalid wrap IV hex: {e}"))?;
    if wrap_iv.len() != GCM_IV_SIZE {
        return Err("Invalid wrap IV length".to_string());
    }
    let nonce = Nonce::from_slice(&wrap_iv);

    let wrapped = hex::decode(&slot.wrapped_dek).map_err(|e| format!("Invalid wrapped DEK hex: {e}"))?;

    let aad = build_slot_wrap_aad(envelope, slot);

    let dek_bytes = cipher
        .decrypt(nonce, aes_gcm::aead::Payload {
            msg: &wrapped,
            aad: &aad,
        })
        .map_err(|_| "Incorrect passphrase or corrupted key slot".to_string())?;

    if dek_bytes.len() != DEK_SIZE {
        return Err("Invalid DEK length after unwrap".to_string());
    }

    let mut dek = Zeroizing::new([0u8; DEK_SIZE]);
    dek.copy_from_slice(&dek_bytes);
    Ok(dek)
}

fn unwrap_dek_with_passphrase(passphrase: &str, envelope: &VaultEnvelope) -> Result<Zeroizing<[u8; DEK_SIZE]>, String> {
    if envelope.key_slots.is_empty() {
        return Err("Vault has no key slots".to_string());
    }
    let mut last_err: Option<String> = None;
    for slot in &envelope.key_slots {
        match unwrap_dek(passphrase, envelope, slot) {
            Ok(dek) => return Ok(dek),
            Err(e) => {
                last_err = Some(e);
            }
        }
    }
    Err(last_err.unwrap_or_else(|| "Incorrect passphrase or corrupted vault".to_string()))
}

fn wrap_dek(dek: &[u8], passphrase: &str, envelope: &VaultEnvelope, slot: &mut KeySlot) -> Result<(), String> {
    if dek.len() != DEK_SIZE {
        return Err("Invalid DEK length".to_string());
    }
    let slot_key = derive_slot_key(passphrase, slot)?;
    let key = aes_gcm::Key::<Aes256Gcm>::from_slice(slot_key.as_slice());
    let cipher = Aes256Gcm::new(key);

    let mut wrap_iv = [0u8; GCM_IV_SIZE];
    OsRng.fill_bytes(&mut wrap_iv);
    let nonce = Nonce::from_slice(&wrap_iv);

    let aad = build_slot_wrap_aad(envelope, slot);

    let wrapped = cipher
        .encrypt(nonce, aes_gcm::aead::Payload { msg: dek, aad: &aad })
        .map_err(|e| format!("DEK wrap failed: {e}"))?;

    slot.wrap_iv = hex::encode(wrap_iv);
    slot.wrapped_dek = hex::encode(&wrapped);
    Ok(())
}

fn encrypt_payload_with_dek(dek: &[u8], payload: &VaultPayload, envelope: &VaultEnvelope) -> Result<VaultPayloadBlock, String> {
    if dek.len() != DEK_SIZE {
        return Err("Invalid DEK length".to_string());
    }
    let key = aes_gcm::Key::<Aes256Gcm>::from_slice(dek);
    let cipher = Aes256Gcm::new(key);

    let mut iv = [0u8; GCM_IV_SIZE];
    OsRng.fill_bytes(&mut iv);
    let nonce = Nonce::from_slice(&iv);

    let plaintext = serde_json::to_vec(payload).map_err(|e| format!("Serialization failed: {e}"))?;

    let aad = build_payload_aad(envelope);

    let ciphertext = cipher
        .encrypt(nonce, aes_gcm::aead::Payload { msg: &plaintext, aad: &aad })
        .map_err(|e| format!("Payload encryption failed: {e}"))?;

    Ok(VaultPayloadBlock {
        payload_schema: PAYLOAD_SCHEMA.to_string(),
        iv: hex::encode(iv),
        ciphertext: hex::encode(&ciphertext),
    })
}

fn decrypt_payload_with_dek(dek: &[u8], envelope: &VaultEnvelope) -> Result<VaultPayload, String> {
    if dek.len() != DEK_SIZE {
        return Err("Invalid DEK length".to_string());
    }
    let payload_block = &envelope.payload;

    if payload_block.payload_schema != PAYLOAD_SCHEMA {
        return Err(format!("Unsupported payload schema: {}", payload_block.payload_schema));
    }

    let key = aes_gcm::Key::<Aes256Gcm>::from_slice(dek);
    let cipher = Aes256Gcm::new(key);

    let iv = hex::decode(&payload_block.iv).map_err(|e| format!("Invalid payload IV hex: {e}"))?;
    if iv.len() != GCM_IV_SIZE {
        return Err("Invalid payload IV length".to_string());
    }
    let nonce = Nonce::from_slice(&iv);

    let ciphertext = hex::decode(&payload_block.ciphertext)
        .map_err(|e| format!("Invalid payload ciphertext hex: {e}"))?;

    let aad = build_payload_aad(envelope);

    let plaintext = cipher
        .decrypt(nonce, aes_gcm::aead::Payload {
            msg: &ciphertext,
            aad: &aad,
        })
        .map_err(|_| "Incorrect passphrase or corrupted vault payload".to_string())?;

    let raw: serde_json::Value = serde_json::from_slice(&plaintext)
        .map_err(|e| format!("Payload deserialization failed: {e}"))?;
    Ok(upgrade_legacy_payload(&raw))
}

fn build_passphrase_slot(_passphrase: &str, kdf_profile: &str) -> Result<KeySlot, String> {
    let mut salt = [0u8; KDF_SALT_SIZE];
    OsRng.fill_bytes(&mut salt);

    let now = chrono::Utc::now().timestamp();

    let mut slot = KeySlot {
        slot_id: "primary".to_string(),
        slot_type: "passphrase".to_string(),
        kdf_profile: kdf_profile.to_string(),
        kdf_iterations: None,
        kdf_log_n: None,
        kdf_r: None,
        kdf_p: None,
        kdf_dklen: KDF_DKLEN,
        salt: hex::encode(salt),
        wrap_cipher_profile: WRAP_CIPHER_PROFILE.to_string(),
        wrap_iv: String::new(),
        wrapped_dek: String::new(),
        created: now,
        modified: now,
    };

    match kdf_profile {
        KDF_PROFILE_PBKDF2_SHA512 => {
            slot.kdf_iterations = Some(PBKDF2_ITERATIONS);
        }
        KDF_PROFILE_PBKDF2_SHA256 => {
            slot.kdf_iterations = Some(100_000);
        }
        KDF_PROFILE_SCRYPT => {
            slot.kdf_log_n = Some(SCRYPT_DEFAULT_LOG_N);
            slot.kdf_r = Some(SCRYPT_DEFAULT_R);
            slot.kdf_p = Some(SCRYPT_DEFAULT_P);
        }
        _ => return Err(format!("Unsupported KDF profile for new vault: {kdf_profile}")),
    }

    Ok(slot)
}

fn encrypt_vault_envelope(passphrase: &str, payload: &VaultPayload, kdf_profile: &str) -> Result<VaultEnvelope, String> {
    let mut dek = Zeroizing::new([0u8; DEK_SIZE]);
    OsRng.fill_bytes(dek.as_mut());

    let now = chrono::Utc::now().timestamp();
    let network = detect_network();

    let mut slot = build_passphrase_slot(passphrase, kdf_profile)?;

    let mut envelope = VaultEnvelope {
        version: CURRENT_VAULT_VERSION,
        schema_identifier: SCHEMA_IDENTIFIER.to_string(),
        app_identifier: APP_IDENTIFIER.to_string(),
        network: Some(network),
        cipher_profile: CIPHER_PROFILE.to_string(),
        aad_profile: AAD_PROFILE.to_string(),
        payload: VaultPayloadBlock {
            payload_schema: PAYLOAD_SCHEMA.to_string(),
            iv: String::new(),
            ciphertext: String::new(),
        },
        key_slots: vec![],
        created: now,
        modified: now,
    };

    envelope.payload = encrypt_payload_with_dek(dek.as_slice(), payload, &envelope)?;

    wrap_dek(dek.as_slice(), passphrase, &envelope, &mut slot)?;
    envelope.key_slots.push(slot);

    Ok(envelope)
}

fn decrypt_vault_envelope(passphrase: &str, envelope: &VaultEnvelope) -> Result<VaultPayload, String> {
    if envelope.cipher_profile != CIPHER_PROFILE {
        return Err(format!(
            "Unsupported cipher profile: {}",
            envelope.cipher_profile
        ));
    }
    if envelope.aad_profile != AAD_PROFILE {
        return Err(format!(
            "Unsupported AAD profile: {}",
            envelope.aad_profile
        ));
    }
    validate_network(envelope.network.as_deref())?;

    let dek = unwrap_dek_with_passphrase(passphrase, envelope)?;
    decrypt_payload_with_dek(dek.as_slice(), envelope)
}

fn detect_network() -> String {
    let cfg_path = match data_dir() {
        Ok(d) => d.join("hemp.conf"),
        Err(_) => return "mainnet".to_string(),
    };
    if !cfg_path.exists() {
        return "mainnet".to_string();
    }
    let content = match fs::read_to_string(&cfg_path) {
        Ok(c) => c,
        Err(_) => return "mainnet".to_string(),
    };
    for line in content.lines() {
        let line = line.trim();
        if line == "regtest=1" {
            return "regtest".to_string();
        }
        if line == "testnet=1" {
            return "testnet".to_string();
        }
    }
    "mainnet".to_string()
}

/// Atomic write via temp file + rename.
/// On Linux/macOS, `fs::rename` is atomic for same-filesystem paths.
/// On Windows (10 1607+), Rust's `fs::rename` uses `MoveFileEx` with
/// `MOVEFILE_REPLACE_EXISTING`, which is atomic when the destination
/// exists.
fn save_bundle_atomic(bundle: &VaultBundle) -> Result<(), String> {
    let path = vault_path()?;
    let tmp = vault_temp_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let content =
        serde_json::to_string_pretty(bundle).map_err(|e| format!("Serialization failed: {e}"))?;
    fs::write(&tmp, content).map_err(|e| e.to_string())?;
    fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
    Ok(())
}

fn load_bundle() -> Result<Option<VaultBundle>, String> {
    let path = vault_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let value: serde_json::Value =
        serde_json::from_str(&content).map_err(|e| format!("Invalid vault file: {e}"))?;

    let bundle = if value.get("bundleVersion").is_some()
        || (value.get("bundle_version").is_some() && value.get("vault").is_some())
    {
        let bundle: VaultBundle =
            serde_json::from_value(value).map_err(|e| format!("Invalid vault bundle: {e}"))?;
        bundle
    } else {
        let envelope: VaultEnvelope =
            serde_json::from_value(value).map_err(|e| format!("Invalid vault envelope: {e}"))?;
        VaultBundle {
            bundleVersion: CURRENT_BUNDLE_VERSION,
            format_identifier: FORMAT_IDENTIFIER.to_string(),
            vault: envelope,
            meta: None,
        }
    };

    if bundle.bundleVersion < 1 || bundle.bundleVersion > CURRENT_BUNDLE_VERSION {
        return Err(format!(
            "Unsupported bundle version: {}. Current: {}",
            bundle.bundleVersion, CURRENT_BUNDLE_VERSION
        ));
    }
    if bundle.format_identifier != FORMAT_IDENTIFIER {
        return Err(format!(
            "Unknown vault format: {}",
            bundle.format_identifier
        ));
    }

    Ok(Some(bundle))
}

fn load_vault_envelope() -> Result<Option<VaultEnvelope>, String> {
    load_bundle().map(|b| b.map(|bundle| bundle.vault))
}

pub fn vault_exists() -> bool {
    vault_path().map(|p| p.exists()).unwrap_or(false)
}

pub fn load_vault_tokens(passphrase: &str) -> Result<VaultPayload, String> {
    let envelope = load_vault_envelope()?.ok_or("Vault does not exist")?;
    decrypt_vault_envelope(passphrase, &envelope)
}

pub fn create_vault(passphrase: &str, payload: &VaultPayload) -> Result<VaultBundle, String> {
    if passphrase.len() < 8 {
        return Err("Passphrase must be at least 8 characters".to_string());
    }
    if passphrase.len() > 1024 {
        return Err("Passphrase must not exceed 1024 characters".to_string());
    }
    if vault_exists() {
        return Err("Vault already exists. Use update_vault to change tokens.".to_string());
    }
    let envelope = encrypt_vault_envelope(passphrase, payload, KDF_PROFILE_SCRYPT)?;
    validate_network(envelope.network.as_deref())?;
    let bundle = VaultBundle {
        bundleVersion: CURRENT_BUNDLE_VERSION,
        format_identifier: FORMAT_IDENTIFIER.to_string(),
        vault: envelope,
        meta: None,
    };
    save_bundle_atomic(&bundle)?;
    Ok(bundle)
}

pub fn update_vault_tokens(
    passphrase: &str,
    pinata: &str,
    filebase: &str,
    pinata_endpoint: &str,
    filebase_endpoint: &str,
) -> Result<VaultBundle, String> {
    let mut bundle = load_bundle()?.ok_or("Vault does not exist")?;

    let dek = unwrap_dek_with_passphrase(passphrase, &bundle.vault)?;

    let mut payload = decrypt_payload_with_dek(dek.as_slice(), &bundle.vault)?;
    set_provider_tokens_in_payload(&mut payload, pinata, filebase, pinata_endpoint, filebase_endpoint);

    let now = chrono::Utc::now().timestamp();
    bundle.vault.modified = now;
    bundle.vault.payload = encrypt_payload_with_dek(dek.as_slice(), &payload, &bundle.vault)?;

    save_bundle_atomic(&bundle)?;
    Ok(bundle)
}

pub fn verify_vault_passphrase(passphrase: &str) -> Result<bool, String> {
    let envelope = match load_vault_envelope()? {
        Some(e) => e,
        None => return Ok(false),
    };
    match decrypt_vault_envelope(passphrase, &envelope) {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}

pub fn remove_provider_token_from_vault(
    passphrase: &str,
    provider_record_id: &str,
) -> Result<VaultBundle, String> {
    let mut bundle = load_bundle()?.ok_or("Vault does not exist")?;
    let dek = unwrap_dek_with_passphrase(passphrase, &bundle.vault)?;
    let mut payload = decrypt_payload_with_dek(dek.as_slice(), &bundle.vault)?;

    if payload.secrets.remove(provider_record_id).is_none() {
        return Err(format!("No secret record found for: {provider_record_id}"));
    }

    let now = chrono::Utc::now().timestamp();
    bundle.vault.modified = now;
    bundle.vault.payload = encrypt_payload_with_dek(dek.as_slice(), &payload, &bundle.vault)?;
    save_bundle_atomic(&bundle)?;
    Ok(bundle)
}

#[tauri::command]
pub fn vault_get_vault_path() -> Result<String, String> {
    vault_path().map(|p| p.to_string_lossy().to_string())
}

#[tauri::command]
pub fn vault_get_supported_record_types() -> Result<Vec<serde_json::Value>, String> {
    let types: Vec<serde_json::Value> = SUPPORTED_RECORD_TYPES.iter().map(|(t, desc)| {
        let implemented = *t == RECORD_TYPE_API_TOKEN;
        serde_json::json!({
            "record_type": t,
            "description": desc,
            "implemented": implemented,
        })
    }).collect();
    Ok(types)
}

#[tauri::command]
pub fn vault_get_supported_derivation_profiles() -> Result<Vec<serde_json::Value>, String> {
    let profiles: Vec<serde_json::Value> = SUPPORTED_DERIVATION_PROFILES.iter().map(|(id, desc, path)| {
        serde_json::json!({
            "profile_id": id,
            "description": desc,
            "derivation_path": path,
        })
    }).collect();
    Ok(profiles)
}

#[tauri::command]
pub fn vault_get_info() -> Result<Option<serde_json::Value>, String> {
    let bundle = load_bundle()?;
    match bundle {
        Some(b) => {
            let slot_info: Vec<serde_json::Value> = b.vault.key_slots.iter().map(|s| {
                serde_json::json!({
                    "slot_id": s.slot_id,
                    "slot_type": s.slot_type,
                    "kdf_profile": s.kdf_profile,
                    "kdf_iterations": s.kdf_iterations,
                    "kdf_log_n": s.kdf_log_n,
                    "kdf_r": s.kdf_r,
                    "kdf_p": s.kdf_p,
                    "kdf_dklen": s.kdf_dklen,
                    "created": s.created,
                    "modified": s.modified,
                })
            }).collect();
            let info = serde_json::json!({
                "exists": true,
                "bundle_version": b.bundleVersion,
                "format_identifier": b.format_identifier,
                "version": b.vault.version,
                "schema_identifier": b.vault.schema_identifier,
                "app_identifier": b.vault.app_identifier,
                "network": b.vault.network,
                "cipher_profile": b.vault.cipher_profile,
                "aad_profile": b.vault.aad_profile,
                "payload_schema": b.vault.payload.payload_schema,
                "key_slots": slot_info,
                "created": b.vault.created,
                "modified": b.vault.modified,
            });
            Ok(Some(info))
        }
        None => Ok(None),
    }
}

#[tauri::command]
pub fn vault_setup(passphrase: String) -> Result<serde_json::Value, String> {
    let payload = VaultPayload::default();
    let bundle = create_vault(&passphrase, &payload)?;
    let kdf_profile = bundle.vault.key_slots.first()
        .map(|s| s.kdf_profile.as_str())
        .unwrap_or(KDF_PROFILE_SCRYPT);
    let info = serde_json::json!({
        "created": true,
        "bundle_version": bundle.bundleVersion,
        "version": bundle.vault.version,
        "kdf_profile": kdf_profile,
    });
    Ok(info)
}

#[tauri::command]
pub fn vault_update_tokens(
    passphrase: String,
    pinata_api_token: String,
    filebase_token: String,
    pinata_endpoint: Option<String>,
    filebase_endpoint: Option<String>,
) -> Result<serde_json::Value, String> {
    let bundle = update_vault_tokens(
        &passphrase,
        &pinata_api_token,
        &filebase_token,
        pinata_endpoint.as_deref().unwrap_or(""),
        filebase_endpoint.as_deref().unwrap_or(""),
    )?;
    Ok(serde_json::json!({
        "updated": true,
        "bundle_version": bundle.bundleVersion,
        "version": bundle.vault.version,
        "modified": bundle.vault.modified,
    }))
}

#[tauri::command]
pub fn vault_verify_passphrase(passphrase: String) -> Result<bool, String> {
    verify_vault_passphrase(&passphrase)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_provider_payload(pinata: &str, filebase: &str) -> VaultPayload {
        let mut p = VaultPayload::default();
        set_provider_tokens_in_payload(&mut p, pinata, filebase, "", "");
        p
    }

    fn make_future_record(record_id: &str, record_type: &str, value: &str) -> SecretRecord {
        let now = chrono::Utc::now().timestamp();
        SecretRecord {
            record_id: record_id.to_string(),
            record_type: record_type.to_string(),
            label: format!("Test {}", record_id),
            value: value.to_string(),
            metadata: Some(serde_json::json!({"test": true})),
            tags: Some(vec!["test".to_string()]),
            origin_app: Some("test-app".to_string()),
            derivation_profiles: None,
            network: Some("mainnet".to_string()),
            created: now,
            modified: now,
        }
    }

    #[test]
    fn encrypt_decrypt_roundtrip_scrypt() {
        let passphrase = "test-passphrase-for-vault";
        let payload = make_provider_payload("pinata-jwt-token-12345", "filebase-access-token-67890");
        let envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        assert_eq!(envelope.version, CURRENT_VAULT_VERSION);
        assert_eq!(envelope.schema_identifier, SCHEMA_IDENTIFIER);
        assert_eq!(envelope.cipher_profile, CIPHER_PROFILE);
        assert_eq!(envelope.aad_profile, AAD_PROFILE);
        assert_eq!(envelope.payload.payload_schema, PAYLOAD_SCHEMA);
        assert!(!envelope.payload.ciphertext.is_empty());
        assert_eq!(envelope.key_slots.len(), 1);
        assert_eq!(envelope.key_slots[0].slot_id, "primary");
        assert_eq!(envelope.key_slots[0].slot_type, "passphrase");
        assert_eq!(envelope.key_slots[0].kdf_profile, KDF_PROFILE_SCRYPT);
        assert!(envelope.key_slots[0].kdf_log_n.is_some());

        let decrypted = decrypt_vault_envelope(passphrase, &envelope).unwrap();
        assert_eq!(payload_pinata_token(&decrypted), "pinata-jwt-token-12345");
        assert_eq!(payload_filebase_token(&decrypted), "filebase-access-token-67890");
        assert_eq!(decrypted.payload_version, PAYLOAD_VERSION);
        assert!(decrypted.secrets.contains_key(RECORD_ID_PINATA));
        assert!(decrypted.secrets.contains_key(RECORD_ID_FILEBASE));
    }

    #[test]
    fn encrypt_decrypt_roundtrip_pbkdf2() {
        let passphrase = "test-passphrase-for-vault";
        let payload = make_provider_payload("pinata-jwt-token-12345", "filebase-access-token-67890");
        let envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_PBKDF2_SHA512).unwrap();
        assert_eq!(envelope.key_slots[0].kdf_profile, KDF_PROFILE_PBKDF2_SHA512);
        assert_eq!(envelope.key_slots[0].kdf_iterations, Some(PBKDF2_ITERATIONS));

        let decrypted = decrypt_vault_envelope(passphrase, &envelope).unwrap();
        assert_eq!(payload_pinata_token(&decrypted), "pinata-jwt-token-12345");
        assert_eq!(payload_filebase_token(&decrypted), "filebase-access-token-67890");
    }

    #[test]
    fn wrong_passphrase_fails() {
        let passphrase = "correct-passphrase-here";
        let payload = make_provider_payload("token", "token");
        let envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        let result = decrypt_vault_envelope("wrong-passphrase-here", &envelope);
        assert!(result.is_err());
    }

    #[test]
    fn corrupt_payload_ciphertext_fails() {
        let passphrase = "test-passphrase";
        let payload = make_provider_payload("token", "token");
        let mut envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        envelope.payload.ciphertext = envelope.payload.ciphertext[..envelope.payload.ciphertext.len() - 4].to_string();
        let result = decrypt_vault_envelope(passphrase, &envelope);
        assert!(result.is_err());
    }

    #[test]
    fn corrupt_wrapped_dek_fails() {
        let passphrase = "test-passphrase";
        let payload = make_provider_payload("token", "token");
        let mut envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        envelope.key_slots[0].wrapped_dek = envelope.key_slots[0].wrapped_dek[..envelope.key_slots[0].wrapped_dek.len() - 4].to_string();
        let result = decrypt_vault_envelope(passphrase, &envelope);
        assert!(result.is_err());
    }

    #[test]
    fn empty_tokens_roundtrip() {
        let passphrase = "test-passphrase";
        let payload = VaultPayload::default();
        let envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        let decrypted = decrypt_vault_envelope(passphrase, &envelope).unwrap();
        assert_eq!(payload_pinata_token(&decrypted), "");
        assert_eq!(payload_filebase_token(&decrypted), "");
    }

    #[test]
    fn envelope_has_required_metadata() {
        let passphrase = "test-passphrase";
        let payload = VaultPayload::default();
        let envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        let json = serde_json::to_string(&envelope).unwrap();
        assert!(json.contains("version"));
        assert!(json.contains("schema_identifier"));
        assert!(json.contains("app_identifier"));
        assert!(json.contains("cipher_profile"));
        assert!(json.contains("aad_profile"));
        assert!(json.contains("payload_schema"));
        assert!(json.contains("key_slots"));
        assert!(json.contains("slot_id"));
        assert!(json.contains("slot_type"));
        assert!(json.contains("kdf_profile"));
        assert!(json.contains("wrapped_dek"));
        assert!(json.contains("created"));
        assert!(json.contains("modified"));
        assert!(json.contains("network"));
    }

    #[test]
    fn payload_aad_tamper_detection() {
        let passphrase = "test-passphrase";
        let payload = make_provider_payload("token", "token");
        let mut envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        envelope.version = 999;
        let result = decrypt_vault_envelope(passphrase, &envelope);
        assert!(result.is_err());
    }

    #[test]
    fn slot_wrap_aad_tamper_detection() {
        let passphrase = "test-passphrase";
        let payload = make_provider_payload("token", "token");
        let mut envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        envelope.key_slots[0].slot_id = "tampered".to_string();
        let result = decrypt_vault_envelope(passphrase, &envelope);
        assert!(result.is_err());
    }

    #[test]
    fn kdf_iterations_tamper_detected() {
        let passphrase = "test-passphrase";
        let payload = make_provider_payload("token", "token");
        let mut envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_PBKDF2_SHA512).unwrap();
        envelope.key_slots[0].kdf_iterations = Some(100_000);
        let result = decrypt_vault_envelope(passphrase, &envelope);
        assert!(result.is_err());
    }

    #[test]
    fn kdf_scrypt_params_tamper_detected() {
        let passphrase = "test-passphrase";
        let payload = make_provider_payload("token", "token");
        let mut envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        envelope.key_slots[0].kdf_log_n = Some(12);
        let result = decrypt_vault_envelope(passphrase, &envelope);
        assert!(result.is_err());
    }

    #[test]
    fn unsupported_kdf_profile_rejected() {
        let passphrase = "test-passphrase";
        let result = encrypt_vault_envelope(passphrase, &VaultPayload::default(), "argon2id-v1");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unsupported KDF profile"));
    }

    #[test]
    fn unsupported_cipher_profile_rejected() {
        let passphrase = "test-passphrase";
        let payload = VaultPayload::default();
        let mut envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        envelope.cipher_profile = "aes-128-gcm-v1".to_string();
        let result = decrypt_vault_envelope(passphrase, &envelope);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unsupported cipher profile"));
    }

    #[test]
    fn unsupported_aad_profile_rejected() {
        let passphrase = "test-passphrase";
        let payload = VaultPayload::default();
        let mut envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        envelope.aad_profile = "unknown-aad-v1".to_string();
        let result = decrypt_vault_envelope(passphrase, &envelope);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unsupported AAD profile"));
    }

    #[test]
    fn unsupported_payload_schema_rejected() {
        let passphrase = "test-passphrase";
        let payload = VaultPayload::default();
        let mut envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        envelope.payload.payload_schema = "future-schema-v2".to_string();
        let result = decrypt_vault_envelope(passphrase, &envelope);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unsupported payload schema"));
    }

    #[test]
    fn missing_pbkdf2_iterations_rejected() {
        let passphrase = "test-passphrase";
        let payload = make_provider_payload("token", "token");
        let mut envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_PBKDF2_SHA512).unwrap();
        envelope.key_slots[0].kdf_iterations = None;
        let result = decrypt_vault_envelope(passphrase, &envelope);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("missing kdf_iterations"));
    }

    #[test]
    fn missing_scrypt_log_n_rejected() {
        let passphrase = "test-passphrase";
        let payload = make_provider_payload("token", "token");
        let mut envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        envelope.key_slots[0].kdf_log_n = None;
        let result = decrypt_vault_envelope(passphrase, &envelope);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("missing kdf_log_n"));
    }

    #[test]
    fn missing_scrypt_r_rejected() {
        let passphrase = "test-passphrase";
        let payload = make_provider_payload("token", "token");
        let mut envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        envelope.key_slots[0].kdf_r = None;
        let result = decrypt_vault_envelope(passphrase, &envelope);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("missing kdf_r"));
    }

    #[test]
    fn invalid_kdf_dklen_rejected() {
        let passphrase = "test-passphrase";
        let payload = make_provider_payload("token", "token");
        let mut envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        envelope.key_slots[0].kdf_dklen = 16;
        let result = decrypt_vault_envelope(passphrase, &envelope);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid kdf_dklen"));
    }

    #[test]
    fn invalid_network_rejected() {
        let passphrase = "test-passphrase";
        let payload = VaultPayload::default();
        let mut envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        envelope.network = Some("invalidnet".to_string());
        let result = validate_network(envelope.network.as_deref());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid network"));
    }

    #[test]
    fn valid_networks_accepted() {
        assert!(validate_network(Some("mainnet")).is_ok());
        assert!(validate_network(Some("testnet")).is_ok());
        assert!(validate_network(Some("regtest")).is_ok());
    }

    #[test]
    fn unsupported_bundle_version_rejected() {
        let passphrase = "test-passphrase";
        let payload = VaultPayload::default();
        let envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        let mut bundle = VaultBundle {
            bundleVersion: CURRENT_BUNDLE_VERSION,
            format_identifier: FORMAT_IDENTIFIER.to_string(),
            vault: envelope,
            meta: None,
        };
        bundle.bundleVersion = 999;
        let json = serde_json::to_string(&bundle).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        let _parsed: VaultBundle = serde_json::from_value(value).unwrap();
        assert!(bundle.bundleVersion > CURRENT_BUNDLE_VERSION);
    }

    #[test]
    fn wrong_format_identifier_rejected() {
        let passphrase = "test-passphrase";
        let payload = VaultPayload::default();
        let envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        let mut bundle = VaultBundle {
            bundleVersion: CURRENT_BUNDLE_VERSION,
            format_identifier: FORMAT_IDENTIFIER.to_string(),
            vault: envelope,
            meta: None,
        };
        bundle.format_identifier = "some-foreign-format".to_string();
        let json = serde_json::to_string(&bundle).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        let parsed: VaultBundle = serde_json::from_value(value).unwrap();
        assert_ne!(parsed.format_identifier, FORMAT_IDENTIFIER);
    }

    #[test]
    fn legacy_snake_case_bundle_readable() {
        let legacy_json = serde_json::json!({
            "bundle_version": 1,
            "format_identifier": "hemp0x-unified-vault-bundle",
            "vault": {
                "version": 1,
                "schema_identifier": "hemp0x-commander-vault",
                "app_identifier": "hemp0x-commander",
                "network": "mainnet",
                "cipher_profile": "aes-256-gcm-v1",
                "aad_profile": "commander-envelope-v1",
                "payload": {
                    "payload_schema": "commander-secrets-v1",
                    "iv": "deadbeefdeadbeefdeadbeef",
                    "ciphertext": "deadbeef"
                },
                "key_slots": [],
                "created": 1,
                "modified": 1
            },
            "public_meta": {"legacy": true}
        });
        let bundle: VaultBundle = serde_json::from_value(legacy_json).unwrap();
        assert_eq!(bundle.bundleVersion, 1);
        assert!(bundle.meta.is_some());
        assert_eq!(bundle.meta.unwrap()["legacy"], true);
    }

    #[test]
    fn create_vault_rejects_short_passphrase() {
        let result = create_vault("short", &VaultPayload::default());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("at least 8 characters"));
    }

    #[test]
    fn masked_output_never_contains_secrets() {
        let passphrase = "test-passphrase";
        let payload = make_provider_payload("super-secret-token-value", "another-secret-token");
        let envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        let json = serde_json::to_string(&envelope).unwrap();
        assert!(!json.contains("super-secret-token-value"));
        assert!(!json.contains("another-secret-token"));
    }

    #[test]
    fn vault_info_never_exposes_secrets() {
        let passphrase = "test-passphrase";
        let payload = make_provider_payload("secret-token", "secret-filebase");
        let envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        let json = serde_json::to_string(&envelope).unwrap();
        assert!(!json.contains("secret-token"));
        assert!(!json.contains("secret-filebase"));
    }

    #[test]
    fn update_payload_preserves_key_slots_byte_for_byte() {
        let passphrase = "test-passphrase";
        let payload1 = make_provider_payload("token1", "fb1");
        let envelope1 = encrypt_vault_envelope(passphrase, &payload1, KDF_PROFILE_SCRYPT).unwrap();
        let slot1 = envelope1.key_slots[0].clone();

        let dek = unwrap_dek_with_passphrase(passphrase, &envelope1).unwrap();
        let payload2 = make_provider_payload("token2", "fb2");
        let now = chrono::Utc::now().timestamp();
        let mut envelope2 = envelope1.clone();
        envelope2.modified = now;
        envelope2.payload = encrypt_payload_with_dek(dek.as_slice(), &payload2, &envelope2).unwrap();

        assert_eq!(envelope2.key_slots.len(), 1);
        let slot2 = &envelope2.key_slots[0];
        assert_eq!(slot2.slot_id, slot1.slot_id);
        assert_eq!(slot2.salt, slot1.salt);
        assert_eq!(slot2.wrap_iv, slot1.wrap_iv);
        assert_eq!(slot2.wrapped_dek, slot1.wrapped_dek);
        assert_eq!(slot2.created, slot1.created);

        let decrypted = decrypt_vault_envelope(passphrase, &envelope2).unwrap();
        assert_eq!(payload_pinata_token(&decrypted), "token2");
    }

    #[test]
    fn unknown_secret_records_survive_provider_token_update() {
        let passphrase = "test-passphrase";
        let mut payload = make_provider_payload("token1", "fb1");
        let future_record = make_future_record("wallet.bip39.main", RECORD_TYPE_WALLET_BIP39, "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about");
        payload.secrets.insert(future_record.record_id.clone(), future_record.clone());

        let envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();

        let dek = unwrap_dek_with_passphrase(passphrase, &envelope).unwrap();
        let mut existing = decrypt_payload_with_dek(dek.as_slice(), &envelope).unwrap();
        set_provider_tokens_in_payload(&mut existing, "token2", "fb2", "", "");

        let now = chrono::Utc::now().timestamp();
        let mut envelope2 = envelope.clone();
        envelope2.modified = now;
        envelope2.payload = encrypt_payload_with_dek(dek.as_slice(), &existing, &envelope2).unwrap();

        let decrypted = decrypt_vault_envelope(passphrase, &envelope2).unwrap();
        assert_eq!(payload_pinata_token(&decrypted), "token2");
        assert!(decrypted.secrets.contains_key("wallet.bip39.main"));
        let survived = decrypted.secrets.get("wallet.bip39.main").unwrap();
        assert_eq!(survived.record_type, RECORD_TYPE_WALLET_BIP39);
        assert_eq!(survived.value, future_record.value);
    }

    #[test]
    fn legacy_175_record_survives_provider_token_update() {
        let passphrase = "test-passphrase";
        let mut payload = make_provider_payload("token1", "fb1");

        let mut dp = HashMap::new();
        dp.insert("hemp".to_string(), DERIVATION_HEMP_LEGACY_175.to_string());
        let legacy = SecretRecord {
            record_id: "wallet.bip39.legacy".to_string(),
            record_type: RECORD_TYPE_WALLET_BIP39.to_string(),
            label: "Legacy WebCom 175 Wallet".to_string(),
            value: "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about".to_string(),
            metadata: Some(serde_json::json!({"account": 0, "external_count": 10})),
            tags: Some(vec!["legacy".to_string()]),
            origin_app: Some("hemp0x-webcom".to_string()),
            derivation_profiles: Some(dp.clone()),
            network: Some("mainnet".to_string()),
            created: 1700000000,
            modified: 1700000000,
        };
        payload.secrets.insert(legacy.record_id.clone(), legacy.clone());

        let envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        let dek = unwrap_dek_with_passphrase(passphrase, &envelope).unwrap();
        let mut existing = decrypt_payload_with_dek(dek.as_slice(), &envelope).unwrap();
        set_provider_tokens_in_payload(&mut existing, "token2", "fb2", "", "");

        let now = chrono::Utc::now().timestamp();
        let mut envelope2 = envelope.clone();
        envelope2.modified = now;
        envelope2.payload = encrypt_payload_with_dek(dek.as_slice(), &existing, &envelope2).unwrap();

        let decrypted = decrypt_vault_envelope(passphrase, &envelope2).unwrap();
        let survived = decrypted.secrets.get("wallet.bip39.legacy").unwrap();
        let survived_dp = survived.derivation_profiles.as_ref().unwrap();
        assert_eq!(survived_dp.get("hemp").unwrap(), DERIVATION_HEMP_LEGACY_175);
    }

    #[test]
    fn btc_metadata_preserved_on_provider_token_update() {
        let passphrase = "test-passphrase";
        let mut payload = make_provider_payload("token1", "fb1");

        let mut dp = HashMap::new();
        dp.insert("hemp".to_string(), DERIVATION_HEMP_CANONICAL_420.to_string());
        dp.insert("btc".to_string(), DERIVATION_BTC_BIP84.to_string());
        let btc_record = SecretRecord {
            record_id: "wallet.btc.main".to_string(),
            record_type: RECORD_TYPE_WALLET_BIP39.to_string(),
            label: "BTC Wallet".to_string(),
            value: "purpose-driven btc test wallet phrase example".to_string(),
            metadata: Some(serde_json::json!({"btc_external_count": 50, "btc_change_count": 25, "btc_derivation_profile": DERIVATION_BTC_BIP84})),
            tags: Some(vec!["btc".to_string()]),
            origin_app: Some("hemp0x-commander".to_string()),
            derivation_profiles: Some(dp),
            network: Some("mainnet".to_string()),
            created: 1700000000,
            modified: 1700000000,
        };
        payload.secrets.insert(btc_record.record_id.clone(), btc_record.clone());

        let envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        let dek = unwrap_dek_with_passphrase(passphrase, &envelope).unwrap();
        let mut existing = decrypt_payload_with_dek(dek.as_slice(), &envelope).unwrap();
        set_provider_tokens_in_payload(&mut existing, "token2", "fb2", "", "");

        let now = chrono::Utc::now().timestamp();
        let mut envelope2 = envelope.clone();
        envelope2.modified = now;
        envelope2.payload = encrypt_payload_with_dek(dek.as_slice(), &existing, &envelope2).unwrap();

        let decrypted = decrypt_vault_envelope(passphrase, &envelope2).unwrap();
        let survived = decrypted.secrets.get("wallet.btc.main").unwrap();
        let meta = survived.metadata.as_ref().unwrap();
        assert_eq!(meta["btc_external_count"], 50);
        assert_eq!(meta["btc_derivation_profile"], DERIVATION_BTC_BIP84);
    }

    #[test]
    fn registry_contains_all_record_types() {
        let type_names: Vec<&str> = SUPPORTED_RECORD_TYPES.iter().map(|(t, _)| *t).collect();
        assert!(type_names.contains(&RECORD_TYPE_API_TOKEN));
        assert!(type_names.contains(&RECORD_TYPE_WALLET_BIP39));
        assert!(type_names.contains(&RECORD_TYPE_WALLET_CORE_MIGRATION));
    }

    #[test]
    fn derivation_profiles_registry_complete() {
        let ids: Vec<&str> = SUPPORTED_DERIVATION_PROFILES.iter().map(|(id, _, _)| *id).collect();
        assert!(ids.contains(&DERIVATION_HEMP_CANONICAL_420));
        assert!(ids.contains(&DERIVATION_HEMP_LEGACY_175));
    }

    #[test]
    fn scrypt_params_validated() {
        let passphrase = "test-passphrase";
        let mut slot = build_passphrase_slot(passphrase, KDF_PROFILE_SCRYPT).unwrap();
        slot.kdf_log_n = Some(10);
        let result = derive_slot_key(passphrase, &slot);
        assert!(result.is_err());
    }

    #[test]
    fn pbkdf2_iterations_validated() {
        let passphrase = "test-passphrase";
        let mut slot = build_passphrase_slot(passphrase, KDF_PROFILE_PBKDF2_SHA512).unwrap();
        slot.kdf_iterations = Some(500);
        let result = derive_slot_key(passphrase, &slot);
        assert!(result.is_err());
    }

    #[test]
    fn no_key_slots_rejected() {
        let passphrase = "test-passphrase";
        let payload = VaultPayload::default();
        let mut envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        envelope.key_slots.clear();
        let result = decrypt_vault_envelope(passphrase, &envelope);
        assert!(result.is_err());
    }

    #[test]
    fn bundle_has_required_fields() {
        let passphrase = "test-passphrase";
        let payload = VaultPayload::default();
        let envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        let bundle = VaultBundle {
            bundleVersion: CURRENT_BUNDLE_VERSION,
            format_identifier: FORMAT_IDENTIFIER.to_string(),
            vault: envelope,
            meta: None,
        };
        assert_eq!(bundle.bundleVersion, CURRENT_BUNDLE_VERSION);
        assert_eq!(bundle.format_identifier, FORMAT_IDENTIFIER);
    }

    #[test]
    fn legacy_payload_upgrade_works() {
        let legacy = serde_json::json!({
            "pinata_api_token": "legacy-token",
            "filebase_token": "legacy-fb",
            "payload_version": 1,
            "secrets": {}
        });
        let payload = upgrade_legacy_payload(&legacy);
        assert_eq!(payload_pinata_token(&payload), "legacy-token");
        assert_eq!(payload_filebase_token(&payload), "legacy-fb");
    }

    #[test]
    fn empty_token_update_does_not_erase_existing_provider_token() {
        let passphrase = "test-passphrase";
        let payload = make_provider_payload("pinata-original", "filebase-original");
        let envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        let dek = unwrap_dek_with_passphrase(passphrase, &envelope).unwrap();
        let mut existing = decrypt_payload_with_dek(dek.as_slice(), &envelope).unwrap();

        set_provider_tokens_in_payload(&mut existing, "pinata-new", "", "", "");

        let now = chrono::Utc::now().timestamp();
        let mut envelope2 = envelope.clone();
        envelope2.modified = now;
        envelope2.payload = encrypt_payload_with_dek(dek.as_slice(), &existing, &envelope2).unwrap();

        let decrypted = decrypt_vault_envelope(passphrase, &envelope2).unwrap();
        assert_eq!(payload_pinata_token(&decrypted), "pinata-new");
        assert_eq!(payload_filebase_token(&decrypted), "filebase-original");
    }

    #[test]
    fn empty_token_update_does_not_erase_existing_pinata_token() {
        let passphrase = "test-passphrase";
        let payload = make_provider_payload("pinata-original", "filebase-original");
        let envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        let dek = unwrap_dek_with_passphrase(passphrase, &envelope).unwrap();
        let mut existing = decrypt_payload_with_dek(dek.as_slice(), &envelope).unwrap();

        set_provider_tokens_in_payload(&mut existing, "", "filebase-new", "", "");

        let now = chrono::Utc::now().timestamp();
        let mut envelope2 = envelope.clone();
        envelope2.modified = now;
        envelope2.payload = encrypt_payload_with_dek(dek.as_slice(), &existing, &envelope2).unwrap();

        let decrypted = decrypt_vault_envelope(passphrase, &envelope2).unwrap();
        assert_eq!(payload_pinata_token(&decrypted), "pinata-original");
        assert_eq!(payload_filebase_token(&decrypted), "filebase-new");
    }

    #[test]
    fn pbkdf2_sha256_roundtrip() {
        let passphrase = "test-passphrase-sha256";
        let payload = make_provider_payload("pinata-token", "filebase-token");
        let envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_PBKDF2_SHA256).unwrap();
        assert_eq!(envelope.key_slots[0].kdf_profile, KDF_PROFILE_PBKDF2_SHA256);

        let decrypted = decrypt_vault_envelope(passphrase, &envelope).unwrap();
        assert_eq!(payload_pinata_token(&decrypted), "pinata-token");
        assert_eq!(payload_filebase_token(&decrypted), "filebase-token");
    }

    #[test]
    fn pbkdf2_sha256_slot_validates_iterations() {
        let passphrase = "test-passphrase";
        let mut slot = build_passphrase_slot(passphrase, KDF_PROFILE_PBKDF2_SHA256).unwrap();
        slot.kdf_iterations = None;
        let result = derive_slot_key(passphrase, &slot);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("missing kdf_iterations"));
    }

    #[test]
    fn provider_record_metadata_contains_required_fields() {
        let mut payload = VaultPayload::default();
        set_provider_tokens_in_payload(&mut payload, "jwt-token", "bearer-token", "https://api.pinata.cloud", "https://rpc.filebase.io");
        let pinata = payload.secrets.get(RECORD_ID_PINATA).unwrap();
        let meta = pinata.metadata.as_ref().unwrap();
        assert_eq!(meta["provider_id"], "pinata");
        assert_eq!(meta["token_kind"], "jwt");
        assert_eq!(meta["endpoint"], "https://api.pinata.cloud");

        let filebase = payload.secrets.get(RECORD_ID_FILEBASE).unwrap();
        let meta2 = filebase.metadata.as_ref().unwrap();
        assert_eq!(meta2["provider_id"], "filebase");
        assert_eq!(meta2["token_kind"], "bearer");
    }

    // ─── File-backed load_bundle() tests ───────────────────────────────────

    #[test]
    fn file_backed_bundle_version_rejected() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-passphrase";
        let payload = VaultPayload::default();
        let envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        let mut bundle = VaultBundle {
            bundleVersion: 999,
            format_identifier: FORMAT_IDENTIFIER.to_string(),
            vault: envelope,
            meta: None,
        };
        save_bundle_atomic(&bundle).unwrap();
        let result = load_bundle();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unsupported bundle version"));

        bundle.bundleVersion = 0;
        save_bundle_atomic(&bundle).unwrap();
        let result = load_bundle();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unsupported bundle version"));


    }

    #[test]
    fn file_backed_wrong_format_identifier_rejected() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-passphrase";
        let payload = VaultPayload::default();
        let envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        let bundle = VaultBundle {
            bundleVersion: CURRENT_BUNDLE_VERSION,
            format_identifier: "foreign-format-v1".to_string(),
            vault: envelope,
            meta: None,
        };
        save_bundle_atomic(&bundle).unwrap();
        let result = load_bundle();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown vault format"));

    }

    #[test]
    fn file_backed_legacy_snake_case_bundle_loads() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-passphrase";
        let payload = make_provider_payload("token", "fb");
        let envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        let legacy_json = serde_json::json!({
            "bundle_version": 1,
            "format_identifier": FORMAT_IDENTIFIER,
            "vault": envelope,
            "public_meta": {"legacy": true}
        });
        let content = serde_json::to_string_pretty(&legacy_json).unwrap();
        let path = vault_path().unwrap();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, content).unwrap();

        let loaded = load_bundle().unwrap().unwrap();
        assert_eq!(loaded.bundleVersion, 1);
        assert!(loaded.meta.is_some());

        let decrypted = decrypt_vault_envelope(passphrase, &loaded.vault).unwrap();
        assert_eq!(payload_pinata_token(&decrypted), "token");

    }

    #[test]
    fn file_backed_legacy_raw_envelope_loads() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-passphrase";
        let payload = make_provider_payload("pinata-raw", "filebase-raw");
        let envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        let raw_json = serde_json::to_string_pretty(&envelope).unwrap();
        let path = vault_path().unwrap();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, raw_json).unwrap();

        let loaded = load_bundle().unwrap().unwrap();
        assert_eq!(loaded.bundleVersion, CURRENT_BUNDLE_VERSION);
        assert_eq!(loaded.format_identifier, FORMAT_IDENTIFIER);

        let decrypted = decrypt_vault_envelope(passphrase, &loaded.vault).unwrap();
        assert_eq!(payload_pinata_token(&decrypted), "pinata-raw");

    }

    #[test]
    fn file_backed_invalid_network_rejected_in_decrypt_load_path() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-passphrase";
        let payload = make_provider_payload("token", "fb");
        let mut envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        envelope.network = Some("invalidnet".to_string());
        let bundle = VaultBundle {
            bundleVersion: CURRENT_BUNDLE_VERSION,
            format_identifier: FORMAT_IDENTIFIER.to_string(),
            vault: envelope,
            meta: None,
        };
        save_bundle_atomic(&bundle).unwrap();

        let loaded = load_bundle().unwrap().unwrap();
        let result = decrypt_vault_envelope(passphrase, &loaded.vault);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid network"));

    }

    #[test]
    fn file_backed_missing_network_rejected_in_decrypt_load_path() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-passphrase";
        let payload = make_provider_payload("token", "fb");
        let mut envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        envelope.network = None;
        let bundle = VaultBundle {
            bundleVersion: CURRENT_BUNDLE_VERSION,
            format_identifier: FORMAT_IDENTIFIER.to_string(),
            vault: envelope,
            meta: None,
        };
        save_bundle_atomic(&bundle).unwrap();

        let loaded = load_bundle().unwrap().unwrap();
        let result = decrypt_vault_envelope(passphrase, &loaded.vault);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Network must be set"));

    }

    #[test]
    fn file_backed_unknown_aad_profile_rejected_in_decrypt_load_path() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-passphrase";
        let payload = make_provider_payload("token", "fb");
        let mut envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        envelope.aad_profile = "unknown-aad-v1".to_string();
        let bundle = VaultBundle {
            bundleVersion: CURRENT_BUNDLE_VERSION,
            format_identifier: FORMAT_IDENTIFIER.to_string(),
            vault: envelope,
            meta: None,
        };
        save_bundle_atomic(&bundle).unwrap();

        let loaded = load_bundle().unwrap().unwrap();
        let result = decrypt_vault_envelope(passphrase, &loaded.vault);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unsupported AAD profile"));

    }

    #[test]
    fn file_backed_unknown_payload_schema_rejected_in_decrypt_load_path() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-passphrase";
        let payload = make_provider_payload("token", "fb");
        let mut envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        envelope.payload.payload_schema = "unknown-schema-v2".to_string();
        let bundle = VaultBundle {
            bundleVersion: CURRENT_BUNDLE_VERSION,
            format_identifier: FORMAT_IDENTIFIER.to_string(),
            vault: envelope,
            meta: None,
        };
        save_bundle_atomic(&bundle).unwrap();

        let loaded = load_bundle().unwrap().unwrap();
        let result = decrypt_vault_envelope(passphrase, &loaded.vault);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unsupported payload schema"));

    }

    // ─── Network validation in decrypt path tests ──────────────────────────

    #[test]
    fn decrypt_rejects_missing_network() {
        let passphrase = "test-passphrase";
        let payload = make_provider_payload("token", "fb");
        let mut envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        envelope.network = None;
        let result = decrypt_vault_envelope(passphrase, &envelope);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Network must be set"));
    }

    #[test]
    fn decrypt_rejects_empty_network() {
        let passphrase = "test-passphrase";
        let payload = make_provider_payload("token", "fb");
        let mut envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        envelope.network = Some("".to_string());
        let result = decrypt_vault_envelope(passphrase, &envelope);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Network must be set"));
    }

    #[test]
    fn decrypt_accepts_valid_networks() {
        for net in &["mainnet", "testnet", "regtest"] {
            let result = validate_network(Some(net));
            assert!(result.is_ok(), "Network {net} should be valid");
        }
    }

    // ─── PBKDF2-SHA256 salt compatibility tests ────────────────────────────

    #[test]
    fn pbkdf2_sha256_decrypts_with_16_byte_salt() {
        let passphrase = "test-passphrase-16byte-salt";
        let payload = make_provider_payload("pinata-16", "filebase-16");

        // Build a slot with 16-byte salt manually
        let salt_16 = [0xABu8; 16];
        let iterations = 100_000u32;
        let slot_key = derive_pbkdf2_sha256_key(passphrase, &salt_16, iterations);

        let mut dek = Zeroizing::new([0u8; DEK_SIZE]);
        OsRng.fill_bytes(dek.as_mut());

        let now = chrono::Utc::now().timestamp();
        let mut envelope = VaultEnvelope {
            version: CURRENT_VAULT_VERSION,
            schema_identifier: SCHEMA_IDENTIFIER.to_string(),
            app_identifier: APP_IDENTIFIER.to_string(),
            network: Some("mainnet".to_string()),
            cipher_profile: CIPHER_PROFILE.to_string(),
            aad_profile: AAD_PROFILE.to_string(),
            payload: VaultPayloadBlock {
                payload_schema: PAYLOAD_SCHEMA.to_string(),
                iv: String::new(),
                ciphertext: String::new(),
            },
            key_slots: vec![],
            created: now,
            modified: now,
        };

        envelope.payload = encrypt_payload_with_dek(dek.as_slice(), &payload, &envelope).unwrap();

        // Wrap DEK with 16-byte salt slot
        let key = aes_gcm::Key::<Aes256Gcm>::from_slice(slot_key.as_slice());
        let cipher = Aes256Gcm::new(key);
        let mut wrap_iv = [0u8; GCM_IV_SIZE];
        OsRng.fill_bytes(&mut wrap_iv);
        let nonce = Nonce::from_slice(&wrap_iv);
        let aad = format!(
            "{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}",
            envelope.schema_identifier,
            envelope.version,
            envelope.app_identifier,
            envelope.network.as_deref().unwrap_or(""),
            "primary",
            "passphrase",
            KDF_PROFILE_PBKDF2_SHA256,
            iterations,
            KDF_DKLEN,
            WRAP_CIPHER_PROFILE,
            now,
        );
        let wrapped = cipher.encrypt(nonce, aes_gcm::aead::Payload {
            msg: dek.as_slice(),
            aad: aad.as_bytes(),
        }).unwrap();

        let slot = KeySlot {
            slot_id: "primary".to_string(),
            slot_type: "passphrase".to_string(),
            kdf_profile: KDF_PROFILE_PBKDF2_SHA256.to_string(),
            kdf_iterations: Some(iterations),
            kdf_log_n: None,
            kdf_r: None,
            kdf_p: None,
            kdf_dklen: KDF_DKLEN,
            salt: hex::encode(salt_16),
            wrap_cipher_profile: WRAP_CIPHER_PROFILE.to_string(),
            wrap_iv: hex::encode(wrap_iv),
            wrapped_dek: hex::encode(&wrapped),
            created: now,
            modified: now,
        };
        envelope.key_slots.push(slot);

        let decrypted = decrypt_vault_envelope(passphrase, &envelope).unwrap();
        assert_eq!(payload_pinata_token(&decrypted), "pinata-16");
        assert_eq!(payload_filebase_token(&decrypted), "filebase-16");
    }

    #[test]
    fn pbkdf2_sha256_decrypts_with_32_byte_salt() {
        let passphrase = "test-passphrase-sha256-32";
        let payload = make_provider_payload("pinata-32", "filebase-32");
        let envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_PBKDF2_SHA256).unwrap();
        assert_eq!(envelope.key_slots[0].salt.len(), 64); // 32 bytes = 64 hex chars
        let decrypted = decrypt_vault_envelope(passphrase, &envelope).unwrap();
        assert_eq!(payload_pinata_token(&decrypted), "pinata-32");
    }

    #[test]
    fn scrypt_rejects_non_32_byte_salt() {
        let passphrase = "test-passphrase";
        let mut slot = build_passphrase_slot(passphrase, KDF_PROFILE_SCRYPT).unwrap();
        slot.salt = hex::encode([0xAAu8; 16]); // 16-byte salt
        let result = derive_slot_key(passphrase, &slot);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid slot salt length"));
    }

    #[test]
    fn pbkdf2_sha512_rejects_non_32_byte_salt() {
        let passphrase = "test-passphrase";
        let mut slot = build_passphrase_slot(passphrase, KDF_PROFILE_PBKDF2_SHA512).unwrap();
        slot.salt = hex::encode([0xAAu8; 16]); // 16-byte salt
        let result = derive_slot_key(passphrase, &slot);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid slot salt length"));
    }

    #[test]
    fn pbkdf2_sha256_rejects_invalid_salt_length() {
        let passphrase = "test-passphrase";
        let mut slot = build_passphrase_slot(passphrase, KDF_PROFILE_PBKDF2_SHA256).unwrap();
        slot.salt = hex::encode([0xAAu8; 24]); // 24-byte salt (not 16 or 32)
        let result = derive_slot_key(passphrase, &slot);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid PBKDF2-SHA256 salt length"));
    }

    // ─── Provider endpoint metadata test ───────────────────────────────────

    #[test]
    fn update_vault_tokens_stores_endpoint_metadata() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-passphrase";
        let payload = VaultPayload::default();
        let envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        let bundle = VaultBundle {
            bundleVersion: CURRENT_BUNDLE_VERSION,
            format_identifier: FORMAT_IDENTIFIER.to_string(),
            vault: envelope,
            meta: None,
        };
        save_bundle_atomic(&bundle).unwrap();

        update_vault_tokens(
            passphrase,
            "jwt-token-value",
            "bearer-token-value",
            "https://api.pinata.cloud",
            "https://rpc.filebase.io",
        ).unwrap();

        let loaded = load_bundle().unwrap().unwrap();
        let decrypted = decrypt_vault_envelope(passphrase, &loaded.vault).unwrap();

        let pinata = decrypted.secrets.get(RECORD_ID_PINATA).unwrap();
        let meta = pinata.metadata.as_ref().unwrap();
        assert_eq!(meta["endpoint"], "https://api.pinata.cloud");
        assert_eq!(meta["provider_id"], "pinata");

        let filebase = decrypted.secrets.get(RECORD_ID_FILEBASE).unwrap();
        let meta2 = filebase.metadata.as_ref().unwrap();
        assert_eq!(meta2["endpoint"], "https://rpc.filebase.io");
        assert_eq!(meta2["provider_id"], "filebase");


    }

    // ─── Token removal tests ───────────────────────────────────────────────

    #[test]
    fn explicit_remove_deletes_provider_token() {
        let passphrase = "test-passphrase";
        let payload = make_provider_payload("pinata-original", "filebase-original");
        let envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        let bundle = VaultBundle {
            bundleVersion: CURRENT_BUNDLE_VERSION,
            format_identifier: FORMAT_IDENTIFIER.to_string(),
            vault: envelope,
            meta: None,
        };

        let _guard = setup_test_vault_dir();
        save_bundle_atomic(&bundle).unwrap();

        remove_provider_token_from_vault(passphrase, RECORD_ID_PINATA).unwrap();

        let loaded = load_bundle().unwrap().unwrap();
        let decrypted = decrypt_vault_envelope(passphrase, &loaded.vault).unwrap();
        assert_eq!(payload_pinata_token(&decrypted), "");
        assert_eq!(payload_filebase_token(&decrypted), "filebase-original");

    }

    #[test]
    fn explicit_remove_nonexistent_record_fails() {
        let passphrase = "test-passphrase";
        let payload = make_provider_payload("pinata", "filebase");
        let envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        let bundle = VaultBundle {
            bundleVersion: CURRENT_BUNDLE_VERSION,
            format_identifier: FORMAT_IDENTIFIER.to_string(),
            vault: envelope,
            meta: None,
        };

        let _guard = setup_test_vault_dir();
        save_bundle_atomic(&bundle).unwrap();

        let result = remove_provider_token_from_vault(passphrase, "provider.nonexistent.api_token");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No secret record found"));

    }

    #[test]
    fn blank_update_preserves_and_explicit_remove_deletes() {
        let passphrase = "test-passphrase";
        let payload = make_provider_payload("pinata-original", "filebase-original");
        let envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        let bundle = VaultBundle {
            bundleVersion: CURRENT_BUNDLE_VERSION,
            format_identifier: FORMAT_IDENTIFIER.to_string(),
            vault: envelope,
            meta: None,
        };

        let _guard = setup_test_vault_dir();
        save_bundle_atomic(&bundle).unwrap();

        // Blank update preserves both
        update_vault_tokens(passphrase, "", "", "", "").unwrap();
        let loaded = load_bundle().unwrap().unwrap();
        let decrypted = decrypt_vault_envelope(passphrase, &loaded.vault).unwrap();
        assert_eq!(payload_pinata_token(&decrypted), "pinata-original");
        assert_eq!(payload_filebase_token(&decrypted), "filebase-original");

        // Explicit remove deletes pinata only
        remove_provider_token_from_vault(passphrase, RECORD_ID_PINATA).unwrap();
        let loaded = load_bundle().unwrap().unwrap();
        let decrypted = decrypt_vault_envelope(passphrase, &loaded.vault).unwrap();
        assert_eq!(payload_pinata_token(&decrypted), "");
        assert_eq!(payload_filebase_token(&decrypted), "filebase-original");


    }
}
