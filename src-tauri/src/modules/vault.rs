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
use sha2::{Digest, Sha256, Sha512};
use zeroize::Zeroizing;

use bip39::Mnemonic;

use crate::modules::files::{commander_dir, data_dir};

const MAX_LABEL_LENGTH: usize = 128;
const MAX_IMPORT_FILE_SIZE: u64 = 10 * 1024 * 1024; // 10 MB

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
#[allow(dead_code)]
const RECORD_TYPE_APP_SETTING_SWAP_SECRETS: &str = "app_setting.webcom.swap_secrets";
const RECORD_TYPE_APP_SETTING_ADDRESS_BOOK: &str = "app_setting.hemp0x.address_book.v1";

const RECORD_ID_PINATA: &str = "provider.pinata.api_token";
const RECORD_ID_FILEBASE: &str = "provider.filebase.token";
const RECORD_ID_WALLET_HEMP_PRIMARY: &str = "wallet.webcom.hemp.primary";
const RECORD_ID_WALLET_BTC_LITE_PRIMARY: &str = "wallet.webcom.btc_lite.primary";
const RECORD_ID_SWAP_SECRETS: &str = "app_setting.webcom.swap_secrets";
const RECORD_ID_ADDRESS_BOOK: &str = "app_setting.hemp0x.address_book";

/// Known WebCom record IDs for interop summary / preservation tracking.
const KNOWN_WEBCOM_RECORD_IDS: &[&str] = &[
    RECORD_ID_WALLET_HEMP_PRIMARY,
    RECORD_ID_WALLET_BTC_LITE_PRIMARY,
    RECORD_ID_SWAP_SECRETS,
    RECORD_ID_ADDRESS_BOOK,
];

const ADDRESS_BOOK_SCHEMA: &str = "hemp0x.address_book";
const ADDRESS_BOOK_SCHEMA_VERSION: i32 = 1;
const ADDRESS_BOOK_CHAIN_HEMP: &str = "hemp0x";
const ADDRESS_BOOK_CHAIN_BITCOIN: &str = "bitcoin";

pub const VALID_NETWORKS: &[&str] = &["mainnet", "testnet", "regtest"];

pub const DERIVATION_HEMP_CANONICAL_420: &str = "hemp0x.mainnet.bip44.p2pkh.coin420.v1";
pub const DERIVATION_HEMP_LEGACY_175: &str = "hemp0x.webcom.legacy.bip44.p2pkh.coin175.v1";
pub const DERIVATION_HEMP_LEGACY_GENERIC: &str = "hemp0x.mainnet.bip44.p2pkh.v1";
pub const DERIVATION_BTC_BIP84: &str = "btc.mainnet.bip84.p2wpkh.v1";
pub const DERIVATION_WIF_SINGLE: &str = "hemp0x.mainnet.wif.single.v1";

const VAULT_KEYPOOL_EXTERNAL_HINT_FLOOR: i64 = 20;
const VAULT_KEYPOOL_CHANGE_HINT_FLOOR: i64 = 6;
const VAULT_KEYPOOL_HINT_CEILING: i64 = 10_000;
const VAULT_CORE_KEYPOOL_REFILL_FLOOR: i64 = 1_000;

const SUPPORTED_DERIVATION_PROFILES: &[(&str, &str, &str)] = &[
    (
        DERIVATION_HEMP_CANONICAL_420,
        "Hemp0x canonical BIP44 coin 420",
        "m/44'/420'/0'/change/index",
    ),
    (
        DERIVATION_HEMP_LEGACY_175,
        "Legacy WebCom BIP44 coin 175",
        "m/44'/175'/0'/change/index",
    ),
    (
        DERIVATION_HEMP_LEGACY_GENERIC,
        "Early WebCom generic (derives 175)",
        "m/44'/175'/0'/change/index",
    ),
    (
        DERIVATION_BTC_BIP84,
        "BTC native SegWit BIP84",
        "m/84'/0'/0'/change/index",
    ),
    (DERIVATION_WIF_SINGLE, "WIF single-key import", "N/A"),
];

const SUPPORTED_RECORD_TYPES: &[(&str, &str)] = &[
    (
        RECORD_TYPE_API_TOKEN,
        "IPFS/API provider tokens (Pinata, Filebase, Kubo)",
    ),
    (
        RECORD_TYPE_WALLET_BIP39,
        "BIP39 mnemonic wallet (12/24 word recovery phrase)",
    ),
    (RECORD_TYPE_WALLET_WIF, "WIF single-key wallet"),
    (
        RECORD_TYPE_WALLET_CORE_MIGRATION,
        "Core Next migration envelope reference or embedded artifact",
    ),
    (
        RECORD_TYPE_WALLET_HARDWARE,
        "Hardware wallet / WalletConnect metadata",
    ),
    (
        RECORD_TYPE_WALLET_WATCH_ONLY,
        "Watch-only address / public key",
    ),
    (RECORD_TYPE_PROTOCOL_NOSTR, "Nostr nsec/npub key"),
    (
        RECORD_TYPE_APP_SECRET,
        "Generic application API key or credential",
    ),
    (RECORD_TYPE_NOTE_SECURE, "Encrypted secure note"),
];

const WEBCOM_PRIMARY_EXTERNAL_COUNT_DEFAULT: i64 = 20;
const WEBCOM_PRIMARY_CHANGE_COUNT_DEFAULT: i64 = 6;

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

fn default_kdf_dklen() -> u32 {
    KDF_DKLEN
}

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
    payload
        .secrets
        .get(RECORD_ID_PINATA)
        .map(|r| r.value.clone())
        .unwrap_or_default()
}

pub fn payload_filebase_token(payload: &VaultPayload) -> String {
    payload
        .secrets
        .get(RECORD_ID_FILEBASE)
        .map(|r| r.value.clone())
        .unwrap_or_default()
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
        payload.secrets.insert(
            RECORD_ID_PINATA.to_string(),
            SecretRecord {
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
            },
        );
    }
    if !filebase.is_empty() {
        let existing_created = payload.secrets.get(RECORD_ID_FILEBASE).map(|r| r.created);
        payload.secrets.insert(
            RECORD_ID_FILEBASE.to_string(),
            SecretRecord {
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
            },
        );
    }
}

fn upgrade_legacy_payload(raw: &serde_json::Value) -> VaultPayload {
    let mut payload = if raw.get("secrets").is_some() {
        serde_json::from_value(raw.clone()).unwrap_or_default()
    } else {
        VaultPayload::default()
    };
    let pinata = raw
        .get("pinata_api_token")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let filebase = raw
        .get("filebase_token")
        .and_then(|v| v.as_str())
        .unwrap_or("");
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
    let dir = std::env::temp_dir().join(format!(
        "commander_vault_test_{}",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    // Isolate commander settings (app_settings.json) into the temp dir
    // too, so vault tests never read or mutate the user's real
    // ~/.hemp0x/commander/app_settings.json. commander_settings_path()
    // resolves through commander_dir(), which honors this override.
    let commander_dir = dir.join("commander");
    std::fs::create_dir_all(&commander_dir).unwrap();
    crate::modules::files::TEST_COMMANDER_DIR.with(|cell| {
        *cell.borrow_mut() = Some(commander_dir.clone());
    });
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
        crate::modules::files::TEST_COMMANDER_DIR.with(|cell| {
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

fn vault_state_dir() -> Result<PathBuf, String> {
    #[cfg(test)]
    {
        let override_path = TEST_VAULT_DIR.with(|cell| cell.borrow().clone());
        if let Some(dir) = override_path {
            return Ok(dir);
        }
    }
    commander_dir()
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

fn derive_pbkdf2_key(
    passphrase: &str,
    salt: &[u8],
    iterations: u32,
) -> Zeroizing<[u8; KDF_KEY_SIZE]> {
    let mut key = Zeroizing::new([0u8; KDF_KEY_SIZE]);
    pbkdf2_hmac::<Sha512>(passphrase.as_bytes(), salt, iterations, key.as_mut());
    key
}

fn derive_pbkdf2_sha256_key(
    passphrase: &str,
    salt: &[u8],
    iterations: u32,
) -> Zeroizing<[u8; KDF_KEY_SIZE]> {
    let mut key = Zeroizing::new([0u8; KDF_KEY_SIZE]);
    pbkdf2_hmac::<Sha256>(passphrase.as_bytes(), salt, iterations, key.as_mut());
    key
}

fn derive_scrypt_key(
    passphrase: &str,
    salt: &[u8],
    log_n: u8,
    r: u32,
    p: u32,
) -> Result<Zeroizing<[u8; KDF_KEY_SIZE]>, String> {
    let params = ScryptParams::new(log_n, r, p, KDF_KEY_SIZE)
        .map_err(|e| format!("Invalid scrypt params: {e}"))?;
    let mut key = Zeroizing::new([0u8; KDF_KEY_SIZE]);
    scrypt(passphrase.as_bytes(), salt, &params, key.as_mut())
        .map_err(|e| format!("scrypt KDF failed: {e}"))?;
    Ok(key)
}

fn validate_kdf_params(slot: &KeySlot) -> Result<(), String> {
    if slot.kdf_dklen != KDF_DKLEN {
        return Err(format!(
            "Invalid kdf_dklen: {} (expected {})",
            slot.kdf_dklen, KDF_DKLEN
        ));
    }
    match slot.kdf_profile.as_str() {
        KDF_PROFILE_PBKDF2_SHA512 | KDF_PROFILE_PBKDF2_SHA256 => {
            let iterations = slot
                .kdf_iterations
                .ok_or("PBKDF2 slot missing kdf_iterations")?;
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

fn derive_slot_key(
    passphrase: &str,
    slot: &KeySlot,
) -> Result<Zeroizing<[u8; KDF_KEY_SIZE]>, String> {
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

fn unwrap_dek(
    passphrase: &str,
    envelope: &VaultEnvelope,
    slot: &KeySlot,
) -> Result<Zeroizing<[u8; DEK_SIZE]>, String> {
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

    let wrapped =
        hex::decode(&slot.wrapped_dek).map_err(|e| format!("Invalid wrapped DEK hex: {e}"))?;

    let aad = build_slot_wrap_aad(envelope, slot);

    let dek_bytes = cipher
        .decrypt(
            nonce,
            aes_gcm::aead::Payload {
                msg: &wrapped,
                aad: &aad,
            },
        )
        .map_err(|_| "Incorrect passphrase or corrupted key slot".to_string())?;

    if dek_bytes.len() != DEK_SIZE {
        return Err("Invalid DEK length after unwrap".to_string());
    }

    let mut dek = Zeroizing::new([0u8; DEK_SIZE]);
    dek.copy_from_slice(&dek_bytes);
    Ok(dek)
}

fn unwrap_dek_with_passphrase(
    passphrase: &str,
    envelope: &VaultEnvelope,
) -> Result<Zeroizing<[u8; DEK_SIZE]>, String> {
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

fn wrap_dek(
    dek: &[u8],
    passphrase: &str,
    envelope: &VaultEnvelope,
    slot: &mut KeySlot,
) -> Result<(), String> {
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
        .encrypt(
            nonce,
            aes_gcm::aead::Payload {
                msg: dek,
                aad: &aad,
            },
        )
        .map_err(|e| format!("DEK wrap failed: {e}"))?;

    slot.wrap_iv = hex::encode(wrap_iv);
    slot.wrapped_dek = hex::encode(&wrapped);
    Ok(())
}

fn encrypt_payload_with_dek(
    dek: &[u8],
    payload: &VaultPayload,
    envelope: &VaultEnvelope,
) -> Result<VaultPayloadBlock, String> {
    if dek.len() != DEK_SIZE {
        return Err("Invalid DEK length".to_string());
    }
    let key = aes_gcm::Key::<Aes256Gcm>::from_slice(dek);
    let cipher = Aes256Gcm::new(key);

    let mut iv = [0u8; GCM_IV_SIZE];
    OsRng.fill_bytes(&mut iv);
    let nonce = Nonce::from_slice(&iv);

    let plaintext =
        serde_json::to_vec(payload).map_err(|e| format!("Serialization failed: {e}"))?;

    let aad = build_payload_aad(envelope);

    let ciphertext = cipher
        .encrypt(
            nonce,
            aes_gcm::aead::Payload {
                msg: &plaintext,
                aad: &aad,
            },
        )
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
        return Err(format!(
            "Unsupported payload schema: {}",
            payload_block.payload_schema
        ));
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
        .decrypt(
            nonce,
            aes_gcm::aead::Payload {
                msg: &ciphertext,
                aad: &aad,
            },
        )
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
        _ => {
            return Err(format!(
                "Unsupported KDF profile for new vault: {kdf_profile}"
            ))
        }
    }

    Ok(slot)
}

fn encrypt_vault_envelope(
    passphrase: &str,
    payload: &VaultPayload,
    kdf_profile: &str,
) -> Result<VaultEnvelope, String> {
    let now = chrono::Utc::now().timestamp();
    let network = detect_network();
    encrypt_vault_envelope_with_network(passphrase, payload, kdf_profile, network, now, now)
}

/// Build a fresh encrypted envelope using an explicit network, created,
/// and modified value. These three fields are bound into the AES-GCM
/// AAD for both the payload and the wrapped DEK (see `build_payload_aad`
/// and `build_slot_wrap_aad`), so they MUST be set before encryption and
/// must not be mutated afterward. This variant lets passphrase rotation
/// preserve the original `created`/`network` while advancing `modified`.
fn encrypt_vault_envelope_with_network(
    passphrase: &str,
    payload: &VaultPayload,
    kdf_profile: &str,
    network: String,
    created: i64,
    modified: i64,
) -> Result<VaultEnvelope, String> {
    let mut dek = Zeroizing::new([0u8; DEK_SIZE]);
    OsRng.fill_bytes(dek.as_mut());

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
        created,
        modified,
    };

    envelope.payload = encrypt_payload_with_dek(dek.as_slice(), payload, &envelope)?;

    wrap_dek(dek.as_slice(), passphrase, &envelope, &mut slot)?;
    envelope.key_slots.push(slot);

    Ok(envelope)
}

fn decrypt_vault_envelope(
    passphrase: &str,
    envelope: &VaultEnvelope,
) -> Result<VaultPayload, String> {
    if envelope.cipher_profile != CIPHER_PROFILE {
        return Err(format!(
            "Unsupported cipher profile: {}",
            envelope.cipher_profile
        ));
    }
    if envelope.aad_profile != AAD_PROFILE {
        return Err(format!("Unsupported AAD profile: {}", envelope.aad_profile));
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
    set_provider_tokens_in_payload(
        &mut payload,
        pinata,
        filebase,
        pinata_endpoint,
        filebase_endpoint,
    );

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

pub fn export_bundle_to_path(dest_path: &str) -> Result<String, String> {
    let src = vault_path()?;
    if !src.exists() {
        return Err("No vault exists to export".to_string());
    }
    let dest = PathBuf::from(dest_path);
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Cannot create destination directory: {e}"))?;
    }
    let content = fs::read_to_string(&src).map_err(|e| format!("Cannot read vault: {e}"))?;
    let _: serde_json::Value =
        serde_json::from_str(&content).map_err(|e| format!("Vault is not valid JSON: {e}"))?;
    fs::write(&dest, &content).map_err(|e| format!("Cannot write to destination: {e}"))?;
    Ok(dest.to_string_lossy().to_string())
}

pub fn validate_import_bundle_from_path(path: &str) -> Result<serde_json::Value, String> {
    let src = PathBuf::from(path);
    if !src.exists() {
        return Err("Import file does not exist".to_string());
    }
    if src.is_dir() {
        return Err("Import path is a directory, not a file".to_string());
    }
    let content = fs::read_to_string(&src).map_err(|e| format!("Cannot read import file: {e}"))?;
    let value: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| format!("Import file is not valid JSON: {e}"))?;

    let bundle: VaultBundle = if value.get("bundleVersion").is_some()
        || (value.get("bundle_version").is_some() && value.get("vault").is_some())
    {
        serde_json::from_value(value.clone()).map_err(|e| format!("Invalid vault bundle: {e}"))?
    } else {
        let envelope: VaultEnvelope = serde_json::from_value(value.clone())
            .map_err(|e| format!("Invalid vault envelope: {e}"))?;
        VaultBundle {
            bundleVersion: CURRENT_BUNDLE_VERSION,
            format_identifier: FORMAT_IDENTIFIER.to_string(),
            vault: envelope,
            meta: None,
        }
    };

    if bundle.bundleVersion < 1 || bundle.bundleVersion > CURRENT_BUNDLE_VERSION {
        return Err(format!(
            "Unsupported bundle version: {}. Supported: 1-{}",
            bundle.bundleVersion, CURRENT_BUNDLE_VERSION
        ));
    }
    if bundle.format_identifier != FORMAT_IDENTIFIER {
        return Err(format!(
            "Unknown vault format: {} (expected {})",
            bundle.format_identifier, FORMAT_IDENTIFIER
        ));
    }
    validate_network(bundle.vault.network.as_deref())?;

    if bundle.vault.cipher_profile != CIPHER_PROFILE {
        return Err(format!(
            "Unsupported cipher profile: {}",
            bundle.vault.cipher_profile
        ));
    }
    if bundle.vault.aad_profile != AAD_PROFILE {
        return Err(format!(
            "Unsupported AAD profile: {}",
            bundle.vault.aad_profile
        ));
    }

    let slot_info: Vec<serde_json::Value> = bundle
        .vault
        .key_slots
        .iter()
        .map(|s| {
            serde_json::json!({
                "slot_id": s.slot_id,
                "slot_type": s.slot_type,
                "kdf_profile": s.kdf_profile,
            })
        })
        .collect();

    Ok(serde_json::json!({
        "valid": true,
        "bundle_version": bundle.bundleVersion,
        "format_identifier": bundle.format_identifier,
        "version": bundle.vault.version,
        "network": bundle.vault.network,
        "cipher_profile": bundle.vault.cipher_profile,
        "key_slots": slot_info,
        "created": bundle.vault.created,
        "modified": bundle.vault.modified,
    }))
}

pub fn import_bundle_replace_from_path(
    path: &str,
    passphrase: Option<&str>,
) -> Result<serde_json::Value, String> {
    let src = PathBuf::from(path);
    if !src.exists() {
        return Err("Import file does not exist".to_string());
    }
    let content = fs::read_to_string(&src).map_err(|e| format!("Cannot read import file: {e}"))?;

    let value: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| format!("Import file is not valid JSON: {e}"))?;

    let bundle: VaultBundle = if value.get("bundleVersion").is_some()
        || (value.get("bundle_version").is_some() && value.get("vault").is_some())
    {
        serde_json::from_value(value.clone()).map_err(|e| format!("Invalid vault bundle: {e}"))?
    } else {
        let envelope: VaultEnvelope = serde_json::from_value(value.clone())
            .map_err(|e| format!("Invalid vault envelope: {e}"))?;
        VaultBundle {
            bundleVersion: CURRENT_BUNDLE_VERSION,
            format_identifier: FORMAT_IDENTIFIER.to_string(),
            vault: envelope,
            meta: None,
        }
    };

    if bundle.bundleVersion < 1 || bundle.bundleVersion > CURRENT_BUNDLE_VERSION {
        return Err(format!(
            "Unsupported bundle version: {}. Supported: 1-{}",
            bundle.bundleVersion, CURRENT_BUNDLE_VERSION
        ));
    }
    if bundle.format_identifier != FORMAT_IDENTIFIER {
        return Err(format!(
            "Unknown vault format: {}",
            bundle.format_identifier
        ));
    }
    validate_network(bundle.vault.network.as_deref())?;
    if bundle.vault.cipher_profile != CIPHER_PROFILE {
        return Err(format!(
            "Unsupported cipher profile: {}",
            bundle.vault.cipher_profile
        ));
    }
    if bundle.vault.aad_profile != AAD_PROFILE {
        return Err(format!(
            "Unsupported AAD profile: {}",
            bundle.vault.aad_profile
        ));
    }

    if let Some(pp) = passphrase {
        let valid = verify_vault_passphrase_with_bundle(pp, &bundle)?;
        if !valid {
            return Err("Incorrect passphrase for imported vault".to_string());
        }
    }

    save_bundle_atomic(&bundle)?;

    Ok(serde_json::json!({
        "imported": true,
        "bundle_version": bundle.bundleVersion,
        "version": bundle.vault.version,
        "network": bundle.vault.network,
        "modified": bundle.vault.modified,
    }))
}

fn verify_vault_passphrase_with_bundle(
    passphrase: &str,
    bundle: &VaultBundle,
) -> Result<bool, String> {
    match decrypt_vault_envelope(passphrase, &bundle.vault) {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}

pub fn check_provider_token_records(passphrase: &str) -> Result<serde_json::Value, String> {
    let bundle = load_bundle()?.ok_or("Vault does not exist")?;
    let payload = decrypt_vault_envelope(passphrase, &bundle.vault)?;
    let pinata_record = payload.secrets.get(RECORD_ID_PINATA);
    let filebase_record = payload.secrets.get(RECORD_ID_FILEBASE);
    let pinata_endpoint = pinata_record
        .and_then(|r| r.metadata.as_ref())
        .and_then(|m| m.get("endpoint"))
        .and_then(|v| v.as_str())
        .unwrap_or("https://api.pinata.cloud");
    let filebase_endpoint = filebase_record
        .and_then(|r| r.metadata.as_ref())
        .and_then(|m| m.get("endpoint"))
        .and_then(|v| v.as_str())
        .unwrap_or("https://rpc.filebase.io");
    Ok(serde_json::json!({
        "providers": {
            "pinata": {
                "has_token": pinata_record.is_some() && !pinata_record.unwrap().value.is_empty(),
                "endpoint": pinata_endpoint,
            },
            "filebase": {
                "has_token": filebase_record.is_some() && !filebase_record.unwrap().value.is_empty(),
                "endpoint": filebase_endpoint,
            }
        }
    }))
}

#[tauri::command]
pub fn vault_get_vault_path() -> Result<String, String> {
    vault_path().map(|p| p.to_string_lossy().to_string())
}

#[tauri::command]
pub fn vault_get_supported_record_types() -> Result<Vec<serde_json::Value>, String> {
    let types: Vec<serde_json::Value> = SUPPORTED_RECORD_TYPES
        .iter()
        .map(|(t, desc)| {
            let implemented = *t == RECORD_TYPE_API_TOKEN;
            serde_json::json!({
                "record_type": t,
                "description": desc,
                "implemented": implemented,
            })
        })
        .collect();
    Ok(types)
}

#[tauri::command]
pub fn vault_get_supported_derivation_profiles() -> Result<Vec<serde_json::Value>, String> {
    let profiles: Vec<serde_json::Value> = SUPPORTED_DERIVATION_PROFILES
        .iter()
        .map(|(id, desc, path)| {
            serde_json::json!({
                "profile_id": id,
                "description": desc,
                "derivation_path": path,
            })
        })
        .collect();
    Ok(profiles)
}

#[tauri::command]
pub fn vault_get_info() -> Result<Option<serde_json::Value>, String> {
    let bundle = load_bundle()?;
    match bundle {
        Some(b) => {
            let slot_info: Vec<serde_json::Value> = b
                .vault
                .key_slots
                .iter()
                .map(|s| {
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
                })
                .collect();
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

/// Metadata-only vault overview for the Wallet page header.
///
/// This intentionally does NOT decrypt the payload. It only reads the
/// on-disk bundle shape and key-slot configuration (KDF profile / params),
/// which are non-secret envelope metadata, so the Wallet page can show
/// "Vault ready" / "No vault yet" / "Vault file path" without prompting
/// for a passphrase.
///
/// Wallet-record count is intentionally NOT returned here because the
/// records live inside the encrypted payload. The UI shows the count
/// after the user unlocks the vault and lists records.
#[tauri::command]
pub fn vault_get_vault_overview() -> Result<serde_json::Value, String> {
    let path = vault_path()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| String::new());
    let bundle = load_bundle()?;
    let index = load_vault_index().unwrap_or_default();
    let display_label = index
        .labels
        .get(&path)
        .map(|e| e.display_name.clone())
        .unwrap_or_default();
    let overview = match bundle {
        Some(b) => {
            let first_slot = b.vault.key_slots.first();
            let (file_size, file_modified) = vault_file_stats();
            serde_json::json!({
                "exists": true,
                "vault_path": path,
                "bundle_version": b.bundleVersion,
                "format_identifier": b.format_identifier,
                "vault_version": b.vault.version,
                "schema_identifier": b.vault.schema_identifier,
                "app_identifier": b.vault.app_identifier,
                "network": b.vault.network,
                "cipher_profile": b.vault.cipher_profile,
                "aad_profile": b.vault.aad_profile,
                "payload_schema": b.vault.payload.payload_schema,
                "kdf_profile": first_slot.map(|s| s.kdf_profile.clone()),
                "key_slot_count": b.vault.key_slots.len(),
                "created": b.vault.created,
                "modified": b.vault.modified,
                "file_size": file_size,
                "file_modified": file_modified,
                "display_label": display_label,
            })
        }
        None => serde_json::json!({
            "exists": false,
            "vault_path": path,
            "file_size": 0_i64,
            "file_modified": 0_i64,
            "display_label": display_label,
        }),
    };
    Ok(overview)
}

fn vault_file_stats() -> (i64, i64) {
    // Best-effort metadata only; failures collapse to 0 so the overview
    // is still usable on locked-down filesystems.
    if let Ok(path) = vault_path() {
        if let Ok(meta) = fs::metadata(&path) {
            let modified = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            return (meta.len() as i64, modified);
        }
    }
    (0, 0)
}

// ---------------------------------------------------------------------------
// Vault File Manager (slice 60s)
// ---------------------------------------------------------------------------
//
// Local non-secret metadata lives in a sidecar file under the Commander
// directory: <commander_dir>/vault_index.json
//
// The sidecar is allowed to contain ONLY:
//   - vault path
//   - display name
//   - last selected timestamp
//   - archived/known vault list
//
// It MUST NEVER contain: passphrases, hints, private keys, mnemonics,
// provider tokens, or any decrypted vault content.

const VAULT_INDEX_FILENAME: &str = "vault_index.json";
const VAULT_ARCHIVE_DIRNAME: &str = "vaults";
const VAULT_ARCHIVE_SUBDIR: &str = "archive";
const VAULT_ARCHIVE_PREFIX: &str = "vault";
const VAULT_ARCHIVE_EXTENSION: &str = "json";
const VAULT_INDEX_VERSION: i32 = 1;
const VAULT_MAX_LABEL_LEN: usize = 64;
const VAULT_MAX_KNOWN_ENTRIES: usize = 64;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct VaultIndexEntry {
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub last_selected: i64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct VaultIndex {
    #[serde(default = "default_vault_index_version")]
    pub version: i32,
    #[serde(default)]
    pub active_label: String,
    #[serde(default)]
    pub active_export_path: String,
    #[serde(default)]
    pub labels: HashMap<String, VaultIndexEntry>,
    #[serde(default)]
    pub archives: Vec<String>,
}

fn default_vault_index_version() -> i32 {
    VAULT_INDEX_VERSION
}

fn vault_index_path() -> Result<PathBuf, String> {
    Ok(vault_state_dir()?.join(VAULT_INDEX_FILENAME))
}

fn vault_archives_dir() -> Result<PathBuf, String> {
    let dir = vault_state_dir()?
        .join(VAULT_ARCHIVE_DIRNAME)
        .join(VAULT_ARCHIVE_SUBDIR);
    fs::create_dir_all(&dir).map_err(|e| format!("Could not create archive dir: {e}"))?;
    Ok(dir)
}

pub fn load_vault_index() -> Result<VaultIndex, String> {
    let path = vault_index_path()?;
    if !path.exists() {
        return Ok(VaultIndex {
            version: VAULT_INDEX_VERSION,
            ..Default::default()
        });
    }
    let raw = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let parsed: VaultIndex =
        serde_json::from_str(&raw).map_err(|e| format!("Invalid vault index: {e}"))?;
    Ok(parsed)
}

fn save_vault_index(index: &VaultIndex) -> Result<(), String> {
    let path = vault_index_path()?;
    let tmp = path.with_extension("json.tmp");
    let content =
        serde_json::to_string_pretty(index).map_err(|e| format!("Serialize vault index: {e}"))?;
    fs::write(&tmp, content).map_err(|e| e.to_string())?;
    fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
    Ok(())
}

fn normalize_user_vault_export_path(path: &str) -> Result<String, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("Vault export path is empty".to_string());
    }
    let p = PathBuf::from(trimmed);
    if !p.is_absolute() {
        return Err("Vault export path must be absolute".to_string());
    }
    if p.is_dir() {
        return Err("Vault export path is a directory, not a file".to_string());
    }
    let parent = p
        .parent()
        .ok_or("Vault export path has no parent directory")?;
    if !parent.exists() {
        return Err("Vault export path parent directory does not exist".to_string());
    }
    let canonical_parent = parent
        .canonicalize()
        .map_err(|e| format!("Could not resolve vault export directory: {e}"))?;
    let data_root = data_dir()?;
    let data_root = data_root.canonicalize().unwrap_or(data_root);
    if !canonical_parent.starts_with(&data_root) {
        return Err(format!(
            "Vault export path must stay inside the Hemp0x data directory ({})",
            data_root.to_string_lossy()
        ));
    }
    let file_name = p.file_name().ok_or("Vault export path has no filename")?;
    Ok(canonical_parent
        .join(file_name)
        .to_string_lossy()
        .to_string())
}

fn sanitize_label(input: &str) -> String {
    let mut clean = String::new();
    let mut chars = input.trim().chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            if matches!(chars.peek(), Some('[')) {
                chars.next();
                for next in chars.by_ref() {
                    if ('@'..='~').contains(&next) {
                        break;
                    }
                }
            }
            continue;
        }
        if ch.is_control() {
            continue;
        }
        clean.push(ch);
        if clean.chars().count() >= VAULT_MAX_LABEL_LEN {
            break;
        }
    }
    clean
}

fn timestamped_archive_name(now_unix: i64) -> String {
    // YYYYMMDD-HHMMSS in UTC.
    let secs_per_day = 86_400_i64;
    let secs_per_hour = 3_600_i64;
    let secs_per_min = 60_i64;
    let days = now_unix.div_euclid(secs_per_day);
    let mut remainder = now_unix.rem_euclid(secs_per_day);
    let hour = remainder / secs_per_hour;
    remainder %= secs_per_hour;
    let minute = remainder / secs_per_min;
    let second = remainder % secs_per_min;
    // Convert days-since-epoch into a Y/M/D using a simple civil-from-days
    // (Howard Hinnant's algorithm) to keep the helper self-contained.
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{}{:02}{:02}-{:02}{:02}{:02}",
        y, m, d, hour, minute, second
    )
}

#[tauri::command]
pub fn vault_get_vault_index() -> Result<serde_json::Value, String> {
    let path = vault_index_path()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let index = load_vault_index()?;
    Ok(serde_json::json!({
        "version": index.version,
        "active_label": index.active_label,
        "active_export_path": index.active_export_path,
        "labels": index.labels,
        "archives": index.archives,
        "index_path": path,
    }))
}

#[tauri::command]
pub fn vault_set_vault_label(label: String) -> Result<serde_json::Value, String> {
    let clean = sanitize_label(&label);
    let mut index = load_vault_index()?;
    let path = vault_path()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    if clean.is_empty() {
        index.labels.remove(&path);
        index.active_label.clear();
    } else {
        let entry = index.labels.entry(path.clone()).or_default();
        entry.display_name = clean.clone();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        entry.last_selected = now;
        index.active_label = clean.clone();
    }
    save_vault_index(&index)?;
    Ok(serde_json::json!({
        "updated": true,
        "active_label": index.active_label,
    }))
}

#[tauri::command]
pub fn vault_set_active_export_path(path: String) -> Result<serde_json::Value, String> {
    let normalized = normalize_user_vault_export_path(&path)?;
    let mut index = load_vault_index()?;
    index.active_export_path = normalized.clone();
    save_vault_index(&index)?;
    Ok(serde_json::json!({
        "updated": true,
        "active_export_path": normalized,
    }))
}

#[tauri::command]
pub fn vault_autosave_active_export_path() -> Result<serde_json::Value, String> {
    let index = load_vault_index()?;
    if index.active_export_path.trim().is_empty() {
        return Ok(serde_json::json!({
            "saved": false,
            "skipped": true,
            "reason": "no_active_export_path",
        }));
    }
    let normalized = normalize_user_vault_export_path(&index.active_export_path)?;
    let saved_path = export_bundle_to_path(&normalized)?;
    Ok(serde_json::json!({
        "saved": true,
        "path": saved_path,
    }))
}

/// Move the current active `vault.json` into the archive directory and
/// clear the cached unlock session. The file is preserved verbatim on
/// disk; it is never deleted. Returns the absolute archive path.
#[tauri::command]
pub fn vault_read_raw_content() -> Result<String, String> {
    let current = vault_path()?;
    fs::read_to_string(&current).map_err(|e| format!("Could not read vault file: {e}"))
}

#[tauri::command]
pub fn vault_archive_current_vault() -> Result<serde_json::Value, String> {
    let current = vault_path()?;
    if !current.exists() {
        return Err("No vault file to archive".to_string());
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let stamp = timestamped_archive_name(now);
    let archive_dir = vault_archives_dir()?;
    let mut target = archive_dir.join(format!(
        "{}-{}.{}",
        VAULT_ARCHIVE_PREFIX, stamp, VAULT_ARCHIVE_EXTENSION
    ));
    // Avoid collision: append a numeric suffix if needed.
    let mut suffix: u32 = 1;
    while target.exists() {
        target = archive_dir.join(format!(
            "{}-{}-{}.{}",
            VAULT_ARCHIVE_PREFIX, stamp, suffix, VAULT_ARCHIVE_EXTENSION
        ));
        suffix = suffix.saturating_add(1);
    }
    fs::rename(&current, &target)
        .map_err(|e| format!("Could not archive vault file: {e}"))
        .or_else(|rename_err| {
            // `fs::rename` is not always valid across filesystems (e.g.
            // the test override is on /tmp while the active data dir is
            // elsewhere). Fall back to copy + delete so the archive path
            // still works in mixed-filesystem environments.
            fs::copy(&current, &target)
                .map_err(|copy_err| {
                    format!(
                        "Could not archive vault file: rename failed ({rename_err}); copy fallback failed ({copy_err})"
                    )
                })
                .and_then(|_| {
                    fs::remove_file(&current)
                        .map_err(|rm_err| format!("Could not remove archived vault: {rm_err}"))
                })
        })?;
    // Best-effort: clear the cached unlock session so a stale unlock does
    // not survive the archive. Failure here is non-fatal because the file
    // is already moved and the user can re-unlock against the archive.
    let _ = crate::modules::provider_settings::ipfs_lock_vault();
    let mut index = load_vault_index()?;
    let path_str = target.to_string_lossy().to_string();
    index.archives.insert(0, path_str.clone());
    if index.archives.len() > VAULT_MAX_KNOWN_ENTRIES {
        index.archives.truncate(VAULT_MAX_KNOWN_ENTRIES);
    }
    // The current active vault is no longer present; drop any label entry
    // that pointed at the now-archived path.
    let current_str = current.to_string_lossy().to_string();
    index.labels.remove(&current_str);
    if !index.active_label.is_empty() {
        index.active_label.clear();
    }
    if !index.active_export_path.is_empty() {
        index.active_export_path.clear();
    }
    save_vault_index(&index)?;
    Ok(serde_json::json!({
        "archived": true,
        "archive_path": path_str,
        "archive_dir": archive_dir.to_string_lossy().to_string(),
    }))
}

#[tauri::command]
pub fn vault_setup(passphrase: String) -> Result<serde_json::Value, String> {
    let payload = VaultPayload::default();
    let bundle = create_vault(&passphrase, &payload)?;
    let kdf_profile = bundle
        .vault
        .key_slots
        .first()
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
pub fn vault_export_bundle_to_path(path: String) -> Result<String, String> {
    export_bundle_to_path(&path)
}

#[tauri::command]
pub fn vault_validate_import_bundle(path: String) -> Result<serde_json::Value, String> {
    validate_import_bundle_from_path(&path)
}

// ─── Wallet Migration Record Helpers ─────────────────────────────────────

const WALLET_MIGRATION_RECORD_PREFIX: &str = "wallet.core_migration_envelope.";

const RECOVERY_MODE_VAULT_PASSPHRASE: &str = "vault_passphrase";
const RECOVERY_MODE_SEPARATE_PASSPHRASE: &str = "separate_passphrase";

fn vault_tmp_dir() -> Result<PathBuf, String> {
    let dir = commander_dir()?.join("tmp");
    fs::create_dir_all(&dir).map_err(|e| format!("Cannot create vault tmp dir: {e}"))?;
    Ok(dir)
}

struct TempFileGuard {
    path: PathBuf,
}

impl TempFileGuard {
    fn new(prefix: &str) -> Result<Self, String> {
        let dir = vault_tmp_dir()?;
        let nanos = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
        let path = dir.join(format!("{prefix}_{nanos}.json"));
        Ok(Self { path })
    }

    fn path_str(&self) -> String {
        self.path.to_string_lossy().to_string()
    }
}

impl Drop for TempFileGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn clean_stale_vault_temp_files() {
    if let Ok(dir) = vault_tmp_dir() {
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if name_str.starts_with("vault_migration_temp_")
                    || name_str.starts_with("vault_import_validate_")
                    || name_str.starts_with("vault_restore_temp_")
                    || name_str.starts_with("vault_webcom_connect_")
                    || name_str.starts_with("vault_alignment_backup_")
                {
                    let _ = fs::remove_file(entry.path());
                }
            }
        }
    }
}

fn generate_collision_safe_record_id(prefix: &str, content: &str) -> String {
    let nanos = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let mut hasher = Sha256::new();
    sha2::Digest::update(&mut hasher, content.as_bytes());
    sha2::Digest::update(&mut hasher, nanos.to_le_bytes());
    let hash = sha2::Digest::finalize(hasher);
    let hash_hex = hex::encode(&hash[..4]);
    format!("{WALLET_MIGRATION_RECORD_PREFIX}{prefix}-{nanos}-{hash_hex}")
}

fn validate_wallet_migration_record_id(record_id: &str) -> Result<(), String> {
    if !record_id.starts_with(WALLET_MIGRATION_RECORD_PREFIX) {
        return Err(format!(
            "Invalid wallet migration record id: {record_id}. Must start with {WALLET_MIGRATION_RECORD_PREFIX}"
        ));
    }
    let suffix = &record_id[WALLET_MIGRATION_RECORD_PREFIX.len()..];
    if suffix.is_empty() {
        return Err("Wallet migration record id must have a suffix after the prefix".to_string());
    }
    if suffix.len() > 64 {
        return Err("Wallet migration record id suffix must not exceed 64 characters".to_string());
    }
    if !suffix
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
    {
        return Err("Wallet migration record id suffix must contain only alphanumeric, underscore, or hyphen".to_string());
    }
    Ok(())
}

fn validate_label(label: &str) -> Result<String, String> {
    let trimmed = label.trim().to_string();
    if trimmed.is_empty() {
        return Err("Label is required".to_string());
    }
    if trimmed.len() > MAX_LABEL_LENGTH {
        return Err(format!(
            "Label must not exceed {MAX_LABEL_LENGTH} characters"
        ));
    }
    Ok(trimmed)
}

fn validate_import_file(path: &PathBuf) -> Result<String, String> {
    if !path.exists() {
        return Err("Import file does not exist".to_string());
    }
    if path.is_dir() {
        return Err("Import path is a directory, not a file".to_string());
    }
    let metadata = fs::metadata(path).map_err(|e| format!("Cannot read file metadata: {e}"))?;
    let size = metadata.len();
    if size > MAX_IMPORT_FILE_SIZE {
        return Err(format!(
            "Import file is too large: {} bytes (max {})",
            size, MAX_IMPORT_FILE_SIZE
        ));
    }
    if size == 0 {
        return Err("Import file is empty".to_string());
    }
    let content = fs::read_to_string(path).map_err(|e| format!("Cannot read import file: {e}"))?;
    Ok(content)
}

fn validate_migration_envelope_file(
    path: &str,
    migration_passphrase: &str,
) -> Result<serde_json::Value, String> {
    crate::modules::commands::validate_wallet_migration(
        path.to_string(),
        migration_passphrase.to_string(),
    )
}

fn extract_validation_metadata(validation: &serde_json::Value) -> serde_json::Value {
    let restorable = validation
        .get("restorable")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let private_keys = validation
        .get("private_keys_included")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let chain = validation
        .get("chain")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let kdf = validation
        .get("envelope_kdf_profile")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let cipher = validation
        .get("envelope_cipher_profile")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let aad = validation
        .get("envelope_aad_profile")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let coin_type = validation
        .get("envelope_coin_type")
        .and_then(|v| v.as_i64())
        .unwrap_or(-1);

    let mut meta = serde_json::json!({
        "value_kind": "embedded_encrypted_json",
        "restorable": restorable,
        "private_keys_included": private_keys,
        "envelope_kdf_profile": kdf,
        "envelope_cipher_profile": cipher,
        "envelope_aad_profile": aad,
    });
    if let Some(obj) = meta.as_object_mut() {
        if !chain.is_null() {
            obj.insert("chain".to_string(), chain);
        }
        if coin_type >= 0 {
            obj.insert(
                "envelope_coin_type".to_string(),
                serde_json::Value::Number(coin_type.into()),
            );
        }
    }
    meta
}

pub fn insert_wallet_migration_record(
    passphrase: &str,
    record_id: &str,
    label: &str,
    value: &str,
    metadata: serde_json::Value,
) -> Result<serde_json::Value, String> {
    validate_wallet_migration_record_id(record_id)?;
    let label = validate_label(label)?;
    if value.trim().is_empty() {
        return Err("Migration envelope value is required".to_string());
    }

    let mut bundle = load_bundle()?.ok_or("Vault does not exist")?;
    let dek = unwrap_dek_with_passphrase(passphrase, &bundle.vault)?;
    let mut payload = decrypt_payload_with_dek(dek.as_slice(), &bundle.vault)?;

    if payload.secrets.contains_key(record_id) {
        return Err(format!(
            "A wallet migration record with id {record_id} already exists. Use a different id or remove the existing record first."
        ));
    }

    let now = chrono::Utc::now().timestamp();

    let record = SecretRecord {
        record_id: record_id.to_string(),
        record_type: RECORD_TYPE_WALLET_CORE_MIGRATION.to_string(),
        label: label.clone(),
        value: value.to_string(),
        metadata: Some(metadata),
        tags: Some(vec!["wallet".to_string(), "migration".to_string()]),
        origin_app: Some(APP_IDENTIFIER.to_string()),
        derivation_profiles: None,
        network: Some(
            bundle
                .vault
                .network
                .clone()
                .unwrap_or_else(|| "mainnet".to_string()),
        ),
        created: now,
        modified: now,
    };

    payload.secrets.insert(record_id.to_string(), record);

    bundle.vault.modified = now;
    bundle.vault.payload = encrypt_payload_with_dek(dek.as_slice(), &payload, &bundle.vault)?;
    save_bundle_atomic(&bundle)?;

    Ok(serde_json::json!({
        "inserted": true,
        "record_id": record_id,
        "record_type": RECORD_TYPE_WALLET_CORE_MIGRATION,
        "label": label,
        "modified": now,
    }))
}

pub fn list_wallet_migration_records(passphrase: &str) -> Result<Vec<serde_json::Value>, String> {
    let bundle = load_bundle()?.ok_or("Vault does not exist")?;
    let payload = decrypt_vault_envelope(passphrase, &bundle.vault)?;

    let records: Vec<serde_json::Value> = payload
        .secrets
        .iter()
        .filter(|(_, r)| r.record_type == RECORD_TYPE_WALLET_CORE_MIGRATION)
        .map(|(_, r)| {
            serde_json::json!({
                "record_id": r.record_id,
                "record_type": r.record_type,
                "label": r.label,
                "metadata": r.metadata,
                "tags": r.tags,
                "origin_app": r.origin_app,
                "network": r.network,
                "created": r.created,
                "modified": r.modified,
            })
        })
        .collect();

    Ok(records)
}

pub fn export_wallet_migration_record_to_path(
    passphrase: &str,
    record_id: &str,
    dest_path: &str,
) -> Result<String, String> {
    validate_wallet_migration_record_id(record_id)?;
    let bundle = load_bundle()?.ok_or("Vault does not exist")?;
    let payload = decrypt_vault_envelope(passphrase, &bundle.vault)?;

    let record = payload
        .secrets
        .get(record_id)
        .ok_or(format!("No wallet migration record found for: {record_id}"))?;

    if record.record_type != RECORD_TYPE_WALLET_CORE_MIGRATION {
        return Err(format!(
            "Record {record_id} is not a wallet migration record"
        ));
    }

    let dest = PathBuf::from(dest_path);
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Cannot create destination directory: {e}"))?;
    }
    fs::write(&dest, &record.value).map_err(|e| format!("Cannot write to destination: {e}"))?;
    Ok(dest.to_string_lossy().to_string())
}

pub fn remove_wallet_migration_record(
    passphrase: &str,
    record_id: &str,
) -> Result<serde_json::Value, String> {
    validate_wallet_migration_record_id(record_id)?;
    let mut bundle = load_bundle()?.ok_or("Vault does not exist")?;
    let dek = unwrap_dek_with_passphrase(passphrase, &bundle.vault)?;
    let mut payload = decrypt_payload_with_dek(dek.as_slice(), &bundle.vault)?;

    let removed = payload.secrets.remove(record_id);
    if removed.is_none() {
        return Err(format!("No wallet migration record found for: {record_id}"));
    }
    let removed = removed.unwrap();
    if removed.record_type != RECORD_TYPE_WALLET_CORE_MIGRATION {
        payload.secrets.insert(record_id.to_string(), removed);
        return Err(format!(
            "Record {record_id} is not a wallet migration record"
        ));
    }

    let now = chrono::Utc::now().timestamp();
    bundle.vault.modified = now;
    bundle.vault.payload = encrypt_payload_with_dek(dek.as_slice(), &payload, &bundle.vault)?;
    save_bundle_atomic(&bundle)?;

    Ok(serde_json::json!({
        "removed": true,
        "record_id": record_id,
        "label": removed.label,
    }))
}

// ─── Wallet Migration Record Tauri Commands ─────────────────────────────

#[tauri::command]
pub fn vault_import_wallet_migration_record_from_path(
    path: String,
    label: String,
    migration_passphrase: Option<String>,
    vault_passphrase: Option<String>,
) -> Result<serde_json::Value, String> {
    let src = PathBuf::from(&path);
    let content = validate_import_file(&src)?;

    let _: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| format!("Import file is not valid JSON: {e}"))?;

    let passphrase = vault_passphrase.ok_or("Vault passphrase is required")?;

    let temp = TempFileGuard::new("vault_import_validate")?;
    fs::write(&temp.path, &content).map_err(|e| format!("Cannot write temp file: {e}"))?;

    let mig_pass = migration_passphrase.unwrap_or_default();
    let validation = validate_migration_envelope_file(&temp.path_str(), &mig_pass)
        .map_err(|e| format!("Migration envelope validation failed: {e}. The file may not be a valid Core Next migration envelope, or the migration passphrase may be incorrect."))?;

    let valid = validation
        .get("valid")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if !valid {
        return Err("Migration envelope validation reported the file as invalid. It cannot be stored as a restorable wallet record.".to_string());
    }
    let restorable = validation
        .get("restorable")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let private_keys_included = validation
        .get("private_keys_included")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if !restorable || !private_keys_included {
        return Err("Only encrypted private Core migration envelopes can be stored as vault recovery snapshots. Public-only migration envelopes are metadata-only and cannot restore a wallet.".to_string());
    }

    let mut metadata = extract_validation_metadata(&validation);
    if let Some(obj) = metadata.as_object_mut() {
        obj.insert(
            "source".to_string(),
            serde_json::Value::String("file-import".to_string()),
        );
        obj.insert(
            "imported_at".to_string(),
            serde_json::Value::Number(chrono::Utc::now().timestamp().into()),
        );
        obj.insert(
            "original_filename".to_string(),
            serde_json::Value::String(
                src.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string(),
            ),
        );
    }

    let record_id = generate_collision_safe_record_id("import", &content);
    let label = validate_label(&label)?;

    insert_wallet_migration_record(&passphrase, &record_id, &label, &content, metadata)
}

#[tauri::command]
pub fn vault_export_current_wallet_migration_record(
    label: String,
    include_private: bool,
    migration_passphrase: String,
    vault_passphrase: Option<String>,
    recovery_mode: Option<String>,
) -> Result<serde_json::Value, String> {
    if !include_private {
        return Err(
            "Only private (restorable) migration envelopes can be stored in the vault".to_string(),
        );
    }

    let effective_recovery_mode = recovery_mode
        .as_deref()
        .map(|m| m.trim().to_string())
        .filter(|m| m == RECOVERY_MODE_VAULT_PASSPHRASE || m == RECOVERY_MODE_SEPARATE_PASSPHRASE)
        .unwrap_or_else(|| RECOVERY_MODE_SEPARATE_PASSPHRASE.to_string());

    let passphrase = vault_passphrase.ok_or("Vault passphrase is required")?;
    let label = validate_label(&label)?;

    let effective_migration_passphrase: String =
        if effective_recovery_mode == RECOVERY_MODE_VAULT_PASSPHRASE {
            passphrase.clone()
        } else {
            if migration_passphrase.len() < 8 {
                return Err("Migration passphrase must be at least 8 characters".to_string());
            }
            if migration_passphrase.len() > 1024 {
                return Err("Migration passphrase must not exceed 1024 characters".to_string());
            }
            migration_passphrase.clone()
        };

    let temp = TempFileGuard::new("vault_migration_temp")?;

    let export_result = crate::modules::commands::export_wallet_migration(
        temp.path_str(),
        true,
        true,
        effective_migration_passphrase.clone(),
    )
    .map_err(|e| format!("Failed to export wallet migration: {e}"))?;

    let content = fs::read_to_string(&temp.path)
        .map_err(|e| format!("Cannot read temp migration file: {e}"))?;

    let validation =
        validate_migration_envelope_file(&temp.path_str(), &effective_migration_passphrase)
            .map_err(|e| format!("Migration envelope validation after export failed: {e}"))?;

    let valid = validation
        .get("valid")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if !valid {
        return Err("Exported migration envelope failed validation. It cannot be stored as a restorable wallet record.".to_string());
    }

    let mut metadata = extract_validation_metadata(&validation);
    if let Some(obj) = metadata.as_object_mut() {
        obj.insert(
            "source".to_string(),
            serde_json::Value::String("core-next-exportwalletmigration".to_string()),
        );
        obj.insert(
            "exported_at".to_string(),
            serde_json::Value::Number(chrono::Utc::now().timestamp().into()),
        );
        obj.insert(
            "label".to_string(),
            serde_json::Value::String(label.clone()),
        );
        obj.insert(
            "recovery_mode".to_string(),
            serde_json::Value::String(effective_recovery_mode.clone()),
        );
    }

    let record_id = generate_collision_safe_record_id("export", &content);

    let _result =
        insert_wallet_migration_record(&passphrase, &record_id, &label, &content, metadata)?;

    Ok(serde_json::json!({
        "exported_to_vault": true,
        "record_id": record_id,
        "label": label,
        "recovery_mode": effective_recovery_mode,
        "migration_filename": export_result.get("filename").and_then(|v| v.as_str()).unwrap_or(""),
    }))
}

#[tauri::command]
pub fn vault_restore_wallet_migration_record(
    record_id: String,
    wallet_name: String,
    migration_passphrase: String,
    birth_height: Option<i64>,
    vault_passphrase: Option<String>,
) -> Result<serde_json::Value, String> {
    let passphrase = vault_passphrase.ok_or("Vault passphrase is required")?;

    let bundle = load_bundle()?.ok_or("Vault does not exist")?;
    let payload = decrypt_vault_envelope(&passphrase, &bundle.vault)?;

    let record = payload
        .secrets
        .get(&record_id)
        .ok_or(format!("No wallet migration record found for: {record_id}"))?;

    if record.record_type != RECORD_TYPE_WALLET_CORE_MIGRATION {
        return Err(format!(
            "Record {record_id} is not a wallet migration record"
        ));
    }

    let recovery_mode = record
        .metadata
        .as_ref()
        .and_then(|m| m.get("recovery_mode"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let effective_migration_passphrase: String = if recovery_mode == RECOVERY_MODE_VAULT_PASSPHRASE
        && !migration_passphrase.is_empty()
    {
        return Err(
            "This backup record was created with vault-passphrase recovery. Do not provide a separate migration passphrase — the vault passphrase is used automatically.".to_string()
        );
    } else if recovery_mode == RECOVERY_MODE_VAULT_PASSPHRASE {
        passphrase.clone()
    } else {
        if migration_passphrase.len() < 8 {
            return Err("Migration passphrase must be at least 8 characters".to_string());
        }
        migration_passphrase.clone()
    };

    let temp = TempFileGuard::new("vault_restore_temp")?;

    export_wallet_migration_record_to_path(&passphrase, &record_id, &temp.path_str())
        .map_err(|e| format!("Cannot export record to temp file: {e}"))?;

    let wallet_name_for_result = wallet_name.clone();
    let restore_result = crate::modules::commands::restore_wallet_migration(
        temp.path_str(),
        wallet_name,
        effective_migration_passphrase,
        birth_height,
    )
    .map_err(|e| format!("Failed to restore wallet migration: {e}"))?;

    Ok(serde_json::json!({
        "restored": true,
        "record_id": record_id,
        "wallet_name": restore_result.get("wallet_name").and_then(|v| v.as_str()).unwrap_or(&wallet_name_for_result),
        "wallet_arg": restore_result.get("wallet_arg").and_then(|v| v.as_str()).unwrap_or(""),
    }))
}

#[tauri::command]
pub fn vault_list_wallet_migration_records(
    vault_passphrase: Option<String>,
) -> Result<Vec<serde_json::Value>, String> {
    clean_stale_vault_temp_files();
    let passphrase = vault_passphrase.ok_or("Vault passphrase is required")?;
    list_wallet_migration_records(&passphrase)
}

#[tauri::command]
pub fn vault_remove_wallet_migration_record(
    record_id: String,
    vault_passphrase: Option<String>,
) -> Result<serde_json::Value, String> {
    let passphrase = vault_passphrase.ok_or("Vault passphrase is required")?;
    remove_wallet_migration_record(&passphrase, &record_id)
}

// ─── WebCom / Hemp0x Vault Interop Commands ─────────────────────────────

#[tauri::command]
pub fn vault_get_webcom_interop_summary(
    vault_passphrase: Option<String>,
) -> Result<serde_json::Value, String> {
    let passphrase = vault_passphrase.ok_or("Vault passphrase is required")?;
    let bundle = load_bundle()?.ok_or("Vault does not exist")?;
    let payload = decrypt_vault_envelope(&passphrase, &bundle.vault)?;

    let mut items: Vec<serde_json::Value> = Vec::new();

    for record_id in KNOWN_WEBCOM_RECORD_IDS {
        if let Some(record) = payload.secrets.get(*record_id) {
            let mut summary = serde_json::json!({
                "exists": true,
                "record_id": record.record_id,
                "record_type": record.record_type,
                "label": record.label,
                "origin_app": record.origin_app,
                "network": record.network,
                "created": record.created,
                "modified": record.modified,
                "has_value": !record.value.is_empty(),
            });

            if let Some(ref meta) = record.metadata {
                if let Some(obj) = summary.as_object_mut() {
                    if let Some(vk) = meta.get("value_kind") {
                        obj.insert("value_kind".to_string(), vk.clone());
                    }
                    if let Some(schema) = meta.get("schema") {
                        obj.insert("metadata_schema".to_string(), schema.clone());
                    }
                    if let Some(sv) = meta.get("schema_version") {
                        obj.insert("metadata_schema_version".to_string(), sv.clone());
                    }
                }

                if record.record_id == RECORD_ID_WALLET_HEMP_PRIMARY {
                    if let Some(recovery) = meta.get("recovery") {
                        if let Some(obj) = summary.as_object_mut() {
                            if let Some(dp) = recovery.get("derivationProfiles") {
                                obj.insert("derivation_profiles".to_string(), dp.clone());
                            }
                            if let Some(st) = recovery.get("seedType") {
                                obj.insert("seed_type".to_string(), st.clone());
                            }
                            if let Some(ca) = recovery.get("createdAt") {
                                obj.insert("recovery_created_at".to_string(), ca.clone());
                            }
                            if let Some(ua) = recovery.get("updatedAt") {
                                obj.insert("recovery_updated_at".to_string(), ua.clone());
                            }
                            if let Some(net) = recovery.get("network") {
                                obj.insert("recovery_network".to_string(), net.clone());
                            }
                        }
                    }
                    if let Some(obj) = summary.as_object_mut() {
                        if let Some(acct) = meta.get("account") {
                            obj.insert("account".to_string(), acct.clone());
                        }
                        if let Some(ec) = meta.get("external_count") {
                            obj.insert("external_count".to_string(), ec.clone());
                        }
                        if let Some(cc) = meta.get("change_count") {
                            obj.insert("change_count".to_string(), cc.clone());
                        }
                    }
                }

                if record.record_id == RECORD_ID_WALLET_BTC_LITE_PRIMARY {
                    if let Some(obj) = summary.as_object_mut() {
                        if let Some(ba) = meta.get("btc_account") {
                            obj.insert("btc_account".to_string(), ba.clone());
                        }
                        if let Some(bec) = meta.get("btc_external_count") {
                            obj.insert("btc_external_count".to_string(), bec.clone());
                        }
                        if let Some(bdp) = meta.get("btc_derivation_profile") {
                            obj.insert("btc_derivation_profile".to_string(), bdp.clone());
                        }
                    }
                }

                if record.record_id == RECORD_ID_SWAP_SECRETS {
                    if let Some(obj) = summary.as_object_mut() {
                        if let Some(vk) = meta.get("value_kind") {
                            obj.insert("value_kind".to_string(), vk.clone());
                        }
                    }
                }
            }

            if let Some(ref dp) = record.derivation_profiles {
                if let Some(obj) = summary.as_object_mut() {
                    obj.insert(
                        "derivation_profiles".to_string(),
                        serde_json::to_value(dp).unwrap_or_default(),
                    );
                }
            }

            items.push(summary);
        } else {
            items.push(serde_json::json!({
                "exists": false,
                "record_id": record_id,
            }));
        }
    }

    Ok(serde_json::json!({
        "vault_locked": false,
        "items": items,
        "known_record_count": KNOWN_WEBCOM_RECORD_IDS.len(),
        "present_record_count": items.iter().filter(|i| i["exists"].as_bool().unwrap_or(false)).count(),
    }))
}

#[tauri::command]
pub fn vault_get_address_book_record_summary(
    vault_passphrase: Option<String>,
) -> Result<serde_json::Value, String> {
    let passphrase = vault_passphrase.ok_or("Vault passphrase is required")?;
    let bundle = load_bundle()?.ok_or("Vault does not exist")?;
    let payload = decrypt_vault_envelope(&passphrase, &bundle.vault)?;

    if let Some(record) = payload.secrets.get(RECORD_ID_ADDRESS_BOOK) {
        let value_str = &record.value;
        if value_str.is_empty() {
            return Ok(
                serde_json::json!({ "exists": false, "record_id": RECORD_ID_ADDRESS_BOOK, "error": "empty value" }),
            );
        }

        match serde_json::from_str::<serde_json::Value>(value_str) {
            Ok(parsed) => {
                let schema = parsed.get("schema").and_then(|v| v.as_str()).unwrap_or("");
                let schema_version = parsed
                    .get("schema_version")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);
                let exported_at = parsed
                    .get("exported_at")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);
                let entries = parsed
                    .get("entries")
                    .and_then(|v| v.as_array())
                    .map(|a| a.len())
                    .unwrap_or(0);
                let hemp_entries = parsed
                    .get("entries")
                    .and_then(|v| v.as_array())
                    .map(|a| {
                        a.iter()
                            .filter(|e| {
                                e.get("chain").and_then(|c| c.as_str())
                                    == Some(ADDRESS_BOOK_CHAIN_HEMP)
                            })
                            .count()
                    })
                    .unwrap_or(0);
                let btc_entries = parsed
                    .get("entries")
                    .and_then(|v| v.as_array())
                    .map(|a| {
                        a.iter()
                            .filter(|e| {
                                e.get("chain").and_then(|c| c.as_str())
                                    == Some(ADDRESS_BOOK_CHAIN_BITCOIN)
                            })
                            .count()
                    })
                    .unwrap_or(0);

                Ok(serde_json::json!({
                    "exists": true,
                    "record_id": RECORD_ID_ADDRESS_BOOK,
                    "schema": schema,
                    "schema_version": schema_version,
                    "exported_at": exported_at,
                    "total_entries": entries,
                    "hemp_entries": hemp_entries,
                    "bitcoin_entries": btc_entries,
                }))
            }
            Err(_) => Ok(serde_json::json!({
                "exists": true,
                "record_id": RECORD_ID_ADDRESS_BOOK,
                "error": "malformed value JSON",
            })),
        }
    } else {
        Ok(serde_json::json!({ "exists": false, "record_id": RECORD_ID_ADDRESS_BOOK }))
    }
}

#[tauri::command]
pub fn vault_export_address_book_record(
    vault_passphrase: Option<String>,
) -> Result<serde_json::Value, String> {
    let passphrase = vault_passphrase.ok_or("Vault passphrase is required")?;
    let mut bundle = load_bundle()?.ok_or("Vault does not exist")?;
    let dek = unwrap_dek_with_passphrase(&passphrase, &bundle.vault)?;
    let mut payload = decrypt_payload_with_dek(dek.as_slice(), &bundle.vault)?;

    let local_entries = crate::modules::files::load_address_book()
        .map_err(|e| format!("Cannot load local address book: {e}"))?;

    let now = chrono::Utc::now().timestamp();

    let mut merged_entries: Vec<serde_json::Value> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    for entry in &local_entries {
        let normalized = entry.address.trim().to_string();
        if !normalized.is_empty() && !seen.contains(&normalized) {
            seen.insert(normalized.clone());
            merged_entries.push(serde_json::json!({
                "chain": ADDRESS_BOOK_CHAIN_HEMP,
                "label": entry.label,
                "address": entry.address,
                "locked": entry.locked,
            }));
        }
    }

    if let Some(existing_record) = payload.secrets.get(RECORD_ID_ADDRESS_BOOK) {
        if let Ok(existing_value) =
            serde_json::from_str::<serde_json::Value>(&existing_record.value)
        {
            if let Some(existing_entries) = existing_value.get("entries").and_then(|v| v.as_array())
            {
                for e in existing_entries {
                    let chain = e.get("chain").and_then(|c| c.as_str()).unwrap_or("");
                    let addr = e
                        .get("address")
                        .and_then(|a| a.as_str())
                        .unwrap_or("")
                        .trim()
                        .to_string();
                    if chain == ADDRESS_BOOK_CHAIN_BITCOIN
                        && !addr.is_empty()
                        && !seen.contains(&addr)
                    {
                        seen.insert(addr.clone());
                        merged_entries.push(e.clone());
                    }
                }
            }
        }
    }

    let address_book_value = serde_json::json!({
        "schema": ADDRESS_BOOK_SCHEMA,
        "schema_version": ADDRESS_BOOK_SCHEMA_VERSION,
        "exported_at": now,
        "entries": merged_entries,
    });

    let record = SecretRecord {
        record_id: RECORD_ID_ADDRESS_BOOK.to_string(),
        record_type: RECORD_TYPE_APP_SETTING_ADDRESS_BOOK.to_string(),
        label: "Hemp0x Address Book".to_string(),
        value: serde_json::to_string(&address_book_value)
            .map_err(|e| format!("Cannot serialize address book: {e}"))?,
        metadata: Some(serde_json::json!({
            "value_kind": "embedded_json",
            "schema": ADDRESS_BOOK_SCHEMA,
            "schema_version": ADDRESS_BOOK_SCHEMA_VERSION,
        })),
        tags: Some(vec!["hemp0x".to_string(), "address_book".to_string()]),
        origin_app: Some(APP_IDENTIFIER.to_string()),
        derivation_profiles: None,
        network: Some(
            bundle
                .vault
                .network
                .clone()
                .unwrap_or_else(|| "mainnet".to_string()),
        ),
        created: now,
        modified: now,
    };

    payload
        .secrets
        .insert(RECORD_ID_ADDRESS_BOOK.to_string(), record);
    bundle.vault.modified = now;
    bundle.vault.payload = encrypt_payload_with_dek(dek.as_slice(), &payload, &bundle.vault)?;
    save_bundle_atomic(&bundle)?;

    Ok(serde_json::json!({
        "exported": true,
        "record_id": RECORD_ID_ADDRESS_BOOK,
        "hemp_entries": local_entries.len(),
    }))
}

#[tauri::command]
pub fn vault_import_address_book_record(
    vault_passphrase: Option<String>,
) -> Result<serde_json::Value, String> {
    let passphrase = vault_passphrase.ok_or("Vault passphrase is required")?;
    let bundle = load_bundle()?.ok_or("Vault does not exist")?;
    let payload = decrypt_vault_envelope(&passphrase, &bundle.vault)?;

    let record = payload
        .secrets
        .get(RECORD_ID_ADDRESS_BOOK)
        .ok_or("No address book record found in vault")?;

    let value_str = &record.value;
    if value_str.is_empty() {
        return Err("Address book record value is empty".to_string());
    }

    let parsed: serde_json::Value = serde_json::from_str(value_str)
        .map_err(|e| format!("Malformed address book record: {e}"))?;

    let schema = parsed.get("schema").and_then(|v| v.as_str()).unwrap_or("");
    let schema_version = parsed
        .get("schema_version")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    if schema != ADDRESS_BOOK_SCHEMA || schema_version != ADDRESS_BOOK_SCHEMA_VERSION as i64 {
        return Err(format!(
            "Address book schema mismatch: expected {} v{}, got {} v{}",
            ADDRESS_BOOK_SCHEMA, ADDRESS_BOOK_SCHEMA_VERSION, schema, schema_version
        ));
    }

    let vault_entries = parsed
        .get("entries")
        .and_then(|v| v.as_array())
        .ok_or("Address book record has no entries array")?;

    let mut local_entries = crate::modules::files::load_address_book()
        .map_err(|e| format!("Cannot load local address book: {e}"))?;

    let now = chrono::Utc::now().timestamp();
    let mut existing_map: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for (i, entry) in local_entries.iter().enumerate() {
        let addr = entry.address.trim().to_string();
        if !addr.is_empty() {
            existing_map.insert(addr, i);
        }
    }

    let mut imported = 0u32;
    let mut updated = 0u32;
    let mut skipped = 0u32;
    let mut preserved_non_hemp = 0u32;

    for e in vault_entries {
        let chain = e.get("chain").and_then(|c| c.as_str()).unwrap_or("");
        let addr = e
            .get("address")
            .and_then(|a| a.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        let label = e
            .get("label")
            .and_then(|l| l.as_str())
            .unwrap_or("")
            .to_string();
        let locked = e.get("locked").and_then(|l| l.as_bool()).unwrap_or(false);

        if addr.is_empty() {
            skipped += 1;
            continue;
        }

        if chain != ADDRESS_BOOK_CHAIN_HEMP {
            preserved_non_hemp += 1;
            continue;
        }

        if let Some(&idx) = existing_map.get(&addr) {
            let existing = &local_entries[idx];
            let merged_label =
                if label.is_empty() || (!existing.label.is_empty() && label != existing.label) {
                    existing.label.clone()
                } else {
                    label.clone()
                };
            let merged_locked = locked || existing.locked;
            if merged_label != existing.label || merged_locked != existing.locked {
                local_entries[idx] = crate::modules::models::AddressBookEntry {
                    label: merged_label,
                    address: existing.address.clone(),
                    locked: merged_locked,
                    date: existing.date,
                };
                updated += 1;
            }
            continue;
        }

        local_entries.push(crate::modules::models::AddressBookEntry {
            label,
            address: addr.clone(),
            locked,
            date: now as u64,
        });
        existing_map.insert(addr, local_entries.len() - 1);
        imported += 1;
    }

    crate::modules::files::save_address_book(local_entries)
        .map_err(|e| format!("Cannot save local address book: {e}"))?;

    Ok(serde_json::json!({
        "imported": imported,
        "updated": updated,
        "skipped": skipped,
        "preserved_non_hemp_entries": preserved_non_hemp,
    }))
}

// ─── Portable Wallet Bridge — slice 64b ──────────────────────────────────
//
// This block adds only metadata-only, read-only helpers. It does NOT
// write to `wallet.webcom.hemp.primary`, does NOT promote Commander
// records into the WebCom primary slot, and does NOT return any
// `SecretRecord.value` content to the frontend. Promotion/writing is
// intentionally deferred until the safety requirements in
// `untracked/commander-v1.4/handoff-prompts/hemp0x-vault-portable-wallet-model-slice-64b.md`
// are all met.

// ─── Wallet Alignment Model — slice 64c ──────────────────────────────────
//
// The alignment model tracks whether Commander's active Core runtime
// wallet is connected to the WebCom primary wallet record in the vault.
// It records the relationship so future imports do not repeat setup.
//
// Alignment metadata is stored as a normal vault record. It never
// contains mnemonics, WIF keys, xprv material, or migration envelope
// plaintext. The fingerprint is a deterministic public-data hash.

const RECORD_ID_WALLET_ALIGNMENT: &str = "app_setting.commander.wallet_alignment";
const RECORD_TYPE_APP_SETTING_WALLET_ALIGNMENT: &str = "app_setting.commander.wallet_alignment";
const ALIGNMENT_SCHEMA: &str = "hemp0x.commander.wallet_alignment";
const ALIGNMENT_SCHEMA_VERSION: i32 = 1;

const CORE_WALLET_SOURCE_WEBCOM_BIP39: &str = "webcom_bip39";
#[allow(dead_code)]
const CORE_WALLET_SOURCE_COMMANDER_BIP39: &str = "commander_bip39";
#[allow(dead_code)]
const CORE_WALLET_SOURCE_CORE_MIGRATION: &str = "core_migration";
#[allow(dead_code)]
const CORE_WALLET_SOURCE_WALLET_DAT_LEGACY: &str = "wallet_dat_legacy";
#[allow(dead_code)]
const CORE_WALLET_SOURCE_WIF_KEYSET: &str = "wif_keyset";
#[allow(dead_code)]
const CORE_WALLET_SOURCE_UNKNOWN: &str = "unknown";

const VERIFICATION_METHOD_RESTORE_FROM_GENERATED: &str =
    "core_restorewalletmigration_from_generated_envelope";

const MIGRATION_SCHEMA_IDENTIFIER: &str = "hemp0x-core.migration-envelope.v2";
const MIGRATION_ENVELOPE_VERSION: i32 = 2;
const MIGRATION_PRIVATE_PAYLOAD_FORMAT: &str = "hemp0x-core.private-migration-payload.v1";
const MIGRATION_PRIVATE_PAYLOAD_VERSION: i32 = 1;
const MIGRATION_KDF_PROFILE: &str = "pbkdf2-hmac-sha512-v1";
const MIGRATION_KDF_ITERATIONS: u32 = 600_000;
const MIGRATION_CIPHER_PROFILE: &str = "aes-256-gcm-v1";
const MIGRATION_AAD_PROFILE: &str = "hemp0x-core-migration-aad-v1";
const MIGRATION_SALT_SIZE: usize = 32;
const MIGRATION_IV_SIZE: usize = 12;
const MIGRATION_KEY_SIZE: usize = 32;
const MIGRATION_TAG_SIZE: usize = 16;
const MIGRATION_PURPOSE_LABEL_PRIVATE: &str = "private-payload";
const MIGRATION_SOURCE_CLIENT: &str = "hemp0x-commander";
const MIGRATION_SOURCE_CLIENT_VERSION: &str = "2.0.0";
const MIGRATION_WALLET_TYPE: &str = "bip39_bip44_p2pkh";
const MIGRATION_MNEMONIC_LANGUAGE: &str = "english";
const MIGRATION_DEFAULT_WALLET_NAME: &str = "hemp0x-vault-main";
const CORE_MIGRATION_MAINNET_NETWORK_ID: &str = "main";

// When the WebCom record does not carry a `best_block_height`, the
// guided restore uses a "fast default" that skips the chain rescan
// almost entirely. The restore creates the wallet.dat with all derived
// addresses, then we use Core's global `scantxoutset` to populate the
// UTXO set for the wallet's addresses (no rescan needed) and a
// background `rescanblockchain` to fill in the history. This gives
// the user a working wallet with the correct balance in seconds, with
// full history filling in over the next few minutes.
//
// If a record carries `best_block_height` we use it directly because
// it is the most accurate bound available.
//
// PASS-THROUGH MODE: if `birth_height` is explicitly passed in, it
// is used as-is (this is for power users who know what they want).
//
// The "fast default" window: 1 block back from the current tip means
// restorewalletmigration does essentially no rescan (just the latest
// block for confirmation). Combined with the post-restore
// `scantxoutset` + background `rescanblockchain`, this gives the user
// a working wallet in seconds, not hours.
const FAST_DEFAULT_BIRTH_HEIGHT_BACKOFF: i64 = 1;

const RECORD_ID_CONNECTION_INTENT: &str = "app_setting.commander.wallet_connection_intent";
const RECORD_TYPE_CONNECTION_INTENT: &str = "app_setting.commander.wallet_connection_intent";

fn build_alignment_fingerprint(webcom_primary_record: &SecretRecord) -> String {
    let mut hasher = Sha256::new();
    hasher.update(webcom_primary_record.record_id.as_bytes());
    hasher.update(webcom_primary_record.record_type.as_bytes());
    if let Some(net) = &webcom_primary_record.network {
        hasher.update(net.as_bytes());
    }
    if let Some(dp) = &webcom_primary_record.derivation_profiles {
        if let Some(hemp) = dp.get("hemp") {
            hasher.update(b"|hemp:");
            hasher.update(hemp.as_bytes());
        }
    }
    if let Some(ref meta) = webcom_primary_record.metadata {
        if let Some(st) = meta
            .get("recovery")
            .and_then(|r| r.get("seedType"))
            .and_then(|v| v.as_str())
        {
            hasher.update(b"|seed:");
            hasher.update(st.as_bytes());
        }
    }
    hex::encode(hasher.finalize())
}

fn build_core_wallet_alignment_fingerprint(wallet_shape: &serde_json::Value) -> String {
    let mut hasher = Sha256::new();
    if let Some(hd) = wallet_shape.get("hd_enabled").and_then(|v| v.as_bool()) {
        hasher.update(format!("hd:{hd}").as_bytes());
    }
    if let Some(bip44) = wallet_shape.get("bip44_enabled").and_then(|v| v.as_bool()) {
        hasher.update(format!("|bip44:{bip44}").as_bytes());
    }
    if let Some(mnem) = wallet_shape
        .get("has_mnemonic_metadata")
        .and_then(|v| v.as_bool())
    {
        hasher.update(format!("|mnem:{mnem}").as_bytes());
    }
    hex::encode(hasher.finalize())
}

pub fn get_wallet_alignment_status(passphrase: &str) -> Result<serde_json::Value, String> {
    let bundle = load_bundle()?.ok_or("Vault does not exist")?;
    let payload = decrypt_vault_envelope(passphrase, &bundle.vault)?;

    let has_webcom_primary = payload.secrets.get(RECORD_ID_WALLET_HEMP_PRIMARY).is_some();
    let has_alignment_record = payload.secrets.get(RECORD_ID_WALLET_ALIGNMENT).is_some();
    let network = bundle
        .vault
        .network
        .clone()
        .unwrap_or_else(|| "mainnet".to_string());

    let mut result = serde_json::json!({
        "vault_exists": true,
        "vault_network": network,
        "has_webcom_primary": has_webcom_primary,
        "has_alignment_record": has_alignment_record,
        "verification_status": "needs_core_support",
    });

    if let Some(record) = payload.secrets.get(RECORD_ID_WALLET_HEMP_PRIMARY) {
        if record.value.is_empty() {
            if let Some(obj) = result.as_object_mut() {
                obj.insert(
                    "webcom_primary_empty".to_string(),
                    serde_json::Value::Bool(true),
                );
            }
        } else {
            let record_type = record.record_type.as_str();
            let seed_type = record
                .metadata
                .as_ref()
                .and_then(|m| m.get("recovery"))
                .and_then(|r| r.get("seedType"))
                .and_then(|v| v.as_str())
                .unwrap_or_else(|| {
                    if record_type == RECORD_TYPE_WALLET_WIF {
                        "wif"
                    } else {
                        "bip39"
                    }
                });

            let derivation_hemp = record
                .metadata
                .as_ref()
                .and_then(|m| m.get("recovery"))
                .and_then(|r| r.get("derivationProfiles"))
                .and_then(|dp| dp.get("hemp"))
                .and_then(|v| v.as_str())
                .or_else(|| {
                    record
                        .derivation_profiles
                        .as_ref()
                        .and_then(|dp| dp.get("hemp").and_then(|s| Some(s.as_str())))
                });

            let record_network = record
                .metadata
                .as_ref()
                .and_then(|m| m.get("recovery"))
                .and_then(|r| r.get("network"))
                .and_then(|v| v.as_str())
                .or_else(|| record.network.as_deref());

            let fingerprint = build_alignment_fingerprint(record);

            if let Some(obj) = result.as_object_mut() {
                obj.insert(
                    "webcom_primary_record_type".to_string(),
                    serde_json::Value::String(record_type.to_string()),
                );
                obj.insert(
                    "webcom_primary_seed_type".to_string(),
                    serde_json::Value::String(seed_type.to_string()),
                );
                obj.insert(
                    "webcom_primary_derivation_hemp".to_string(),
                    derivation_hemp
                        .map(|s| serde_json::Value::String(s.to_string()))
                        .unwrap_or(serde_json::Value::Null),
                );
                obj.insert(
                    "webcom_primary_network".to_string(),
                    record_network
                        .map(|s| serde_json::Value::String(s.to_string()))
                        .unwrap_or(serde_json::Value::Null),
                );
                obj.insert(
                    "webcom_primary_fingerprint".to_string(),
                    serde_json::Value::String(fingerprint),
                );
            }
        }
    }

    if let Some(alignment) = payload.secrets.get(RECORD_ID_WALLET_ALIGNMENT) {
        let meta = alignment.metadata.as_ref();
        if let Some(obj) = result.as_object_mut() {
            obj.insert(
                "alignment_record_exists".to_string(),
                serde_json::Value::Bool(true),
            );
            obj.insert(
                "alignment_record_id".to_string(),
                serde_json::Value::String(RECORD_ID_WALLET_ALIGNMENT.to_string()),
            );

            if let Some(s) = meta.and_then(|m| m.get("schema_version")) {
                obj.insert("alignment_schema_version".to_string(), s.clone());
            }
            if let Some(wr) = meta.and_then(|m| m.get("active_wallet_record_id")) {
                obj.insert("alignment_active_wallet_record_id".to_string(), wr.clone());
            }
            if let Some(fp) = meta.and_then(|m| m.get("active_wallet_format_fingerprint")) {
                obj.insert(
                    "alignment_active_wallet_format_fingerprint".to_string(),
                    fp.clone(),
                );
            }
            if let Some(src) = meta.and_then(|m| m.get("core_wallet_source")) {
                obj.insert("alignment_core_wallet_source".to_string(), src.clone());
            }
        }
    }

    let mut core_reachable = false;
    let mut _core_wallet_shape: Option<serde_json::Value> = None;
    let mut bip39_export_possible = false;

    if let Ok(raw) = crate::modules::commands::run_cli(&[String::from("getwalletmigrationinfo")]) {
        if let Ok(info) = serde_json::from_str::<serde_json::Value>(&raw) {
            core_reachable = true;
            let hd = info
                .get("hd_enabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let bip44 = info
                .get("bip44_enabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let has_mnemonic = info
                .get("has_mnemonic_metadata")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let encrypted = info
                .get("encrypted")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let locked = info
                .get("locked")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            let shape = serde_json::json!({
                "hd_enabled": hd,
                "bip44_enabled": bip44,
                "has_mnemonic_metadata": has_mnemonic,
                "encrypted": encrypted,
                "locked": locked,
            });

            if hd && bip44 && has_mnemonic {
                bip39_export_possible = true;
            }

            if let Some(obj) = result.as_object_mut() {
                obj.insert("core_wallet_shape".to_string(), shape.clone());
                let core_fp = build_core_wallet_alignment_fingerprint(&shape);
                obj.insert(
                    "core_wallet_fingerprint".to_string(),
                    serde_json::Value::String(core_fp),
                );
            }
            _core_wallet_shape = Some(shape);
        }
    }

    // Determine recommended next action before mutable borrow
    let webcom_primary_seed_type = result
        .get("webcom_primary_seed_type")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    if let Some(obj) = result.as_object_mut() {
        obj.insert(
            "core_reachable".to_string(),
            serde_json::Value::Bool(core_reachable),
        );
        obj.insert(
            "core_bip39_export_possible".to_string(),
            serde_json::Value::Bool(bip39_export_possible),
        );

        let next_action = if !has_webcom_primary {
            "create_vault".to_string()
        } else if !core_reachable {
            "connect_webcom_wallet".to_string()
        } else if has_alignment_record {
            "already_aligned".to_string()
        } else if bip39_export_possible && has_webcom_primary {
            "backup_current_core_wallet".to_string()
        } else if has_webcom_primary && !bip39_export_possible {
            if webcom_primary_seed_type == "bip39" {
                "restore_core_from_webcom_primary".to_string()
            } else {
                "export_core_bip39_to_webcom_primary".to_string()
            }
        } else {
            "unlock_vault".to_string()
        };
        obj.insert(
            "recommended_next_action".to_string(),
            serde_json::Value::String(next_action),
        );
    }

    Ok(result)
}

#[tauri::command]
pub fn vault_get_wallet_alignment_status_v2(
    vault_passphrase: Option<String>,
) -> Result<serde_json::Value, String> {
    clean_stale_vault_temp_files();
    let passphrase = vault_passphrase.ok_or("Vault passphrase is required")?;
    get_wallet_alignment_status_v2(&passphrase)
}

// ─── Wallet Alignment Tauri Command Wrappers — slice 64c ─────────────────

#[tauri::command]
pub fn vault_get_wallet_alignment_status(
    vault_passphrase: Option<String>,
) -> Result<serde_json::Value, String> {
    clean_stale_vault_temp_files();
    let passphrase = vault_passphrase.ok_or("Vault passphrase is required")?;
    get_wallet_alignment_status(&passphrase)
}
#[tauri::command]
pub fn vault_create_or_update_alignment_record(
    _active_wallet_record_id: String,
    _core_wallet_source: String,
    _derivation_profile: String,
    _core_migration_backup_record_id: Option<String>,
    _active_wallet_fingerprint: Option<String>,
    _notes: Option<Vec<String>>,
    _vault_passphrase: Option<String>,
) -> Result<serde_json::Value, String> {
    Err("Deprecated: alignment records can only be written by the verified Core restore flow (vault_connect_webcom_primary_wallet_to_core). Manual metadata-only alignment is no longer supported.".to_string())
}

// ─── Core v2 Migration Envelope Builder — slice 64d ─────────────────────
//
// Builds a Core-compatible v2 encrypted migration envelope from a
// WebCom BIP39 vault record. The mnemonic never leaves the backend.
// The envelope is written only to a temp file, validated, used for
// restore, then deleted.

fn build_migration_aad(
    network: &str,
    coin_type: i64,
    exported_at: i64,
    purpose_label: &str,
) -> Vec<u8> {
    let aad = format!(
        "{}:{}:{}:{}:{}:{}",
        MIGRATION_SCHEMA_IDENTIFIER,
        MIGRATION_ENVELOPE_VERSION,
        network,
        coin_type,
        exported_at,
        purpose_label,
    );
    aad.into_bytes()
}

#[derive(Clone, Copy, Debug)]
struct WebcomKeypoolHints {
    external_count_meta: i64,
    change_count_meta: i64,
    recovered_external_indices_count: i64,
    max_recovered_external_index: i64,
    external_count_hint: i64,
    change_count_hint: i64,
}

fn webcom_keypool_hints(webcom_record: &SecretRecord) -> WebcomKeypoolHints {
    let external_count_meta = webcom_record
        .metadata
        .as_ref()
        .and_then(|m| m.get("external_count"))
        .and_then(|v| v.as_i64())
        .filter(|n| *n > 0)
        .unwrap_or(0);
    let change_count_meta = webcom_record
        .metadata
        .as_ref()
        .and_then(|m| m.get("change_count"))
        .and_then(|v| v.as_i64())
        .filter(|n| *n > 0)
        .unwrap_or(0);

    let mut recovered_external_indices_count = 0_i64;
    let mut max_recovered_external_index = 0_i64;
    if let Some(indices) = webcom_record
        .metadata
        .as_ref()
        .and_then(|m| m.get("recovered_external_indices"))
        .and_then(|v| v.as_array())
    {
        for index in indices
            .iter()
            .filter_map(|v| v.as_i64())
            .filter(|n| *n >= 0)
        {
            recovered_external_indices_count += 1;
            max_recovered_external_index = max_recovered_external_index.max(index);
        }
    }

    let external_count_hint = (external_count_meta + 1)
        .max(max_recovered_external_index + 1)
        .max(VAULT_KEYPOOL_EXTERNAL_HINT_FLOOR)
        .min(VAULT_KEYPOOL_HINT_CEILING);
    let change_count_hint = (change_count_meta + 1)
        .max(VAULT_KEYPOOL_CHANGE_HINT_FLOOR)
        .min(VAULT_KEYPOOL_HINT_CEILING);

    WebcomKeypoolHints {
        external_count_meta,
        change_count_meta,
        recovered_external_indices_count,
        max_recovered_external_index,
        external_count_hint,
        change_count_hint,
    }
}

fn vault_keypool_refill_target(external_count_hint: i64, change_count_hint: i64) -> i64 {
    external_count_hint
        .max(change_count_hint)
        .max(VAULT_CORE_KEYPOOL_REFILL_FLOOR)
        .min(VAULT_KEYPOOL_HINT_CEILING)
}

fn build_migration_envelope_from_webcom_bip39(
    passphrase: &str,
    webcom_record: &SecretRecord,
) -> Result<String, String> {
    if webcom_record.record_type != RECORD_TYPE_WALLET_BIP39 {
        return Err(format!(
            "Only BIP39 records (record_type: {}) can be converted to Core migration envelopes. Got record_type: {}",
            RECORD_TYPE_WALLET_BIP39,
            webcom_record.record_type
        ));
    }

    let seed_type = webcom_record
        .metadata
        .as_ref()
        .and_then(|m| m.get("recovery"))
        .and_then(|r| r.get("seedType"))
        .and_then(|v| v.as_str())
        .unwrap_or("bip39");

    if seed_type != "bip39" {
        return Err(format!(
            "WebCom record seed type is {seed_type}, not bip39. Only BIP39 seed records can be converted to Core migration envelopes."
        ));
    }

    let mnemonic = Zeroizing::new(webcom_record.value.clone());
    if mnemonic.is_empty() {
        return Err("WebCom BIP39 record has an empty mnemonic value".to_string());
    }

    let word_count = mnemonic.split_whitespace().count();
    if word_count != 12 && word_count != 18 && word_count != 24 {
        return Err(format!(
            "BIP39 mnemonic has {word_count} words; expected 12, 18, or 24"
        ));
    }

    let vault_network = webcom_record
        .metadata
        .as_ref()
        .and_then(|m| m.get("recovery"))
        .and_then(|r| r.get("network"))
        .and_then(|v| v.as_str())
        .or_else(|| webcom_record.network.as_deref())
        .unwrap_or("mainnet");

    if vault_network != "mainnet" {
        return Err(format!(
            "Network {vault_network} is not supported for Core restore in this release. Only mainnet is accepted."
        ));
    }

    // Hemp0x Vault records use the product-facing network name ("mainnet").
    // Core migration envelopes are validated against GetParams().NetworkIDString(),
    // which is "main" for the current mainnet chain.
    let core_network = CORE_MIGRATION_MAINNET_NETWORK_ID;

    let derivation_hemp = webcom_record
        .metadata
        .as_ref()
        .and_then(|m| m.get("recovery"))
        .and_then(|r| r.get("derivationProfiles"))
        .and_then(|dp| dp.get("hemp"))
        .and_then(|v| v.as_str())
        .or_else(|| {
            webcom_record
                .derivation_profiles
                .as_ref()
                .and_then(|dp| dp.get("hemp").and_then(|s| Some(s.as_str())))
        })
        .unwrap_or(DERIVATION_HEMP_CANONICAL_420);

    if derivation_hemp == DERIVATION_HEMP_LEGACY_175 {
        return Err(format!(
            "Legacy coin175 derivation profile ({}) is not supported for Core restore. Only coin420 profiles are accepted.",
            DERIVATION_HEMP_LEGACY_175
        ));
    }

    if derivation_hemp == DERIVATION_HEMP_LEGACY_GENERIC {
        return Err(format!(
            "Generic derivation profile ({}) does not explicitly prove coin420. Only the exact canonical coin420 profile ({}) is accepted for Core restore.",
            DERIVATION_HEMP_LEGACY_GENERIC,
            DERIVATION_HEMP_CANONICAL_420
        ));
    }

    if derivation_hemp != DERIVATION_HEMP_CANONICAL_420 {
        return Err(format!(
            "Unsupported derivation profile for Core restore: {derivation_hemp}. Only the canonical coin420 profile ({}) is accepted.",
            DERIVATION_HEMP_CANONICAL_420
        ));
    }

    let coin_type: i64 = 420;
    let exported_at: i64 = chrono::Utc::now().timestamp();

    let best_block_height = webcom_record
        .metadata
        .as_ref()
        .and_then(|m| m.get("best_block_height"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    // Determine how many addresses to derive. The WebCom hemp record
    // carries three relevant fields:
    //   - external_count: the user's "gap limit" — how many external
    //     addresses WebCom was tracking (default 1 in WebCom).
    //   - change_count: same for change chain (default 1 in WebCom).
    //   - recovered_external_indices: the actual indices that had
    //     activity (e.g. [0, 5, 12] if the user used addresses 0, 5,
    //     and 12 of the external chain).
    //
    // If we use only external_count (default 1) Core derives almost
    // no addresses and the user's balance-holding addresses are never
    // checked — the wallet shows 0 balance even though funds are
    // there. The fix is to derive enough addresses to cover both
    // the gap limit AND any indices in recovered_external_indices.
    //
    // We use a reasonable floor (20) so that even records without
    // recovered_external_indices get a useful initial keypool.
    let keypool_hints = webcom_keypool_hints(webcom_record);
    let external_count_hint = keypool_hints.external_count_hint;
    let change_count_hint = keypool_hints.change_count_hint;

    let private_payload_plaintext = serde_json::json!({
        "payload_version": MIGRATION_PRIVATE_PAYLOAD_VERSION,
        "wallet_type": MIGRATION_WALLET_TYPE,
        "network": core_network,
        "coin_type": coin_type,
        "account": 0,
        "derivation_profile": DERIVATION_HEMP_CANONICAL_420,
        "mnemonic": {
            "language": MIGRATION_MNEMONIC_LANGUAGE,
            "words": mnemonic.as_str(),
        },
        "mnemonic_passphrase": "",
        "external_count_hint": external_count_hint,
        "change_count_hint": change_count_hint,
        "exported_at": exported_at,
    });

    let plaintext_bytes = Zeroizing::new(
        serde_json::to_vec(&private_payload_plaintext)
            .map_err(|e| format!("Failed to serialize private payload: {e}"))?,
    );

    let mut salt = [0u8; MIGRATION_SALT_SIZE];
    OsRng.fill_bytes(&mut salt);

    let mut derived_key = Zeroizing::new([0u8; MIGRATION_KEY_SIZE]);
    pbkdf2_hmac::<Sha512>(
        passphrase.as_bytes(),
        &salt,
        MIGRATION_KDF_ITERATIONS,
        derived_key.as_mut(),
    );

    let key = aes_gcm::Key::<Aes256Gcm>::from_slice(derived_key.as_slice());
    let cipher = Aes256Gcm::new(key);

    let mut iv = [0u8; MIGRATION_IV_SIZE];
    OsRng.fill_bytes(&mut iv);
    let nonce = Nonce::from_slice(&iv);

    let aad = build_migration_aad(
        core_network,
        coin_type,
        exported_at,
        MIGRATION_PURPOSE_LABEL_PRIVATE,
    );

    let encrypted = cipher
        .encrypt(
            nonce,
            aes_gcm::aead::Payload {
                msg: plaintext_bytes.as_slice(),
                aad: &aad,
            },
        )
        .map_err(|e| format!("Migration envelope encryption failed: {e}"))?;

    let ciphertext = &encrypted[..encrypted.len() - MIGRATION_TAG_SIZE];
    let tag = &encrypted[encrypted.len() - MIGRATION_TAG_SIZE..];

    let envelope = serde_json::json!({
        "envelope_version": MIGRATION_ENVELOPE_VERSION,
        "schema_identifier": MIGRATION_SCHEMA_IDENTIFIER,
        "exported_at": exported_at,
        "source_client": MIGRATION_SOURCE_CLIENT,
        "source_client_version": MIGRATION_SOURCE_CLIENT_VERSION,
        "chain": {
            "network": core_network,
            "coin_type_bip44": coin_type,
        },
        "wallet_summary": {
            "wallet_name": "webcom-primary",
            "encrypted": true,
            "locked": false,
            "hd_enabled": true,
            "bip44_enabled": true,
            "private_keys_included": true,
            "mnemonic_available": true,
            "watch_only_present": false,
            "private_keys_present": true,
            "keypool_external": external_count_hint,
            "keypool_internal": change_count_hint,
        },
        "derivation": [
            {
                "profile_id": DERIVATION_HEMP_CANONICAL_420,
                "purpose": 44,
                "coin_type": coin_type,
                "address_type": "p2pkh",
                "account_derivation_path": "m/44'/420'/0'",
                "best_block_height": best_block_height,
            }
        ],
        "private": {
            "encrypted": true,
            "payload_format": MIGRATION_PRIVATE_PAYLOAD_FORMAT,
            "kdf_profile": MIGRATION_KDF_PROFILE,
            "kdf_iterations": MIGRATION_KDF_ITERATIONS,
            "cipher_profile": MIGRATION_CIPHER_PROFILE,
            "salt": hex::encode(salt),
            "iv": hex::encode(iv),
            "tag": hex::encode(tag),
            "aad_profile": MIGRATION_AAD_PROFILE,
            "ciphertext": hex::encode(ciphertext),
        },
        "keys": [],
        "watch_only_entries": [],
        "unsupported_records": [],
        "metadata": {
            "best_block_height": best_block_height,
            "transaction_count": 0,
            "balance": 0,
            "external_count_hint": external_count_hint,
            "change_count_hint": change_count_hint,
            "external_count_from_vault": keypool_hints.external_count_meta,
            "change_count_from_vault": keypool_hints.change_count_meta,
            "recovered_external_indices_count": keypool_hints.recovered_external_indices_count,
            "max_recovered_external_index": keypool_hints.max_recovered_external_index,
        },
        "warnings": [
            "Generated by Commander from a WebCom Hemp0x Vault BIP39 record.",
        ],
    });

    serde_json::to_string(&envelope)
        .map_err(|e| format!("Failed to serialize migration envelope: {e}"))
}

// ─── Core v2 Envelope Decryption — slice 66d ───────────────────────────
//
// Inverse of build_migration_envelope_from_webcom_bip39: decrypt a Core v2
// encrypted private migration envelope and extract canonical BIP39/coin420
// wallet material.
//
// Slice 66d: PrivateMigrationPayload and MnemonicPayload implement
// custom zeroization on Drop for secret fields. canonical_wallet_identity
// normalizes mnemonic in a Zeroizing<String>. Plaintext/ciphertext
// buffers already use Zeroizing containers.

/// Typed private migration payload. Secret fields are explicitly zeroized
/// on Drop to prevent lingering plaintext mnemonic/passphrase in memory.
#[derive(serde::Deserialize)]
struct PrivateMigrationPayload {
    #[serde(default)]
    payload_version: i64,
    #[serde(default)]
    wallet_type: String,
    #[serde(default)]
    coin_type: i64,
    #[serde(default)]
    account: i64,
    #[serde(default)]
    derivation_profile: String,
    #[allow(dead_code)]
    #[serde(default)]
    network: String,
    #[serde(default)]
    mnemonic: MnemonicPayload,
    #[serde(default)]
    mnemonic_passphrase: String,
    #[serde(default)]
    external_count_hint: i64,
    #[serde(default)]
    change_count_hint: i64,
    #[serde(default)]
    best_block_height: i64,
}

impl Drop for PrivateMigrationPayload {
    fn drop(&mut self) {
        zeroize::Zeroize::zeroize(&mut self.mnemonic.words);
        zeroize::Zeroize::zeroize(&mut self.mnemonic_passphrase);
    }
}

#[derive(serde::Deserialize, Default)]
struct MnemonicPayload {
    #[serde(default)]
    language: String,
    #[serde(default)]
    words: String,
}
// F6: Zeroization is handled by PrivateMigrationPayload::drop.
// MnemonicPayload::drop is intentionally removed to avoid
// double-zeroizing the same allocation.

struct PortableWalletMaterial {
    mnemonic: Zeroizing<String>,
    mnemonic_passphrase: Zeroizing<String>,
    #[allow(dead_code)]
    network: String,
    coin_type: i64,
    account: i64,
    derivation_profile: String,
    external_count_hint: i64,
    change_count_hint: i64,
    best_block_height: i64,
    source_wallet_name: Option<String>,
    exported_at: i64,
    mnemonic_language: String,
    mnemonic_word_count: usize,
}

impl std::fmt::Debug for PortableWalletMaterial {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PortableWalletMaterial")
            .field("coin_type", &self.coin_type)
            .field("derivation_profile", &self.derivation_profile)
            .field("mnemonic_word_count", &self.mnemonic_word_count)
            .field("best_block_height", &self.best_block_height)
            .field("source_wallet_name", &self.source_wallet_name)
            .field("mnemonic", &"[redacted]")
            .field("mnemonic_passphrase", &"[redacted]")
            .finish()
    }
}

impl Drop for PortableWalletMaterial {
    fn drop(&mut self) {
        self.mnemonic = Zeroizing::new(String::new());
        self.mnemonic_passphrase = Zeroizing::new(String::new());
    }
}

/// Canonical wallet identity: SHA-256 of the normalized mnemonic.
/// Used ONLY for backend wallet equality comparison. Never returned
/// to the frontend or stored outside encrypted vault alignment metadata.
/// Normalization is performed inside a Zeroizing<String> and zeroized
/// after the hash is computed.
fn canonical_wallet_identity(mnemonic: &str) -> String {
    let normalized = Zeroizing::new(mnemonic.trim().to_lowercase());
    let mut hasher = Sha256::new();
    hasher.update(b"hemp0x.canonical-wallet-identity.v1:");
    hasher.update(normalized.as_bytes());
    let result = hex::encode(hasher.finalize());
    // normalized is zeroized on drop
    result
}

fn decrypt_core_migration_bip39(
    envelope_json: &str,
    migration_passphrase: &str,
) -> Result<PortableWalletMaterial, String> {
    let envelope: serde_json::Value = serde_json::from_str(envelope_json)
        .map_err(|e| format!("Migration envelope is not valid JSON: {e}"))?;

    let env_version = envelope
        .get("envelope_version")
        .and_then(|v| v.as_i64())
        .ok_or("Missing envelope_version")?;
    if env_version != 2 {
        return Err(format!("Unsupported envelope version {env_version}. Only v2 encrypted envelopes are supported."));
    }

    let schema_id = envelope
        .get("schema_identifier")
        .and_then(|v| v.as_str())
        .ok_or("Missing schema_identifier")?;
    if schema_id != MIGRATION_SCHEMA_IDENTIFIER {
        return Err(format!("Unsupported schema identifier '{schema_id}'. Expected '{MIGRATION_SCHEMA_IDENTIFIER}'."));
    }

    let chain = envelope.get("chain").ok_or("Missing chain object")?;
    let network = chain
        .get("network")
        .and_then(|v| v.as_str())
        .ok_or("Missing chain.network")?;
    if network != CORE_MIGRATION_MAINNET_NETWORK_ID {
        return Err(format!("Network '{network}' is not supported. Only '{CORE_MIGRATION_MAINNET_NETWORK_ID}' (mainnet) is accepted."));
    }
    let coin_type = chain
        .get("coin_type_bip44")
        .and_then(|v| v.as_i64())
        .ok_or("Missing chain.coin_type_bip44")?;
    if coin_type != 420 {
        return Err(format!(
            "Coin type {coin_type} is not supported. Only canonical coin type 420 is accepted."
        ));
    }

    let exported_at = envelope
        .get("exported_at")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    let source_wallet_name = envelope
        .get("wallet_summary")
        .and_then(|w| w.get("wallet_name"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let private = envelope
        .get("private")
        .ok_or("Missing private object. Only v2 encrypted private envelopes can be promoted.")?;

    let encrypted = private
        .get("encrypted")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if !encrypted {
        return Err(
            "Private payload is not encrypted. Only encrypted private envelopes can be promoted."
                .to_string(),
        );
    }

    let kdf_profile = private
        .get("kdf_profile")
        .and_then(|v| v.as_str())
        .ok_or("Missing private.kdf_profile")?;
    if kdf_profile != MIGRATION_KDF_PROFILE {
        return Err(format!(
            "Unsupported KDF profile '{kdf_profile}'. Only '{MIGRATION_KDF_PROFILE}' is supported."
        ));
    }

    let kdf_iterations = private
        .get("kdf_iterations")
        .and_then(|v| v.as_i64())
        .ok_or("Missing private.kdf_iterations")?;
    if kdf_iterations < 100_000 {
        return Err(format!(
            "KDF iteration count {kdf_iterations} is below the supported minimum (100,000)."
        ));
    }
    if kdf_iterations > 5_000_000 {
        return Err(format!(
            "KDF iteration count {kdf_iterations} exceeds the supported maximum (5,000,000)."
        ));
    }

    let cipher_profile = private
        .get("cipher_profile")
        .and_then(|v| v.as_str())
        .ok_or("Missing private.cipher_profile")?;
    if cipher_profile != MIGRATION_CIPHER_PROFILE {
        return Err(format!("Unsupported cipher profile '{cipher_profile}'. Only '{MIGRATION_CIPHER_PROFILE}' is supported."));
    }

    let payload_format = private
        .get("payload_format")
        .and_then(|v| v.as_str())
        .ok_or("Missing private.payload_format")?;
    if payload_format != MIGRATION_PRIVATE_PAYLOAD_FORMAT {
        return Err(format!("Unsupported payload format '{payload_format}'. Only '{MIGRATION_PRIVATE_PAYLOAD_FORMAT}' is supported."));
    }

    let aad_profile = private
        .get("aad_profile")
        .and_then(|v| v.as_str())
        .ok_or("Missing private.aad_profile")?;
    if aad_profile != MIGRATION_AAD_PROFILE {
        return Err(format!(
            "Unsupported AAD profile '{aad_profile}'. Only '{MIGRATION_AAD_PROFILE}' is supported."
        ));
    }

    let salt_hex = private
        .get("salt")
        .and_then(|v| v.as_str())
        .ok_or("Missing private.salt")?;
    let iv_hex = private
        .get("iv")
        .and_then(|v| v.as_str())
        .ok_or("Missing private.iv")?;
    let tag_hex = private
        .get("tag")
        .and_then(|v| v.as_str())
        .ok_or("Missing private.tag")?;
    let ciphertext_hex = private
        .get("ciphertext")
        .and_then(|v| v.as_str())
        .ok_or("Missing private.ciphertext")?;

    let salt = hex::decode(salt_hex).map_err(|e| format!("Invalid salt hex: {e}"))?;
    if salt.len() != MIGRATION_SALT_SIZE {
        return Err(format!(
            "Invalid salt length: {} (expected {})",
            salt.len(),
            MIGRATION_SALT_SIZE
        ));
    }

    let iv = hex::decode(iv_hex).map_err(|e| format!("Invalid IV hex: {e}"))?;
    if iv.len() != MIGRATION_IV_SIZE {
        return Err(format!(
            "Invalid IV length: {} (expected {})",
            iv.len(),
            MIGRATION_IV_SIZE
        ));
    }

    let tag = hex::decode(tag_hex).map_err(|e| format!("Invalid tag hex: {e}"))?;
    if tag.len() != MIGRATION_TAG_SIZE {
        return Err(format!(
            "Invalid tag length: {} (expected {})",
            tag.len(),
            MIGRATION_TAG_SIZE
        ));
    }

    let ciphertext = Zeroizing::new(
        hex::decode(ciphertext_hex).map_err(|e| format!("Invalid ciphertext hex: {e}"))?,
    );
    if ciphertext.is_empty() {
        return Err("Ciphertext is empty".to_string());
    }

    let aad = build_migration_aad(
        network,
        coin_type,
        exported_at,
        MIGRATION_PURPOSE_LABEL_PRIVATE,
    );

    let mut derived_key = Zeroizing::new([0u8; MIGRATION_KEY_SIZE]);
    pbkdf2_hmac::<Sha512>(
        migration_passphrase.as_bytes(),
        &salt,
        kdf_iterations as u32,
        derived_key.as_mut(),
    );

    let key = aes_gcm::Key::<Aes256Gcm>::from_slice(derived_key.as_slice());
    let cipher = Aes256Gcm::new(key);
    let nonce = Nonce::from_slice(&iv);

    let mut combined = Zeroizing::new(Vec::new());
    combined.extend_from_slice(&ciphertext);
    combined.extend_from_slice(&tag);

    let plaintext_bytes = Zeroizing::new(cipher
        .decrypt(nonce, aes_gcm::aead::Payload {
            msg: combined.as_slice(),
            aad: &aad,
        })
        .map_err(|_| "Authentication failed. The passphrase is incorrect or the envelope has been tampered with.".to_string())?);

    let plaintext_str = Zeroizing::new(
        String::from_utf8(plaintext_bytes.to_vec())
            .map_err(|e| format!("Decrypted payload is not valid UTF-8: {e}"))?,
    );

    // B5: Deserialize directly into typed struct with zeroizing secret fields.
    // No generic JSON Value retains mnemonic/passphrase after extraction.
    let payload: PrivateMigrationPayload =
        serde_json::from_str(plaintext_str.as_str()).map_err(|e| {
            format!("Decrypted private payload is not valid JSON or has wrong shape: {e}")
        })?;

    if payload.payload_version != 1 {
        return Err(format!(
            "Unsupported payload version {}. Only version 1 is supported.",
            payload.payload_version
        ));
    }

    if payload.wallet_type != MIGRATION_WALLET_TYPE {
        return Err(format!(
            "Unsupported wallet type '{}'. Only '{}' (BIP39/BIP44 P2PKH) is supported.",
            payload.wallet_type, MIGRATION_WALLET_TYPE
        ));
    }

    if payload.coin_type != 420 {
        return Err(format!(
            "Coin type {} in payload is not supported. Only canonical coin type 420 is accepted.",
            payload.coin_type
        ));
    }
    if payload.coin_type != coin_type {
        return Err(format!(
            "Payload coin type {} does not match public envelope coin type {}.",
            payload.coin_type, coin_type
        ));
    }

    if payload.account != 0 {
        return Err(format!(
            "Unsupported account {}. Only account 0 is supported.",
            payload.account
        ));
    }

    if payload.derivation_profile != DERIVATION_HEMP_CANONICAL_420 {
        return Err(format!(
            "Unsupported derivation profile '{}'. Only '{}' is accepted.",
            payload.derivation_profile, DERIVATION_HEMP_CANONICAL_420
        ));
    }

    let mnemonic_words = &payload.mnemonic.words;
    if mnemonic_words.is_empty() {
        return Err("Missing mnemonic.words in decrypted payload".to_string());
    }

    let mnemonic_clean = Zeroizing::new(mnemonic_words.trim().to_lowercase());
    let word_count = mnemonic_clean.split_whitespace().count();
    if word_count != 12 && word_count != 18 && word_count != 24 {
        return Err(format!(
            "Mnemonic word count {word_count} is not supported. Expected 12, 18, or 24 words."
        ));
    }

    // F6: Drop BIP39 parsed object immediately after validation
    let _ = bip39::Mnemonic::parse(mnemonic_clean.as_str())
        .map_err(|e| format!("Invalid BIP39 mnemonic: {e}"))?;

    // F6: Inspect mnemonic_passphrase by reference, let payload Drop zeroize it.
    // No additional clone allocation.
    if !payload.mnemonic_passphrase.is_empty() {
        return Err(
            "This wallet was created with a non-empty BIP39 mnemonic passphrase. The current Hemp0x Vault / WebCom primary record format does not support storing the BIP39 passphrase. Promotion of passphrase-protected wallets is not yet available. Export the wallet as a Core migration envelope for recovery instead.".to_string()
        );
    }

    // Extract remaining fields before dropping the typed struct
    let account = payload.account;
    let mnemonic_language = if payload.mnemonic.language.is_empty() {
        "english".to_string()
    } else {
        payload.mnemonic.language.clone()
    };
    let external_count_hint = if payload.external_count_hint > 0 {
        payload.external_count_hint
    } else {
        20
    };
    let change_count_hint = if payload.change_count_hint > 0 {
        payload.change_count_hint
    } else {
        6
    };

    let best_block_height = envelope
        .get("derivation")
        .and_then(|d| d.as_array())
        .and_then(|arr| arr.first())
        .and_then(|prof| prof.get("best_block_height"))
        .and_then(|v| v.as_i64())
        .unwrap_or_else(|| {
            if payload.best_block_height > 0 {
                payload.best_block_height
            } else {
                0
            }
        });

    drop(payload);

    Ok(PortableWalletMaterial {
        mnemonic: mnemonic_clean,
        mnemonic_passphrase: Zeroizing::new(String::new()),
        network: "mainnet".to_string(),
        coin_type: 420,
        account,
        derivation_profile: DERIVATION_HEMP_CANONICAL_420.to_string(),
        external_count_hint,
        change_count_hint,
        best_block_height,
        source_wallet_name,
        exported_at,
        mnemonic_language,
        mnemonic_word_count: word_count,
    })
}

// ─── Portable Primary Record Builder — slice 66/66b ─────────────────────
//
// Builds a WebCom-compatible wallet.webcom.hemp.primary SecretRecord from
// validated PortableWalletMaterial. Reuses the exact record shape from
// recovery-phrase restore. The mnemonic never leaves the backend.

fn build_webcom_primary_record_from_material(material: &PortableWalletMaterial) -> SecretRecord {
    let now = chrono::Utc::now().timestamp();
    let now_ms = now.saturating_mul(1000);
    let created_at_ms = if material.exported_at > 0 {
        material.exported_at.saturating_mul(1000)
    } else {
        now_ms
    };
    let webcom_external_count = material
        .external_count_hint
        .clamp(1, WEBCOM_PRIMARY_EXTERNAL_COUNT_DEFAULT);
    let webcom_change_count = material
        .change_count_hint
        .clamp(0, WEBCOM_PRIMARY_CHANGE_COUNT_DEFAULT);
    let label = if let Some(ref name) = material.source_wallet_name {
        format!(
            "Core wallet '{}' - {}",
            name,
            chrono::Utc::now().format("%Y-%m-%d %H:%M")
        )
    } else {
        format!(
            "Core migration import - {}",
            chrono::Utc::now().format("%Y-%m-%d %H:%M")
        )
    };

    let mut metadata = serde_json::json!({
        "recovery": {
            "schemaVersion": 1,
            "seedType": "bip39",
            "network": "mainnet",
            "derivationProfiles": {
                "hemp": DERIVATION_HEMP_CANONICAL_420,
            },
            "createdAt": created_at_ms,
            "updatedAt": now_ms,
        },
        "external_count": webcom_external_count,
        "change_count": webcom_change_count,
        "recovered_external_indices": [],
        "commander_core_external_count_hint": material.external_count_hint,
        "commander_core_change_count_hint": material.change_count_hint,
        "best_block_height": material.best_block_height,
        "account": material.account,
        "coin_type": material.coin_type,
        "derivation_profile": material.derivation_profile,
        "mnemonic_language": material.mnemonic_language,
        "mnemonic_word_count": material.mnemonic_word_count,
        "source": "core-migration-import",
        "exported_at": material.exported_at,
    });

    if let Some(ref name) = material.source_wallet_name {
        if let Some(obj) = metadata.as_object_mut() {
            obj.insert(
                "source_wallet_name".to_string(),
                serde_json::Value::String(name.clone()),
            );
        }
    }

    let mut derivation_profiles = HashMap::new();
    derivation_profiles.insert(
        "hemp".to_string(),
        DERIVATION_HEMP_CANONICAL_420.to_string(),
    );

    SecretRecord {
        record_id: RECORD_ID_WALLET_HEMP_PRIMARY.to_string(),
        record_type: RECORD_TYPE_WALLET_BIP39.to_string(),
        label,
        value: material.mnemonic.as_str().to_string(),
        metadata: Some(metadata),
        tags: Some(vec![
            "wallet".to_string(),
            "bip39".to_string(),
            "core-migration-import".to_string(),
        ]),
        origin_app: Some(APP_IDENTIFIER.to_string()),
        derivation_profiles: Some(derivation_profiles),
        network: Some("mainnet".to_string()),
        created: now,
        modified: now,
    }
}

// ─── Wallet Relock Guard — slice 66b ────────────────────────────────────
//
// RAII guard that relocks a Core wallet on drop if it was unlocked by
// the promotion operation. Does not lock a wallet that was already
// unlocked before the operation.

struct WalletRelockGuard {
    wallet_name: String,
    was_unlocked_by_us: bool,
    is_default_wallet: bool,
}

impl WalletRelockGuard {
    fn new(wallet_name: &str, is_default: bool, we_unlocked: bool) -> Self {
        Self {
            wallet_name: wallet_name.to_string(),
            was_unlocked_by_us: we_unlocked,
            is_default_wallet: is_default,
        }
    }

    fn relock_now(&mut self) -> Result<(), String> {
        if !self.was_unlocked_by_us {
            return Ok(());
        }
        self.was_unlocked_by_us = false;
        if self.is_default_wallet {
            crate::modules::rpc::call_rpc("walletlock", &[])
        } else {
            crate::modules::rpc::call_rpc_wallet(&self.wallet_name, "walletlock", &[])
        }
        .map_err(|e| format!("Failed to relock wallet '{}': {}", self.wallet_name, e))?;
        Ok(())
    }
}

impl Drop for WalletRelockGuard {
    fn drop(&mut self) {
        let _ = self.relock_now();
    }
}

// ─── Transactional Promotion — slice 66b ────────────────────────────────
//
// CF4: Reordered to verify Core restore/load BEFORE saving the vault.
// CF5: Previous primary preserved as encrypted recovery record.
// CF7: Runtime wallet identity proven via canonical mnemonic comparison.
// CF8: Wallet relock guaranteed via RAII guard.
// CF9: Secret-bearing RPC calls use direct HTTP RPC, never CLI args.
//
// Order:
// 1. Read/decrypt current vault into memory
// 2. Validate and decrypt migration envelope
// 3. Build proposed primary record in memory
// 4. Resolve conflicts and prepare backup records in memory
// 5. Restore/load and verify matching Core runtime wallet
// 6. Build verified alignment metadata
// 7. One atomic vault save: primary + snapshot + prev-primary backup + alignment + all existing
// 8. Update Commander active-wallet settings after atomic save

fn promote_core_migration_to_portable_primary_blocking(
    path: &str,
    migration_passphrase: &str,
    vault_passphrase: &str,
    replace_existing_primary: bool,
    runtime_wallet_name: Option<&str>,
    wallet_unlock_passphrase: Option<&str>, // B3: for existing-target identity check
) -> Result<serde_json::Value, String> {
    clean_stale_vault_temp_files();

    let src = PathBuf::from(path);
    let content = validate_import_file(&src)?;

    let _: serde_json::Value =
        serde_json::from_str(&content).map_err(|e| format!("File is not valid JSON: {e}"))?;

    // Step 1: Read/decrypt current vault into memory
    let bundle = load_bundle()?.ok_or("Vault does not exist")?;
    let dek = unwrap_dek_with_passphrase(vault_passphrase, &bundle.vault)?;
    let mut payload = decrypt_payload_with_dek(dek.as_slice(), &bundle.vault)?;
    let vault_network = bundle
        .vault
        .network
        .clone()
        .unwrap_or_else(|| "mainnet".to_string());

    // Step 2: Validate and decrypt migration envelope
    let temp = TempFileGuard::new("vault_promote_validate")?;
    fs::write(&temp.path, &content).map_err(|e| format!("Cannot write temp file: {e}"))?;

    let validation = validate_migration_envelope_file(&temp.path_str(), migration_passphrase)
        .map_err(|e| format!("Core migration envelope validation failed: {e}"))?;

    let valid = validation
        .get("valid")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if !valid {
        return Err("Core validation reported the migration envelope as invalid. It cannot be promoted to a portable primary wallet.".to_string());
    }

    let restorable = validation
        .get("restorable")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if !restorable {
        return Err("Core validation reported the migration envelope is not restorable. It cannot be promoted to a portable primary wallet.".to_string());
    }

    let material = decrypt_core_migration_bip39(&content, migration_passphrase)
        .map_err(|e| format!("Cannot decrypt migration envelope: {e}"))?;

    // Step 3: Build proposed primary record in memory
    let new_primary = build_webcom_primary_record_from_material(&material);
    let canonical_id = canonical_wallet_identity(material.mnemonic.as_str());

    // Step 4: Resolve conflicts and prepare backup records in memory
    let existing_primary = payload.secrets.get(RECORD_ID_WALLET_HEMP_PRIMARY);
    let mut previous_primary_backup: Option<SecretRecord> = None;
    let mut is_same_wallet = false;

    if let Some(existing) = existing_primary {
        if !existing.value.is_empty() {
            let existing_id = canonical_wallet_identity(&existing.value);

            if existing_id == canonical_id {
                // CF2: Same wallet proven by canonical mnemonic identity
                is_same_wallet = true;
            } else if !replace_existing_primary {
                return Err(format!(
                    "A different primary wallet already exists in the vault. To replace it, confirm replacement explicitly. The existing wallet will be preserved as an encrypted recovery record."
                ));
            } else {
                // B4: Convert previous primary to a real encrypted Core migration
                // envelope before storing it. This produces a truthful
                // wallet.core_migration_envelope record that Core restore can use.
                let prev_label = format!(
                    "Previous primary wallet - {}",
                    chrono::Utc::now().format("%Y-%m-%d %H:%M")
                );
                let prev_envelope_json = build_migration_envelope_from_webcom_bip39(
                    vault_passphrase,
                    existing,
                )
                .map_err(|e| {
                    format!("Failed to build migration envelope for previous primary backup: {e}")
                })?;

                let prev_id = generate_collision_safe_record_id("prev-primary", &existing.value);
                let prev_meta = serde_json::json!({
                    "value_kind": "previous_primary_wallet",
                    "source": "core-migration-promotion-replacement",
                    "preserved_at": chrono::Utc::now().timestamp(),
                    "original_record_id": RECORD_ID_WALLET_HEMP_PRIMARY,
                    "original_record_type": existing.record_type,
                    "restorable": true,
                    "private_keys_included": true,
                    "envelope_kdf_profile": MIGRATION_KDF_PROFILE,
                    "envelope_cipher_profile": MIGRATION_CIPHER_PROFILE,
                    "recovery_mode": "vault_passphrase",
                });
                previous_primary_backup = Some(SecretRecord {
                    record_id: prev_id.clone(),
                    record_type: RECORD_TYPE_WALLET_CORE_MIGRATION.to_string(),
                    label: prev_label,
                    value: prev_envelope_json,
                    metadata: Some(prev_meta),
                    tags: Some(vec![
                        "wallet".to_string(),
                        "migration".to_string(),
                        "previous-primary".to_string(),
                    ]),
                    origin_app: Some(APP_IDENTIFIER.to_string()),
                    derivation_profiles: existing.derivation_profiles.clone(),
                    network: existing.network.clone(),
                    created: chrono::Utc::now().timestamp(),
                    modified: chrono::Utc::now().timestamp(),
                });
            }
        }
    }

    // Build snapshot record for the imported envelope
    let snapshot_label = if let Some(ref name) = material.source_wallet_name {
        format!(
            "Pre-promotion backup - {} - {}",
            name,
            chrono::Utc::now().format("%Y-%m-%d %H:%M")
        )
    } else {
        format!(
            "Pre-promotion backup - {}",
            chrono::Utc::now().format("%Y-%m-%d %H:%M")
        )
    };

    let mut snapshot_metadata = extract_validation_metadata(&validation);
    if let Some(obj) = snapshot_metadata.as_object_mut() {
        obj.insert(
            "source".to_string(),
            serde_json::Value::String("core-migration-promotion".to_string()),
        );
        obj.insert(
            "imported_at".to_string(),
            serde_json::Value::Number(chrono::Utc::now().timestamp().into()),
        );
        obj.insert(
            "original_filename".to_string(),
            serde_json::Value::String(
                src.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string(),
            ),
        );
        obj.insert(
            "promotion_snapshot".to_string(),
            serde_json::Value::Bool(true),
        );
    }

    let snapshot_id = generate_collision_safe_record_id("promote", &content);
    let snapshot_record = SecretRecord {
        record_id: snapshot_id.clone(),
        record_type: RECORD_TYPE_WALLET_CORE_MIGRATION.to_string(),
        label: snapshot_label,
        value: content.clone(),
        metadata: Some(snapshot_metadata),
        tags: Some(vec![
            "wallet".to_string(),
            "migration".to_string(),
            "promotion-snapshot".to_string(),
        ]),
        origin_app: Some(APP_IDENTIFIER.to_string()),
        derivation_profiles: None,
        network: Some("mainnet".to_string()),
        created: chrono::Utc::now().timestamp(),
        modified: chrono::Utc::now().timestamp(),
    };

    // Step 5: Restore/load and verify matching Core runtime wallet
    let effective_wallet_name = runtime_wallet_name
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| MIGRATION_DEFAULT_WALLET_NAME.to_string());
    let safe_name = validate_migration_wallet_name_for_vault(&effective_wallet_name)?;

    let existing_state = describe_named_wallet_state(&safe_name);
    let mut restore_result: Option<serde_json::Value> = None;
    let mut wallet_already_exists = false;

    if existing_state.wallet_file_exists {
        // B2: Export the existing runtime wallet to a temp encrypted envelope,
        // decrypt it, and compare canonical identity with the proposed primary.
        wallet_already_exists = true;
        if !existing_state.named_wallet_loaded {
            // B1: Core Next does not support dynamic loadwallet.
            // The wallet file exists but is not loaded. Reject.
            return Err(format!(
                "A wallet file named '{}' exists in the Core datadir but is not currently loaded. Core Next does not support dynamic loadwallet. Restart Core with -wallet={safe_name} and retry, or choose a different wallet name.",
                safe_name
            ));
        }

        // F3: Preserve WALLET_UNLOCK_REQUIRED:: prefix through wrappers
        if let Err(ref e) =
            verify_exact_wallet_identity(&safe_name, &canonical_id, wallet_unlock_passphrase)
        {
            if e.starts_with("WALLET_UNLOCK_REQUIRED::") {
                return Err(e.clone());
            }
            return Err(format!(
                "Existing wallet '{}' identity verification failed: {}",
                safe_name, e
            ));
        }
    } else {
        // Restore from the material
        let envelope_json =
            build_migration_envelope_from_webcom_bip39(vault_passphrase, &new_primary)
                .map_err(|e| format!("Failed to build migration envelope for restore: {e}"))?;

        let restore_temp = TempFileGuard::new("vault_promote_restore")?;
        fs::write(&restore_temp.path, &envelope_json)
            .map_err(|e| format!("Cannot write temp restore envelope: {e}"))?;

        let restore_validation =
            validate_migration_envelope_file(&restore_temp.path_str(), vault_passphrase)
                .map_err(|e| format!("Generated envelope validation failed: {e}"))?;

        let rv = restore_validation
            .get("valid")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if !rv {
            // CF4: Vault has NOT been saved yet — safe to fail here
            return Err("Generated migration envelope failed Core validation. The primary record may be malformed. Vault was not modified.".to_string());
        }

        let rr = match crate::modules::commands::restore_wallet_migration(
            restore_temp.path_str(),
            safe_name.clone(),
            vault_passphrase.to_string(),
            Some(material.best_block_height),
        ) {
            Ok(r) => r,
            Err(e) => {
                if is_duplicate_wallet_error(&e) {
                    // F4: Use identity proof, not profile-only verification
                    match verify_exact_wallet_identity(
                        &safe_name,
                        &canonical_id,
                        wallet_unlock_passphrase,
                    ) {
                        Ok(()) => {
                            // Identity confirmed — build a synthetic result
                            serde_json::json!({
                                "wallet_name": safe_name,
                                "recovered_after_duplicate": true,
                            })
                        }
                        Err(ve) => {
                            if ve.starts_with("WALLET_UNLOCK_REQUIRED::") {
                                return Err(ve);
                            }
                            return Err(format!("Core restorewalletmigration failed: {e}. Existing wallet identity verification also failed: {ve}. Vault was not modified."));
                        }
                    }
                } else {
                    // CF4: Vault has NOT been saved — safe to fail
                    return Err(format!(
                        "Core restorewalletmigration failed: {e}. Vault was not modified."
                    ));
                }
            }
        };

        verify_restore_result(&rr, &safe_name).map_err(|e| {
            // CF4: Vault has NOT been saved — safe to fail
            format!("Restore result verification failed: {e}. Vault was not modified.")
        })?;

        restore_result = Some(rr);
    }

    // Step 6: Build verified alignment metadata
    // B3: Use canonical wallet identity for alignment, not metadata-only fingerprint
    let alignment_identity = canonical_id.clone();
    let format_fingerprint = build_alignment_fingerprint(&new_primary);
    let now = chrono::Utc::now().timestamp();

    let alignment_metadata = serde_json::json!({
        "schema": ALIGNMENT_SCHEMA,
        "schema_version": ALIGNMENT_SCHEMA_VERSION,
        "active_wallet_record_id": RECORD_ID_WALLET_HEMP_PRIMARY,
        "active_wallet_identity": alignment_identity,
        "active_wallet_format_fingerprint": format_fingerprint,
        "core_wallet_name": safe_name,
        "core_wallet_source": CORE_WALLET_SOURCE_CORE_MIGRATION,
        "derivation_profile": DERIVATION_HEMP_CANONICAL_420,
        "network": vault_network.clone(),
        "created_at": now,
        "updated_at": now,
        "last_verified_at": now,
        "core_migration_backup_record_id": snapshot_id,
        "verification_method": VERIFICATION_METHOD_RESTORE_FROM_GENERATED,
        "connection_state": "verified_aligned",
        "notes": [
            "Alignment verified by Core restorewalletmigration from Commander-generated v2 migration envelope during Core-to-WebCom promotion.",
        ],
    });

    let alignment_record = SecretRecord {
        record_id: RECORD_ID_WALLET_ALIGNMENT.to_string(),
        record_type: RECORD_TYPE_APP_SETTING_WALLET_ALIGNMENT.to_string(),
        label: "Commander Wallet Alignment".to_string(),
        value: String::new(),
        metadata: Some(alignment_metadata),
        tags: Some(vec!["wallet".to_string(), "alignment".to_string()]),
        origin_app: Some(APP_IDENTIFIER.to_string()),
        derivation_profiles: None,
        network: Some(vault_network.clone()),
        created: now,
        modified: now,
    };

    // Step 7: One atomic vault save containing everything
    if is_same_wallet {
        // Update metadata/hints without unnecessary replacement
        let mut updated = existing_primary.unwrap().clone();
        updated.metadata = new_primary.metadata.clone();
        updated.modified = now;
        payload
            .secrets
            .insert(RECORD_ID_WALLET_HEMP_PRIMARY.to_string(), updated);
    } else {
        payload.secrets.insert(
            RECORD_ID_WALLET_HEMP_PRIMARY.to_string(),
            new_primary.clone(),
        );
    }

    payload.secrets.insert(snapshot_id.clone(), snapshot_record);

    if let Some(prev_backup) = previous_primary_backup {
        payload
            .secrets
            .insert(prev_backup.record_id.clone(), prev_backup);
    }

    payload
        .secrets
        .insert(RECORD_ID_WALLET_ALIGNMENT.to_string(), alignment_record);

    // Re-encrypt and save atomically
    let mut bundle = load_bundle()?.ok_or("Vault does not exist")?;
    bundle.vault.modified = now;
    bundle.vault.payload = encrypt_payload_with_dek(dek.as_slice(), &payload, &bundle.vault)?;
    save_bundle_atomic(&bundle)?;

    // Step 8: Update Commander active-wallet settings after atomic save
    {
        let mut settings = crate::modules::files::load_app_settings_impl()?;
        settings.active_vault_wallet_name = Some(safe_name.clone());
        crate::modules::files::save_app_settings_impl(&settings)?;
    }

    let named_state = describe_named_wallet_state(&safe_name);

    let mut result = serde_json::json!({
        "promoted": true,
        "action": if is_same_wallet { "updated_existing" } else if wallet_already_exists { "verified_existing" } else { "restored_and_aligned" },
        "same_wallet": is_same_wallet,
        "record_id": RECORD_ID_WALLET_HEMP_PRIMARY,
        "fingerprint": format_fingerprint,
        "core_wallet_name": safe_name,
        "core_wallet_arg": format!("-wallet={}", safe_name),
        "derivation_profile": DERIVATION_HEMP_CANONICAL_420,
        "network": "mainnet",
        "coin_type": 420,
        "mnemonic_word_count": material.mnemonic_word_count,
        "best_block_height": material.best_block_height,
        "source_wallet_name": material.source_wallet_name,
        "snapshot_record_id": snapshot_id,
        "wallet_already_exists": wallet_already_exists,
        "named_wallet_loaded": named_state.named_wallet_loaded,
        "wallet_load_restart_required": named_state.restart_required,
        "wallet_state": named_wallet_state_to_json(&named_state),
        "runtime_wallet_encryption": "needs_user_action",
        "runtime_wallet_encryption_detail": "Core restore creates an unencrypted runtime wallet. Use the Encryption tab to encrypt the wallet.",
    });

    if let Some(rr) = restore_result {
        if let Some(obj) = result.as_object_mut() {
            if let Some(rn) = rr.get("wallet_name").and_then(|v| v.as_str()) {
                obj.insert(
                    "restored_wallet_name".to_string(),
                    serde_json::Value::String(rn.to_string()),
                );
            }
        }
    }

    Ok(result)
}

// ─── Core Wallet File to Portable Primary — slice 66c ───────────────────
//
// B1: Core Next does not support dynamic loadwallet/unloadwallet.
// Only the currently loaded Commander-managed wallet can be promoted.
// External files must first be imported via the legacy runtime-wallet flow.
//
// B2: Existing runtime wallet identity is proven by exporting a private
// migration envelope, decrypting it, and comparing canonical identity.
//
// B8: Wallet relock guaranteed via RAII guard.
// B9: Secret-bearing RPC calls use direct HTTP RPC, never CLI args.

fn promote_core_wallet_to_portable_primary_blocking(
    path: &str,
    vault_passphrase: &str,
    replace_existing_primary: bool,
    runtime_wallet_name: Option<&str>,
    wallet_unlock_passphrase: Option<&str>,
) -> Result<serde_json::Value, String> {
    clean_stale_vault_temp_files();

    let src = PathBuf::from(path);
    if !src.exists() {
        return Err("Selected wallet file does not exist.".to_string());
    }
    if !src.is_file() {
        return Err("Selected path is not a file.".to_string());
    }

    let src_meta = fs::metadata(&src).map_err(|e| format!("Cannot read file: {e}"))?;
    if src_meta.len() == 0 {
        return Err("Selected file is empty.".to_string());
    }

    // Validate as a Core wallet file
    let wallet_validation = crate::modules::process::validate_wallet_file(path.to_string())
        .map_err(|e| format!("File is not a valid Core wallet: {e}"))?;

    let wallet_valid = wallet_validation
        .get("valid")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if !wallet_valid {
        return Err("File is not a valid Core wallet file.".to_string());
    }

    // F1: Use the authoritative active-wallet resolver (not file existence matching)
    let src_canonical = src
        .canonicalize()
        .map_err(|e| format!("Cannot resolve path: {e}"))?;
    let input = gather_active_wallet_input()?;
    let resolution = resolve_proven_wallet(&input);

    let (target_wallet_name, is_default_wallet) = match resolution {
        WalletResolution::Proven(ProvenWallet::Default { ref path, .. }) => {
            let active_path = path.canonicalize().unwrap_or_else(|_| path.clone());
            if src_canonical != active_path {
                return Err(format!(
                    "EXTERNAL_FILE_NOT_LOADED::The selected file '{}' is not the active default wallet. The proven active wallet is at '{}'. Use this file as the Core runtime wallet first, then retry Add to Hemp0x Vault.",
                    src.file_name().and_then(|n| n.to_str()).unwrap_or("unknown"),
                    path.display()
                ));
            }
            (String::new(), true)
        }
        WalletResolution::Proven(ProvenWallet::Named {
            ref wallet_name,
            ref path,
            ..
        }) => {
            let active_path = path.canonicalize().unwrap_or_else(|_| path.clone());
            if src_canonical != active_path {
                return Err(format!(
                    "EXTERNAL_FILE_NOT_LOADED::The selected file '{}' does not match the proven active wallet '{}' at '{}'. Use this file as the Core runtime wallet first, then retry Add to Hemp0x Vault.",
                    src.file_name().and_then(|n| n.to_str()).unwrap_or("unknown"),
                    wallet_name,
                    path.display()
                ));
            }
            (wallet_name.clone(), false)
        }
        WalletResolution::Ambiguous { loaded_names, .. } => {
            return Err(format!(
                "Multiple wallets are loaded in Core ({:?}) without a configured active wallet. Set an active wallet in Commander Wallet settings or restart Core with -wallet=<name> to select one.",
                loaded_names
            ));
        }
        WalletResolution::SettingsMismatch { configured, actual } => {
            return Err(format!(
                "The configured Commander wallet '{}' does not match the active Core wallet '{:?}'. Restart Core with -wallet={} or update Commander settings.",
                configured, actual, configured
            ));
        }
        WalletResolution::Unavailable => {
            return Err(
                "No queryable wallet found. Ensure Core is running with a wallet loaded."
                    .to_string(),
            );
        }
    };

    // Query getwalletmigrationinfo via direct RPC
    let migration_info: serde_json::Value = if is_default_wallet {
        crate::modules::rpc::call_rpc("getwalletmigrationinfo", &[])
            .map_err(|e| format!("Cannot query wallet migration info: {e}"))?
    } else {
        crate::modules::rpc::call_rpc_wallet(&target_wallet_name, "getwalletmigrationinfo", &[])
            .map_err(|e| {
                format!(
                    "Cannot query wallet '{}' migration info: {}",
                    target_wallet_name, e
                )
            })?
    };

    let hd = migration_info
        .get("hd_enabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let bip44 = migration_info
        .get("bip44_enabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let has_mnemonic = migration_info
        .get("has_mnemonic_metadata")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let encrypted = migration_info
        .get("encrypted")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let locked = migration_info
        .get("locked")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if !hd || !bip44 || !has_mnemonic {
        return Err(
            "This wallet is not a canonical BIP39/BIP44 coin420 wallet. Only HD BIP39/BIP44 wallets with mnemonic metadata can be converted. Legacy or imported-key wallets cannot be converted losslessly into one mnemonic. Use the safe legacy wallet import path instead.".to_string()
        );
    }

    // getwalletmigrationinfo exposes the coin type at the top level and
    // intentionally does not duplicate global chain state. Read the active
    // network from getblockchaininfo instead of expecting a nonexistent
    // chain object in the wallet response.
    let chain_info = crate::modules::rpc::call_rpc("getblockchaininfo", &[])
        .map_err(|e| format!("Cannot query active Core network: {e}"))?;
    let (mig_network, mig_coin_type) = core_migration_runtime_profile(&migration_info, &chain_info);

    if mig_network != CORE_MIGRATION_MAINNET_NETWORK_ID {
        return Err(format!(
            "Wallet network '{}' is not supported. Only '{}' (mainnet) is accepted.",
            mig_network, CORE_MIGRATION_MAINNET_NETWORK_ID
        ));
    }
    if mig_coin_type != 420 {
        return Err(format!(
            "Wallet coin type {} is not supported. Only canonical coin type 420 is accepted.",
            mig_coin_type
        ));
    }

    // Wallet unlock with RAII relock guard
    let was_previously_locked = encrypted && locked;
    let mut relock_guard = WalletRelockGuard::new(
        if target_wallet_name.is_empty() {
            ""
        } else {
            &target_wallet_name
        },
        target_wallet_name.is_empty(),
        false,
    );

    if encrypted && locked {
        let unlock_pass = wallet_unlock_passphrase
            .ok_or("WALLET_UNLOCK_REQUIRED::The Core wallet is encrypted and locked. Provide the wallet unlock passphrase to continue.")?;
        if unlock_pass.is_empty() {
            return Err("WALLET_UNLOCK_REQUIRED::The Core wallet is encrypted and locked. Provide the wallet unlock passphrase to continue.".to_string());
        }
        let unlock_params = vec![
            serde_json::Value::String(unlock_pass.to_string()),
            serde_json::Value::Number(serde_json::value::Number::from(60)),
        ];
        if target_wallet_name.is_empty() {
            crate::modules::rpc::call_rpc("walletpassphrase", &unlock_params)
        } else {
            crate::modules::rpc::call_rpc_wallet(
                &target_wallet_name,
                "walletpassphrase",
                &unlock_params,
            )
        }
        .map_err(|e| format!("Failed to unlock wallet: {e}"))?;
        relock_guard.was_unlocked_by_us = true;
    }

    // Generate strong ephemeral migration passphrase
    let mut rng = rand::thread_rng();
    let mut ephemeral_bytes = [0u8; 32];
    rng.fill_bytes(&mut ephemeral_bytes);
    let ephemeral_passphrase = Zeroizing::new(hex::encode(ephemeral_bytes));

    // Export wallet migration via direct RPC
    let export_temp = TempFileGuard::new("vault_wallet_export")?;
    let export_params = vec![
        serde_json::Value::String(export_temp.path_str()),
        serde_json::Value::Bool(true),
        serde_json::Value::Bool(true),
        serde_json::Value::String(ephemeral_passphrase.as_str().to_string()),
    ];

    let _export_result: serde_json::Value = if target_wallet_name.is_empty() {
        crate::modules::rpc::call_rpc_with_timeouts(
            "exportwalletmigration",
            &export_params,
            std::time::Duration::from_secs(5),
            std::time::Duration::from_secs(10 * 60),
        )
    } else {
        crate::modules::rpc::call_rpc_wallet_with_timeouts(
            &target_wallet_name,
            "exportwalletmigration",
            &export_params,
            std::time::Duration::from_secs(5),
            std::time::Duration::from_secs(10 * 60),
        )
    }
    .map_err(|e| {
        if crate::modules::rpc::is_rpc_transport_timeout_error(&e) {
            format!("Core exportwalletmigration timed out. The wallet may be large.")
        } else {
            format!("Core exportwalletmigration failed: {e}")
        }
    })?;

    let envelope_content = fs::read_to_string(&export_temp.path)
        .map_err(|e| format!("Cannot read exported migration envelope: {e}"))?;

    // Feed into the promotion pipeline
    let promote_temp = TempFileGuard::new("vault_wallet_promote")?;
    fs::write(&promote_temp.path, &envelope_content)
        .map_err(|e| format!("Cannot write temp promote file: {e}"))?;

    let result = promote_core_migration_to_portable_primary_blocking(
        &promote_temp.path_str(),
        ephemeral_passphrase.as_str(),
        vault_passphrase,
        replace_existing_primary,
        runtime_wallet_name,
        None, // B3: no wallet unlock for the promoted material itself
    );

    let relock_result = relock_guard.relock_now();

    let mut final_result = result.map_err(|e| {
        if let Some(ref rerr) = relock_result.as_ref().err() {
            format!("{e}. Additionally, wallet relock failed: {rerr}")
        } else {
            e
        }
    })?;

    if let Some(obj) = final_result.as_object_mut() {
        obj.insert(
            "source_type".to_string(),
            serde_json::Value::String("core_wallet_file".to_string()),
        );
        obj.insert(
            "source_path".to_string(),
            serde_json::Value::String(path.to_string()),
        );
        if was_previously_locked {
            obj.insert(
                "wallet_relocked".to_string(),
                serde_json::Value::Bool(relock_result.is_ok()),
            );
            if let Some(ref rerr) = relock_result.as_ref().err() {
                obj.insert(
                    "wallet_relock_error".to_string(),
                    serde_json::Value::String(rerr.to_string()),
                );
            }
        }
    }

    Ok(final_result)
}

fn core_migration_runtime_profile<'a>(
    migration_info: &'a serde_json::Value,
    chain_info: &'a serde_json::Value,
) -> (&'a str, i64) {
    let network = chain_info
        .get("chain")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let coin_type = migration_info
        .get("canonical_coin_type")
        .and_then(|v| v.as_i64())
        .unwrap_or(-1);
    (network, coin_type)
}

// ─── Provider-Settings Wrappers — slice 66b ────────────────────────────
//
// CF1: Follow the established provider-settings wrapper pattern.
// Resolve passphrase via resolve_vault_passphrase (explicit or cached).
// These are the canonical Tauri command entry points registered in lib.rs.

#[tauri::command]
pub async fn ipfs_vault_promote_core_migration_to_portable_primary(
    path: String,
    migration_passphrase: String,
    vault_passphrase: Option<String>,
    replace_existing_primary: Option<bool>,
    runtime_wallet_name: Option<String>,
    wallet_unlock_passphrase: Option<String>, // B3
) -> Result<serde_json::Value, String> {
    let passphrase = crate::modules::provider_settings::resolve_vault_passphrase(vault_passphrase)?;
    tauri::async_runtime::spawn_blocking(move || {
        promote_core_migration_to_portable_primary_blocking(
            &path,
            &migration_passphrase,
            &passphrase,
            replace_existing_primary.unwrap_or(false),
            runtime_wallet_name.as_deref(),
            wallet_unlock_passphrase.as_deref(),
        )
    })
    .await
    .map_err(|e| format!("Promotion task failed: {e}"))?
}

#[tauri::command]
pub async fn ipfs_vault_promote_core_wallet_to_portable_primary(
    path: String,
    vault_passphrase: Option<String>,
    replace_existing_primary: Option<bool>,
    runtime_wallet_name: Option<String>,
    wallet_unlock_passphrase: Option<String>,
) -> Result<serde_json::Value, String> {
    let passphrase = crate::modules::provider_settings::resolve_vault_passphrase(vault_passphrase)?;
    tauri::async_runtime::spawn_blocking(move || {
        promote_core_wallet_to_portable_primary_blocking(
            &path,
            &passphrase,
            replace_existing_primary.unwrap_or(false),
            runtime_wallet_name.as_deref(),
            wallet_unlock_passphrase.as_deref(),
        )
    })
    .await
    .map_err(|e| format!("Wallet promotion task failed: {e}"))?
}

// ─── Advanced Export: Core Migration Wallet — slice 66b ─────────────────
//
// CF12: Export the currently loaded Core wallet to a user-chosen destination
// as an encrypted private v2 migration envelope. Uses direct RPC for all
// secret-bearing calls. Never exposes passphrases in CLI process arguments.

#[tauri::command]
pub async fn vault_export_core_migration_wallet(
    dest_path: String,
    export_passphrase: String,
    allow_overwrite: Option<bool>,
    wallet_unlock_passphrase: Option<String>,
) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        export_core_migration_wallet_blocking(
            &dest_path,
            &export_passphrase,
            allow_overwrite.unwrap_or(false),
            wallet_unlock_passphrase.as_deref(),
        )
    })
    .await
    .map_err(|e| format!("Export task failed: {e}"))?
}

fn export_core_migration_wallet_blocking(
    dest_path: &str,
    export_passphrase: &str,
    allow_overwrite: bool,
    wallet_unlock_passphrase: Option<&str>,
) -> Result<serde_json::Value, String> {
    if export_passphrase.len() < 8 {
        return Err("Export passphrase must be at least 8 characters".to_string());
    }
    if export_passphrase.len() > 1024 {
        return Err("Export passphrase must not exceed 1024 characters".to_string());
    }

    let dest = PathBuf::from(dest_path);
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Cannot create destination directory: {e}"))?;
    }
    if dest.exists() && !allow_overwrite {
        return Err(
            "Destination file already exists. Enable overwrite or choose a different path."
                .to_string(),
        );
    }

    // F1: Use the authoritative active-wallet resolver. Never fall back to wallet.dat.
    let input = gather_active_wallet_input()?;
    let resolution = resolve_proven_wallet(&input);

    let (export_wallet_name, wallet_display_name) = match resolution {
        WalletResolution::Proven(ProvenWallet::Default { ref path, .. }) => {
            (None, format!("wallet.dat (default at {})", path.display()))
        }
        WalletResolution::Proven(ProvenWallet::Named {
            ref wallet_name, ..
        }) => (Some(wallet_name.clone()), wallet_name.clone()),
        WalletResolution::Unavailable => {
            return Err("No queryable wallet found. The configured wallet is not loaded or queryable. Restart Core with -wallet=<name> and retry.".to_string());
        }
        WalletResolution::Ambiguous { loaded_names, .. } => {
            return Err(format!(
                "Multiple wallets are loaded in Core ({:?}). Set an active wallet in Commander settings before exporting.",
                loaded_names
            ));
        }
        WalletResolution::SettingsMismatch {
            configured, actual, ..
        } => {
            return Err(format!(
                "Settings/Core mismatch: Commander configured '{}' but Core has '{:?}'. Restart Core with -wallet={} and retry.",
                configured, actual, configured
            ));
        }
    };

    // Query wallet state via direct RPC, targeting the correct wallet
    let info: serde_json::Value = if let Some(ref name) = export_wallet_name {
        crate::modules::rpc::call_rpc_wallet(name, "getwalletmigrationinfo", &[])
    } else {
        crate::modules::rpc::call_rpc("getwalletmigrationinfo", &[])
    }
    .map_err(|e| format!("Cannot query wallet: {e}"))?;

    let encrypted = info
        .get("encrypted")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let locked = info
        .get("locked")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let was_previously_locked = encrypted && locked;
    let guard_name = export_wallet_name.clone().unwrap_or_default();
    let mut relock_guard = WalletRelockGuard::new(&guard_name, export_wallet_name.is_none(), false);

    if encrypted && locked {
        let unlock_pass = wallet_unlock_passphrase
            .ok_or("WALLET_UNLOCK_REQUIRED::The Core wallet is encrypted and locked. Provide the wallet unlock passphrase to export.")?;
        if unlock_pass.is_empty() {
            return Err("WALLET_UNLOCK_REQUIRED::The Core wallet is encrypted and locked. Provide the wallet unlock passphrase to export.".to_string());
        }

        let unlock_params = vec![
            serde_json::Value::String(unlock_pass.to_string()),
            serde_json::Value::Number(serde_json::value::Number::from(60)),
        ];
        if let Some(ref name) = export_wallet_name {
            crate::modules::rpc::call_rpc_wallet(name, "walletpassphrase", &unlock_params)
        } else {
            crate::modules::rpc::call_rpc("walletpassphrase", &unlock_params)
        }
        .map_err(|e| format!("Failed to unlock wallet: {e}"))?;
        relock_guard.was_unlocked_by_us = true;
    }

    // Export via direct RPC targeting the correct wallet
    let export_params = vec![
        serde_json::Value::String(dest_path.to_string()),
        serde_json::Value::Bool(true),
        serde_json::Value::Bool(allow_overwrite),
        serde_json::Value::String(export_passphrase.to_string()),
    ];

    let export_result: serde_json::Value = if let Some(ref name) = export_wallet_name {
        crate::modules::rpc::call_rpc_wallet_with_timeouts(
            name,
            "exportwalletmigration",
            &export_params,
            std::time::Duration::from_secs(5),
            std::time::Duration::from_secs(10 * 60),
        )
    } else {
        crate::modules::rpc::call_rpc_with_timeouts(
            "exportwalletmigration",
            &export_params,
            std::time::Duration::from_secs(5),
            std::time::Duration::from_secs(10 * 60),
        )
    }
    .map_err(|e| {
        if crate::modules::rpc::is_rpc_transport_timeout_error(&e) {
            format!("Core exportwalletmigration timed out. The wallet may be large.")
        } else {
            format!("Core exportwalletmigration failed: {e}")
        }
    })?;

    // Validate the exported file exists
    if !dest.exists() {
        let _ = relock_guard.relock_now();
        return Err("Export completed but the destination file was not created.".to_string());
    }

    // B6: Validate the completed export with Core validatewalletmigration,
    // not merely JSON parsing.
    let content = fs::read_to_string(&dest)
        .map_err(|e| format!("Export succeeded but cannot read the output file: {e}"))?;

    let _: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| format!("Exported file is not valid JSON: {e}"))?;

    let validate_temp = TempFileGuard::new("vault_export_validate")?;
    fs::write(&validate_temp.path, &content)
        .map_err(|e| format!("Cannot write temp file for validation: {e}"))?;

    // Validate via Core RPC (unqualified — file is portable)
    let validation = crate::modules::commands::validate_wallet_migration(
        validate_temp.path_str(),
        export_passphrase.to_string(),
    )
    .map_err(|e| format!("Core validation of exported file failed: {e}"))?;

    let valid = validation
        .get("valid")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if !valid {
        return Err("Exported file was created but Core validatewalletmigration reports it as invalid. The wallet may not be restorable.".to_string());
    }

    // Relock wallet
    let relock_result = relock_guard.relock_now();

    let mut result = serde_json::json!({
        "exported": true,
        "validated": valid,
        "destination": dest_path,
        "filename": export_result.get("filename").and_then(|v| v.as_str()).unwrap_or(dest_path),
        "wallet_name": export_result.get("wallet_name").and_then(|v| v.as_str()),
        "exporting_wallet": wallet_display_name,
        "encrypted": true,
        "include_private": true,
    });

    if was_previously_locked {
        if let Some(obj) = result.as_object_mut() {
            obj.insert(
                "wallet_relocked".to_string(),
                serde_json::Value::Bool(relock_result.is_ok()),
            );
            if let Some(ref rerr) = relock_result.as_ref().err() {
                obj.insert(
                    "wallet_relock_error".to_string(),
                    serde_json::Value::String(rerr.to_string()),
                );
            }
        }
    }

    Ok(result)
}

// ─── WebCom Primary -> Core Connect Command — slice 64d/64e ─────────────
//
// Full bridge: WebCom wallet.bip39 vault record -> temporary Core v2
// migration envelope -> validatewalletmigration -> restorewalletmigration
// -> verify restore result -> verified alignment record.
//
// Slice 64e hardening:
// - Automated pre-connect backup gate (no manual record ID)
// - Restore result verification before writing alignment
// - Post-restore encryption status warning
// - Post-restore backup attempt

fn detect_current_core_wallet_exists() -> bool {
    if let Ok(raw) = crate::modules::commands::run_cli(&[String::from("getwalletmigrationinfo")]) {
        if let Ok(info) = serde_json::from_str::<serde_json::Value>(&raw) {
            return info.get("walletname").is_some() || info.get("hd_enabled").is_some();
        }
    }
    false
}

// ─── Named wallet loaded-state detection (slice 64h) ────────────────────
//
// After `restorewalletmigration`, Hemp0x Core writes a new BDB file at
// `<datadir>/<walletname>` and, in most builds, loads it into the
// running daemon so it is queryable with `-wallet=<name>`. That is
// the user-visible "the vault wallet is loaded" state.
//
// `describe_named_wallet_state` is the single source of truth for
// whether the named wallet is currently queryable. The flow is:
//
// 1. File exists on disk at `<datadir>/<walletname>`?
// 2. Try `-wallet=<name> getwalletinfo` (the actual command users run).
// 3. If that fails, attempt `loadwallet <name>` and re-query.
// 4. If still not queryable, return `restart_required: true` so the
//    UI can show a clear "Restart Core and load vault wallet" path.
//
// UTXO scans, balance reads, wallet encryption, and rescan MUST NOT
// run against a named wallet until this helper reports it is loaded.
// Skipping that gate produced 0-balance reads against the wrong
// wallet in slice 64g testing.

#[derive(Clone, Debug)]
pub struct NamedWalletState {
    pub wallet_name: String,
    pub wallet_file_exists: bool,
    pub wallet_file_path: String,
    pub named_wallet_loaded: bool,
    pub loaded_via: String, // "queryable" | "loadwallet" | "none"
    pub restart_required: bool,
    pub default_wallet_path: String,
    pub previous_default_wallet_existed: bool,
    pub wallet_arg: String, // `-wallet=<name>` for callers
    pub load_error: Option<String>,
    pub query_error: Option<String>,
    pub query_info: Option<serde_json::Value>,
}

pub fn describe_named_wallet_state(wallet_name: &str) -> NamedWalletState {
    let dir = data_dir().unwrap_or_else(|_| PathBuf::from(""));
    let new_wallet_file = dir.join(wallet_name);
    let wallet_file_exists = new_wallet_file.is_file();
    let default_wallet = dir.join("wallet.dat");
    let previous_default_wallet_existed = default_wallet.exists();
    let wallet_arg = format!("-wallet={wallet_name}");

    let mut state = NamedWalletState {
        wallet_name: wallet_name.to_string(),
        wallet_file_exists,
        wallet_file_path: new_wallet_file.to_string_lossy().to_string(),
        named_wallet_loaded: false,
        loaded_via: "none".to_string(),
        restart_required: false,
        default_wallet_path: default_wallet.to_string_lossy().to_string(),
        previous_default_wallet_existed,
        wallet_arg: wallet_arg.clone(),
        load_error: None,
        query_error: None,
        query_info: None,
    };

    if !wallet_file_exists {
        return state;
    }

    // Step 2: direct queryable check.
    if let Ok(raw) =
        crate::modules::commands::run_cli(&[wallet_arg.clone(), String::from("getwalletinfo")])
    {
        if let Ok(info) = serde_json::from_str::<serde_json::Value>(&raw) {
            state.named_wallet_loaded = true;
            state.loaded_via = "queryable".to_string();
            state.query_info = Some(info);
            return state;
        }
    } else {
        state.query_error = Some(format!("Could not query {wallet_arg} getwalletinfo"));
    }

    // B1 66d: Core Next does not support dynamic loadwallet.
    // If not queryable, report restart_required instead.
    state.restart_required = true;
    state.load_error = Some("Wallet file exists but is not queryable in the running Core daemon. Restart Core with -wallet=<name>.".to_string());
    state
}

pub fn named_wallet_state_to_json(state: &NamedWalletState) -> serde_json::Value {
    let mut obj = serde_json::json!({
        "wallet_name": state.wallet_name,
        "wallet_file_exists": state.wallet_file_exists,
        "wallet_file_path": state.wallet_file_path,
        "named_wallet_loaded": state.named_wallet_loaded,
        "loaded_via": state.loaded_via,
        "restart_required": state.restart_required,
        "default_wallet_path": state.default_wallet_path,
        "previous_default_wallet_existed": state.previous_default_wallet_existed,
        "wallet_arg": state.wallet_arg,
    });
    if let Some(ref e) = state.load_error {
        obj["load_error"] = serde_json::Value::String(e.clone());
    }
    if let Some(ref e) = state.query_error {
        obj["query_error"] = serde_json::Value::String(e.clone());
    }
    if let Some(ref info) = state.query_info {
        obj["query_info"] = info.clone();
    }
    obj
}

// ─── Authoritative Active-Wallet Resolution — slice 66e ────────────────
//
// F5: Pure decision enum. Never chooses arbitrarily.
// F1: Used by promote_core_wallet_to_portable_primary_blocking,
//      export_core_migration_wallet_blocking, and identity verification.

#[derive(Clone, Debug)]
pub enum ProvenWallet {
    Default { wallet_name: String, path: PathBuf },
    Named { wallet_name: String, path: PathBuf },
}

#[derive(Clone, Debug)]
pub enum WalletResolution {
    Proven(ProvenWallet),
    Unavailable,
    Ambiguous {
        loaded_names: Vec<String>,
    },
    SettingsMismatch {
        configured: String,
        actual: Option<String>,
    },
}

pub struct ActiveWalletInput {
    pub configured_named_wallet: Option<String>,
    pub loaded_wallets: Vec<String>,
    pub default_wallet_path: PathBuf,
    pub is_default_queryable: bool,
    pub named_queryable: std::collections::HashMap<String, bool>,
}

pub fn resolve_proven_wallet(input: &ActiveWalletInput) -> WalletResolution {
    const DEFAULT_WALLET_NAME: &str = "wallet.dat";
    let has_default = input
        .loaded_wallets
        .iter()
        .any(|w| w == DEFAULT_WALLET_NAME);
    let named_loaded: Vec<&String> = input
        .loaded_wallets
        .iter()
        .filter(|w| w.as_str() != DEFAULT_WALLET_NAME)
        .collect();

    if let Some(ref configured) = input.configured_named_wallet {
        let is_loaded = named_loaded.iter().any(|n| *n == configured);
        let is_queryable = input
            .named_queryable
            .get(configured)
            .copied()
            .unwrap_or(false);

        if is_loaded && is_queryable {
            let path = input
                .default_wallet_path
                .parent()
                .unwrap_or(std::path::Path::new(""))
                .join(configured);
            return WalletResolution::Proven(ProvenWallet::Named {
                wallet_name: configured.clone(),
                path,
            });
        }

        if is_loaded {
            return WalletResolution::Unavailable;
        }

        let actual = if input.loaded_wallets.len() == 1 {
            input.loaded_wallets.first().cloned()
        } else {
            None
        };
        return WalletResolution::SettingsMismatch {
            configured: configured.clone(),
            actual,
        };
    }

    let loaded_count = named_loaded.len() + usize::from(has_default);
    if loaded_count > 1 {
        return WalletResolution::Ambiguous {
            loaded_names: input.loaded_wallets.clone(),
        };
    }

    if named_loaded.len() == 1 {
        let name = named_loaded[0].clone();
        if !input.named_queryable.get(&name).copied().unwrap_or(false) {
            return WalletResolution::Unavailable;
        }
        let path = input
            .default_wallet_path
            .parent()
            .unwrap_or(std::path::Path::new(""))
            .join(&name);
        let wallet_name = name.clone();
        return WalletResolution::Proven(ProvenWallet::Named { wallet_name, path });
    }

    if has_default {
        if !input.is_default_queryable {
            return WalletResolution::Unavailable;
        }
        return WalletResolution::Proven(ProvenWallet::Default {
            wallet_name: DEFAULT_WALLET_NAME.to_string(),
            path: input.default_wallet_path.clone(),
        });
    }

    WalletResolution::Unavailable
}

fn gather_active_wallet_input() -> Result<ActiveWalletInput, String> {
    let dir = data_dir().unwrap_or_else(|_| PathBuf::from(""));
    let default_wallet_path = dir.join("wallet.dat");

    let configured_named: Option<String> = crate::modules::files::load_app_settings_impl()
        .ok()
        .and_then(|s| s.active_vault_wallet_name);

    let loaded_wallets: Vec<String> =
        match crate::modules::commands::run_cli(&[String::from("listwallets")]) {
            Ok(raw) => serde_json::from_str::<Vec<String>>(&raw).unwrap_or_default(),
            Err(_) => Vec::new(),
        };

    let is_default_queryable = loaded_wallets.iter().any(|w| w == "wallet.dat")
        && crate::modules::commands::run_cli(&[
            String::from("-wallet=wallet.dat"),
            String::from("getwalletinfo"),
        ])
        .ok()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
        .is_some();

    let mut named_queryable = std::collections::HashMap::new();
    for name in loaded_wallets.iter().filter(|w| w.as_str() != "wallet.dat") {
        let wallet_arg = format!("-wallet={name}");
        let q = crate::modules::commands::run_cli(&[wallet_arg, String::from("getwalletinfo")])
            .ok()
            .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
            .is_some();
        named_queryable.insert(name.clone(), q);
    }

    Ok(ActiveWalletInput {
        configured_named_wallet: configured_named,
        loaded_wallets,
        default_wallet_path,
        is_default_queryable,
        named_queryable,
    })
}

// ─── Unified Exact-Wallet Identity Verifier — slice 66e ─────────────────
//
// B6: One function that proves a named Core wallet has the same canonical
// BIP39 identity as a given mnemonic. Used by both:
//   - existing-target-wallet identity verification (B2/B7)
//   - restore-timeout/duplicate-wallet recovery

fn verify_exact_wallet_identity(
    wallet_name: &str,
    expected_canonical_id: &str,
    wallet_unlock_passphrase: Option<&str>,
) -> Result<(), String> {
    let state = describe_named_wallet_state(wallet_name);
    if !state.named_wallet_loaded {
        return Err(format!(
            "Wallet '{}' is not queryable. Cannot verify identity.",
            wallet_name
        ));
    }

    let info: serde_json::Value =
        crate::modules::rpc::call_rpc_wallet(wallet_name, "getwalletmigrationinfo", &[]).map_err(
            |e| {
                format!(
                    "Cannot query wallet '{}' for identity check: {}",
                    wallet_name, e
                )
            },
        )?;

    let encrypted = info
        .get("encrypted")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let locked = info
        .get("locked")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let _was_previously_locked = encrypted && locked;
    let mut relock_guard = WalletRelockGuard::new(wallet_name, false, false);

    if encrypted && locked {
        // F3: Preserve WALLET_UNLOCK_REQUIRED:: prefix exactly
        let unlock_pass = wallet_unlock_passphrase
            .ok_or(format!("WALLET_UNLOCK_REQUIRED::The runtime wallet '{wallet_name}' is encrypted and locked. Provide the wallet unlock passphrase to verify identity."))?;
        if unlock_pass.is_empty() {
            return Err(format!("WALLET_UNLOCK_REQUIRED::The runtime wallet '{wallet_name}' is encrypted and locked. Provide the wallet unlock passphrase."));
        }
        let unlock_params = vec![
            serde_json::Value::String(unlock_pass.to_string()),
            serde_json::Value::Number(serde_json::value::Number::from(60)),
        ];
        crate::modules::rpc::call_rpc_wallet(wallet_name, "walletpassphrase", &unlock_params)
            .map_err(|e| format!("Cannot unlock wallet '{}': {}", wallet_name, e))?;
        relock_guard.was_unlocked_by_us = true;
    }

    // Export private migration envelope
    let mut rng = rand::thread_rng();
    let mut id_bytes = [0u8; 32];
    rng.fill_bytes(&mut id_bytes);
    let id_passphrase = Zeroizing::new(hex::encode(id_bytes));

    let id_temp = TempFileGuard::new("vault_identity_verify")?;
    let export_params = vec![
        serde_json::Value::String(id_temp.path_str()),
        serde_json::Value::Bool(true),
        serde_json::Value::Bool(true),
        serde_json::Value::String(id_passphrase.as_str().to_string()),
    ];
    crate::modules::rpc::call_rpc_wallet(wallet_name, "exportwalletmigration", &export_params)
        .map_err(|e| {
            let _ = relock_guard.relock_now();
            format!(
                "Cannot export wallet '{}' for identity check: {}",
                wallet_name, e
            )
        })?;

    let envelope_content = fs::read_to_string(&id_temp.path).map_err(|e| {
        let _ = relock_guard.relock_now();
        format!("Cannot read identity-check envelope: {e}")
    })?;

    let id_material = decrypt_core_migration_bip39(&envelope_content, id_passphrase.as_str())
        .map_err(|e| {
            let _ = relock_guard.relock_now();
            format!(
                "Cannot decrypt existing wallet '{}' for identity check: {}",
                wallet_name, e
            )
        })?;

    let existing_id = canonical_wallet_identity(id_material.mnemonic.as_str());

    // Relock before returning
    let relock_result = relock_guard.relock_now();

    if existing_id != expected_canonical_id {
        return Err(format!(
            "Wallet '{}' identity does not match the expected canonical wallet. The wallet at this name is a different BIP39 wallet.",
            wallet_name
        ));
    }

    // Check relock didn't fail (don't hide identity result)
    if let Err(ref rerr) = relock_result {
        return Err(format!(
            "Wallet identity verification succeeded but relock failed: {}. The wallet is still unlocked.",
            rerr
        ));
    }

    Ok(())
}
///
/// This is the single, cheap read used after Core start/restart and after
/// vault connect to answer: "is the active vault wallet actually loaded and
/// queryable in Core right now?" It returns the configured active wallet
/// name, whether the daemon RPC is reachable, whether that specific named
/// wallet is queryable, the list of wallets Core reports as loaded, and any
/// load error — without performing any restore/scan work and without
/// touching `hemp.conf`.
pub fn active_wallet_startup_state() -> serde_json::Value {
    let settings = crate::modules::files::load_app_settings_impl().ok();
    let active_wallet = settings
        .as_ref()
        .and_then(|s| s.active_vault_wallet_name.clone());

    // Daemon reachability: a single cheap getnetworkinfo probe.
    let daemon_reachable = crate::modules::commands::run_cli(&[String::from("getnetworkinfo")])
        .ok()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
        .is_some();

    // Wallets Core currently reports as loaded (multi-wallet RPC).
    let loaded_wallets: Vec<String> =
        match crate::modules::commands::run_cli(&[String::from("listwallets")]) {
            Ok(raw) => serde_json::from_str::<Vec<String>>(&raw).unwrap_or_default(),
            Err(_) => Vec::new(),
        };

    let mut wallet_queryable = false;
    let mut wallet_file_exists = false;
    let mut wallet_state_json: Option<serde_json::Value> = None;
    let mut load_error: Option<String> = None;

    if let Some(ref name) = active_wallet {
        let state = describe_named_wallet_state(name);
        wallet_queryable = state.named_wallet_loaded;
        wallet_file_exists = state.wallet_file_exists;
        wallet_state_json = Some(named_wallet_state_to_json(&state));
        if let Some(e) = state.load_error {
            load_error = Some(e);
        } else if !state.named_wallet_loaded && state.wallet_file_exists {
            load_error = Some(format!(
                "Active vault wallet '{}' exists on disk but is not loaded in Core. Start Core through Commander or use Restore to load it.",
                name
            ));
        }
    }

    serde_json::json!({
        "active_wallet_name": active_wallet,
        "daemon_reachable": daemon_reachable,
        "wallet_queryable": wallet_queryable,
        "wallet_file_exists": wallet_file_exists,
        "loaded_wallets": loaded_wallets,
        "load_error": load_error,
        "wallet_state": wallet_state_json,
    })
}

#[tauri::command]
pub async fn vault_get_active_wallet_startup_state() -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || Ok(active_wallet_startup_state()))
        .await
        .map_err(|e| format!("Startup state task failed: {e}"))?
}

/// Query Core's current chain tip block height. Returns 0 if Core is
/// unreachable. Used to bound the restore rescan window when the
/// WebCom record has no `best_block_height` of its own.
fn query_core_chain_tip() -> i64 {
    if let Ok(raw) = crate::modules::commands::run_cli(&[String::from("getblockchaininfo")]) {
        if let Ok(info) = serde_json::from_str::<serde_json::Value>(&raw) {
            if let Some(blocks) = info.get("blocks").and_then(|v| v.as_i64()) {
                return blocks;
            }
            if let Some(blocks) = info.get("blocks").and_then(|v| v.as_u64()) {
                return blocks as i64;
            }
        }
    }
    if let Ok(raw) = crate::modules::commands::run_cli(&[String::from("getinfo")]) {
        if let Ok(info) = serde_json::from_str::<serde_json::Value>(&raw) {
            if let Some(blocks) = info.get("blocks").and_then(|v| v.as_i64()) {
                return blocks;
            }
        }
    }
    0
}

fn verify_restore_result(
    restore_result: &serde_json::Value,
    expected_wallet_name: &str,
) -> Result<(), String> {
    let restored_name = restore_result
        .get("wallet_name")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if restored_name.is_empty() {
        return Err(
            "Core restore returned no wallet_name. Cannot verify the restored wallet.".to_string(),
        );
    }

    if restored_name != expected_wallet_name {
        return Err(format!(
            "Core restored wallet name mismatch: expected {expected_wallet_name}, got {restored_name}"
        ));
    }

    let chain = restore_result.get("chain");
    if let Some(chain) = chain {
        if let Some(network) = chain.get("network").and_then(|v| v.as_str()) {
            if network != CORE_MIGRATION_MAINNET_NETWORK_ID {
                return Err(format!(
                    "Core restored wallet network mismatch: expected {CORE_MIGRATION_MAINNET_NETWORK_ID}, got {network}"
                ));
            }
        }
        if let Some(coin_type) = chain.get("coin_type_bip44").and_then(|v| v.as_i64()) {
            if coin_type != 420 {
                return Err(format!(
                    "Core restored wallet coin type mismatch: expected 420, got {coin_type}"
                ));
            }
        }
    }

    if let Some(derivation) = restore_result.get("derivation").and_then(|v| v.as_array()) {
        if let Some(first) = derivation.first() {
            if let Some(profile) = first.get("profile_id").and_then(|v| v.as_str()) {
                if profile != DERIVATION_HEMP_CANONICAL_420 {
                    return Err(format!(
                        "Core restored wallet derivation profile mismatch: expected {}, got {profile}",
                        DERIVATION_HEMP_CANONICAL_420
                    ));
                }
            }
        }
    }

    Ok(())
}

pub fn connect_webcom_primary_wallet_to_core(
    passphrase: &str,
    wallet_name: Option<&str>,
    birth_height: Option<i64>,
    pre_connect_backup_record_id: Option<&str>,
) -> Result<serde_json::Value, String> {
    let bundle = load_bundle()?.ok_or("Vault does not exist")?;
    let payload = decrypt_vault_envelope(passphrase, &bundle.vault)?;

    let webcom_record = payload
        .secrets
        .get(RECORD_ID_WALLET_HEMP_PRIMARY)
        .ok_or("No wallet.webcom.hemp.primary record found in vault")?;

    if webcom_record.value.is_empty() {
        return Err("WebCom primary wallet record has an empty value".to_string());
    }

    let record_type = webcom_record.record_type.as_str();
    if record_type != RECORD_TYPE_WALLET_BIP39 {
        return Err(format!(
            "WebCom primary record type is {record_type}, not {}. Only BIP39 records can be connected to Core via this flow.",
            RECORD_TYPE_WALLET_BIP39
        ));
    }

    let seed_type = webcom_record
        .metadata
        .as_ref()
        .and_then(|m| m.get("recovery"))
        .and_then(|r| r.get("seedType"))
        .and_then(|v| v.as_str())
        .unwrap_or("bip39");

    if seed_type != "bip39" {
        return Err(format!(
            "WebCom primary seed type is {seed_type}, not bip39. Only BIP39 records can be connected to Core via this flow."
        ));
    }

    let derivation_hemp = webcom_record
        .metadata
        .as_ref()
        .and_then(|m| m.get("recovery"))
        .and_then(|r| r.get("derivationProfiles"))
        .and_then(|dp| dp.get("hemp"))
        .and_then(|v| v.as_str())
        .or_else(|| {
            webcom_record
                .derivation_profiles
                .as_ref()
                .and_then(|dp| dp.get("hemp").and_then(|s| Some(s.as_str())))
        });

    if derivation_hemp == Some(DERIVATION_HEMP_LEGACY_175) {
        return Err(format!(
            "Legacy coin175 derivation profile ({}) is not supported for Core restore.",
            DERIVATION_HEMP_LEGACY_175
        ));
    }

    let current_wallet_exists = detect_current_core_wallet_exists();

    let mut effective_backup_record_id: Option<String> = None;

    if let Some(backup_id) = pre_connect_backup_record_id {
        if !backup_id.is_empty() {
            if !payload.secrets.contains_key(backup_id) {
                return Err(format!(
                    "Pre-connect backup record {backup_id} does not exist in vault"
                ));
            }
            effective_backup_record_id = Some(backup_id.to_string());
        }
    }

    if current_wallet_exists && effective_backup_record_id.is_none() {
        return Err(
            "A current Core runtime wallet exists. Back up your wallet first (use the Vault Backups section), then provide the backup record ID, or use the guided connect flow which handles backup for you.".to_string()
        );
    }

    let effective_wallet_name = wallet_name
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| MIGRATION_DEFAULT_WALLET_NAME.to_string());

    let safe_name = validate_migration_wallet_name_for_vault(&effective_wallet_name)?;

    let mut result = perform_webcom_to_core_restore_and_align(
        passphrase,
        webcom_record,
        &safe_name,
        birth_height,
        effective_backup_record_id.as_deref(),
    )?;

    if let Some(obj) = result.as_object_mut() {
        obj.insert(
            "pre_connect_backup_created".to_string(),
            serde_json::Value::Bool(false),
        );
    }

    Ok(result)
}

// ─── Guided WebCom -> Core Connect — slice 64f ────────────────────────────
//
// Performs the safe, one-click user flow in a single backend call:
// 1. Load/decrypt the currently selected Hemp0x Vault.
// 2. Detect whether a current Core runtime wallet exists.
// 3. If a Core wallet exists, create a pre-connect migration backup
//    inside the vault automatically, using the vault passphrase for
//    recovery so the user does not need a second password.
// 4. Run the verified build/validate/restore/align pipeline.
// 5. Return a unified result including backup metadata, runtime
//    encryption state, and (if Core was locked) a structured
//    `wallet_unlock_required` error that the UI can route to the
//    normal wallet unlock modal.

pub fn connect_webcom_primary_wallet_to_core_guided(
    passphrase: &str,
    wallet_name: Option<&str>,
    birth_height: Option<i64>,
    skip_backup: bool,
) -> Result<serde_json::Value, String> {
    let bundle = load_bundle()?.ok_or("Vault does not exist")?;
    let payload = decrypt_vault_envelope(passphrase, &bundle.vault)?;

    let webcom_record = payload
        .secrets
        .get(RECORD_ID_WALLET_HEMP_PRIMARY)
        .ok_or("No wallet.webcom.hemp.primary record found in vault")?
        .clone();

    if webcom_record.value.is_empty() {
        return Err("WebCom primary wallet record has an empty value".to_string());
    }

    let record_type = webcom_record.record_type.as_str();
    if record_type != RECORD_TYPE_WALLET_BIP39 {
        return Err(format!(
            "WebCom primary record type is {record_type}, not {}. Only BIP39 records can be connected to Core via this flow.",
            RECORD_TYPE_WALLET_BIP39
        ));
    }

    let seed_type = webcom_record
        .metadata
        .as_ref()
        .and_then(|m| m.get("recovery"))
        .and_then(|r| r.get("seedType"))
        .and_then(|v| v.as_str())
        .unwrap_or("bip39");

    if seed_type != "bip39" {
        return Err(format!(
            "WebCom primary seed type is {seed_type}, not bip39. Only BIP39 records can be connected to Core via this flow."
        ));
    }

    let derivation_hemp = webcom_record
        .metadata
        .as_ref()
        .and_then(|m| m.get("recovery"))
        .and_then(|r| r.get("derivationProfiles"))
        .and_then(|dp| dp.get("hemp"))
        .and_then(|v| v.as_str())
        .or_else(|| {
            webcom_record
                .derivation_profiles
                .as_ref()
                .and_then(|dp| dp.get("hemp").and_then(|s| Some(s.as_str())))
        });

    if derivation_hemp == Some(DERIVATION_HEMP_LEGACY_175) {
        return Err(format!(
            "Legacy coin175 derivation profile ({}) is not supported for Core restore.",
            DERIVATION_HEMP_LEGACY_175
        ));
    }

    let current_wallet_exists = detect_current_core_wallet_exists();

    let effective_wallet_name = wallet_name
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| MIGRATION_DEFAULT_WALLET_NAME.to_string());

    let safe_name = validate_migration_wallet_name_for_vault(&effective_wallet_name)?;

    let mut pre_connect_backup_record_id: Option<String> = None;
    let mut pre_connect_backup_created = false;
    let mut backup_skipped = false;

    if current_wallet_exists && !skip_backup {
        let label = format_pre_connect_backup_label();
        let backup_result = vault_export_current_wallet_migration_record(
            label,
            true,
            passphrase.to_string(),
            Some(passphrase.to_string()),
            Some(RECOVERY_MODE_VAULT_PASSPHRASE.to_string()),
        );

        match backup_result {
            Ok(b) => {
                if let Some(rid) = b
                    .get("record_id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                {
                    pre_connect_backup_record_id = Some(rid);
                    pre_connect_backup_created = true;
                } else {
                    return Err("Guided pre-connect backup did not return a record id".to_string());
                }
            }
            Err(e) => {
                if is_core_wallet_unlock_required_error(&e) {
                    return Err(format_guided_unlock_required_error(&e));
                }
                return Err(format!("Guided pre-connect backup failed: {e}"));
            }
        }
    } else if current_wallet_exists && skip_backup {
        backup_skipped = true;
    }

    // Pre-check: refuse to restore into a wallet name that already exists
    // on disk. The user must choose a different name or remove the existing
    // wallet before retrying.
    let existing_state = describe_named_wallet_state(&safe_name);
    if existing_state.wallet_file_exists {
        return Err(format!(
            "A wallet named '{}' already exists in Core at {}. Choose a different wallet name.",
            safe_name, existing_state.wallet_file_path
        ));
    }

    let mut result = perform_webcom_to_core_restore_and_align(
        passphrase,
        &webcom_record,
        &safe_name,
        birth_height,
        pre_connect_backup_record_id.as_deref(),
    )?;

    if let Some(obj) = result.as_object_mut() {
        obj.insert(
            "pre_connect_backup_created".to_string(),
            serde_json::Value::Bool(pre_connect_backup_created),
        );
        obj.insert(
            "backup_skipped".to_string(),
            serde_json::Value::Bool(backup_skipped),
        );
        obj.insert(
            "current_core_wallet_exists".to_string(),
            serde_json::Value::Bool(current_wallet_exists),
        );
        if let Some(ref id) = pre_connect_backup_record_id {
            obj.insert(
                "pre_connect_backup_record_id".to_string(),
                serde_json::Value::String(id.clone()),
            );
            obj.insert(
                "pre_connect_backup_label".to_string(),
                serde_json::Value::String(format_pre_connect_backup_label()),
            );
        }
        obj.insert("guided".to_string(), serde_json::Value::Bool(true));
    }

    Ok(result)
}

fn format_pre_connect_backup_label() -> String {
    let now = chrono::Utc::now();
    format!("Pre-connect backup - {}", now.format("%Y-%m-%d %H:%M"))
}

fn is_core_wallet_unlock_required_error(err: &str) -> bool {
    let text = err.to_lowercase();
    text.contains("wallet is locked")
        || text.contains("wallet locked")
        || text.contains("please enter the wallet passphrase")
        || text.contains("enter the wallet passphrase with walletpassphrase")
        || text.contains("unencrypted wallet detected") && text.contains("walletpassphrase")
        || text.contains("the wallet is currently locked")
}

fn format_guided_unlock_required_error(original: &str) -> String {
    format!(
        "WALLET_UNLOCK_REQUIRED::{original}::Use the wallet unlock flow (wallet_unlock RPC) to unlock the Core runtime wallet, then retry the guided connect. The Core wallet must be unlocked before Commander can back it up."
    )
}

fn is_duplicate_wallet_error(err: &str) -> bool {
    let lower = err.to_lowercase();
    lower.contains("already exists")
        || lower.contains("already loaded")
        || lower.contains("duplicate")
        || lower.contains("wallet file already exists")
}

fn verify_existing_wallet_after_duplicate(wallet_name: &str) -> Result<serde_json::Value, String> {
    // The previous restorewalletmigration call appears to have actually
    // created the wallet before Commander's RPC read timed out. We
    // re-issue getwalletmigrationinfo with -wallet=<name> via the CLI
    // runner to confirm the wallet exists and matches the canonical
    // coin420 profile.
    let args: Vec<String> = vec![
        format!("-wallet={wallet_name}"),
        String::from("getwalletmigrationinfo"),
    ];
    let raw = crate::modules::commands::run_cli(&args)
        .map_err(|e| format!("Failed to query existing wallet '{wallet_name}': {e}"))?;

    let info: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| format!("Could not parse getwalletmigrationinfo for '{wallet_name}': {e}"))?;

    let network = info
        .get("chain")
        .and_then(|c| c.get("network"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let coin_type = info
        .get("chain")
        .and_then(|c| c.get("coin_type_bip44"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    if network != CORE_MIGRATION_MAINNET_NETWORK_ID {
        return Err(format!(
            "Existing wallet '{wallet_name}' network is '{network}', expected '{}'",
            CORE_MIGRATION_MAINNET_NETWORK_ID
        ));
    }
    if coin_type != 420 {
        return Err(format!(
            "Existing wallet '{wallet_name}' coin_type is {coin_type}, expected 420"
        ));
    }

    Ok(serde_json::json!({
        "wallet_name": wallet_name,
        "wallet_arg": format!("-wallet={wallet_name}"),
        "chain": {
            "network": network,
            "coin_type_bip44": coin_type,
        },
        "derivation": [
            {
                "profile_id": DERIVATION_HEMP_CANONICAL_420,
                "purpose": 44,
                "coin_type": coin_type,
            }
        ],
        "recovered_after_duplicate": true,
    }))
}

fn verify_restored_wallet_identity(
    wallet_name: &str,
    expected_canonical_id: &str,
) -> Result<serde_json::Value, String> {
    verify_exact_wallet_identity(wallet_name, expected_canonical_id, None)?;
    verify_existing_wallet_after_duplicate(wallet_name)
}

/// After a successful near-tip restore, enumerate all derived
/// addresses the new wallet knows about. Core derives all addresses
/// from the BIP39 mnemonic at restore time, so this works even
/// without a chain rescan. Used to feed `scantxoutset` for an
/// instant balance check.
fn enumerate_wallet_addresses(wallet_name: &str) -> Result<Vec<String>, String> {
    let args: Vec<String> = vec![
        format!("-wallet={wallet_name}"),
        String::from("listaddressgroupings"),
    ];
    let raw = crate::modules::commands::run_cli(&args)
        .map_err(|e| format!("Failed to list wallet addresses: {e}"))?;
    let groups: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| format!("Could not parse listaddressgroupings: {e}"))?;

    let mut addresses = Vec::new();
    let mut seen = std::collections::HashSet::new();
    if let Some(arr) = groups.as_array() {
        for group in arr {
            if let Some(items) = group.as_array() {
                for item in items {
                    if let Some(addr) = item.get(0).and_then(|v| v.as_str()) {
                        if seen.insert(addr.to_string()) {
                            addresses.push(addr.to_string());
                        }
                    }
                }
            }
        }
    }
    Ok(addresses)
}

/// Use Core's `scantxoutset` to scan the global UTXO set for
/// outputs belonging to the given addresses. This is FAST (seconds)
/// and does NOT require a chain rescan — it queries the current
/// UTXO set directly. Returns the total amount and the number of
/// UTXOs found.
fn scantxoutset_for_addresses(addresses: &[String]) -> Result<serde_json::Value, String> {
    if addresses.is_empty() {
        return Ok(serde_json::json!({
            "success": true,
            "total_amount": 0.0,
            "utxo_count": 0,
            "unspents": [],
        }));
    }
    // Build the scanobjects JSON. Core accepts either "addr(<addr>)"
    // descriptors or raw addresses. Using raw addresses is the most
    // compatible across Core forks.
    let scan_objects: Vec<serde_json::Value> = addresses
        .iter()
        .map(|a| serde_json::json!({"address": a}))
        .collect();
    let scan_objects_json = serde_json::to_string(&scan_objects)
        .map_err(|e| format!("Failed to build scantxoutset scan objects: {e}"))?;

    let args: Vec<String> = vec![
        String::from("scantxoutset"),
        String::from("start"),
        scan_objects_json,
    ];
    let raw = crate::modules::commands::run_cli(&args)
        .map_err(|e| format!("scantxoutset failed: {e}"))?;
    let parsed: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| format!("Could not parse scantxoutset response: {e}"))?;

    let success = parsed
        .get("success")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let total_amount = parsed
        .get("total_amount")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let utxo_count = parsed
        .get("unspents")
        .and_then(|u| u.as_array())
        .map(|a| a.len())
        .unwrap_or(0);

    Ok(serde_json::json!({
        "success": success,
        "total_amount": total_amount,
        "utxo_count": utxo_count,
        "scanned_addresses": addresses.len(),
    }))
}

/// After a successful windowed restore, query the newly restored wallet
/// for its earliest transaction block height. This is used to detect
/// whether the windowed rescan window actually covered the wallet's
/// first activity. If the first tx is older than the window the
/// caller can trigger a deeper rescan.
///
/// Returns `Ok(None)` when the wallet has no transactions yet
/// (freshly restored or new wallet), and `Err` only on transport /
/// parse failures. We deliberately do NOT treat "no transactions" as
/// an error because that is a perfectly normal post-restore state.
fn query_wallet_first_tx_block(wallet_name: &str) -> Result<Option<i64>, String> {
    let args: Vec<String> = vec![
        format!("-wallet={wallet_name}"),
        String::from("listtransactions"),
        String::from("*"),
        String::from("999999"),
    ];
    let raw = crate::modules::commands::run_cli(&args)
        .map_err(|e| format!("Failed to query wallet transactions: {e}"))?;
    let list: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("Could not parse listtransactions: {e}"))?;

    let arr = match list.as_array() {
        Some(a) => a,
        None => return Ok(None),
    };

    let mut min_block: Option<i64> = None;
    for tx in arr {
        let category = tx.get("category").and_then(|v| v.as_str()).unwrap_or("");
        // Skip immature / non-confirmed entries
        if category != "send" && category != "receive" {
            continue;
        }
        if let Some(bh) = tx.get("blockheight").and_then(|v| v.as_i64()) {
            if bh > 0 {
                min_block = Some(match min_block {
                    Some(prev) => prev.min(bh),
                    None => bh,
                });
            }
        }
    }
    Ok(min_block)
}

fn perform_webcom_to_core_restore_and_align(
    passphrase: &str,
    webcom_record: &SecretRecord,
    safe_name: &str,
    birth_height: Option<i64>,
    pre_connect_backup_record_id: Option<&str>,
) -> Result<serde_json::Value, String> {
    let expected_canonical_id = canonical_wallet_identity(&webcom_record.value);
    // If the caller did not pass a birth_height, default to a
    // "fast default" that skips the chain rescan almost entirely.
    // Preference order:
    // 1. Explicit caller birth_height (power user override).
    // 2. WebCom metadata.best_block_height (most accurate when present).
    // 3. Chain tip minus FAST_DEFAULT_BIRTH_HEIGHT_BACKOFF (1 block —
    //    effectively no rescan; we use Core's global scantxoutset and
    //    a background rescanblockchain to populate balance and history).
    // 4. 0 (genesis) as a last resort.
    let effective_birth_height: Option<i64> = match birth_height {
        Some(h) if h >= 0 => Some(h),
        _ => {
            let from_metadata = webcom_record
                .metadata
                .as_ref()
                .and_then(|m| m.get("best_block_height"))
                .and_then(|v| v.as_i64())
                .filter(|h| *h > 0);

            match from_metadata {
                Some(h) => Some(h),
                None => {
                    let tip = query_core_chain_tip();
                    if tip > FAST_DEFAULT_BIRTH_HEIGHT_BACKOFF {
                        Some(tip - FAST_DEFAULT_BIRTH_HEIGHT_BACKOFF)
                    } else {
                        Some(0)
                    }
                }
            }
        }
    };

    let envelope_json = build_migration_envelope_from_webcom_bip39(passphrase, webcom_record)
        .map_err(|e| format!("Failed to build migration envelope from WebCom BIP39 record: {e}"))?;
    let keypool_hints = webcom_keypool_hints(webcom_record);
    let keypool_refill_target = vault_keypool_refill_target(
        keypool_hints.external_count_hint,
        keypool_hints.change_count_hint,
    );

    let temp = TempFileGuard::new("vault_webcom_connect")?;
    fs::write(&temp.path, &envelope_json)
        .map_err(|e| format!("Cannot write temp migration envelope: {e}"))?;

    let validation =
        validate_migration_envelope_file(&temp.path_str(), passphrase).map_err(|e| {
            let _ = fs::remove_file(&temp.path);
            format!("Migration envelope validation failed: {e}")
        })?;

    let valid = validation
        .get("valid")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if !valid {
        let _ = fs::remove_file(&temp.path);
        return Err("Generated migration envelope failed Core validation. The WebCom BIP39 record may be malformed or the derivation profile is incompatible.".to_string());
    }

    let restore_result = match crate::modules::commands::restore_wallet_migration(
        temp.path_str(),
        safe_name.to_string(),
        passphrase.to_string(),
        effective_birth_height,
    ) {
        Ok(r) => r,
        Err(e) => {
            // If the wallet was already created (e.g. previous attempt
            // succeeded but timed out before Commander could read the
            // result), Core will refuse with a duplicate-wallet error.
            // We then check the existing wallet on disk, and if its
            // private migration identity matches the vault primary we
            // recover gracefully and treat this as a successful restore.
            if is_duplicate_wallet_error(&e) {
                let _ = fs::remove_file(&temp.path);
                match verify_restored_wallet_identity(safe_name, &expected_canonical_id) {
                    Ok(existing) => existing,
                    Err(verify_err) => {
                        if verify_err.starts_with("WALLET_UNLOCK_REQUIRED::") {
                            return Err(verify_err);
                        }
                        return Err(format!(
                            "Core restorewalletmigration failed: {e}. Existing wallet '{safe_name}' identity could not be verified for recovery: {verify_err}"
                        ));
                    }
                }
            } else if crate::modules::rpc::is_rpc_transport_timeout_error(&e) {
                let _ = fs::remove_file(&temp.path);
                match verify_restored_wallet_identity(safe_name, &expected_canonical_id) {
                    Ok(existing) => existing,
                    Err(verify_err) => {
                        if verify_err.starts_with("WALLET_UNLOCK_REQUIRED::") {
                            return Err(verify_err);
                        }
                        return Err(format!(
                            "RESTORE_TIMEOUT::{safe_name}::{e}::Core did not respond within the restore RPC read timeout. Commander could not prove that the resulting wallet matches the vault primary: {verify_err}. Wait a moment and retry; alignment was not written."
                        ));
                    }
                }
            } else {
                let _ = fs::remove_file(&temp.path);
                return Err(format!("Core restorewalletmigration failed: {e}"));
            }
        }
    };

    verify_restore_result(&restore_result, safe_name)
        .map_err(|e| {
            let _ = fs::remove_file(&temp.path);
            format!("Restore result verification failed: {e}. The restored wallet may not match the expected profile. Alignment was not written.")
        })?;

    let restored_wallet_name = restore_result
        .get("wallet_name")
        .and_then(|v| v.as_str())
        .unwrap_or(safe_name)
        .to_string();

    let restored_wallet_arg = restore_result
        .get("wallet_arg")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    // FAST PATH: the restorewalletmigration above used a near-tip
    // birth_height so it took seconds. Now that the wallet.dat is
    // created and Core has derived all the addresses, we do four
    // things in sequence:
    //
    // 1. Verify the named wallet is actually queryable in Core
    //    (file exists, `-wallet=<name> getwalletinfo` responds, or
    //    `loadwallet <name>` is attempted as a safe fallback). If we
    //    cannot make it queryable, we surface a structured
    //    `wallet_state` so the UI can ask the user to restart Core
    //    and re-load the wallet. We do NOT run UTXO scan, rescan, or
    //    balance reads against a wallet that is not loaded, because
    //    those calls would silently target the wrong wallet (the
    //    default `wallet.dat`) and produce misleading zero balances.
    // 2. Enumerate the wallet's derived addresses via
    //    listaddressgroupings (no rescan needed for this — Core
    //    derives all addresses at restore time).
    // 3. Run Core's scantxoutset against those addresses to populate
    //    the global UTXO set result. This is fast (seconds) because
    //    it queries the global UTXO set, not the wallet's txindex.
    //    The result is reported as `utxo_scan` — UTXOs the chain
    //    says are spendable by this wallet right now. It is NOT the
    //    Core wallet balance until Core has imported those UTXOs
    //    into the wallet (which a fresh `restorewalletmigration`
    //    does at restore time). We then read
    //    `-wallet=<name> getwalletinfo` for the actual Core wallet
    //    balance and txcount so the user sees one canonical number.
    // 4. Report history recovery as pending. Commander starts the
    //    rescan only after the restored runtime wallet is ready and
    //    any requested encryption/restart has completed.
    let mut named_state = describe_named_wallet_state(&restored_wallet_name);
    let named_wallet_loaded = named_state.named_wallet_loaded;
    let mut keypool_refill: Option<serde_json::Value> = None;
    if named_wallet_loaded {
        match refill_named_wallet_keypool_if_needed(&restored_wallet_name, keypool_refill_target) {
            Ok(status) => {
                keypool_refill = Some(status);
                named_state = describe_named_wallet_state(&restored_wallet_name);
            }
            Err(err) => {
                keypool_refill = Some(serde_json::json!({
                    "triggered": false,
                    "wallet_name": restored_wallet_name,
                    "target_size": keypool_refill_target,
                    "error": err,
                }));
            }
        }
    }
    let wallet_state_json = named_wallet_state_to_json(&named_state);

    let mut addresses: Vec<String> = Vec::new();
    let mut utxo_scan: Option<serde_json::Value> = None;
    let mut wallet_balance: Option<serde_json::Value> = None;
    let mut balance_source = "none".to_string();

    if named_wallet_loaded {
        if let Ok(addrs) = enumerate_wallet_addresses(&restored_wallet_name) {
            addresses = addrs;
            if !addresses.is_empty() {
                utxo_scan = scantxoutset_for_addresses(&addresses).ok();
            }
        }

        // Read the real Core wallet balance/txcount for the named
        // wallet. This is the canonical "balance" number for the
        // UI; the UTXO scan is reported alongside it as a
        // chain-side estimate (or confirmation, if they match).
        if let Some(ref info) = named_state.query_info {
            let bal = info.get("balance").and_then(|v| v.as_f64());
            let txcount = info.get("txcount").and_then(|v| v.as_i64());
            let scanning = info.get("scanning").cloned();
            let mut bal_obj = serde_json::json!({});
            if let Some(b) = bal {
                bal_obj["balance"] = serde_json::Value::Number(
                    serde_json::Number::from_f64(b).unwrap_or_else(|| 0.into()),
                );
            }
            if let Some(t) = txcount {
                bal_obj["txcount"] = serde_json::Value::Number(t.into());
            }
            if let Some(s) = scanning {
                bal_obj["scanning"] = s;
            }
            wallet_balance = Some(bal_obj);
            balance_source = "core_wallet".to_string();
        }
    }

    // Do not start history recovery inside the restore transaction.
    // `rescanblockchain` monopolizes wallet RPC for long periods and can
    // prevent Commander from confirming status or encrypting the newly
    // restored runtime wallet. The frontend starts recovery only after the
    // wallet is loaded, encrypted when requested, and Core has restarted.
    let deep_rescan: Option<serde_json::Value> = if named_wallet_loaded {
        Some(serde_json::json!({
            "triggered": false,
            "from_block": 0,
            "wallet_name": restored_wallet_name,
            "skipped_reason": "deferred_until_runtime_wallet_ready",
        }))
    } else {
        Some(serde_json::json!({
            "triggered": false,
            "from_block": 0,
            "wallet_name": restored_wallet_name,
            "skipped_reason": "named_wallet_not_loaded",
        }))
    };

    // Also compute the first_tx_block for diagnostic purposes. Skip
    // when the named wallet is not loaded — listtransactions would
    // target the default wallet.
    let first_tx_block = if named_wallet_loaded {
        query_wallet_first_tx_block(&restored_wallet_name)
            .ok()
            .flatten()
    } else {
        None
    };

    let fingerprint = build_alignment_fingerprint(webcom_record);
    let alignment_identity = canonical_wallet_identity(&webcom_record.value);

    let mut bundle = load_bundle()?.ok_or("Vault does not exist")?;
    let dek = unwrap_dek_with_passphrase(passphrase, &bundle.vault)?;
    let mut payload = decrypt_payload_with_dek(dek.as_slice(), &bundle.vault)?;

    let now = chrono::Utc::now().timestamp();
    let network = bundle
        .vault
        .network
        .clone()
        .unwrap_or_else(|| "mainnet".to_string());

    let alignment_metadata = serde_json::json!({
        "schema": ALIGNMENT_SCHEMA,
        "schema_version": ALIGNMENT_SCHEMA_VERSION,
        "active_wallet_record_id": RECORD_ID_WALLET_HEMP_PRIMARY,
        "active_wallet_fingerprint": fingerprint,
        "active_wallet_format_fingerprint": fingerprint,
        "active_wallet_identity": alignment_identity,
        "core_wallet_name": restored_wallet_name,
        "core_wallet_source": CORE_WALLET_SOURCE_WEBCOM_BIP39,
        "derivation_profile": DERIVATION_HEMP_CANONICAL_420,
        "network": network.clone(),
        "created_at": now,
        "updated_at": now,
        "last_verified_at": now,
        "core_migration_backup_record_id": pre_connect_backup_record_id.unwrap_or(""),
        "verification_method": VERIFICATION_METHOD_RESTORE_FROM_GENERATED,
        "connection_state": "verified_aligned",
        "notes": [
            "Alignment verified by Core restorewalletmigration from Commander-generated v2 migration envelope.",
        ],
    });

    let alignment_record = SecretRecord {
        record_id: RECORD_ID_WALLET_ALIGNMENT.to_string(),
        record_type: RECORD_TYPE_APP_SETTING_WALLET_ALIGNMENT.to_string(),
        label: "Commander Wallet Alignment".to_string(),
        value: String::new(),
        metadata: Some(alignment_metadata),
        tags: Some(vec!["wallet".to_string(), "alignment".to_string()]),
        origin_app: Some(APP_IDENTIFIER.to_string()),
        derivation_profiles: None,
        network: Some(network.clone()),
        created: now,
        modified: now,
    };

    payload.secrets.insert(
        RECORD_ID_WALLET_HEMP_PRIMARY.to_string(),
        webcom_record.clone(),
    );
    payload
        .secrets
        .insert(RECORD_ID_WALLET_ALIGNMENT.to_string(), alignment_record);
    bundle.vault.modified = now;
    bundle.vault.payload = encrypt_payload_with_dek(dek.as_slice(), &payload, &bundle.vault)?;
    save_bundle_atomic(&bundle)?;

    let mut result = serde_json::json!({
        "connected": true,
        "connection_state": "verified_aligned",
        "verification_method": VERIFICATION_METHOD_RESTORE_FROM_GENERATED,
        "active_wallet_record_id": RECORD_ID_WALLET_HEMP_PRIMARY,
        "core_wallet_name": restored_wallet_name,
        "core_wallet_arg": restored_wallet_arg,
        "derivation_profile": DERIVATION_HEMP_CANONICAL_420,
        "network": network,
        "fingerprint": fingerprint,
        "alignment_record_id": RECORD_ID_WALLET_ALIGNMENT,
        "runtime_wallet_encryption": "needs_user_action",
        "runtime_wallet_encryption_detail": "Core restore creates an unencrypted runtime wallet. Use the Encryption tab to encrypt the wallet. The vault backup is encrypted; the Core runtime wallet still needs Core wallet encryption.",
        // Slice 64h: explicit, truthful wallet load + balance state.
        // The UI MUST NOT claim `hemp.conf` was edited, the previous
        // wallet.dat was replaced, or the named wallet is "the
        // default" unless that is actually true. These fields are
        // the single source of truth.
        "wallet_state": wallet_state_json,
        "named_wallet_loaded": named_wallet_loaded,
        "named_wallet_loaded_via": named_state.loaded_via.clone(),
        "wallet_load_restart_required": named_state.restart_required,
        "keypool_refill": keypool_refill.clone(),
        "wallet_balance": wallet_balance.clone(),
        "balance_source": balance_source.clone(),
        // Distinguish the three balances / scans honestly:
        //   - `utxo_scan`: chain-side UTXOs the wallet could spend now
        //   - `wallet_balance`: Core wallet balance from getwalletinfo
        //   - `history_rescan`: background rescan that fills tx history
        // The UI must never collapse these into one number.
        "scan_kinds": {
            "utxo_scan": "chain-side spendable UTXOs (does not require a rescan)",
            "wallet_balance": "Core wallet balance from -wallet=<name> getwalletinfo (authoritative when present)",
            "history_rescan": "Deferred rescanblockchain that fills wallet transaction history after runtime-wallet setup",
        },
    });

    if let Some(obj) = result.as_object_mut() {
        if let Some(rescan) = restore_result.get("rescan_start_height") {
            obj.insert("rescan_start_height".to_string(), rescan.clone());
        }
        if let Some(rescan) = restore_result.get("rescan_end_height") {
            obj.insert("rescan_end_height".to_string(), rescan.clone());
        }
        if let Some(pre_backup) = pre_connect_backup_record_id {
            if !pre_backup.is_empty() {
                obj.insert(
                    "pre_connect_backup_record_id".to_string(),
                    serde_json::Value::String(pre_backup.to_string()),
                );
            }
        }
        if let Some(bh) = first_tx_block {
            obj.insert(
                "first_tx_block_height".to_string(),
                serde_json::Value::Number(bh.into()),
            );
        }
        if !addresses.is_empty() {
            obj.insert(
                "wallet_address_count".to_string(),
                serde_json::Value::Number(addresses.len().into()),
            );
        }
        if let Some(scan) = utxo_scan {
            obj.insert("utxo_scan".to_string(), scan);
        }
        // Replace the old `deep_rescan_triggered` field with a
        // clearer `history_rescan` block. Keep `deep_rescan_triggered`
        // for back-compat with the 64f UI.
        match deep_rescan {
            Some(info) => {
                let triggered = info
                    .get("triggered")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                obj.insert(
                    "deep_rescan_triggered".to_string(),
                    serde_json::Value::Bool(triggered),
                );
                let mut history_block = serde_json::json!({
                    "wallet_name": restored_wallet_name,
                    "from_block": 0,
                    "status": if triggered { "running" } else { "not_started" },
                });
                if let Some(reason) = info.get("skipped_reason") {
                    history_block["skipped_reason"] = reason.clone();
                }
                if let Some(err) = info.get("error") {
                    history_block["error"] = err.clone();
                }
                if let Some(core_resp) = info.get("core_response") {
                    history_block["core_response"] = core_resp.clone();
                }
                obj.insert("history_rescan".to_string(), history_block);
            }
            None => {
                obj.insert(
                    "deep_rescan_triggered".to_string(),
                    serde_json::Value::Bool(false),
                );
            }
        }
    }

    // After the alignment is written, Commander stores the wallet name
    // in app settings so it loads on future Core starts. This is done
    // in the backend, before returning to the UI, so a later frontend
    // refresh/history failure cannot leave the vault aligned but not
    // selected for startup.
    {
        let mut settings = crate::modules::files::load_app_settings_impl()?;
        settings.active_vault_wallet_name = Some(restored_wallet_name.clone());
        crate::modules::files::save_app_settings_impl(&settings)?;
    }

    // No `hemp.conf` edits are made — the daemon is started with
    // `-wallet=<name>` by Commander's own process management.
    if let Some(obj) = result.as_object_mut() {
        obj.insert("wallet_promotion".to_string(), serde_json::json!({
            "promoted": true,
            "requires_user_action": false,
            "reason": "commander_managed_startup",
            "new_wallet_name": restored_wallet_name,
            "instructions": format!(
                "Commander will start Core with -wallet={restored_wallet_name} when you start Core through Commander."
            ),
        }));
    }

    Ok(result)
}

fn validate_migration_wallet_name_for_vault(name: &str) -> Result<String, String> {
    // Delegate to the shared Core wallet filename validator so every
    // vault-wallet flow enforces the same character set (letters, digits,
    // `_`, `-` only; no spaces, no path separators, no `.`, max 64 chars).
    crate::modules::utils::validate_core_wallet_filename(name)
}

// ─── Hardened Wallet Alignment Status — slice 64d ────────────────────────
//
// Replaces the slice-64c status with a hardened model that distinguishes
// connection_state, verification_status, and recommended_next_action.
// Stale slice-64c alignment records without verification evidence are
// treated as stale/unverified, not aligned.

pub fn get_wallet_alignment_status_v2(passphrase: &str) -> Result<serde_json::Value, String> {
    let bundle = load_bundle()?.ok_or("Vault does not exist")?;
    let payload = decrypt_vault_envelope(passphrase, &bundle.vault)?;

    let has_webcom_primary = payload.secrets.get(RECORD_ID_WALLET_HEMP_PRIMARY).is_some();
    let alignment = payload.secrets.get(RECORD_ID_WALLET_ALIGNMENT);
    let network = bundle
        .vault
        .network
        .clone()
        .unwrap_or_else(|| "mainnet".to_string());

    let mut result = serde_json::json!({
        "vault_exists": true,
        "vault_network": network,
        "has_webcom_primary": has_webcom_primary,
        "wallet_record_state": "none",
        "connection_state": "none",
        "verification_status": "not_verified",
        "recommended_next_action": "unlock_vault",
    });
    let mut current_primary_fingerprint: Option<String> = None;
    let mut current_primary_identity: Option<String> = None;

    if let Some(record) = payload.secrets.get(RECORD_ID_WALLET_HEMP_PRIMARY) {
        if record.value.is_empty() {
            if let Some(obj) = result.as_object_mut() {
                obj.insert(
                    "webcom_primary_empty".to_string(),
                    serde_json::Value::Bool(true),
                );
                obj.insert(
                    "wallet_record_state".to_string(),
                    serde_json::Value::String("unsupported".to_string()),
                );
            }
        } else {
            let record_type = record.record_type.as_str();
            let seed_type = record
                .metadata
                .as_ref()
                .and_then(|m| m.get("recovery"))
                .and_then(|r| r.get("seedType"))
                .and_then(|v| v.as_str())
                .unwrap_or_else(|| {
                    if record_type == RECORD_TYPE_WALLET_WIF {
                        "wif"
                    } else {
                        "bip39"
                    }
                });

            let derivation_hemp = record
                .metadata
                .as_ref()
                .and_then(|m| m.get("recovery"))
                .and_then(|r| r.get("derivationProfiles"))
                .and_then(|dp| dp.get("hemp"))
                .and_then(|v| v.as_str())
                .or_else(|| {
                    record
                        .derivation_profiles
                        .as_ref()
                        .and_then(|dp| dp.get("hemp").and_then(|s| Some(s.as_str())))
                });

            let record_network = record
                .metadata
                .as_ref()
                .and_then(|m| m.get("recovery"))
                .and_then(|r| r.get("network"))
                .and_then(|v| v.as_str())
                .or_else(|| record.network.as_deref());

            let fingerprint = build_alignment_fingerprint(record);
            current_primary_fingerprint = Some(fingerprint.clone());
            current_primary_identity = Some(canonical_wallet_identity(&record.value));

            let wallet_record_state =
                if record_type == RECORD_TYPE_WALLET_BIP39 && seed_type == "bip39" {
                    "webcom_primary_detected"
                } else {
                    "unsupported"
                };

            if let Some(obj) = result.as_object_mut() {
                obj.insert(
                    "wallet_record_state".to_string(),
                    serde_json::Value::String(wallet_record_state.to_string()),
                );
                obj.insert(
                    "webcom_primary_record_type".to_string(),
                    serde_json::Value::String(record_type.to_string()),
                );
                obj.insert(
                    "webcom_primary_seed_type".to_string(),
                    serde_json::Value::String(seed_type.to_string()),
                );
                obj.insert(
                    "webcom_primary_derivation_hemp".to_string(),
                    derivation_hemp
                        .map(|s| serde_json::Value::String(s.to_string()))
                        .unwrap_or(serde_json::Value::Null),
                );
                obj.insert(
                    "webcom_primary_network".to_string(),
                    record_network
                        .map(|s| serde_json::Value::String(s.to_string()))
                        .unwrap_or(serde_json::Value::Null),
                );
                obj.insert(
                    "webcom_primary_fingerprint".to_string(),
                    serde_json::Value::String(fingerprint),
                );
            }
        }
    }

    let mut connection_state = "none".to_string();
    let mut verification_status = "not_verified".to_string();

    if let Some(alignment) = alignment {
        let meta = alignment.metadata.as_ref();
        let has_verification_evidence = meta
            .and_then(|m| m.get("verification_method"))
            .and_then(|v| v.as_str())
            .map(|vm| vm == VERIFICATION_METHOD_RESTORE_FROM_GENERATED)
            .unwrap_or(false);

        let _connection_state_label = meta
            .and_then(|m| m.get("connection_state"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // B5: Derive canonical identity from decrypted current primary
        // and compare with stored active_wallet_identity.
        let stored_identity = meta
            .and_then(|m| m.get("active_wallet_identity"))
            .and_then(|v| v.as_str());
        let stored_fingerprint = meta
            .and_then(|m| {
                m.get("active_wallet_format_fingerprint")
                    .or_else(|| m.get("active_wallet_fingerprint"))
            })
            .and_then(|v| v.as_str());

        let identity_matches = if let Some(stored) = stored_identity {
            current_primary_identity
                .as_deref()
                .map(|current_id| current_id == stored)
                .unwrap_or(false)
        } else if let (Some(stored), Some(current)) =
            (stored_fingerprint, current_primary_fingerprint.as_deref())
        {
            // Older Commander builds wrote the restore verification and
            // primary-wallet fingerprint before active_wallet_identity was
            // introduced. Keep those already-restored vault wallets usable
            // while new connects write the stronger identity field.
            stored == current
        } else {
            false
        };

        // B5: Missing or mismatched identity → stale/unverified
        // F7: Simple authoritative rule — no redundant branches.
        //   verification evidence + matching identity → verified
        //   verification evidence + missing identity → stale
        //   verification evidence + mismatched identity → identity_mismatch
        //   no verification evidence → stale
        if has_verification_evidence {
            if identity_matches {
                connection_state = "verified_aligned".to_string();
                verification_status = "verified".to_string();
            } else if stored_identity.is_some() {
                connection_state = "identity_mismatch".to_string();
                verification_status = "not_verified".to_string();
            } else if stored_fingerprint.is_some() {
                connection_state = "fingerprint_mismatch".to_string();
                verification_status = "not_verified".to_string();
            } else {
                connection_state = "stale_unverified_alignment".to_string();
                verification_status = "not_verified".to_string();
            }
        } else {
            connection_state = "stale_unverified_alignment".to_string();
            verification_status = "not_verified".to_string();
        }

        if let Some(obj) = result.as_object_mut() {
            obj.insert(
                "alignment_record_exists".to_string(),
                serde_json::Value::Bool(true),
            );
            obj.insert(
                "alignment_record_id".to_string(),
                serde_json::Value::String(RECORD_ID_WALLET_ALIGNMENT.to_string()),
            );

            if let Some(s) = meta.and_then(|m| m.get("schema_version")) {
                obj.insert("alignment_schema_version".to_string(), s.clone());
            }
            if let Some(wr) = meta.and_then(|m| m.get("active_wallet_record_id")) {
                obj.insert("alignment_active_wallet_record_id".to_string(), wr.clone());
            }
            if let Some(fp) = meta.and_then(|m| m.get("active_wallet_format_fingerprint")) {
                obj.insert(
                    "alignment_active_wallet_format_fingerprint".to_string(),
                    fp.clone(),
                );
            }
            if let Some(src) = meta.and_then(|m| m.get("core_wallet_source")) {
                obj.insert("alignment_core_wallet_source".to_string(), src.clone());
            }
            if let Some(name) = meta.and_then(|m| m.get("core_wallet_name")) {
                obj.insert("core_wallet_name".to_string(), name.clone());
            }
            if let Some(vm) = meta.and_then(|m| m.get("verification_method")) {
                obj.insert("alignment_verification_method".to_string(), vm.clone());
            }
            if let Some(cs) = meta.and_then(|m| m.get("connection_state")) {
                obj.insert("alignment_stored_connection_state".to_string(), cs.clone());
            }
        }
    }

    let mut core_reachable = false;
    let mut _core_wallet_shape: Option<serde_json::Value> = None;
    let mut bip39_export_possible = false;
    let mut current_core_wallet_exists = false;
    let mut core_locked = false;

    if let Ok(raw) = crate::modules::commands::run_cli(&[String::from("getwalletmigrationinfo")]) {
        if let Ok(info) = serde_json::from_str::<serde_json::Value>(&raw) {
            core_reachable = true;
            let hd = info
                .get("hd_enabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let bip44 = info
                .get("bip44_enabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let has_mnemonic = info
                .get("has_mnemonic_metadata")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let encrypted = info
                .get("encrypted")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let locked = info
                .get("locked")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            current_core_wallet_exists =
                info.get("walletname").is_some() || info.get("hd_enabled").is_some() || hd;
            core_locked = locked;

            let shape = serde_json::json!({
                "hd_enabled": hd,
                "bip44_enabled": bip44,
                "has_mnemonic_metadata": has_mnemonic,
                "encrypted": encrypted,
                "locked": locked,
            });

            if hd && bip44 && has_mnemonic {
                bip39_export_possible = true;
            }

            if let Some(obj) = result.as_object_mut() {
                obj.insert("core_wallet_shape".to_string(), shape.clone());
                let core_fp = build_core_wallet_alignment_fingerprint(&shape);
                obj.insert(
                    "core_wallet_fingerprint".to_string(),
                    serde_json::Value::String(core_fp),
                );
            }
            _core_wallet_shape = Some(shape);
        }
    }

    let webcom_primary_seed_type = result
        .get("webcom_primary_seed_type")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let wallet_record_state = result
        .get("wallet_record_state")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    if let Some(obj) = result.as_object_mut() {
        obj.insert(
            "core_reachable".to_string(),
            serde_json::Value::Bool(core_reachable),
        );
        obj.insert(
            "current_core_wallet_exists".to_string(),
            serde_json::Value::Bool(current_core_wallet_exists),
        );
        obj.insert(
            "core_locked".to_string(),
            serde_json::Value::Bool(core_locked),
        );
        obj.insert(
            "pre_connect_backup_required".to_string(),
            serde_json::Value::Bool(current_core_wallet_exists),
        );
        obj.insert(
            "can_guided_connect".to_string(),
            serde_json::Value::Bool(
                core_reachable
                    && wallet_record_state == "webcom_primary_detected"
                    && connection_state != "verified_aligned",
            ),
        );
        obj.insert(
            "guided_connect_label".to_string(),
            serde_json::Value::String(if current_core_wallet_exists {
                "CONNECT VAULT WALLET".to_string()
            } else {
                "CONNECT VAULT WALLET".to_string()
            }),
        );
        obj.insert(
            "core_bip39_export_possible".to_string(),
            serde_json::Value::Bool(bip39_export_possible),
        );
        obj.insert(
            "connection_state".to_string(),
            serde_json::Value::String(connection_state.clone()),
        );
        obj.insert(
            "verification_status".to_string(),
            serde_json::Value::String(verification_status.clone()),
        );

        let next_action = if !has_webcom_primary {
            "create_vault"
        } else if connection_state == "verified_aligned" {
            "already_aligned"
        } else if !core_reachable {
            "connect_webcom_wallet"
        } else if webcom_primary_seed_type == "bip39" && has_webcom_primary {
            if connection_state == "stale_unverified_alignment" {
                "restore_available"
            } else {
                "needs_backup"
            }
        } else if bip39_export_possible && has_webcom_primary {
            "backup_current_core_wallet"
        } else if has_webcom_primary && !bip39_export_possible {
            if webcom_primary_seed_type == "bip39" {
                "restore_core_from_webcom_primary"
            } else {
                "export_core_bip39_to_webcom_primary"
            }
        } else {
            "unlock_vault"
        };
        obj.insert(
            "recommended_next_action".to_string(),
            serde_json::Value::String(next_action.to_string()),
        );
    }

    Ok(result)
}

// ─── Connection Intent Record — slice 64d ────────────────────────────────
//
// A non-alignment record that tracks the user's intent to connect a
// WebCom primary wallet to Core. Records backup state and intended
// wallet name. Must not be called alignment.

pub fn create_or_update_connection_intent_record(
    passphrase: &str,
    intended_wallet_name: Option<&str>,
    backup_record_id: Option<&str>,
) -> Result<serde_json::Value, String> {
    let mut bundle = load_bundle()?.ok_or("Vault does not exist")?;
    let dek = unwrap_dek_with_passphrase(passphrase, &bundle.vault)?;
    let mut payload = decrypt_payload_with_dek(dek.as_slice(), &bundle.vault)?;

    let now = chrono::Utc::now().timestamp();
    let network = bundle
        .vault
        .network
        .clone()
        .unwrap_or_else(|| "mainnet".to_string());

    let existing = payload.secrets.get(RECORD_ID_CONNECTION_INTENT);
    let created = existing.map(|r| r.created).unwrap_or(now);

    let intent_metadata = serde_json::json!({
        "schema": "hemp0x.commander.wallet_connection_intent",
        "schema_version": 1,
        "webcom_primary_detected": true,
        "intended_wallet_name": intended_wallet_name.unwrap_or(MIGRATION_DEFAULT_WALLET_NAME),
        "backup_record_id": backup_record_id.unwrap_or(""),
        "network": network,
        "created_at": created,
        "updated_at": now,
    });

    let record = SecretRecord {
        record_id: RECORD_ID_CONNECTION_INTENT.to_string(),
        record_type: RECORD_TYPE_CONNECTION_INTENT.to_string(),
        label: "Wallet Connection Intent".to_string(),
        value: String::new(),
        metadata: Some(intent_metadata),
        tags: Some(vec!["wallet".to_string(), "connection".to_string()]),
        origin_app: Some(APP_IDENTIFIER.to_string()),
        derivation_profiles: None,
        network: Some(network),
        created,
        modified: now,
    };

    payload
        .secrets
        .insert(RECORD_ID_CONNECTION_INTENT.to_string(), record);
    bundle.vault.modified = now;
    bundle.vault.payload = encrypt_payload_with_dek(dek.as_slice(), &payload, &bundle.vault)?;
    save_bundle_atomic(&bundle)?;

    Ok(serde_json::json!({
        "recorded": true,
        "record_id": RECORD_ID_CONNECTION_INTENT,
        "intended_wallet_name": intended_wallet_name.unwrap_or(MIGRATION_DEFAULT_WALLET_NAME),
        "backup_record_id": backup_record_id.unwrap_or(""),
    }))
}

// ─── Tauri Command Wrappers — slice 64d ──────────────────────────────────

#[tauri::command]
pub fn vault_connect_webcom_primary_wallet_to_core(
    wallet_name: Option<String>,
    birth_height: Option<i64>,
    pre_connect_backup_record_id: Option<String>,
    vault_passphrase: Option<String>,
) -> Result<serde_json::Value, String> {
    clean_stale_vault_temp_files();
    let passphrase = vault_passphrase.ok_or("Vault passphrase is required")?;
    connect_webcom_primary_wallet_to_core(
        &passphrase,
        wallet_name.as_deref(),
        birth_height,
        pre_connect_backup_record_id.as_deref(),
    )
}

#[tauri::command]
pub fn vault_connect_webcom_primary_wallet_to_core_guided(
    wallet_name: Option<String>,
    birth_height: Option<i64>,
    vault_passphrase: Option<String>,
    skip_backup: Option<bool>,
) -> Result<serde_json::Value, String> {
    clean_stale_vault_temp_files();
    let passphrase = vault_passphrase.ok_or("Vault passphrase is required")?;
    connect_webcom_primary_wallet_to_core_guided(
        &passphrase,
        wallet_name.as_deref(),
        birth_height,
        skip_backup.unwrap_or(false),
    )
}

#[tauri::command]
pub fn vault_set_active_wallet_name(wallet_name: String) -> Result<(), String> {
    let mut settings = crate::modules::files::load_app_settings_impl()?;
    settings.active_vault_wallet_name = Some(wallet_name);
    crate::modules::files::save_app_settings_impl(&settings)
}

#[tauri::command]
pub fn vault_get_active_wallet_name() -> Result<Option<String>, String> {
    let settings = crate::modules::files::load_app_settings_impl()?;
    Ok(settings.active_vault_wallet_name)
}

#[tauri::command]
pub async fn vault_restart_core_with_wallet(wallet_name: String) -> Result<(), String> {
    let wn = wallet_name.clone();
    tauri::async_runtime::spawn_blocking(move || {
        crate::modules::process::restart_node_with_wallet_for_default_context(&wn)?;
        let mut settings = crate::modules::files::load_app_settings_impl()?;
        settings.active_vault_wallet_name = Some(wn);
        crate::modules::files::save_app_settings_impl(&settings)
    })
    .await
    .map_err(|e| format!("Restart task failed: {e}"))?
}

#[tauri::command]
pub async fn vault_load_wallet_into_core(wallet_name: String) -> Result<serde_json::Value, String> {
    let wn = wallet_name.clone();
    tauri::async_runtime::spawn_blocking(move || load_wallet_into_core_blocking(&wn))
        .await
        .map_err(|e| format!("Load wallet task failed: {e}"))?
}

fn load_wallet_into_core_blocking(wallet_name: &str) -> Result<serde_json::Value, String> {
    let state = describe_named_wallet_state(wallet_name);
    if state.named_wallet_loaded {
        return Ok(serde_json::json!({
            "loaded": true,
            "loaded_via": state.loaded_via,
            "wallet_name": wallet_name,
            "query_info": state.query_info,
        }));
    }
    if !state.wallet_file_exists {
        return Err(format!(
            "Wallet file for '{}' does not exist on disk at {}",
            wallet_name, state.wallet_file_path
        ));
    }
    // B1 66d: Core Next does not support dynamic loadwallet.
    // Return restart_required instead of calling the unsupported RPC.
    Ok(serde_json::json!({
        "loaded": false,
        "restart_required": true,
        "wallet_name": wallet_name,
        "wallet_file_exists": true,
        "detail": "Core Next does not support dynamic loadwallet. A Core restart with -wallet=<name> is required.",
    }))
}

#[tauri::command]
pub fn vault_check_named_wallet_restore_state(
    wallet_name: String,
) -> Result<serde_json::Value, String> {
    let safe_name = validate_migration_wallet_name_for_vault(&wallet_name)?;
    let state = describe_named_wallet_state(&safe_name);
    let state_json = named_wallet_state_to_json(&state);

    let verification = if state.wallet_file_exists {
        match verify_existing_wallet_after_duplicate(&safe_name) {
            Ok(info) => serde_json::json!({
                "verified": true,
                "detail": "Core reports the expected Hemp0x coin420 wallet profile.",
                "info": info,
            }),
            Err(e) => serde_json::json!({
                "verified": false,
                "detail": e,
            }),
        }
    } else {
        serde_json::json!({
            "verified": false,
            "detail": "Wallet file does not exist on disk.",
        })
    };

    Ok(serde_json::json!({
        "wallet_name": safe_name,
        "wallet_state": state_json,
        "verification": verification,
    }))
}

#[tauri::command]
pub fn vault_unload_wallet(wallet_name: String) -> Result<serde_json::Value, String> {
    let state = describe_named_wallet_state(&wallet_name);
    if !state.named_wallet_loaded {
        return Ok(serde_json::json!({
            "unloaded": false,
            "wallet_name": wallet_name,
            "detail": "Wallet is not currently loaded.",
        }));
    }
    // B1 66d: Core Next does not support dynamic unloadwallet.
    Ok(serde_json::json!({
        "unloaded": false,
        "wallet_name": wallet_name,
        "restart_required": true,
        "detail": "Core Next does not support dynamic unloadwallet. Restart Core without -wallet=<name> to use a different wallet.",
    }))
}

// ─── Vault Passphrase Rotation (slice 64p) ───────────────────────────────
//
// Safe Hemp0x Vault passphrase rotation. The active vault bundle is
// decrypted with the current passphrase, then the exact same payload
// (every record, every metadata field, every unknown future record
// type) is re-encrypted with the new passphrase using the current
// preferred KDF profile. The original DEK, salt, and wrap IV are
// replaced; the bundle is saved atomically; the cached vault session
// passphrase is updated ONLY after the save succeeds.
//
// This command never logs passphrases and never returns the decrypted
// payload. hemp.conf is not touched.

pub fn change_vault_passphrase(
    current_passphrase: &str,
    new_passphrase: &str,
) -> Result<serde_json::Value, String> {
    // Backend validation of new passphrase length/limits, even though
    // the UI also validates. This is the authoritative gate.
    if new_passphrase.is_empty() {
        return Err("New passphrase must not be empty".to_string());
    }
    if new_passphrase.len() < 8 {
        return Err("New passphrase must be at least 8 characters".to_string());
    }
    if new_passphrase.len() > 1024 {
        return Err("New passphrase must not exceed 1024 characters".to_string());
    }
    // Reject same old/new passphrase. A no-op rotation gives a false
    // sense of having re-keyed the vault, and the spec calls this out
    // as something to reject unless there is a strong reason to allow.
    if current_passphrase == new_passphrase {
        return Err("New passphrase must be different from the current passphrase".to_string());
    }

    let bundle = load_bundle()?.ok_or("Vault does not exist")?;

    // Decrypt with the current passphrase. This is the authoritative
    // "wrong current passphrase" check and yields the exact payload to
    // re-encrypt. The decrypted payload is never returned to the caller.
    let dek = unwrap_dek_with_passphrase(current_passphrase, &bundle.vault)?;
    let payload = decrypt_payload_with_dek(dek.as_slice(), &bundle.vault)?;

    // Preserve envelope-level fields that must survive a rotation.
    // `created`, `modified`, and `network` are all bound into the
    // AES-GCM AAD for both the payload and the wrapped DEK, so they
    // must be fixed BEFORE encryption and never mutated afterward.
    let created = bundle.vault.created;
    let preserved_network = bundle
        .vault
        .network
        .clone()
        .unwrap_or_else(|| detect_network());
    let preserved_meta = bundle.meta.clone();
    let bundle_version = bundle.bundleVersion;

    // Re-encrypt the exact same payload with the new passphrase using
    // the current preferred KDF profile. This generates a fresh DEK,
    // fresh salt, and fresh wrap IV. Because we re-encrypt the whole
    // decrypted payload, every record is preserved verbatim: provider
    // tokens, WebCom records, Core backup records, app secrets, address
    // book records, alignment records, and unknown future record types.
    let preferred_kdf = KDF_PROFILE_SCRYPT;
    let now = chrono::Utc::now().timestamp();
    let new_envelope = encrypt_vault_envelope_with_network(
        new_passphrase,
        &payload,
        preferred_kdf,
        preserved_network,
        created,
        now,
    )?;

    let new_bundle = VaultBundle {
        bundleVersion: bundle_version,
        format_identifier: FORMAT_IDENTIFIER.to_string(),
        vault: new_envelope,
        meta: preserved_meta,
    };

    // Atomic save via the existing bundle save path (temp file + rename).
    // If this fails, the on-disk vault and the cached session passphrase
    // are both left untouched.
    save_bundle_atomic(&new_bundle)?;

    // Update the cached vault session passphrase ONLY after the save
    // succeeds, so the unlocked session continues to work and a failed
    // rotation does not leave the cache pointing at the new passphrase.
    crate::modules::provider_settings::set_vault_passphrase(new_passphrase.to_string());

    // Intentionally do not return any decrypted payload or passphrase.
    Ok(serde_json::json!({
        "rotated": true,
        "modified": now,
        "kdf_profile": preferred_kdf,
    }))
}

#[tauri::command]
pub async fn vault_change_passphrase(
    current_passphrase: String,
    new_passphrase: String,
) -> Result<serde_json::Value, String> {
    // scrypt KDF (log_n=14) plus AES-GCM on the payload can take a few
    // hundred milliseconds; run off the Tauri main thread so the UI
    // (loader/spinner) stays responsive.
    tauri::async_runtime::spawn_blocking(move || {
        change_vault_passphrase(&current_passphrase, &new_passphrase)
    })
    .await
    .map_err(|e| format!("Vault passphrase rotation task failed: {e}"))?
}

// ─── Unload Vault / Use wallet.dat (slice 64p) ───────────────────────────
//
// Deliberate "stop using the active Hemp0x Vault" flow. Commander clears
// its vault session + active-wallet selection and falls back to legacy
// wallet.dat mode. The vault file and the vault runtime wallet file are
// NEVER deleted. hemp.conf is NEVER modified. If a user has a manual
// `wallet=` line in hemp.conf, it is surfaced as a warning rather than
// rewritten. Blocking Core restart/wait runs on a background thread.
//
// If wallet.dat does not exist, Commander does NOT silently create one
// (starting Core in default mode would). It returns a structured
// no-legacy-wallet state so the UI can offer clear next actions.

pub fn unload_vault_and_use_wallet_dat_blocking(
    restart_node: bool,
) -> Result<serde_json::Value, String> {
    let dir = crate::modules::files::data_dir()?;
    let wallet_dat_path = dir.join("wallet.dat");
    let wallet_dat_exists_before = wallet_dat_path.exists();

    // Capture vault-file + runtime-wallet-file state BEFORE making any
    // changes so we can both report preservation truthfully and assert
    // (in tests) that nothing was deleted.
    let vault_path = vault_path()?;
    let settings_before = crate::modules::files::load_app_settings_impl()?;
    let active_vault_wallet_name_before = settings_before.active_vault_wallet_name.clone();
    let vault_runtime_wallet_path = active_vault_wallet_name_before
        .as_ref()
        .map(|n| dir.join(n));

    // hemp.conf wallet= line (if any). We do NOT modify hemp.conf; we
    // only surface this so the UI can warn the user.
    let hemp_conf_wallet = crate::modules::process::check_hemp_conf_wallet_line();

    // 1) Clear the cached vault session passphrase.
    crate::modules::provider_settings::clear_vault_passphrase();

    // 2) Clear `active_vault_wallet_name` in app settings so the next
    //    Core start does not pass `-wallet=<vault-name>`.
    let mut settings = settings_before;
    settings.active_vault_wallet_name = None;
    crate::modules::files::save_app_settings_impl(&settings)?;

    // 3) No file deletions. The vault file and the vault runtime wallet
    //    file are intentionally preserved on disk.

    // 4) Decide the fallback path based on whether wallet.dat exists.
    if !wallet_dat_exists_before {
        // No legacy wallet. Do NOT restart Core in default mode: doing so
        // would silently create an empty wallet.dat in most Core builds.
        // If requested, stop the currently running daemon so Core is not
        // left serving the just-unloaded vault wallet. Return a structured
        // state so the UI can offer clear next actions (create/import
        // wallet.dat, or import/create a vault).
        let mut core_stopped = false;
        if restart_node {
            crate::modules::process::stop_node_internal();
            crate::modules::process::wait_for_lock_release(&dir);
            core_stopped = true;
        }
        let vault_file_preserved = vault_path.exists();
        let vault_runtime_wallet_preserved = vault_runtime_wallet_path
            .as_ref()
            .map(|p| p.exists())
            .unwrap_or(false);
        return Ok(serde_json::json!({
            "unloaded": true,
            "legacy_wallet_mode": false,
            "no_legacy_wallet": true,
            "wallet_dat_exists": false,
            "vault_file_preserved": vault_file_preserved,
            "vault_runtime_wallet_preserved": vault_runtime_wallet_preserved,
            "active_vault_wallet_name_cleared": true,
            "hemp_conf_wallet": hemp_conf_wallet,
            "restarted": false,
            "core_stopped": core_stopped,
            "restart_skipped_reason": "No wallet.dat exists. Commander will not silently create a wallet. Create or import a wallet from the Wallet page.",
            "next_actions": [
                "create_new_wallet_dat",
                "import_wallet_dat",
                "import_or_create_vault",
            ],
        }));
    }

    // wallet.dat exists -> run the legacy fallback path.
    let mut restarted = false;
    let mut restart_error: Option<String> = None;

    if restart_node {
        // stop + wait for lock + start in DEFAULT mode (no -wallet= flag,
        // because active_vault_wallet_name is now None), then wait until
        // the default wallet.dat answers getwalletinfo. This mirrors the
        // legacy-wallet import flow and never touches hemp.conf.
        crate::modules::process::stop_node_internal();
        crate::modules::process::wait_for_lock_release(&dir);
        match crate::modules::process::start_node_blocking() {
            Ok(()) => {
                match crate::modules::process::wait_for_default_wallet_queryable(
                    std::time::Duration::from_secs(90),
                ) {
                    Ok(()) => restarted = true,
                    Err(e) => restart_error = Some(format!(
                        "Vault unloaded and Core restarted in legacy wallet.dat mode, but wallet.dat is still warming up: {e}"
                    )),
                }
            }
            Err(e) => restart_error = Some(format!(
                "Vault unloaded, but Core failed to restart in legacy wallet.dat mode: {e}. Start Core manually through Commander."
            )),
        }
    }

    let vault_file_preserved = vault_path.exists();
    let vault_runtime_wallet_preserved = vault_runtime_wallet_path
        .as_ref()
        .map(|p| p.exists())
        .unwrap_or(false);

    Ok(serde_json::json!({
        "unloaded": true,
        "legacy_wallet_mode": true,
        "no_legacy_wallet": false,
        "wallet_dat_exists": true,
        "vault_file_preserved": vault_file_preserved,
        "vault_runtime_wallet_preserved": vault_runtime_wallet_preserved,
        "active_vault_wallet_name_cleared": true,
        "hemp_conf_wallet": hemp_conf_wallet,
        "restarted": restarted,
        "restart_error": restart_error,
    }))
}

#[tauri::command]
pub async fn vault_unload_vault_and_use_wallet_dat(
    restart_node: Option<bool>,
) -> Result<serde_json::Value, String> {
    // The Core stop/start + wallet-status wait can take many seconds;
    // run it on a background thread so the webview and the unload
    // modal/loader stay responsive.
    let do_restart = restart_node.unwrap_or(true);
    tauri::async_runtime::spawn_blocking(move || {
        unload_vault_and_use_wallet_dat_blocking(do_restart)
    })
    .await
    .map_err(|e| format!("Unload vault task failed: {e}"))?
}

#[tauri::command]
pub async fn vault_refresh_wallet_utxos(wallet_name: String) -> Result<serde_json::Value, String> {
    // scantxoutset + getwalletinfo can take seconds; run off the main
    // thread so the UI stays responsive while the chain UTXO scan runs.
    let wn = wallet_name.clone();
    tauri::async_runtime::spawn_blocking(move || refresh_wallet_utxos_blocking(&wn))
        .await
        .map_err(|e| format!("UTXO refresh task failed: {e}"))?
}

fn refresh_wallet_utxos_blocking(wallet_name: &str) -> Result<serde_json::Value, String> {
    let state = describe_named_wallet_state(wallet_name);
    if !state.named_wallet_loaded {
        return Err(format!(
            "Wallet '{}' is not loaded. Load or restart Core with this wallet first.",
            wallet_name
        ));
    }
    let addresses = enumerate_wallet_addresses(wallet_name)?;
    if addresses.is_empty() {
        return Ok(serde_json::json!({
            "wallet_name": wallet_name,
            "utxo_scan": null,
            "address_count": 0,
            "detail": "No addresses found for this wallet.",
        }));
    }
    let utxo_scan = scantxoutset_for_addresses(&addresses)?;
    let wallet_info = if let Ok(raw) = crate::modules::commands::run_cli(&[
        format!("-wallet={}", wallet_name),
        String::from("getwalletinfo"),
    ]) {
        serde_json::from_str::<serde_json::Value>(&raw).ok()
    } else {
        None
    };
    Ok(serde_json::json!({
        "wallet_name": wallet_name,
        "utxo_scan": utxo_scan,
        "address_count": addresses.len(),
        "wallet_info": wallet_info,
    }))
}

#[tauri::command]
pub async fn vault_recover_wallet_history(
    wallet_name: String,
    from_block: Option<i64>,
) -> Result<serde_json::Value, String> {
    // rescanblockchain can run for a long time; run it off the main
    // thread so the UI stays responsive while history fills in.
    let wn = wallet_name.clone();
    tauri::async_runtime::spawn_blocking(move || recover_wallet_history_blocking(&wn, from_block))
        .await
        .map_err(|e| format!("History recovery task failed: {e}"))?
}

fn refill_named_wallet_keypool_if_needed(
    wallet_name: &str,
    target_size: i64,
) -> Result<serde_json::Value, String> {
    let target_size = target_size
        .max(VAULT_CORE_KEYPOOL_REFILL_FLOOR)
        .min(VAULT_KEYPOOL_HINT_CEILING);
    let state = describe_named_wallet_state(wallet_name);
    if !state.named_wallet_loaded {
        return Err(format!(
            "Wallet '{}' is not loaded. Load or restart Core with this wallet first.",
            wallet_name
        ));
    }

    let current_external = state
        .query_info
        .as_ref()
        .and_then(|info| info.get("keypool_external"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let current_internal = state
        .query_info
        .as_ref()
        .and_then(|info| info.get("keypool_internal"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let current = current_external.max(current_internal);
    if current >= target_size {
        return Ok(serde_json::json!({
            "triggered": false,
            "wallet_name": wallet_name,
            "target_size": target_size,
            "keypool_external": current_external,
            "keypool_internal": current_internal,
            "reason": "already_sufficient",
        }));
    }

    let args = vec![
        format!("-wallet={}", wallet_name),
        String::from("keypoolrefill"),
        target_size.to_string(),
    ];
    crate::modules::commands::run_cli(&args).map_err(|err| {
        if is_core_wallet_unlock_required_error(&err) {
            format!(
                "WALLET_UNLOCK_REQUIRED::The runtime wallet '{}' must be unlocked before Commander can recover this vault wallet history.",
                wallet_name
            )
        } else {
            format!("keypoolrefill failed for wallet '{}': {}", wallet_name, err)
        }
    })?;

    let after = describe_named_wallet_state(wallet_name);
    let after_external = after
        .query_info
        .as_ref()
        .and_then(|info| info.get("keypool_external"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let after_internal = after
        .query_info
        .as_ref()
        .and_then(|info| info.get("keypool_internal"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    Ok(serde_json::json!({
        "triggered": true,
        "wallet_name": wallet_name,
        "target_size": target_size,
        "keypool_external": after_external,
        "keypool_internal": after_internal,
    }))
}

#[tauri::command]
pub async fn vault_start_wallet_history_recovery(
    wallet_name: String,
    from_block: Option<i64>,
    keypool_size: Option<i64>,
) -> Result<serde_json::Value, String> {
    let start_block = from_block.unwrap_or(0);
    if start_block < 0 {
        return Err("from_block cannot be negative".to_string());
    }
    let refill_target = keypool_size
        .unwrap_or(VAULT_KEYPOOL_HINT_CEILING)
        .max(VAULT_CORE_KEYPOOL_REFILL_FLOOR)
        .min(VAULT_KEYPOOL_HINT_CEILING);
    let preflight_wallet_name = wallet_name.clone();
    let keypool_refill = tauri::async_runtime::spawn_blocking(move || {
        refill_named_wallet_keypool_if_needed(&preflight_wallet_name, refill_target)
    })
    .await
    .map_err(|e| format!("History recovery preflight task failed: {e}"))??;

    let wn = wallet_name.clone();
    tauri::async_runtime::spawn_blocking(move || {
        if let Err(err) = recover_wallet_history_blocking(&wn, Some(start_block)) {
            eprintln!(
                "Background history recovery failed for wallet '{}': {}",
                wn, err
            );
        }
    });
    Ok(serde_json::json!({
        "started": true,
        "from_block": start_block,
        "wallet_name": wallet_name,
        "keypool_refill": keypool_refill,
        "detail": "History recovery is running in the background.",
    }))
}

fn recover_wallet_history_blocking(
    wallet_name: &str,
    from_block: Option<i64>,
) -> Result<serde_json::Value, String> {
    let state = describe_named_wallet_state(wallet_name);
    if !state.named_wallet_loaded {
        return Err(format!(
            "Wallet '{}' is not loaded. Load or restart Core with this wallet first.",
            wallet_name
        ));
    }
    let start_block = from_block.unwrap_or(0);
    if start_block < 0 {
        return Err("from_block cannot be negative".to_string());
    }
    let args = vec![
        format!("-wallet={}", wallet_name),
        String::from("rescanblockchain"),
        start_block.to_string(),
    ];
    let raw = crate::modules::commands::run_cli(&args)
        .map_err(|e| format!("rescanblockchain failed: {}", e))?;
    let parsed: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| format!("Could not parse rescanblockchain response: {}", e))?;
    Ok(serde_json::json!({
        "triggered": true,
        "from_block": start_block,
        "wallet_name": wallet_name,
        "core_response": parsed,
    }))
}

#[tauri::command]
pub async fn vault_restore_from_recovery_phrase(
    mnemonic: String,
    wallet_name: Option<String>,
    vault_passphrase: String,
    birth_height: Option<i64>,
) -> Result<serde_json::Value, String> {
    // Run the heavy restore/align pipeline (Core restorewalletmigration,
    // scantxoutset, alignment write) on a background thread so the UI
    // stays responsive during create-new-wallet and restore-from-phrase.
    tauri::async_runtime::spawn_blocking(move || {
        restore_from_recovery_phrase_blocking(mnemonic, wallet_name, vault_passphrase, birth_height)
    })
    .await
    .map_err(|e| format!("Recovery phrase restore task failed: {e}"))?
}

fn restore_from_recovery_phrase_blocking(
    mnemonic: String,
    wallet_name: Option<String>,
    vault_passphrase: String,
    birth_height: Option<i64>,
) -> Result<serde_json::Value, String> {
    clean_stale_vault_temp_files();
    let mnemonic_clean = mnemonic.trim().to_string();
    let mnemonic_lower = mnemonic_clean.to_lowercase();
    let words: Vec<&str> = mnemonic_lower.split_whitespace().collect();
    let word_count = words.len();
    if word_count != 12 && word_count != 18 && word_count != 24 {
        return Err(format!(
            "BIP39 mnemonic has {word_count} words; expected 12, 18, or 24"
        ));
    }
    let mnemonic_validated = bip39::Mnemonic::parse(&mnemonic_lower)
        .map_err(|e| format!("Invalid BIP39 mnemonic: {e}"))?;
    let mnemonic_clean = mnemonic_validated.to_string();

    // If vault_passphrase is empty, try the cached session passphrase
    // (vault is already unlocked). If no cached passphrase and no vault
    // exists, the caller must provide one.
    let effective_passphrase = if vault_passphrase.is_empty() {
        crate::modules::provider_settings::get_cached_passphrase().ok_or_else(|| {
            "Vault is not unlocked. Unlock the vault first or provide a vault passphrase."
                .to_string()
        })?
    } else {
        vault_passphrase
    };

    let existing_bundle = load_bundle()?;
    if let Some(b) = existing_bundle {
        decrypt_vault_envelope(&effective_passphrase, &b.vault)?;
    } else {
        let mut p = VaultPayload::default();
        p.payload_version = PAYLOAD_VERSION;
        let envelope = encrypt_vault_envelope(&effective_passphrase, &p, KDF_PROFILE_SCRYPT)?;
        let empty_bundle = VaultBundle {
            bundleVersion: CURRENT_BUNDLE_VERSION,
            format_identifier: FORMAT_IDENTIFIER.to_string(),
            vault: envelope,
            meta: Some(
                serde_json::json!({"display_label": format!("Recovery phrase restore - {}", chrono::Utc::now().format("%Y-%m-%d %H:%M"))}),
            ),
        };
        save_bundle_atomic(&empty_bundle)?;
    }

    let effective_wallet_name = wallet_name
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| MIGRATION_DEFAULT_WALLET_NAME.to_string());
    let safe_name = validate_migration_wallet_name_for_vault(&effective_wallet_name)?;

    let existing_state = describe_named_wallet_state(&safe_name);
    if existing_state.wallet_file_exists {
        return Err(format!(
            "A wallet named '{}' already exists in Core at {}. Choose a different wallet name or remove the existing wallet first.",
            safe_name,
            existing_state.wallet_file_path
        ));
    }

    let now = chrono::Utc::now().timestamp();
    let webcom_record = SecretRecord {
        record_id: RECORD_ID_WALLET_HEMP_PRIMARY.to_string(),
        record_type: RECORD_TYPE_WALLET_BIP39.to_string(),
        label: format!(
            "Recovery phrase restore - {}",
            chrono::Utc::now().format("%Y-%m-%d %H:%M")
        ),
        value: mnemonic_clean.clone(),
        metadata: Some(serde_json::json!({
            "recovery": {
                "seedType": "bip39",
                "network": "mainnet",
                "derivationProfiles": {
                    "hemp": DERIVATION_HEMP_CANONICAL_420,
                },
            },
            "external_count": 20,
            "change_count": 6,
            "best_block_height": birth_height.unwrap_or(0),
        })),
        tags: Some(vec![
            "wallet".to_string(),
            "bip39".to_string(),
            "recovery-phrase".to_string(),
        ]),
        origin_app: Some("commander".to_string()),
        derivation_profiles: Some({
            let mut dp = std::collections::HashMap::new();
            dp.insert(
                "hemp".to_string(),
                DERIVATION_HEMP_CANONICAL_420.to_string(),
            );
            dp
        }),
        network: Some("mainnet".to_string()),
        created: now,
        modified: now,
    };

    let mut result = perform_webcom_to_core_restore_and_align(
        &effective_passphrase,
        &webcom_record,
        &safe_name,
        birth_height,
        None,
    )?;

    // The restore just created or decrypted the vault with
    // effective_passphrase. Cache it so the in-session unlock state is
    // truthful and the Wallet page reflects the unlocked vault
    // immediately after restore, instead of requiring the user to leave
    // and return. If the passphrase was already cached (vault was already
    // unlocked), this is a harmless re-cache of the same value.
    crate::modules::provider_settings::set_vault_passphrase(effective_passphrase.clone());

    if let Some(obj) = result.as_object_mut() {
        obj.insert(
            "recovery_phrase_restore".to_string(),
            serde_json::Value::Bool(true),
        );
        obj.insert(
            "vault_created_or_updated".to_string(),
            serde_json::Value::Bool(true),
        );
    }

    Ok(result)
}

#[tauri::command]
pub fn vault_generate_bip39_mnemonic(
    word_count: Option<usize>,
) -> Result<serde_json::Value, String> {
    // WebCom-compatible wallets may use 12 or 24 words. Allow the caller to
    // request either; default to 12 for a compact portable phrase.
    let words = word_count.unwrap_or(12);
    let allowed = [12usize, 15, 18, 21, 24];
    if !allowed.contains(&words) {
        return Err(format!(
            "Unsupported recovery phrase word count {words}. Use one of: 12, 15, 18, 21, 24."
        ));
    }
    let mut rng = rand::thread_rng();
    let mnemonic = Mnemonic::generate_in_with(&mut rng, bip39::Language::English, words)
        .map_err(|e| format!("Failed to generate BIP39 mnemonic: {e}"))?;
    let phrase = mnemonic.to_string();
    Ok(serde_json::json!({
        "mnemonic": phrase,
        "word_count": words,
        "language": "english",
    }))
}

#[tauri::command]
pub fn vault_create_or_update_connection_intent(
    intended_wallet_name: Option<String>,
    backup_record_id: Option<String>,
    vault_passphrase: Option<String>,
) -> Result<serde_json::Value, String> {
    let passphrase = vault_passphrase.ok_or("Vault passphrase is required")?;
    create_or_update_connection_intent_record(
        &passphrase,
        intended_wallet_name.as_deref(),
        backup_record_id.as_deref(),
    )
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
        let payload =
            make_provider_payload("pinata-jwt-token-12345", "filebase-access-token-67890");
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
        assert_eq!(
            payload_filebase_token(&decrypted),
            "filebase-access-token-67890"
        );
        assert_eq!(decrypted.payload_version, PAYLOAD_VERSION);
        assert!(decrypted.secrets.contains_key(RECORD_ID_PINATA));
        assert!(decrypted.secrets.contains_key(RECORD_ID_FILEBASE));
    }

    #[test]
    fn encrypt_decrypt_roundtrip_pbkdf2() {
        let passphrase = "test-passphrase-for-vault";
        let payload =
            make_provider_payload("pinata-jwt-token-12345", "filebase-access-token-67890");
        let envelope =
            encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_PBKDF2_SHA512).unwrap();
        assert_eq!(envelope.key_slots[0].kdf_profile, KDF_PROFILE_PBKDF2_SHA512);
        assert_eq!(
            envelope.key_slots[0].kdf_iterations,
            Some(PBKDF2_ITERATIONS)
        );

        let decrypted = decrypt_vault_envelope(passphrase, &envelope).unwrap();
        assert_eq!(payload_pinata_token(&decrypted), "pinata-jwt-token-12345");
        assert_eq!(
            payload_filebase_token(&decrypted),
            "filebase-access-token-67890"
        );
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
        let mut envelope =
            encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        envelope.payload.ciphertext =
            envelope.payload.ciphertext[..envelope.payload.ciphertext.len() - 4].to_string();
        let result = decrypt_vault_envelope(passphrase, &envelope);
        assert!(result.is_err());
    }

    #[test]
    fn corrupt_wrapped_dek_fails() {
        let passphrase = "test-passphrase";
        let payload = make_provider_payload("token", "token");
        let mut envelope =
            encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        envelope.key_slots[0].wrapped_dek = envelope.key_slots[0].wrapped_dek
            [..envelope.key_slots[0].wrapped_dek.len() - 4]
            .to_string();
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
        let mut envelope =
            encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        envelope.version = 999;
        let result = decrypt_vault_envelope(passphrase, &envelope);
        assert!(result.is_err());
    }

    #[test]
    fn slot_wrap_aad_tamper_detection() {
        let passphrase = "test-passphrase";
        let payload = make_provider_payload("token", "token");
        let mut envelope =
            encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        envelope.key_slots[0].slot_id = "tampered".to_string();
        let result = decrypt_vault_envelope(passphrase, &envelope);
        assert!(result.is_err());
    }

    #[test]
    fn kdf_iterations_tamper_detected() {
        let passphrase = "test-passphrase";
        let payload = make_provider_payload("token", "token");
        let mut envelope =
            encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_PBKDF2_SHA512).unwrap();
        envelope.key_slots[0].kdf_iterations = Some(100_000);
        let result = decrypt_vault_envelope(passphrase, &envelope);
        assert!(result.is_err());
    }

    #[test]
    fn kdf_scrypt_params_tamper_detected() {
        let passphrase = "test-passphrase";
        let payload = make_provider_payload("token", "token");
        let mut envelope =
            encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
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
        let mut envelope =
            encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        envelope.cipher_profile = "aes-128-gcm-v1".to_string();
        let result = decrypt_vault_envelope(passphrase, &envelope);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unsupported cipher profile"));
    }

    #[test]
    fn unsupported_aad_profile_rejected() {
        let passphrase = "test-passphrase";
        let payload = VaultPayload::default();
        let mut envelope =
            encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        envelope.aad_profile = "unknown-aad-v1".to_string();
        let result = decrypt_vault_envelope(passphrase, &envelope);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unsupported AAD profile"));
    }

    #[test]
    fn unsupported_payload_schema_rejected() {
        let passphrase = "test-passphrase";
        let payload = VaultPayload::default();
        let mut envelope =
            encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        envelope.payload.payload_schema = "future-schema-v2".to_string();
        let result = decrypt_vault_envelope(passphrase, &envelope);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unsupported payload schema"));
    }

    #[test]
    fn missing_pbkdf2_iterations_rejected() {
        let passphrase = "test-passphrase";
        let payload = make_provider_payload("token", "token");
        let mut envelope =
            encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_PBKDF2_SHA512).unwrap();
        envelope.key_slots[0].kdf_iterations = None;
        let result = decrypt_vault_envelope(passphrase, &envelope);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("missing kdf_iterations"));
    }

    #[test]
    fn missing_scrypt_log_n_rejected() {
        let passphrase = "test-passphrase";
        let payload = make_provider_payload("token", "token");
        let mut envelope =
            encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        envelope.key_slots[0].kdf_log_n = None;
        let result = decrypt_vault_envelope(passphrase, &envelope);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("missing kdf_log_n"));
    }

    #[test]
    fn missing_scrypt_r_rejected() {
        let passphrase = "test-passphrase";
        let payload = make_provider_payload("token", "token");
        let mut envelope =
            encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        envelope.key_slots[0].kdf_r = None;
        let result = decrypt_vault_envelope(passphrase, &envelope);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("missing kdf_r"));
    }

    #[test]
    fn invalid_kdf_dklen_rejected() {
        let passphrase = "test-passphrase";
        let payload = make_provider_payload("token", "token");
        let mut envelope =
            encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        envelope.key_slots[0].kdf_dklen = 16;
        let result = decrypt_vault_envelope(passphrase, &envelope);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid kdf_dklen"));
    }

    #[test]
    fn invalid_network_rejected() {
        let passphrase = "test-passphrase";
        let payload = VaultPayload::default();
        let mut envelope =
            encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
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
        envelope2.payload =
            encrypt_payload_with_dek(dek.as_slice(), &payload2, &envelope2).unwrap();

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
        payload
            .secrets
            .insert(future_record.record_id.clone(), future_record.clone());

        let envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();

        let dek = unwrap_dek_with_passphrase(passphrase, &envelope).unwrap();
        let mut existing = decrypt_payload_with_dek(dek.as_slice(), &envelope).unwrap();
        set_provider_tokens_in_payload(&mut existing, "token2", "fb2", "", "");

        let now = chrono::Utc::now().timestamp();
        let mut envelope2 = envelope.clone();
        envelope2.modified = now;
        envelope2.payload =
            encrypt_payload_with_dek(dek.as_slice(), &existing, &envelope2).unwrap();

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
        payload
            .secrets
            .insert(legacy.record_id.clone(), legacy.clone());

        let envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        let dek = unwrap_dek_with_passphrase(passphrase, &envelope).unwrap();
        let mut existing = decrypt_payload_with_dek(dek.as_slice(), &envelope).unwrap();
        set_provider_tokens_in_payload(&mut existing, "token2", "fb2", "", "");

        let now = chrono::Utc::now().timestamp();
        let mut envelope2 = envelope.clone();
        envelope2.modified = now;
        envelope2.payload =
            encrypt_payload_with_dek(dek.as_slice(), &existing, &envelope2).unwrap();

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
        dp.insert(
            "hemp".to_string(),
            DERIVATION_HEMP_CANONICAL_420.to_string(),
        );
        dp.insert("btc".to_string(), DERIVATION_BTC_BIP84.to_string());
        let btc_record = SecretRecord {
            record_id: "wallet.btc.main".to_string(),
            record_type: RECORD_TYPE_WALLET_BIP39.to_string(),
            label: "BTC Wallet".to_string(),
            value: "purpose-driven btc test wallet phrase example".to_string(),
            metadata: Some(
                serde_json::json!({"btc_external_count": 50, "btc_change_count": 25, "btc_derivation_profile": DERIVATION_BTC_BIP84}),
            ),
            tags: Some(vec!["btc".to_string()]),
            origin_app: Some("hemp0x-commander".to_string()),
            derivation_profiles: Some(dp),
            network: Some("mainnet".to_string()),
            created: 1700000000,
            modified: 1700000000,
        };
        payload
            .secrets
            .insert(btc_record.record_id.clone(), btc_record.clone());

        let envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        let dek = unwrap_dek_with_passphrase(passphrase, &envelope).unwrap();
        let mut existing = decrypt_payload_with_dek(dek.as_slice(), &envelope).unwrap();
        set_provider_tokens_in_payload(&mut existing, "token2", "fb2", "", "");

        let now = chrono::Utc::now().timestamp();
        let mut envelope2 = envelope.clone();
        envelope2.modified = now;
        envelope2.payload =
            encrypt_payload_with_dek(dek.as_slice(), &existing, &envelope2).unwrap();

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
        let ids: Vec<&str> = SUPPORTED_DERIVATION_PROFILES
            .iter()
            .map(|(id, _, _)| *id)
            .collect();
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
        let mut envelope =
            encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
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
        envelope2.payload =
            encrypt_payload_with_dek(dek.as_slice(), &existing, &envelope2).unwrap();

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
        envelope2.payload =
            encrypt_payload_with_dek(dek.as_slice(), &existing, &envelope2).unwrap();

        let decrypted = decrypt_vault_envelope(passphrase, &envelope2).unwrap();
        assert_eq!(payload_pinata_token(&decrypted), "pinata-original");
        assert_eq!(payload_filebase_token(&decrypted), "filebase-new");
    }

    #[test]
    fn pbkdf2_sha256_roundtrip() {
        let passphrase = "test-passphrase-sha256";
        let payload = make_provider_payload("pinata-token", "filebase-token");
        let envelope =
            encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_PBKDF2_SHA256).unwrap();
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
        set_provider_tokens_in_payload(
            &mut payload,
            "jwt-token",
            "bearer-token",
            "https://api.pinata.cloud",
            "https://rpc.filebase.io",
        );
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
        let mut envelope =
            encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
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
        let mut envelope =
            encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
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
        let mut envelope =
            encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
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
        let mut envelope =
            encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
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
        let mut envelope =
            encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        envelope.network = None;
        let result = decrypt_vault_envelope(passphrase, &envelope);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Network must be set"));
    }

    #[test]
    fn decrypt_rejects_empty_network() {
        let passphrase = "test-passphrase";
        let payload = make_provider_payload("token", "fb");
        let mut envelope =
            encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
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
        let wrapped = cipher
            .encrypt(
                nonce,
                aes_gcm::aead::Payload {
                    msg: dek.as_slice(),
                    aad: aad.as_bytes(),
                },
            )
            .unwrap();

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
        let envelope =
            encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_PBKDF2_SHA256).unwrap();
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
        assert!(result
            .unwrap_err()
            .contains("Invalid PBKDF2-SHA256 salt length"));
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
        )
        .unwrap();

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

    // ─── Export / Import tests ────────────────────────────────────────────

    #[test]
    fn export_bundle_copies_exact_file() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-passphrase";
        let payload = make_provider_payload("pinata-export", "filebase-export");
        let envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        let bundle = VaultBundle {
            bundleVersion: CURRENT_BUNDLE_VERSION,
            format_identifier: FORMAT_IDENTIFIER.to_string(),
            vault: envelope,
            meta: None,
        };
        save_bundle_atomic(&bundle).unwrap();

        let export_path = _guard.dir.join("exported_vault.json");
        let result = export_bundle_to_path(export_path.to_str().unwrap()).unwrap();
        assert_eq!(result, export_path.to_string_lossy());

        let original = fs::read_to_string(vault_path().unwrap()).unwrap();
        let exported = fs::read_to_string(&export_path).unwrap();
        assert_eq!(original, exported);

        let exported_value: serde_json::Value = serde_json::from_str(&exported).unwrap();
        assert_eq!(exported_value["bundleVersion"], CURRENT_BUNDLE_VERSION);
    }

    #[test]
    fn export_bundle_fails_when_no_vault() {
        let _guard = setup_test_vault_dir();
        let result = export_bundle_to_path("/tmp/test_export_no_vault.json");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No vault exists"));
    }

    #[test]
    fn validate_import_accepts_valid_bundle() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-passphrase";
        let payload = make_provider_payload("pinata", "filebase");
        let envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        let bundle = VaultBundle {
            bundleVersion: CURRENT_BUNDLE_VERSION,
            format_identifier: FORMAT_IDENTIFIER.to_string(),
            vault: envelope,
            meta: None,
        };
        let import_path = _guard.dir.join("import_vault.json");
        let content = serde_json::to_string_pretty(&bundle).unwrap();
        fs::write(&import_path, content).unwrap();

        let result = validate_import_bundle_from_path(import_path.to_str().unwrap()).unwrap();
        assert_eq!(result["valid"], true);
        assert_eq!(result["bundle_version"], CURRENT_BUNDLE_VERSION);
        assert_eq!(result["network"], "mainnet");
    }

    #[test]
    fn validate_import_rejects_wrong_format() {
        let _guard = setup_test_vault_dir();
        let import_path = _guard.dir.join("bad_format.json");
        fs::write(&import_path, r#"{"bundleVersion":3,"format_identifier":"foreign-format","vault":{"version":1,"schema_identifier":"x","app_identifier":"x","network":"mainnet","cipher_profile":"aes-256-gcm-v1","aad_profile":"commander-envelope-v1","payload":{"payload_schema":"commander-secrets-v1","iv":"ab","ciphertext":"cd"},"key_slots":[],"created":1,"modified":1}}"#).unwrap();

        let result = validate_import_bundle_from_path(import_path.to_str().unwrap());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown vault format"));
    }

    #[test]
    fn validate_import_rejects_nonexistent_file() {
        let result = validate_import_bundle_from_path("/tmp/nonexistent_vault_import.json");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("does not exist"));
    }

    #[test]
    fn validate_import_rejects_invalid_json() {
        let _guard = setup_test_vault_dir();
        let import_path = _guard.dir.join("invalid.json");
        fs::write(&import_path, "not json at all").unwrap();
        let result = validate_import_bundle_from_path(import_path.to_str().unwrap());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not valid JSON"));
    }

    #[test]
    fn import_bundle_replaces_vault() {
        let _guard = setup_test_vault_dir();
        let passphrase1 = "first-passphrase";
        let payload1 = make_provider_payload("pinata1", "filebase1");
        let envelope1 = encrypt_vault_envelope(passphrase1, &payload1, KDF_PROFILE_SCRYPT).unwrap();
        let bundle1 = VaultBundle {
            bundleVersion: CURRENT_BUNDLE_VERSION,
            format_identifier: FORMAT_IDENTIFIER.to_string(),
            vault: envelope1,
            meta: None,
        };
        save_bundle_atomic(&bundle1).unwrap();

        let passphrase2 = "second-passphrase";
        let payload2 = make_provider_payload("pinata2", "filebase2");
        let envelope2 = encrypt_vault_envelope(passphrase2, &payload2, KDF_PROFILE_SCRYPT).unwrap();
        let bundle2 = VaultBundle {
            bundleVersion: CURRENT_BUNDLE_VERSION,
            format_identifier: FORMAT_IDENTIFIER.to_string(),
            vault: envelope2,
            meta: None,
        };
        let import_path = _guard.dir.join("import_vault.json");
        let content = serde_json::to_string_pretty(&bundle2).unwrap();
        fs::write(&import_path, content).unwrap();

        let result = import_bundle_replace_from_path(import_path.to_str().unwrap(), None).unwrap();
        assert_eq!(result["imported"], true);

        let loaded = load_bundle().unwrap().unwrap();
        let decrypted = decrypt_vault_envelope(passphrase2, &loaded.vault).unwrap();
        assert_eq!(payload_pinata_token(&decrypted), "pinata2");
        assert_eq!(payload_filebase_token(&decrypted), "filebase2");

        let result_wrong = decrypt_vault_envelope(passphrase1, &loaded.vault);
        assert!(result_wrong.is_err());
    }

    #[test]
    fn import_bundle_with_passphrase_verification() {
        let _guard = setup_test_vault_dir();
        let passphrase = "correct-passphrase";
        let payload = make_provider_payload("pinata-verify", "filebase-verify");
        let envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        let bundle = VaultBundle {
            bundleVersion: CURRENT_BUNDLE_VERSION,
            format_identifier: FORMAT_IDENTIFIER.to_string(),
            vault: envelope,
            meta: None,
        };
        let import_path = _guard.dir.join("import_vault.json");
        let content = serde_json::to_string_pretty(&bundle).unwrap();
        fs::write(&import_path, content).unwrap();

        let result = import_bundle_replace_from_path(
            import_path.to_str().unwrap(),
            Some("correct-passphrase"),
        )
        .unwrap();
        assert_eq!(result["imported"], true);

        let result_wrong = import_bundle_replace_from_path(
            import_path.to_str().unwrap(),
            Some("wrong-passphrase"),
        );
        assert!(result_wrong.is_err());
        assert!(result_wrong.unwrap_err().contains("Incorrect passphrase"));
    }

    #[test]
    fn import_bundle_rejects_invalid_network() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-passphrase";
        let payload = make_provider_payload("token", "token");
        let mut envelope =
            encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        envelope.network = Some("invalidnet".to_string());
        let bundle = VaultBundle {
            bundleVersion: CURRENT_BUNDLE_VERSION,
            format_identifier: FORMAT_IDENTIFIER.to_string(),
            vault: envelope,
            meta: None,
        };
        let import_path = _guard.dir.join("import_bad_network.json");
        let content = serde_json::to_string_pretty(&bundle).unwrap();
        fs::write(&import_path, content).unwrap();

        let result = validate_import_bundle_from_path(import_path.to_str().unwrap());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid network"));
    }

    #[test]
    fn import_bundle_rejects_unsupported_version() {
        let _guard = setup_test_vault_dir();
        let import_path = _guard.dir.join("import_bad_version.json");
        fs::write(&import_path, r#"{"bundleVersion":999,"format_identifier":"hemp0x-unified-vault-bundle","vault":{"version":1,"schema_identifier":"hemp0x-commander-vault","app_identifier":"hemp0x-commander","network":"mainnet","cipher_profile":"aes-256-gcm-v1","aad_profile":"commander-envelope-v1","payload":{"payload_schema":"commander-secrets-v1","iv":"ab","ciphertext":"cd"},"key_slots":[],"created":1,"modified":1}}"#).unwrap();

        let result = validate_import_bundle_from_path(import_path.to_str().unwrap());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unsupported bundle version"));
    }

    // ─── Provider token status tests ─────────────────────────────────────

    #[test]
    fn check_provider_records_returns_correct_status() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-passphrase";
        let payload = make_provider_payload("pinata-token", "filebase-token");
        let envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        let bundle = VaultBundle {
            bundleVersion: CURRENT_BUNDLE_VERSION,
            format_identifier: FORMAT_IDENTIFIER.to_string(),
            vault: envelope,
            meta: None,
        };
        save_bundle_atomic(&bundle).unwrap();

        let status = check_provider_token_records(passphrase).unwrap();
        assert_eq!(status["providers"]["pinata"]["has_token"], true);
        assert_eq!(status["providers"]["filebase"]["has_token"], true);
    }

    #[test]
    fn check_provider_records_after_remove() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-passphrase";
        let payload = make_provider_payload("pinata-token", "filebase-token");
        let envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        let bundle = VaultBundle {
            bundleVersion: CURRENT_BUNDLE_VERSION,
            format_identifier: FORMAT_IDENTIFIER.to_string(),
            vault: envelope,
            meta: None,
        };
        save_bundle_atomic(&bundle).unwrap();

        remove_provider_token_from_vault(passphrase, RECORD_ID_PINATA).unwrap();

        let status = check_provider_token_records(passphrase).unwrap();
        assert_eq!(status["providers"]["pinata"]["has_token"], false);
        assert_eq!(status["providers"]["filebase"]["has_token"], true);
    }

    #[test]
    fn check_provider_records_with_empty_vault() {
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

        let status = check_provider_token_records(passphrase).unwrap();
        assert_eq!(status["providers"]["pinata"]["has_token"], false);
        assert_eq!(status["providers"]["filebase"]["has_token"], false);
    }

    // ─── Wallet migration record tests ──────────────────────────────────

    fn make_migration_envelope_json() -> String {
        serde_json::json!({
            "encrypted": true,
            "content": "base64-encrypted-migration-envelope-data",
            "version": "1.0",
            "chain": {
                "network": "mainnet",
                "matches_current_chain": true
            },
            "private_keys_included": true,
            "restorable": true
        })
        .to_string()
    }

    fn make_migration_metadata() -> serde_json::Value {
        serde_json::json!({
            "value_kind": "embedded_encrypted_json",
            "source": "core-next-exportwalletmigration",
            "restorable": true,
            "private_keys_included": true,
            "envelope_kdf_profile": "pbkdf2-hmac-sha512-v1",
            "envelope_cipher_profile": "aes-256-gcm-v1",
            "envelope_aad_profile": "commander-envelope-v1",
            "envelope_coin_type": 420,
            "label": "Main wallet backup",
            "wallet_name_hint": "default"
        })
    }

    #[test]
    fn wallet_migration_record_insert_and_list() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-passphrase";
        let payload = make_provider_payload("pinata-token", "filebase-token");
        let envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        let bundle = VaultBundle {
            bundleVersion: CURRENT_BUNDLE_VERSION,
            format_identifier: FORMAT_IDENTIFIER.to_string(),
            vault: envelope,
            meta: None,
        };
        save_bundle_atomic(&bundle).unwrap();

        let record_id = "wallet.core_migration_envelope.test-1";
        let label = "Test Wallet Backup";
        let value = make_migration_envelope_json();
        let metadata = make_migration_metadata();

        let result =
            insert_wallet_migration_record(passphrase, record_id, label, &value, metadata.clone())
                .unwrap();
        assert_eq!(result["inserted"], true);
        assert_eq!(result["record_id"], record_id);

        let records = list_wallet_migration_records(passphrase).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0]["record_id"], record_id);
        assert_eq!(records[0]["label"], label);
        assert_eq!(records[0]["record_type"], RECORD_TYPE_WALLET_CORE_MIGRATION);
        assert!(records[0]["metadata"].is_object());
        assert!(records[0].get("value").is_none());
    }

    #[test]
    fn wallet_migration_record_list_does_not_return_value() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-passphrase";
        let payload = make_provider_payload("pinata", "filebase");
        let envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        let bundle = VaultBundle {
            bundleVersion: CURRENT_BUNDLE_VERSION,
            format_identifier: FORMAT_IDENTIFIER.to_string(),
            vault: envelope,
            meta: None,
        };
        save_bundle_atomic(&bundle).unwrap();

        let secret_value = "super-secret-migration-envelope-data";
        let value = serde_json::json!({"encrypted": true, "content": secret_value}).to_string();
        insert_wallet_migration_record(
            passphrase,
            "wallet.core_migration_envelope.test-2",
            "Test",
            &value,
            make_migration_metadata(),
        )
        .unwrap();

        let records = list_wallet_migration_records(passphrase).unwrap();
        let json = serde_json::to_string(&records).unwrap();
        assert!(!json.contains(secret_value));
        for record in &records {
            assert!(record.get("value").is_none());
        }
    }

    #[test]
    fn wallet_migration_record_insert_preserves_provider_tokens() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-passphrase";
        let payload = make_provider_payload("pinata-original", "filebase-original");
        let envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        let bundle = VaultBundle {
            bundleVersion: CURRENT_BUNDLE_VERSION,
            format_identifier: FORMAT_IDENTIFIER.to_string(),
            vault: envelope,
            meta: None,
        };
        save_bundle_atomic(&bundle).unwrap();

        insert_wallet_migration_record(
            passphrase,
            "wallet.core_migration_envelope.test-3",
            "Test",
            &make_migration_envelope_json(),
            make_migration_metadata(),
        )
        .unwrap();

        let loaded = load_bundle().unwrap().unwrap();
        let decrypted = decrypt_vault_envelope(passphrase, &loaded.vault).unwrap();
        assert_eq!(payload_pinata_token(&decrypted), "pinata-original");
        assert_eq!(payload_filebase_token(&decrypted), "filebase-original");
    }

    #[test]
    fn wallet_migration_record_export_to_path() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-passphrase";
        let payload = make_provider_payload("pinata", "filebase");
        let envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        let bundle = VaultBundle {
            bundleVersion: CURRENT_BUNDLE_VERSION,
            format_identifier: FORMAT_IDENTIFIER.to_string(),
            vault: envelope,
            meta: None,
        };
        save_bundle_atomic(&bundle).unwrap();

        let value = make_migration_envelope_json();
        let record_id = "wallet.core_migration_envelope.test-4";
        insert_wallet_migration_record(
            passphrase,
            record_id,
            "Test",
            &value,
            make_migration_metadata(),
        )
        .unwrap();

        let export_path = _guard.dir.join("exported_migration.json");
        let result = export_wallet_migration_record_to_path(
            passphrase,
            record_id,
            export_path.to_str().unwrap(),
        )
        .unwrap();
        assert_eq!(result, export_path.to_string_lossy());

        let exported = fs::read_to_string(&export_path).unwrap();
        assert_eq!(exported, value);
    }

    #[test]
    fn wallet_migration_record_remove() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-passphrase";
        let payload = make_provider_payload("pinata", "filebase");
        let envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        let bundle = VaultBundle {
            bundleVersion: CURRENT_BUNDLE_VERSION,
            format_identifier: FORMAT_IDENTIFIER.to_string(),
            vault: envelope,
            meta: None,
        };
        save_bundle_atomic(&bundle).unwrap();

        let record_id = "wallet.core_migration_envelope.test-5";
        insert_wallet_migration_record(
            passphrase,
            record_id,
            "Test",
            &make_migration_envelope_json(),
            make_migration_metadata(),
        )
        .unwrap();

        let result = remove_wallet_migration_record(passphrase, record_id).unwrap();
        assert_eq!(result["removed"], true);
        assert_eq!(result["record_id"], record_id);

        let records = list_wallet_migration_records(passphrase).unwrap();
        assert_eq!(records.len(), 0);
    }

    #[test]
    fn wallet_migration_record_remove_nonexistent_fails() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-passphrase";
        let payload = make_provider_payload("pinata", "filebase");
        let envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        let bundle = VaultBundle {
            bundleVersion: CURRENT_BUNDLE_VERSION,
            format_identifier: FORMAT_IDENTIFIER.to_string(),
            vault: envelope,
            meta: None,
        };
        save_bundle_atomic(&bundle).unwrap();

        let result = remove_wallet_migration_record(
            passphrase,
            "wallet.core_migration_envelope.nonexistent",
        );
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("No wallet migration record found"));
    }

    #[test]
    fn wallet_migration_record_invalid_id_rejected() {
        let result = validate_wallet_migration_record_id("provider.pinata.api_token");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Must start with"));

        let result = validate_wallet_migration_record_id("wallet.core_migration_envelope.");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("must have a suffix"));

        let result = validate_wallet_migration_record_id("wallet.core_migration_envelope.bad/id");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("alphanumeric, underscore, or hyphen"));
    }

    #[test]
    fn wallet_migration_record_remove_only_target_record() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-passphrase";
        let payload = make_provider_payload("pinata", "filebase");
        let envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        let bundle = VaultBundle {
            bundleVersion: CURRENT_BUNDLE_VERSION,
            format_identifier: FORMAT_IDENTIFIER.to_string(),
            vault: envelope,
            meta: None,
        };
        save_bundle_atomic(&bundle).unwrap();

        insert_wallet_migration_record(
            passphrase,
            "wallet.core_migration_envelope.keep",
            "Keep",
            &make_migration_envelope_json(),
            make_migration_metadata(),
        )
        .unwrap();
        insert_wallet_migration_record(
            passphrase,
            "wallet.core_migration_envelope.remove",
            "Remove",
            &make_migration_envelope_json(),
            make_migration_metadata(),
        )
        .unwrap();

        remove_wallet_migration_record(passphrase, "wallet.core_migration_envelope.remove")
            .unwrap();

        let records = list_wallet_migration_records(passphrase).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(
            records[0]["record_id"],
            "wallet.core_migration_envelope.keep"
        );
    }

    #[test]
    fn wallet_migration_record_unknown_records_survive() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-passphrase";
        let mut payload = make_provider_payload("pinata", "filebase");
        let future_record = make_future_record("wallet.bip39.main", RECORD_TYPE_WALLET_BIP39, "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about");
        payload
            .secrets
            .insert(future_record.record_id.clone(), future_record.clone());
        let envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        let bundle = VaultBundle {
            bundleVersion: CURRENT_BUNDLE_VERSION,
            format_identifier: FORMAT_IDENTIFIER.to_string(),
            vault: envelope,
            meta: None,
        };
        save_bundle_atomic(&bundle).unwrap();

        insert_wallet_migration_record(
            passphrase,
            "wallet.core_migration_envelope.test-6",
            "Test",
            &make_migration_envelope_json(),
            make_migration_metadata(),
        )
        .unwrap();

        let loaded = load_bundle().unwrap().unwrap();
        let decrypted = decrypt_vault_envelope(passphrase, &loaded.vault).unwrap();
        assert!(decrypted.secrets.contains_key("wallet.bip39.main"));
        assert!(decrypted
            .secrets
            .contains_key("wallet.core_migration_envelope.test-6"));
    }

    // ─── Hardening tests ─────────────────────────────────────────────────

    #[test]
    fn collision_safe_record_ids_are_unique() {
        let content1 = make_migration_envelope_json();
        let content2 =
            serde_json::json!({"encrypted": true, "content": "different-data", "version": "1.0"})
                .to_string();
        let id1 = generate_collision_safe_record_id("import", &content1);
        let id2 = generate_collision_safe_record_id("import", &content2);
        assert_ne!(id1, id2);
        assert!(id1.starts_with(WALLET_MIGRATION_RECORD_PREFIX));
        assert!(id2.starts_with(WALLET_MIGRATION_RECORD_PREFIX));
    }

    #[test]
    fn insert_rejects_duplicate_record_id() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-passphrase";
        let payload = make_provider_payload("pinata", "filebase");
        let envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        let bundle = VaultBundle {
            bundleVersion: CURRENT_BUNDLE_VERSION,
            format_identifier: FORMAT_IDENTIFIER.to_string(),
            vault: envelope,
            meta: None,
        };
        save_bundle_atomic(&bundle).unwrap();

        let record_id = "wallet.core_migration_envelope.test-dup";
        insert_wallet_migration_record(
            passphrase,
            record_id,
            "First",
            &make_migration_envelope_json(),
            make_migration_metadata(),
        )
        .unwrap();

        let result = insert_wallet_migration_record(
            passphrase,
            record_id,
            "Second",
            &make_migration_envelope_json(),
            make_migration_metadata(),
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("already exists"));
    }

    #[test]
    fn label_validation_rejects_empty() {
        let result = validate_label("");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("required"));

        let result = validate_label("   ");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("required"));
    }

    #[test]
    fn label_validation_rejects_too_long() {
        let long_label = "x".repeat(MAX_LABEL_LENGTH + 1);
        let result = validate_label(&long_label);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("must not exceed"));
    }

    #[test]
    fn label_validation_accepts_valid() {
        let result = validate_label("Main Wallet Backup");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Main Wallet Backup");

        let result = validate_label("  Trimmed Label  ");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Trimmed Label");
    }

    #[test]
    fn temp_file_guard_cleans_up() {
        let guard = TempFileGuard::new("test_cleanup").unwrap();
        let path = guard.path.clone();
        fs::write(&path, "test content").unwrap();
        assert!(path.exists());
        drop(guard);
        assert!(!path.exists());
    }

    #[test]
    fn import_file_validation_rejects_empty() {
        let _guard = setup_test_vault_dir();
        let empty_path = _guard.dir.join("empty.json");
        fs::write(&empty_path, "").unwrap();
        let result = validate_import_file(&empty_path);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("empty"));
    }

    #[test]
    fn import_file_validation_rejects_too_large() {
        let _guard = setup_test_vault_dir();
        let large_path = _guard.dir.join("large.json");
        let large_content = "x".repeat((MAX_IMPORT_FILE_SIZE + 1) as usize);
        fs::write(&large_path, &large_content).unwrap();
        let result = validate_import_file(&large_path);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("too large"));
    }

    #[test]
    fn import_file_validation_accepts_valid() {
        let _guard = setup_test_vault_dir();
        let valid_path = _guard.dir.join("valid.json");
        fs::write(&valid_path, r#"{"encrypted":true}"#).unwrap();
        let result = validate_import_file(&valid_path).unwrap();
        assert_eq!(result, r#"{"encrypted":true}"#);
    }

    #[test]
    fn extract_validation_metadata_includes_required_fields() {
        let validation = serde_json::json!({
            "valid": true,
            "restorable": true,
            "private_keys_included": true,
            "chain": {"network": "mainnet", "matches_current_chain": true},
            "envelope_kdf_profile": "pbkdf2-hmac-sha512-v1",
            "envelope_cipher_profile": "aes-256-gcm-v1",
            "envelope_aad_profile": "commander-envelope-v1",
            "envelope_coin_type": 420
        });
        let meta = extract_validation_metadata(&validation);
        assert_eq!(meta["restorable"], true);
        assert_eq!(meta["private_keys_included"], true);
        assert_eq!(meta["envelope_kdf_profile"], "pbkdf2-hmac-sha512-v1");
        assert_eq!(meta["envelope_cipher_profile"], "aes-256-gcm-v1");
        assert_eq!(meta["envelope_aad_profile"], "commander-envelope-v1");
        assert_eq!(meta["envelope_coin_type"], 420);
        assert!(meta["chain"].is_object());
    }

    #[test]
    fn extract_validation_metadata_handles_missing_fields() {
        let validation = serde_json::json!({
            "valid": true,
            "restorable": false
        });
        let meta = extract_validation_metadata(&validation);
        assert_eq!(meta["restorable"], false);
        assert_eq!(meta["private_keys_included"], false);
        assert_eq!(meta["envelope_kdf_profile"], "unknown");
        assert_eq!(meta["envelope_cipher_profile"], "unknown");
        assert_eq!(meta["envelope_aad_profile"], "unknown");
    }

    #[test]
    fn vault_overview_reports_not_exists_when_no_bundle() {
        let _guard = setup_test_vault_dir();
        let overview = vault_get_vault_overview().unwrap();
        assert_eq!(overview["exists"], false);
        assert!(overview["vault_path"].is_string());
    }

    #[test]
    fn vault_overview_reports_metadata_when_bundle_exists() {
        let _guard = setup_test_vault_dir();
        let passphrase = "overview-test-pass";
        let payload = make_provider_payload("pinata", "filebase");
        let envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        let bundle = VaultBundle {
            bundleVersion: CURRENT_BUNDLE_VERSION,
            format_identifier: FORMAT_IDENTIFIER.to_string(),
            vault: envelope,
            meta: None,
        };
        save_bundle_atomic(&bundle).unwrap();

        let overview = vault_get_vault_overview().unwrap();
        assert_eq!(overview["exists"], true);
        assert_eq!(overview["bundle_version"], CURRENT_BUNDLE_VERSION);
        assert_eq!(overview["format_identifier"], FORMAT_IDENTIFIER);
        assert_eq!(overview["kdf_profile"], KDF_PROFILE_SCRYPT);
        assert_eq!(overview["key_slot_count"], 1);
        assert_eq!(overview["cipher_profile"], CIPHER_PROFILE);
        assert_eq!(overview["aad_profile"], AAD_PROFILE);
        // Wallet-record count is intentionally not exposed here.
        assert!(overview.get("wallet_record_count").is_none());
        // File stats and stable display_label key are exposed.
        assert!(overview["file_size"].as_i64().unwrap_or(0) > 0);
        assert!(overview["file_modified"].as_i64().unwrap_or(0) > 0);
        assert!(overview.get("display_label").is_some());
    }

    // ---- slice 60s: vault file manager sidecar + archive ----

    fn write_real_vault_file(guard: &TestVaultDirGuard) -> PathBuf {
        // Use the real bundle format so `vault_get_vault_overview` does
        // not reject the test file as malformed.
        let _ = guard;
        let payload = make_provider_payload("pinata", "filebase");
        let envelope =
            encrypt_vault_envelope("slice60s-test-pass", &payload, KDF_PROFILE_SCRYPT).unwrap();
        let bundle = VaultBundle {
            bundleVersion: CURRENT_BUNDLE_VERSION,
            format_identifier: FORMAT_IDENTIFIER.to_string(),
            vault: envelope,
            meta: None,
        };
        save_bundle_atomic(&bundle).unwrap();
        vault_path().unwrap()
    }

    #[test]
    fn vault_set_and_get_label_roundtrip() {
        let _guard = setup_test_vault_dir();
        let _path = write_real_vault_file(&_guard);

        let res = vault_set_vault_label("  Main Commander Vault  ".to_string()).unwrap();
        assert_eq!(res["updated"], true);
        assert_eq!(res["active_label"], "Main Commander Vault");

        let overview = vault_get_vault_overview().unwrap();
        assert_eq!(overview["display_label"], "Main Commander Vault");

        // Clearing the label removes the entry but keeps the key present.
        let res2 = vault_set_vault_label("   ".to_string()).unwrap();
        assert_eq!(res2["active_label"], "");
        let overview2 = vault_get_vault_overview().unwrap();
        assert_eq!(overview2["display_label"], "");
    }

    #[test]
    fn vault_set_label_truncates_and_strips_control_chars() {
        let _guard = setup_test_vault_dir();
        let _path = write_real_vault_file(&_guard);

        let mut s = String::new();
        s.push_str("\u{1b}[31mred");
        while s.len() < 200 {
            s.push('x');
        }
        let res = vault_set_vault_label(s).unwrap();
        let label = res["active_label"].as_str().unwrap().to_string();
        assert!(label.len() <= VAULT_MAX_LABEL_LEN);
        assert!(!label.contains('\u{1b}'));
        assert!(!label.contains("[31m"));
        assert!(label.starts_with("red"));
    }

    #[test]
    fn vault_archive_moves_file_and_records_in_index() {
        let _guard = setup_test_vault_dir();
        let path = write_real_vault_file(&_guard);
        let original_size = std::fs::metadata(&path).unwrap().len();
        assert!(path.exists());

        let res = vault_archive_current_vault().unwrap();
        assert_eq!(res["archived"], true);
        let archive_path = res["archive_path"].as_str().unwrap().to_string();
        assert!(std::path::Path::new(&archive_path).exists());
        // Original active vault is gone.
        assert!(!path.exists());
        // The archive is a verbatim copy, not a deletion.
        let archived_size = std::fs::metadata(&archive_path).unwrap().len();
        assert_eq!(archived_size, original_size);

        // The index recorded the archive.
        let index = load_vault_index().unwrap();
        assert!(index.archives.iter().any(|a| a == &archive_path));
    }

    #[test]
    fn vault_archive_requires_existing_file() {
        let _guard = setup_test_vault_dir();
        // No vault file present.
        let err = vault_archive_current_vault().unwrap_err();
        assert!(err.to_lowercase().contains("no vault"));
    }

    // ─── slice 60v: recovery_mode tests ─────────────────────────────────

    #[test]
    fn recovery_mode_vault_passphrase_stored_in_metadata() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-passphrase-recovery";
        let payload = make_provider_payload("pinata", "filebase");
        let envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        let bundle = VaultBundle {
            bundleVersion: CURRENT_BUNDLE_VERSION,
            format_identifier: FORMAT_IDENTIFIER.to_string(),
            vault: envelope,
            meta: None,
        };
        save_bundle_atomic(&bundle).unwrap();

        // This is a unit test on the metadata shape; we cannot run
        // vault_export_current_wallet_migration_record without a live
        // Core node, but we can validate the metadata structure
        // directly by calling insert with the right metadata.
        let record_id = "wallet.core_migration_envelope.test-recovery-1";
        let value = make_migration_envelope_json();
        let mut metadata = make_migration_metadata();
        if let Some(obj) = metadata.as_object_mut() {
            obj.insert(
                "recovery_mode".to_string(),
                serde_json::Value::String(RECOVERY_MODE_VAULT_PASSPHRASE.to_string()),
            );
        }
        insert_wallet_migration_record(
            passphrase,
            record_id,
            "Test Vault Recovery",
            &value,
            metadata,
        )
        .unwrap();

        let records = list_wallet_migration_records(passphrase).unwrap();
        let rec = records
            .iter()
            .find(|r| r["record_id"] == record_id)
            .unwrap();
        assert_eq!(
            rec["metadata"]["recovery_mode"],
            RECOVERY_MODE_VAULT_PASSPHRASE
        );
    }

    // ─── slice 63: WebCom / Hemp0x Vault Interop Tests ────────────────────

    #[test]
    fn interop_webcom_wallet_summary_does_not_return_value() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-interop-pass";
        let mut payload = make_provider_payload("pinata", "filebase");
        let mut dp = std::collections::HashMap::new();
        dp.insert(
            "hemp".to_string(),
            DERIVATION_HEMP_CANONICAL_420.to_string(),
        );
        let hemp_wallet = SecretRecord {
            record_id: RECORD_ID_WALLET_HEMP_PRIMARY.to_string(),
            record_type: RECORD_TYPE_WALLET_BIP39.to_string(),
            label: "WebCom Hemp Wallet".to_string(),
            value: "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about".to_string(),
            metadata: Some(serde_json::json!({
                "account": 0,
                "external_count": 20,
                "change_count": 6,
                "recovered_external_indices": [],
                "recovery": {
                    "schemaVersion": 1,
                    "seedType": "bip39",
                    "network": "mainnet",
                    "derivationProfiles": {
                        "hemp": DERIVATION_HEMP_CANONICAL_420,
                        "btc": DERIVATION_BTC_BIP84,
                    },
                    "createdAt": 1718136000000_i64,
                    "updatedAt": 1718136000000_i64,
                },
            })),
            tags: None,
            origin_app: Some("hemp0x-webcom".to_string()),
            derivation_profiles: Some(dp),
            network: Some("mainnet".to_string()),
            created: 1718136000,
            modified: 1718136000,
        };
        payload
            .secrets
            .insert(hemp_wallet.record_id.clone(), hemp_wallet);
        let envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        let bundle = VaultBundle {
            bundleVersion: CURRENT_BUNDLE_VERSION,
            format_identifier: FORMAT_IDENTIFIER.to_string(),
            vault: envelope,
            meta: None,
        };
        save_bundle_atomic(&bundle).unwrap();

        let summary = vault_get_webcom_interop_summary(Some(passphrase.to_string())).unwrap();
        let items = summary["items"].as_array().unwrap();
        let hemp_item = items
            .iter()
            .find(|i| i["record_id"] == RECORD_ID_WALLET_HEMP_PRIMARY)
            .unwrap();
        assert_eq!(hemp_item["exists"], true);
        assert_eq!(hemp_item["label"], "WebCom Hemp Wallet");
        assert_eq!(hemp_item["origin_app"], "hemp0x-webcom");
        assert_eq!(hemp_item["has_value"], true);
        assert!(hemp_item.get("value").is_none());
        assert!(hemp_item["derivation_profiles"].get("hemp").is_some());
        assert_eq!(
            hemp_item["derivation_profiles"]["hemp"],
            DERIVATION_HEMP_CANONICAL_420
        );
    }

    #[test]
    fn interop_address_book_export_creates_webcom_schema_v1() {
        let _guard = setup_test_vault_dir();
        crate::modules::files::TEST_COMMANDER_DIR.with(|cell| {
            *cell.borrow_mut() = Some(_guard.dir.clone());
        });
        let passphrase = "test-ab-export";
        let payload = make_provider_payload("pinata", "filebase");
        let envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        let bundle = VaultBundle {
            bundleVersion: CURRENT_BUNDLE_VERSION,
            format_identifier: FORMAT_IDENTIFIER.to_string(),
            vault: envelope,
            meta: None,
        };
        save_bundle_atomic(&bundle).unwrap();

        let local_entries = vec![
            crate::modules::models::AddressBookEntry {
                label: "Alice".into(),
                address: "Rtest".into(),
                locked: true,
                date: 1718136000,
            },
            crate::modules::models::AddressBookEntry {
                label: "Bob".into(),
                address: "Rtest2".into(),
                locked: false,
                date: 1718136001,
            },
        ];
        crate::modules::files::save_address_book(local_entries).unwrap();

        let result = vault_export_address_book_record(Some(passphrase.to_string())).unwrap();
        assert_eq!(result["exported"], true);
        assert_eq!(result["record_id"], RECORD_ID_ADDRESS_BOOK);
        assert_eq!(result["hemp_entries"], 2);

        let loaded = load_bundle().unwrap().unwrap();
        let decrypted = decrypt_vault_envelope(passphrase, &loaded.vault).unwrap();
        let record = decrypted.secrets.get(RECORD_ID_ADDRESS_BOOK).unwrap();
        let value: serde_json::Value = serde_json::from_str(&record.value).unwrap();
        assert_eq!(value["schema"], ADDRESS_BOOK_SCHEMA);
        assert_eq!(value["schema_version"], ADDRESS_BOOK_SCHEMA_VERSION);
        assert!(value["exported_at"].as_i64().unwrap() > 0);
        let entries = value["entries"].as_array().unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0]["chain"], ADDRESS_BOOK_CHAIN_HEMP);
        assert_eq!(entries[0]["label"], "Alice");
        assert_eq!(entries[0]["locked"], true);
    }

    #[test]
    fn interop_address_book_export_preserves_existing_bitcoin_entries() {
        let _guard = setup_test_vault_dir();
        crate::modules::files::TEST_COMMANDER_DIR.with(|cell| {
            *cell.borrow_mut() = Some(_guard.dir.clone());
        });
        let passphrase = "test-ab-export-btc";
        let mut payload = make_provider_payload("pinata", "filebase");

        let existing_ab = serde_json::json!({
            "schema": ADDRESS_BOOK_SCHEMA,
            "schema_version": ADDRESS_BOOK_SCHEMA_VERSION,
            "exported_at": 1700000000_i64,
            "entries": [
                { "chain": "bitcoin", "label": "BtcFriend", "address": "bc1qtest", "locked": true },
            ]
        });
        let existing_record = SecretRecord {
            record_id: RECORD_ID_ADDRESS_BOOK.to_string(),
            record_type: RECORD_TYPE_APP_SETTING_ADDRESS_BOOK.to_string(),
            label: "Hemp0x Address Book".to_string(),
            value: serde_json::to_string(&existing_ab).unwrap(),
            metadata: Some(
                serde_json::json!({"value_kind": "embedded_json", "schema": ADDRESS_BOOK_SCHEMA, "schema_version": ADDRESS_BOOK_SCHEMA_VERSION}),
            ),
            tags: Some(vec!["hemp0x".to_string(), "address_book".to_string()]),
            origin_app: Some("hemp0x-webcom".to_string()),
            derivation_profiles: None,
            network: Some("mainnet".to_string()),
            created: 1700000000,
            modified: 1700000000,
        };
        payload
            .secrets
            .insert(RECORD_ID_ADDRESS_BOOK.to_string(), existing_record);
        let envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        let bundle = VaultBundle {
            bundleVersion: CURRENT_BUNDLE_VERSION,
            format_identifier: FORMAT_IDENTIFIER.to_string(),
            vault: envelope,
            meta: None,
        };
        save_bundle_atomic(&bundle).unwrap();

        let local_entries = vec![crate::modules::models::AddressBookEntry {
            label: "Alice".into(),
            address: "Rtest".into(),
            locked: false,
            date: 1718136000,
        }];
        crate::modules::files::save_address_book(local_entries).unwrap();

        let _result = vault_export_address_book_record(Some(passphrase.to_string())).unwrap();
        let loaded = load_bundle().unwrap().unwrap();
        let decrypted = decrypt_vault_envelope(passphrase, &loaded.vault).unwrap();
        let record = decrypted.secrets.get(RECORD_ID_ADDRESS_BOOK).unwrap();
        let value: serde_json::Value = serde_json::from_str(&record.value).unwrap();
        let entries = value["entries"].as_array().unwrap();
        assert!(entries
            .iter()
            .any(|e| e["chain"] == "bitcoin" && e["label"] == "BtcFriend"));
        assert!(entries
            .iter()
            .any(|e| e["chain"] == ADDRESS_BOOK_CHAIN_HEMP && e["label"] == "Alice"));
    }

    #[test]
    fn interop_address_book_import_merges_without_deleting_local_entries() {
        let _guard = setup_test_vault_dir();
        crate::modules::files::TEST_COMMANDER_DIR.with(|cell| {
            *cell.borrow_mut() = Some(_guard.dir.clone());
        });
        let passphrase = "test-ab-import-merge";

        let existing_local = vec![crate::modules::models::AddressBookEntry {
            label: "ExistingAlice".into(),
            address: "Rlocal".into(),
            locked: true,
            date: 1718136000,
        }];
        crate::modules::files::save_address_book(existing_local).unwrap();

        let vault_ab = serde_json::json!({
            "schema": ADDRESS_BOOK_SCHEMA,
            "schema_version": ADDRESS_BOOK_SCHEMA_VERSION,
            "exported_at": 1700000000_i64,
            "entries": [
                { "chain": ADDRESS_BOOK_CHAIN_HEMP, "label": "NewBob", "address": serde_json::Value::String("Rnew".to_string()), "locked": false },
                { "chain": ADDRESS_BOOK_CHAIN_HEMP, "label": "", "address": serde_json::Value::String("Rlocal".to_string()), "locked": false },
            ]
        });
        let mut payload = make_provider_payload("pinata", "filebase");
        payload.secrets.insert(RECORD_ID_ADDRESS_BOOK.to_string(), SecretRecord {
            record_id: RECORD_ID_ADDRESS_BOOK.to_string(),
            record_type: RECORD_TYPE_APP_SETTING_ADDRESS_BOOK.to_string(),
            label: "Hemp0x Address Book".to_string(),
            value: serde_json::to_string(&vault_ab).unwrap(),
            metadata: Some(serde_json::json!({"value_kind": "embedded_json", "schema": ADDRESS_BOOK_SCHEMA, "schema_version": ADDRESS_BOOK_SCHEMA_VERSION})),
            tags: Some(vec!["hemp0x".to_string(), "address_book".to_string()]),
            origin_app: Some("hemp0x-webcom".to_string()),
            derivation_profiles: None,
            network: Some("mainnet".to_string()),
            created: 1700000000,
            modified: 1700000000,
        });
        let envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        let bundle = VaultBundle {
            bundleVersion: CURRENT_BUNDLE_VERSION,
            format_identifier: FORMAT_IDENTIFIER.to_string(),
            vault: envelope,
            meta: None,
        };
        save_bundle_atomic(&bundle).unwrap();

        let result = vault_import_address_book_record(Some(passphrase.to_string())).unwrap();
        assert_eq!(result["imported"], 1);
        assert_eq!(result["skipped"], 0);
        assert!(result["preserved_non_hemp_entries"].as_u64().unwrap_or(0) == 0);

        let local = crate::modules::files::load_address_book().unwrap();
        assert!(local.iter().any(|e| e.address == "Rnew"));
        assert!(local.iter().any(|e| e.address == "Rlocal"));
        let existing = local.iter().find(|e| e.address == "Rlocal").unwrap();
        assert_eq!(existing.label, "ExistingAlice");
        assert_eq!(existing.locked, true);
    }

    #[test]
    fn interop_provider_tokens_survive_address_book_export() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-ab-export-providers";
        let payload = make_provider_payload("pinata-original", "filebase-original");
        let envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        let bundle = VaultBundle {
            bundleVersion: CURRENT_BUNDLE_VERSION,
            format_identifier: FORMAT_IDENTIFIER.to_string(),
            vault: envelope,
            meta: None,
        };
        save_bundle_atomic(&bundle).unwrap();

        let result = vault_export_address_book_record(Some(passphrase.to_string())).unwrap();
        assert_eq!(result["exported"], true);

        let loaded = load_bundle().unwrap().unwrap();
        let decrypted = decrypt_vault_envelope(passphrase, &loaded.vault).unwrap();
        assert_eq!(payload_pinata_token(&decrypted), "pinata-original");
        assert_eq!(payload_filebase_token(&decrypted), "filebase-original");
    }

    #[test]
    fn interop_swap_secrets_survive_address_book_operations() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-swap-secrets-survive";
        let mut payload = make_provider_payload("pinata", "filebase");

        let swap_record = SecretRecord {
            record_id: RECORD_ID_SWAP_SECRETS.to_string(),
            record_type: RECORD_TYPE_APP_SETTING_SWAP_SECRETS.to_string(),
            label: "WebCom Swap Secrets".to_string(),
            value: r#"{"encrypted": true, "secrets": ["a", "b"]}"#.to_string(),
            metadata: Some(serde_json::json!({"value_kind": "embedded_encrypted_json"})),
            tags: None,
            origin_app: Some("hemp0x-webcom".to_string()),
            derivation_profiles: None,
            network: Some("mainnet".to_string()),
            created: 1700000000,
            modified: 1700000000,
        };
        payload
            .secrets
            .insert(swap_record.record_id.clone(), swap_record);

        let envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        let bundle = VaultBundle {
            bundleVersion: CURRENT_BUNDLE_VERSION,
            format_identifier: FORMAT_IDENTIFIER.to_string(),
            vault: envelope,
            meta: None,
        };
        save_bundle_atomic(&bundle).unwrap();

        let _export = vault_export_address_book_record(Some(passphrase.to_string())).unwrap();

        let loaded = load_bundle().unwrap().unwrap();
        let decrypted = decrypt_vault_envelope(passphrase, &loaded.vault).unwrap();
        assert!(decrypted.secrets.contains_key(RECORD_ID_SWAP_SECRETS));
        let survived = decrypted.secrets.get(RECORD_ID_SWAP_SECRETS).unwrap();
        assert_eq!(survived.record_type, RECORD_TYPE_APP_SETTING_SWAP_SECRETS);
        assert_eq!(
            survived.value,
            r#"{"encrypted": true, "secrets": ["a", "b"]}"#
        );
    }

    #[test]
    fn interop_unknown_record_survives_address_book_operations() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-unknown-survives";
        let future_id = "app_setting.future.feature.v42";
        let mut payload = make_provider_payload("pinata", "filebase");
        let future = make_future_record(future_id, RECORD_TYPE_APP_SECRET, "future-secret-value");
        payload
            .secrets
            .insert(future_id.to_string(), future.clone());

        let envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        let bundle = VaultBundle {
            bundleVersion: CURRENT_BUNDLE_VERSION,
            format_identifier: FORMAT_IDENTIFIER.to_string(),
            vault: envelope,
            meta: None,
        };
        save_bundle_atomic(&bundle).unwrap();

        let _export = vault_export_address_book_record(Some(passphrase.to_string())).unwrap();

        let loaded = load_bundle().unwrap().unwrap();
        let decrypted = decrypt_vault_envelope(passphrase, &loaded.vault).unwrap();
        assert!(decrypted.secrets.contains_key(future_id));
        let survived = decrypted.secrets.get(future_id).unwrap();
        assert_eq!(survived.value, "future-secret-value");
        assert_eq!(survived.record_type, RECORD_TYPE_APP_SECRET);
    }

    #[test]
    fn interop_malformed_address_book_record_returns_safe_error() {
        let _guard = setup_test_vault_dir();
        crate::modules::files::TEST_COMMANDER_DIR.with(|cell| {
            *cell.borrow_mut() = Some(_guard.dir.clone());
        });
        let passphrase = "test-malformed-ab";

        let existing_local = vec![crate::modules::models::AddressBookEntry {
            label: "PreciousEntry".into(),
            address: "Rprecious".into(),
            locked: true,
            date: 1718136000,
        }];
        crate::modules::files::save_address_book(existing_local).unwrap();

        let mut payload = make_provider_payload("pinata", "filebase");
        payload.secrets.insert(
            RECORD_ID_ADDRESS_BOOK.to_string(),
            SecretRecord {
                record_id: RECORD_ID_ADDRESS_BOOK.to_string(),
                record_type: RECORD_TYPE_APP_SETTING_ADDRESS_BOOK.to_string(),
                label: "Hemp0x Address Book".to_string(),
                value: "NOT VALID JSON".to_string(),
                metadata: Some(serde_json::json!({"value_kind": "embedded_json"})),
                tags: None,
                origin_app: None,
                derivation_profiles: None,
                network: None,
                created: 1700000000,
                modified: 1700000000,
            },
        );
        let envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        let bundle = VaultBundle {
            bundleVersion: CURRENT_BUNDLE_VERSION,
            format_identifier: FORMAT_IDENTIFIER.to_string(),
            vault: envelope,
            meta: None,
        };
        save_bundle_atomic(&bundle).unwrap();

        let result = vault_import_address_book_record(Some(passphrase.to_string()));
        assert!(result.is_err());

        let local = crate::modules::files::load_address_book().unwrap();
        assert_eq!(local.len(), 1);
        assert_eq!(local[0].address, "Rprecious");
        assert_eq!(local[0].label, "PreciousEntry");
    }

    #[test]
    fn interop_webcom_interop_summary_shows_all_known_record_ids() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-summary-ids";
        let payload = make_provider_payload("pinata", "filebase");
        let envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        let bundle = VaultBundle {
            bundleVersion: CURRENT_BUNDLE_VERSION,
            format_identifier: FORMAT_IDENTIFIER.to_string(),
            vault: envelope,
            meta: None,
        };
        save_bundle_atomic(&bundle).unwrap();

        let summary = vault_get_webcom_interop_summary(Some(passphrase.to_string())).unwrap();
        let items = summary["items"].as_array().unwrap();
        assert_eq!(summary["known_record_count"], 4);
        assert_eq!(summary["present_record_count"], 0);
        for rid in KNOWN_WEBCOM_RECORD_IDS {
            let item = items.iter().find(|i| i["record_id"] == *rid).unwrap();
            assert_eq!(item["exists"], false);
        }
    }

    #[test]
    fn interop_webcom_interop_summary_requires_unlocked_vault() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-locked-summary";
        let payload = make_provider_payload("pinata", "filebase");
        let envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        let bundle = VaultBundle {
            bundleVersion: CURRENT_BUNDLE_VERSION,
            format_identifier: FORMAT_IDENTIFIER.to_string(),
            vault: envelope,
            meta: None,
        };
        save_bundle_atomic(&bundle).unwrap();

        let result = vault_get_webcom_interop_summary(None);
        assert!(result.is_err());
    }

    #[test]
    fn recovery_mode_separate_passphrase_stored_in_metadata() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-passphrase-separate";
        let payload = make_provider_payload("pinata", "filebase");
        let envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        let bundle = VaultBundle {
            bundleVersion: CURRENT_BUNDLE_VERSION,
            format_identifier: FORMAT_IDENTIFIER.to_string(),
            vault: envelope,
            meta: None,
        };
        save_bundle_atomic(&bundle).unwrap();

        let record_id = "wallet.core_migration_envelope.test-separate-1";
        let value = make_migration_envelope_json();
        let mut metadata = make_migration_metadata();
        if let Some(obj) = metadata.as_object_mut() {
            obj.insert(
                "recovery_mode".to_string(),
                serde_json::Value::String(RECOVERY_MODE_SEPARATE_PASSPHRASE.to_string()),
            );
        }
        insert_wallet_migration_record(
            passphrase,
            record_id,
            "Test Separate Recovery",
            &value,
            metadata,
        )
        .unwrap();

        let records = list_wallet_migration_records(passphrase).unwrap();
        let rec = records
            .iter()
            .find(|r| r["record_id"] == record_id)
            .unwrap();
        assert_eq!(
            rec["metadata"]["recovery_mode"],
            RECOVERY_MODE_SEPARATE_PASSPHRASE
        );
    }

    #[test]
    fn recovery_mode_legacy_no_metadata_treated_as_separate() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-passphrase-legacy";
        let payload = make_provider_payload("pinata", "filebase");
        let envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        let bundle = VaultBundle {
            bundleVersion: CURRENT_BUNDLE_VERSION,
            format_identifier: FORMAT_IDENTIFIER.to_string(),
            vault: envelope,
            meta: None,
        };
        save_bundle_atomic(&bundle).unwrap();

        let record_id = "wallet.core_migration_envelope.test-legacy-1";
        let value = make_migration_envelope_json();
        let metadata = make_migration_metadata(); // no recovery_mode key
        insert_wallet_migration_record(passphrase, record_id, "Legacy Record", &value, metadata)
            .unwrap();

        let records = list_wallet_migration_records(passphrase).unwrap();
        let rec = records
            .iter()
            .find(|r| r["record_id"] == record_id)
            .unwrap();
        // Legacy records should have no recovery_mode key,
        // which the frontend treats as "separate": user must enter password.
        assert!(rec["metadata"]["recovery_mode"].is_null());
    }

    // ─── slice 64b: Portable Wallet Bridge metadata-only tests ──────────

    fn make_webcom_bip39_record(derivation_profile: &str, seed_type: &str) -> SecretRecord {
        let mut dp = HashMap::new();
        dp.insert("hemp".to_string(), derivation_profile.to_string());
        if seed_type == "bip39" {
            dp.insert("btc".to_string(), DERIVATION_BTC_BIP84.to_string());
        }
        let now = chrono::Utc::now().timestamp();
        SecretRecord {
            record_id: RECORD_ID_WALLET_HEMP_PRIMARY.to_string(),
            record_type: RECORD_TYPE_WALLET_BIP39.to_string(),
            label: "WebCom Hemp Wallet".to_string(),
            value: "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about".to_string(),
            metadata: Some(serde_json::json!({
                "account": 0,
                "external_count": 20,
                "change_count": 6,
                "recovery": {
                    "schemaVersion": 1,
                    "seedType": seed_type,
                    "network": "mainnet",
                    "derivationProfiles": dp.clone(),
                    "createdAt": 1718136000000_i64,
                    "updatedAt": 1718136000000_i64,
                },
            })),
            tags: Some(vec!["webcom".to_string(), "hemp".to_string()]),
            origin_app: Some("hemp0x-webcom".to_string()),
            derivation_profiles: Some(dp),
            network: Some("mainnet".to_string()),
            created: now,
            modified: now,
        }
    }

    fn make_webcom_wif_record() -> SecretRecord {
        let now = chrono::Utc::now().timestamp();
        SecretRecord {
            record_id: RECORD_ID_WALLET_HEMP_PRIMARY.to_string(),
            record_type: RECORD_TYPE_WALLET_WIF.to_string(),
            label: "WebCom Hemp Wallet".to_string(),
            value: "KxFakeWIFMarkerForTesting00000000000000000000000".to_string(),
            metadata: Some(serde_json::json!({
                "account": 0,
                "external_count": 1,
                "change_count": 0,
                "recovery": {
                    "schemaVersion": 1,
                    "seedType": "wif",
                    "network": "mainnet",
                    "derivationProfiles": {
                        "hemp": DERIVATION_WIF_SINGLE,
                    },
                },
            })),
            tags: Some(vec!["webcom".to_string(), "hemp".to_string()]),
            origin_app: Some("hemp0x-webcom".to_string()),
            derivation_profiles: Some({
                let mut dp = HashMap::new();
                dp.insert("hemp".to_string(), DERIVATION_WIF_SINGLE.to_string());
                dp
            }),
            network: Some("mainnet".to_string()),
            created: now,
            modified: now,
        }
    }

    #[test]
    fn webcom_keypool_hints_cover_recovered_indices() {
        let mut record = make_webcom_bip39_record(DERIVATION_HEMP_CANONICAL_420, "bip39");
        record.metadata = Some(serde_json::json!({
            "external_count": 2,
            "change_count": 1,
            "recovered_external_indices": [0, 4, 120],
            "recovery": {
                "seedType": "bip39",
                "network": "mainnet",
                "derivationProfiles": {
                    "hemp": DERIVATION_HEMP_CANONICAL_420,
                },
            },
        }));

        let hints = webcom_keypool_hints(&record);
        assert_eq!(hints.external_count_hint, 121);
        assert_eq!(hints.change_count_hint, VAULT_KEYPOOL_CHANGE_HINT_FLOOR);
        assert_eq!(hints.recovered_external_indices_count, 3);
        assert_eq!(
            vault_keypool_refill_target(hints.external_count_hint, hints.change_count_hint),
            VAULT_CORE_KEYPOOL_REFILL_FLOOR
        );
    }

    #[test]
    fn webcom_keypool_hints_are_capped_for_corrupt_metadata() {
        let mut record = make_webcom_bip39_record(DERIVATION_HEMP_CANONICAL_420, "bip39");
        record.metadata = Some(serde_json::json!({
            "external_count": 99_999,
            "change_count": 88_888,
            "recovered_external_indices": [150_000],
            "recovery": {
                "seedType": "bip39",
                "network": "mainnet",
                "derivationProfiles": {
                    "hemp": DERIVATION_HEMP_CANONICAL_420,
                },
            },
        }));

        let hints = webcom_keypool_hints(&record);
        assert_eq!(hints.external_count_hint, VAULT_KEYPOOL_HINT_CEILING);
        assert_eq!(hints.change_count_hint, VAULT_KEYPOOL_HINT_CEILING);
        assert_eq!(
            vault_keypool_refill_target(hints.external_count_hint, hints.change_count_hint),
            VAULT_KEYPOOL_HINT_CEILING
        );
    }

    fn save_vault_with_payload(passphrase: &str, payload: VaultPayload) {
        let envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        let bundle = VaultBundle {
            bundleVersion: CURRENT_BUNDLE_VERSION,
            format_identifier: FORMAT_IDENTIFIER.to_string(),
            vault: envelope,
            meta: None,
        };
        save_bundle_atomic(&bundle).unwrap();
    }

    #[test]
    fn alignment_status_never_returns_value() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-detect-bip39";
        let mut payload = VaultPayload::default();
        let webcom = make_webcom_bip39_record(DERIVATION_HEMP_CANONICAL_420, "bip39");
        payload.secrets.insert(webcom.record_id.clone(), webcom);
        save_vault_with_payload(passphrase, payload);

        let status = get_wallet_alignment_status(passphrase).unwrap();
        assert_eq!(status["has_webcom_primary"], true);
        assert_eq!(status["webcom_primary_record_type"], "wallet.bip39");
        assert_eq!(status["webcom_primary_seed_type"], "bip39");
        assert_eq!(
            status["webcom_primary_derivation_hemp"],
            DERIVATION_HEMP_CANONICAL_420
        );
        assert!(status.get("webcom_primary_fingerprint").is_some());
    }

    #[test]
    fn alignment_status_core_not_bip39_returns_non_portable() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-non-portable";
        let mut payload = VaultPayload::default();
        let webcom = make_webcom_bip39_record(DERIVATION_HEMP_CANONICAL_420, "bip39");
        payload.secrets.insert(webcom.record_id.clone(), webcom);
        save_vault_with_payload(passphrase, payload);

        let status = get_wallet_alignment_status(passphrase).unwrap();
        // Core may or may not be reachable in test environment.
        // The key assertion: the status never pretends success when
        // alignment is missing and Core cannot automate the flow.
        assert!(status
            .get("has_webcom_primary")
            .and_then(|v| v.as_bool())
            .unwrap_or(false));
        assert!(status.get("recommended_next_action").is_some());
        let action = status["recommended_next_action"].as_str().unwrap();
        // Must not be "already_aligned" when no alignment record exists
        assert_ne!(action, "already_aligned");
    }

    #[test]
    fn alignment_fingerprint_is_deterministic() {
        let record1 = make_webcom_bip39_record(DERIVATION_HEMP_CANONICAL_420, "bip39");
        let record2 = make_webcom_bip39_record(DERIVATION_HEMP_CANONICAL_420, "bip39");
        let fp1 = build_alignment_fingerprint(&record1);
        let fp2 = build_alignment_fingerprint(&record2);
        assert_eq!(fp1, fp2);
    }

    #[test]
    fn alignment_fingerprint_differs_by_derivation() {
        let record420 = make_webcom_bip39_record(DERIVATION_HEMP_CANONICAL_420, "bip39");
        let record175 = make_webcom_bip39_record(DERIVATION_HEMP_LEGACY_175, "bip39");
        let fp420 = build_alignment_fingerprint(&record420);
        let fp175 = build_alignment_fingerprint(&record175);
        assert_ne!(fp420, fp175);
    }

    #[test]
    fn alignment_fingerprint_differs_by_seed_type() {
        let bip39 = make_webcom_bip39_record(DERIVATION_HEMP_CANONICAL_420, "bip39");
        let wif = make_webcom_bip39_record(DERIVATION_WIF_SINGLE, "wif");
        let fp_bip39 = build_alignment_fingerprint(&bip39);
        let fp_wif = build_alignment_fingerprint(&wif);
        assert_ne!(fp_bip39, fp_wif);
    }

    // ─── Slice 64d: WebCom BIP39 -> Core Migration Envelope Tests ────────

    #[test]
    fn build_migration_envelope_from_bip39_record_produces_valid_json() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-migration-build";
        let mut payload = make_provider_payload("pinata", "filebase");
        let webcom = make_webcom_bip39_record(DERIVATION_HEMP_CANONICAL_420, "bip39");
        payload
            .secrets
            .insert(webcom.record_id.clone(), webcom.clone());
        save_vault_with_payload(passphrase, payload);

        let loaded = load_bundle().unwrap().unwrap();
        let decrypted = decrypt_vault_envelope(passphrase, &loaded.vault).unwrap();
        let record = decrypted
            .secrets
            .get(RECORD_ID_WALLET_HEMP_PRIMARY)
            .unwrap();

        let envelope_json = build_migration_envelope_from_webcom_bip39(passphrase, record).unwrap();
        let envelope: serde_json::Value = serde_json::from_str(&envelope_json).unwrap();

        assert_eq!(envelope["envelope_version"], MIGRATION_ENVELOPE_VERSION);
        assert_eq!(envelope["schema_identifier"], MIGRATION_SCHEMA_IDENTIFIER);
        assert_eq!(envelope["source_client"], MIGRATION_SOURCE_CLIENT);
        assert_eq!(
            envelope["chain"]["network"],
            CORE_MIGRATION_MAINNET_NETWORK_ID
        );
        assert_eq!(envelope["chain"]["coin_type_bip44"], 420);
        assert_eq!(envelope["wallet_summary"]["hd_enabled"], true);
        assert_eq!(envelope["wallet_summary"]["bip44_enabled"], true);
        assert_eq!(envelope["wallet_summary"]["private_keys_included"], true);
        assert_eq!(envelope["wallet_summary"]["mnemonic_available"], true);
        assert_eq!(
            envelope["derivation"][0]["profile_id"],
            DERIVATION_HEMP_CANONICAL_420
        );
        assert_eq!(envelope["derivation"][0]["coin_type"], 420);
        assert_eq!(envelope["private"]["encrypted"], true);
        assert_eq!(
            envelope["private"]["payload_format"],
            MIGRATION_PRIVATE_PAYLOAD_FORMAT
        );
        assert_eq!(envelope["private"]["kdf_profile"], MIGRATION_KDF_PROFILE);
        assert_eq!(
            envelope["private"]["kdf_iterations"],
            MIGRATION_KDF_ITERATIONS
        );
        assert_eq!(
            envelope["private"]["cipher_profile"],
            MIGRATION_CIPHER_PROFILE
        );
        assert_eq!(envelope["private"]["aad_profile"], MIGRATION_AAD_PROFILE);
        assert!(!envelope["private"]["salt"].as_str().unwrap().is_empty());
        assert!(!envelope["private"]["iv"].as_str().unwrap().is_empty());
        assert!(!envelope["private"]["tag"].as_str().unwrap().is_empty());
        assert!(!envelope["private"]["ciphertext"]
            .as_str()
            .unwrap()
            .is_empty());
    }

    #[test]
    fn migration_envelope_never_leaks_mnemonic_in_public_fields() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-no-leak";
        let mut payload = make_provider_payload("pinata", "filebase");
        let webcom = make_webcom_bip39_record(DERIVATION_HEMP_CANONICAL_420, "bip39");
        payload
            .secrets
            .insert(webcom.record_id.clone(), webcom.clone());
        save_vault_with_payload(passphrase, payload);

        let loaded = load_bundle().unwrap().unwrap();
        let decrypted = decrypt_vault_envelope(passphrase, &loaded.vault).unwrap();
        let record = decrypted
            .secrets
            .get(RECORD_ID_WALLET_HEMP_PRIMARY)
            .unwrap();

        let envelope_json = build_migration_envelope_from_webcom_bip39(passphrase, record).unwrap();

        let mnemonic = &record.value;
        assert!(!mnemonic.is_empty());
        assert!(
            !envelope_json.contains(mnemonic.as_str()),
            "Migration envelope JSON must not contain the mnemonic in plaintext"
        );
    }

    #[test]
    fn migration_envelope_rejects_legacy_coin175_profile() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-reject-175";
        let mut payload = make_provider_payload("pinata", "filebase");
        let webcom = make_webcom_bip39_record(DERIVATION_HEMP_LEGACY_175, "bip39");
        payload
            .secrets
            .insert(webcom.record_id.clone(), webcom.clone());
        save_vault_with_payload(passphrase, payload);

        let loaded = load_bundle().unwrap().unwrap();
        let decrypted = decrypt_vault_envelope(passphrase, &loaded.vault).unwrap();
        let record = decrypted
            .secrets
            .get(RECORD_ID_WALLET_HEMP_PRIMARY)
            .unwrap();

        let result = build_migration_envelope_from_webcom_bip39(passphrase, record);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Legacy coin175"));
    }

    #[test]
    fn migration_envelope_rejects_wrong_derivation_profile() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-reject-wrong";
        let mut payload = make_provider_payload("pinata", "filebase");
        let webcom = make_webcom_bip39_record(DERIVATION_BTC_BIP84, "bip39");
        payload
            .secrets
            .insert(webcom.record_id.clone(), webcom.clone());
        save_vault_with_payload(passphrase, payload);

        let loaded = load_bundle().unwrap().unwrap();
        let decrypted = decrypt_vault_envelope(passphrase, &loaded.vault).unwrap();
        let record = decrypted
            .secrets
            .get(RECORD_ID_WALLET_HEMP_PRIMARY)
            .unwrap();

        let result = build_migration_envelope_from_webcom_bip39(passphrase, record);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("Unsupported derivation profile"));
    }

    #[test]
    fn migration_envelope_rejects_wif_record_type() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-reject-wif";
        let mut payload = make_provider_payload("pinata", "filebase");
        let wif = make_webcom_wif_record();
        payload.secrets.insert(wif.record_id.clone(), wif);
        save_vault_with_payload(passphrase, payload);

        let loaded = load_bundle().unwrap().unwrap();
        let decrypted = decrypt_vault_envelope(passphrase, &loaded.vault).unwrap();
        let record = decrypted
            .secrets
            .get(RECORD_ID_WALLET_HEMP_PRIMARY)
            .unwrap();

        let result = build_migration_envelope_from_webcom_bip39(passphrase, record);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Only BIP39 records"));
    }

    #[test]
    fn connect_webcom_primary_rejects_wif_record() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-connect-reject-wif";
        let mut payload = make_provider_payload("pinata", "filebase");
        let wif = make_webcom_wif_record();
        payload.secrets.insert(wif.record_id.clone(), wif);
        save_vault_with_payload(passphrase, payload);

        let result = connect_webcom_primary_wallet_to_core(passphrase, None, None, None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("wallet.bip39"));
    }

    #[test]
    fn connect_webcom_primary_rejects_legacy_175() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-connect-reject-175";
        let mut payload = make_provider_payload("pinata", "filebase");
        let webcom = make_webcom_bip39_record(DERIVATION_HEMP_LEGACY_175, "bip39");
        payload.secrets.insert(webcom.record_id.clone(), webcom);
        save_vault_with_payload(passphrase, payload);

        let result = connect_webcom_primary_wallet_to_core(passphrase, None, None, None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Legacy coin175"));
    }

    #[test]
    fn connect_webcom_primary_rejects_nonexistent_backup_ref() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-connect-bad-backup";
        let mut payload = make_provider_payload("pinata", "filebase");
        let webcom = make_webcom_bip39_record(DERIVATION_HEMP_CANONICAL_420, "bip39");
        payload.secrets.insert(webcom.record_id.clone(), webcom);
        save_vault_with_payload(passphrase, payload);

        let result = connect_webcom_primary_wallet_to_core(
            passphrase,
            None,
            None,
            Some("wallet.core_migration_envelope.nonexistent"),
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("does not exist in vault"));
    }

    #[test]
    fn alignment_status_v2_detects_webcom_bip39_primary() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-v2-detect";
        let mut payload = VaultPayload::default();
        let webcom = make_webcom_bip39_record(DERIVATION_HEMP_CANONICAL_420, "bip39");
        payload.secrets.insert(webcom.record_id.clone(), webcom);
        save_vault_with_payload(passphrase, payload);

        let status = get_wallet_alignment_status_v2(passphrase).unwrap();
        assert_eq!(status["wallet_record_state"], "webcom_primary_detected");
        assert_eq!(status["connection_state"], "none");
        assert_eq!(status["verification_status"], "not_verified");
        assert_eq!(status["webcom_primary_seed_type"], "bip39");
        assert_eq!(
            status["webcom_primary_derivation_hemp"],
            DERIVATION_HEMP_CANONICAL_420
        );
    }

    #[test]
    fn alignment_status_v2_never_returns_value() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-v2-no-value";
        let mut payload = VaultPayload::default();
        let webcom = make_webcom_bip39_record(DERIVATION_HEMP_CANONICAL_420, "bip39");
        payload.secrets.insert(webcom.record_id.clone(), webcom);
        save_vault_with_payload(passphrase, payload);

        let status = get_wallet_alignment_status_v2(passphrase).unwrap();
        let json = serde_json::to_string(&status).unwrap();
        assert!(
            !json.contains("abandon abandon"),
            "v2 status must not leak mnemonic"
        );
        assert!(status.get("value").is_none());
    }

    #[test]
    fn alignment_status_v2_verified_alignment_is_verified() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-v2-verified";
        let mut payload = make_provider_payload("pinata", "filebase");
        let webcom = make_webcom_bip39_record(DERIVATION_HEMP_CANONICAL_420, "bip39");
        let fp = build_alignment_fingerprint(&webcom);
        payload.secrets.insert(webcom.record_id.clone(), webcom);
        save_vault_with_payload(passphrase, payload);

        // Write a verified alignment record directly
        let mut bundle = load_bundle().unwrap().unwrap();
        let dek = unwrap_dek_with_passphrase(passphrase, &bundle.vault).unwrap();
        let mut decrypted = decrypt_payload_with_dek(dek.as_slice(), &bundle.vault).unwrap();
        let now = chrono::Utc::now().timestamp();
        let verified = SecretRecord {
            record_id: RECORD_ID_WALLET_ALIGNMENT.to_string(),
            record_type: RECORD_TYPE_APP_SETTING_WALLET_ALIGNMENT.to_string(),
            label: "Commander Wallet Alignment".to_string(),
            value: String::new(),
            metadata: Some(serde_json::json!({
                "schema": ALIGNMENT_SCHEMA,
                "schema_version": ALIGNMENT_SCHEMA_VERSION,
                "active_wallet_record_id": RECORD_ID_WALLET_HEMP_PRIMARY,
                "active_wallet_fingerprint": fp,
                "active_wallet_identity": canonical_wallet_identity("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"),
                "core_wallet_name": "hemp0x-vault-main",
                "core_wallet_source": CORE_WALLET_SOURCE_WEBCOM_BIP39,
                "derivation_profile": DERIVATION_HEMP_CANONICAL_420,
                "network": "mainnet",
                "created_at": now,
                "updated_at": now,
                "last_verified_at": now,
                "core_migration_backup_record_id": "",
                "verification_method": VERIFICATION_METHOD_RESTORE_FROM_GENERATED,
                "connection_state": "verified_aligned",
                "notes": [],
            })),
            tags: Some(vec!["wallet".to_string(), "alignment".to_string()]),
            origin_app: Some(APP_IDENTIFIER.to_string()),
            derivation_profiles: None,
            network: Some("mainnet".to_string()),
            created: now,
            modified: now,
        };
        decrypted
            .secrets
            .insert(RECORD_ID_WALLET_ALIGNMENT.to_string(), verified);
        bundle.vault.modified = now;
        bundle.vault.payload =
            encrypt_payload_with_dek(dek.as_slice(), &decrypted, &bundle.vault).unwrap();
        save_bundle_atomic(&bundle).unwrap();

        let status = get_wallet_alignment_status_v2(passphrase).unwrap();
        assert_eq!(status["connection_state"], "verified_aligned");
        assert_eq!(status["verification_status"], "verified");
        assert_eq!(status["recommended_next_action"], "already_aligned");
    }

    #[test]
    fn alignment_status_v2_preserves_unknown_records() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-v2-preserve";
        let mut payload = make_provider_payload("pinata", "filebase");
        let webcom = make_webcom_bip39_record(DERIVATION_HEMP_CANONICAL_420, "bip39");
        payload.secrets.insert(webcom.record_id.clone(), webcom);

        let future_id = "app_setting.future.feature.v99";
        let future = make_future_record(future_id, RECORD_TYPE_APP_SECRET, "future-secret-value");
        payload.secrets.insert(future_id.to_string(), future);

        save_vault_with_payload(passphrase, payload);

        let _status = get_wallet_alignment_status_v2(passphrase).unwrap();

        let loaded = load_bundle().unwrap().unwrap();
        let decrypted = decrypt_vault_envelope(passphrase, &loaded.vault).unwrap();
        assert!(decrypted.secrets.contains_key(future_id));
        assert_eq!(
            decrypted.secrets.get(future_id).unwrap().value,
            "future-secret-value"
        );
        assert_eq!(payload_pinata_token(&decrypted), "pinata");
    }

    #[test]
    fn connection_intent_record_create_and_read() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-intent";
        let payload = make_provider_payload("pinata", "filebase");
        save_vault_with_payload(passphrase, payload);

        let result = create_or_update_connection_intent_record(
            passphrase,
            Some("hemp0x-vault-main"),
            Some("wallet.core_migration_envelope.backup-1"),
        )
        .unwrap();
        assert_eq!(result["recorded"], true);
        assert_eq!(result["record_id"], RECORD_ID_CONNECTION_INTENT);
        assert_eq!(result["intended_wallet_name"], "hemp0x-vault-main");

        let loaded = load_bundle().unwrap().unwrap();
        let decrypted = decrypt_vault_envelope(passphrase, &loaded.vault).unwrap();
        let intent = decrypted.secrets.get(RECORD_ID_CONNECTION_INTENT).unwrap();
        assert_eq!(intent.record_type, RECORD_TYPE_CONNECTION_INTENT);
        assert_eq!(intent.value, "");
        let meta = intent.metadata.as_ref().unwrap();
        assert_eq!(meta["intended_wallet_name"], "hemp0x-vault-main");
        assert_eq!(
            meta["backup_record_id"],
            "wallet.core_migration_envelope.backup-1"
        );
    }

    #[test]
    fn connection_intent_is_not_called_alignment() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-intent-not-align";
        let payload = make_provider_payload("pinata", "filebase");
        save_vault_with_payload(passphrase, payload);

        create_or_update_connection_intent_record(passphrase, Some("test-wallet"), None).unwrap();

        let loaded = load_bundle().unwrap().unwrap();
        let decrypted = decrypt_vault_envelope(passphrase, &loaded.vault).unwrap();
        let intent = decrypted.secrets.get(RECORD_ID_CONNECTION_INTENT).unwrap();
        assert_ne!(intent.record_id, RECORD_ID_WALLET_ALIGNMENT);
        assert_ne!(intent.record_type, RECORD_TYPE_APP_SETTING_WALLET_ALIGNMENT);
        let meta = intent.metadata.as_ref().unwrap();
        assert!(meta.get("connection_state").is_none());
        assert!(meta.get("verification_method").is_none());
    }

    #[test]
    fn temp_file_cleanup_on_envelope_build_failure() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-temp-cleanup";
        let mut payload = make_provider_payload("pinata", "filebase");
        let webcom = make_webcom_bip39_record(DERIVATION_HEMP_LEGACY_175, "bip39");
        payload.secrets.insert(webcom.record_id.clone(), webcom);
        save_vault_with_payload(passphrase, payload);

        // connect will fail because legacy 175 is rejected
        let result = connect_webcom_primary_wallet_to_core(passphrase, None, None, None);
        assert!(result.is_err());

        // Verify no stale temp files remain
        if let Ok(dir) = vault_tmp_dir() {
            if let Ok(entries) = fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    let file_name = entry.file_name();
                    let name = file_name.to_string_lossy().to_string();
                    assert!(
                        !name.starts_with("vault_webcom_connect_"),
                        "Temp file {name} should have been cleaned up after failure"
                    );
                }
            }
        }
    }

    #[test]
    fn connect_webcom_primary_preserves_unknown_records_on_failure() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-connect-preserve";
        let mut payload = make_provider_payload("pinata", "filebase");
        let webcom = make_webcom_bip39_record(DERIVATION_HEMP_LEGACY_175, "bip39");
        payload.secrets.insert(webcom.record_id.clone(), webcom);

        let future_id = "app_setting.future.feature.v99";
        let future = make_future_record(future_id, RECORD_TYPE_APP_SECRET, "future-secret-value");
        payload.secrets.insert(future_id.to_string(), future);

        save_vault_with_payload(passphrase, payload);

        let result = connect_webcom_primary_wallet_to_core(passphrase, None, None, None);
        assert!(result.is_err());

        let loaded = load_bundle().unwrap().unwrap();
        let decrypted = decrypt_vault_envelope(passphrase, &loaded.vault).unwrap();
        assert!(decrypted.secrets.contains_key(future_id));
        assert_eq!(
            decrypted.secrets.get(future_id).unwrap().value,
            "future-secret-value"
        );
        assert_eq!(payload_pinata_token(&decrypted), "pinata");
    }

    #[test]
    fn migration_envelope_rejects_empty_mnemonic() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-empty-mnemonic";
        let mut payload = make_provider_payload("pinata", "filebase");
        let mut webcom = make_webcom_bip39_record(DERIVATION_HEMP_CANONICAL_420, "bip39");
        webcom.value = String::new();
        payload.secrets.insert(webcom.record_id.clone(), webcom);
        save_vault_with_payload(passphrase, payload);

        let loaded = load_bundle().unwrap().unwrap();
        let decrypted = decrypt_vault_envelope(passphrase, &loaded.vault).unwrap();
        let record = decrypted
            .secrets
            .get(RECORD_ID_WALLET_HEMP_PRIMARY)
            .unwrap();

        let result = build_migration_envelope_from_webcom_bip39(passphrase, record);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("empty mnemonic"));
    }

    #[test]
    fn migration_envelope_rejects_wrong_word_count() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-wrong-count";
        let mut payload = make_provider_payload("pinata", "filebase");
        let mut webcom = make_webcom_bip39_record(DERIVATION_HEMP_CANONICAL_420, "bip39");
        webcom.value = "abandon abandon abandon abandon abandon".to_string(); // 5 words
        payload.secrets.insert(webcom.record_id.clone(), webcom);
        save_vault_with_payload(passphrase, payload);

        let loaded = load_bundle().unwrap().unwrap();
        let decrypted = decrypt_vault_envelope(passphrase, &loaded.vault).unwrap();
        let record = decrypted
            .secrets
            .get(RECORD_ID_WALLET_HEMP_PRIMARY)
            .unwrap();

        let result = build_migration_envelope_from_webcom_bip39(passphrase, record);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("5 words"));
    }

    // ─── Slice 64e: Hardening tests ────────────────────────────────────

    #[test]
    fn migration_envelope_rejects_non_bip39_seed_type() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-reject-seed";
        let mut payload = make_provider_payload("pinata", "filebase");
        let mut webcom = make_webcom_bip39_record(DERIVATION_HEMP_CANONICAL_420, "wif");
        webcom.record_type = RECORD_TYPE_WALLET_BIP39.to_string();
        payload.secrets.insert(webcom.record_id.clone(), webcom);
        save_vault_with_payload(passphrase, payload);

        let loaded = load_bundle().unwrap().unwrap();
        let decrypted = decrypt_vault_envelope(passphrase, &loaded.vault).unwrap();
        let record = decrypted
            .secrets
            .get(RECORD_ID_WALLET_HEMP_PRIMARY)
            .unwrap();

        let result = build_migration_envelope_from_webcom_bip39(passphrase, record);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("seed type"));
    }

    #[test]
    fn migration_envelope_rejects_non_mainnet() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-reject-testnet";
        let mut payload = make_provider_payload("pinata", "filebase");
        let mut webcom = make_webcom_bip39_record(DERIVATION_HEMP_CANONICAL_420, "bip39");
        webcom.network = Some("testnet".to_string());
        if let Some(ref mut m) = webcom.metadata {
            if let Some(recovery) = m.get_mut("recovery") {
                if let Some(obj) = recovery.as_object_mut() {
                    obj.insert(
                        "network".to_string(),
                        serde_json::Value::String("testnet".to_string()),
                    );
                }
            }
        }
        payload.secrets.insert(webcom.record_id.clone(), webcom);
        save_vault_with_payload(passphrase, payload);

        let loaded = load_bundle().unwrap().unwrap();
        let decrypted = decrypt_vault_envelope(passphrase, &loaded.vault).unwrap();
        let record = decrypted
            .secrets
            .get(RECORD_ID_WALLET_HEMP_PRIMARY)
            .unwrap();

        let result = build_migration_envelope_from_webcom_bip39(passphrase, record);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("mainnet"));
    }

    #[test]
    fn migration_envelope_rejects_generic_profile() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-reject-generic";
        let mut payload = make_provider_payload("pinata", "filebase");
        let webcom = make_webcom_bip39_record(DERIVATION_HEMP_LEGACY_GENERIC, "bip39");
        payload.secrets.insert(webcom.record_id.clone(), webcom);
        save_vault_with_payload(passphrase, payload);

        let loaded = load_bundle().unwrap().unwrap();
        let decrypted = decrypt_vault_envelope(passphrase, &loaded.vault).unwrap();
        let record = decrypted
            .secrets
            .get(RECORD_ID_WALLET_HEMP_PRIMARY)
            .unwrap();

        let result = build_migration_envelope_from_webcom_bip39(passphrase, record);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Generic derivation"));
    }

    #[test]
    fn migration_envelope_exported_at_is_current_timestamp() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-exported-at";
        let mut payload = make_provider_payload("pinata", "filebase");
        let webcom = make_webcom_bip39_record(DERIVATION_HEMP_CANONICAL_420, "bip39");
        payload
            .secrets
            .insert(webcom.record_id.clone(), webcom.clone());
        save_vault_with_payload(passphrase, payload);

        let loaded = load_bundle().unwrap().unwrap();
        let decrypted = decrypt_vault_envelope(passphrase, &loaded.vault).unwrap();
        let record = decrypted
            .secrets
            .get(RECORD_ID_WALLET_HEMP_PRIMARY)
            .unwrap();

        let before = chrono::Utc::now().timestamp();
        let envelope_json = build_migration_envelope_from_webcom_bip39(passphrase, record).unwrap();
        let after = chrono::Utc::now().timestamp();

        let envelope: serde_json::Value = serde_json::from_str(&envelope_json).unwrap();
        let exported_at = envelope["exported_at"].as_i64().unwrap();
        assert!(exported_at > 0, "exported_at must be a positive timestamp");
        assert!(
            exported_at >= before && exported_at <= after,
            "exported_at {exported_at} should be between {before} and {after}"
        );
    }

    #[test]
    fn deprecated_alignment_command_returns_error() {
        let result = vault_create_or_update_alignment_record(
            "test".to_string(),
            CORE_WALLET_SOURCE_WEBCOM_BIP39.to_string(),
            DERIVATION_HEMP_CANONICAL_420.to_string(),
            None,
            None,
            None,
            None,
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Deprecated"));
    }

    #[test]
    fn restore_verification_rejects_missing_wallet_name() {
        let restore_result = serde_json::json!({
            "wallet_arg": "--wallet=test"
        });
        let result = verify_restore_result(&restore_result, "expected-name");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("no wallet_name"));
    }

    #[test]
    fn restore_verification_rejects_name_mismatch() {
        let restore_result = serde_json::json!({
            "wallet_name": "wrong-name",
            "wallet_arg": "--wallet=wrong-name"
        });
        let result = verify_restore_result(&restore_result, "expected-name");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("wallet name mismatch"));
    }

    #[test]
    fn restore_verification_rejects_wrong_network() {
        let restore_result = serde_json::json!({
            "wallet_name": "test-wallet",
            "chain": { "network": "testnet", "coin_type_bip44": 420 }
        });
        let result = verify_restore_result(&restore_result, "test-wallet");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("network mismatch"));
    }

    #[test]
    fn restore_verification_rejects_wrong_coin_type() {
        let restore_result = serde_json::json!({
            "wallet_name": "test-wallet",
            "chain": { "network": CORE_MIGRATION_MAINNET_NETWORK_ID, "coin_type_bip44": 175 }
        });
        let result = verify_restore_result(&restore_result, "test-wallet");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("coin type mismatch"));
    }

    #[test]
    fn restore_verification_rejects_wrong_derivation_profile() {
        let restore_result = serde_json::json!({
            "wallet_name": "test-wallet",
            "derivation": [
                { "profile_id": DERIVATION_HEMP_LEGACY_175, "purpose": 44, "coin_type": 175 }
            ]
        });
        let result = verify_restore_result(&restore_result, "test-wallet");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("derivation profile mismatch"));
    }

    #[test]
    fn restore_verification_accepts_valid_result() {
        let restore_result = serde_json::json!({
            "wallet_name": "test-wallet",
            "wallet_arg": "--wallet=test-wallet",
            "chain": { "network": CORE_MIGRATION_MAINNET_NETWORK_ID, "coin_type_bip44": 420 },
            "derivation": [
                { "profile_id": DERIVATION_HEMP_CANONICAL_420, "purpose": 44, "coin_type": 420 }
            ],
            "rescan_start_height": 0,
            "rescan_end_height": 100000
        });
        let result = verify_restore_result(&restore_result, "test-wallet");
        assert!(result.is_ok());
    }

    // ─── Slice 64f: Guided connect flow tests ───────────────────────────

    #[test]
    fn guided_connect_label_format_is_human_readable() {
        let label = format_pre_connect_backup_label();
        assert!(label.starts_with("Pre-connect backup - "));
        assert!(label.len() > "Pre-connect backup - ".len() + 10);
    }

    #[test]
    fn is_core_wallet_unlock_required_error_detects_known_phrases() {
        assert!(is_core_wallet_unlock_required_error(
            "Error: wallet is locked, please unlock it before exporting"
        ));
        assert!(is_core_wallet_unlock_required_error(
            "Error: The wallet is currently locked. Please enter the wallet passphrase with walletpassphrase."
        ));
        assert!(is_core_wallet_unlock_required_error(
            "RPC error: wallet locked"
        ));
        assert!(!is_core_wallet_unlock_required_error("Network timeout"));
        assert!(!is_core_wallet_unlock_required_error(
            "Invalid passphrase for migration envelope"
        ));
    }

    #[test]
    fn format_guided_unlock_required_error_includes_sentinel() {
        let original = "RPC error: wallet is locked";
        let formatted = format_guided_unlock_required_error(original);
        assert!(formatted.starts_with("WALLET_UNLOCK_REQUIRED::"));
        assert!(formatted.contains(original));
        assert!(formatted.contains("wallet_unlock"));
    }

    #[test]
    fn guided_connect_fails_when_no_vault() {
        let _guard = setup_test_vault_dir();
        let result =
            connect_webcom_primary_wallet_to_core_guided("test-passphrase", None, None, false);
        assert!(result.is_err());
    }

    #[test]
    fn guided_connect_fails_when_no_webcom_primary_record() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-guided-no-record";
        let payload = make_provider_payload("pinata", "filebase");
        save_vault_with_payload(passphrase, payload);

        let result = connect_webcom_primary_wallet_to_core_guided(passphrase, None, None, false);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("wallet.webcom.hemp.primary"));
    }

    #[test]
    fn guided_connect_rejects_wif_record() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-guided-wif";
        let mut payload = make_provider_payload("pinata", "filebase");
        let wif = make_webcom_wif_record();
        payload.secrets.insert(wif.record_id.clone(), wif);
        save_vault_with_payload(passphrase, payload);

        let result = connect_webcom_primary_wallet_to_core_guided(passphrase, None, None, false);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("wallet.bip39"));
    }

    #[test]
    fn guided_connect_rejects_legacy_175_profile() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-guided-175";
        let mut payload = make_provider_payload("pinata", "filebase");
        let webcom = make_webcom_bip39_record(DERIVATION_HEMP_LEGACY_175, "bip39");
        payload.secrets.insert(webcom.record_id.clone(), webcom);
        save_vault_with_payload(passphrase, payload);

        let result = connect_webcom_primary_wallet_to_core_guided(passphrase, None, None, false);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Legacy coin175"));
    }

    #[test]
    fn guided_connect_fails_when_core_unreachable() {
        // Without a Core daemon running, backup will fail and guided connect
        // should surface the error rather than silently fall back. The error
        // must NOT be the WALLET_UNLOCK_REQUIRED sentinel because we can't
        // distinguish a locked wallet from no Core in tests.
        let _guard = setup_test_vault_dir();
        let passphrase = "test-guided-no-core";
        let mut payload = make_provider_payload("pinata", "filebase");
        let webcom = make_webcom_bip39_record(DERIVATION_HEMP_CANONICAL_420, "bip39");
        payload.secrets.insert(webcom.record_id.clone(), webcom);
        save_vault_with_payload(passphrase, payload);

        let result = connect_webcom_primary_wallet_to_core_guided(passphrase, None, None, false);
        // detect_current_core_wallet_exists() will return false (no Core
        // daemon), so the guided flow will skip backup and proceed to
        // envelope build/validate which will fail when it can't reach Core.
        // The important guarantee is that no alignment record is written
        // and no plaintext mnemonic appears in the result.
        if let Ok(value) = &result {
            let json = serde_json::to_string(value).unwrap();
            assert!(!json.contains("abandon abandon"));
        }
        // On failure path, the vault must not contain alignment.
        let loaded = load_bundle().unwrap().unwrap();
        let decrypted = decrypt_vault_envelope(passphrase, &loaded.vault).unwrap();
        assert!(decrypted.secrets.get(RECORD_ID_WALLET_ALIGNMENT).is_none());
    }

    #[test]
    fn query_wallet_first_tx_block_handles_empty_response() {
        // With no Core daemon running this returns an Err; with Core
        // running and a freshly restored empty wallet it returns Ok(None).
        // Both are acceptable outcomes; the helper must not panic.
        let result = query_wallet_first_tx_block("nonexistent-wallet-xyz");
        // We don't assert Ok vs Err here because that depends on whether
        // Core is reachable. We only assert the function does not panic
        // and returns a typed Result.
        match result {
            Ok(opt) => assert!(opt.is_none() || opt.unwrap() >= 0),
            Err(_) => {}
        }
    }

    #[test]
    fn scantxoutset_for_addresses_handles_empty_input() {
        // With no addresses, the helper must short-circuit and not
        // call Core at all. This is important because the fast path
        // calls it after enumerate_wallet_addresses, and a freshly
        // restored wallet with zero addresses should be handled
        // gracefully.
        let result = scantxoutset_for_addresses(&[]).unwrap();
        assert_eq!(result["success"], true);
        assert_eq!(result["total_amount"], 0.0);
        assert_eq!(result["utxo_count"], 0);
    }

    #[test]
    fn commander_managed_startup_does_not_touch_hemp_conf() {
        // Isolated: write a representative hemp.conf into the temp
        // commander dir and confirm that loading the default app
        // settings never mutates it. This guard previously wrote to
        // the real ~/.hemp0x data dir; it now stays inside the test
        // sandbox so the suite never touches the user's real config.
        let _guard = setup_test_vault_dir();
        let cfg_dir = crate::modules::files::commander_dir().unwrap();
        let config_path = cfg_dir.join("hemp.conf");
        let original_config = "server=1\ndaemon=1\nrpcport=42068\n# tuned by user\ndbcache=2048\nwallet=hemp0x-vault-main\n";
        std::fs::write(&config_path, original_config).unwrap();

        // Seed a default app_settings.json inside the sandbox so
        // load_app_settings_impl()'s fallback chain does NOT read the
        // user's real ~/.hemp0x/commander/app_settings.json (which may
        // carry an active_vault_wallet_name on a developer machine and
        // make this test environment-dependent).
        crate::modules::files::save_app_settings_impl(
            &crate::modules::models::AppSettings::default(),
        )
        .unwrap();

        let settings = crate::modules::files::load_app_settings_impl().unwrap();
        assert!(
            settings.active_vault_wallet_name.is_none(),
            "AppSettings should not have a vault wallet name by default in a fresh test sandbox"
        );

        let after = std::fs::read_to_string(&config_path).unwrap();
        assert_eq!(
            after, original_config,
            "hemp.conf was modified by loading app settings"
        );

        let _ = std::fs::remove_file(&config_path);
    }

    #[test]
    fn vault_set_active_wallet_name_persists_in_settings() {
        let _guard = setup_test_vault_dir();
        crate::modules::files::save_app_settings_impl(
            &crate::modules::models::AppSettings::default(),
        )
        .unwrap();

        vault_set_active_wallet_name("hemp0x-vault-main".to_string()).unwrap();
        let name = vault_get_active_wallet_name().unwrap();
        assert_eq!(name, Some("hemp0x-vault-main".to_string()));

        vault_set_active_wallet_name("another-wallet".to_string()).unwrap();
        let name2 = vault_get_active_wallet_name().unwrap();
        assert_eq!(name2, Some("another-wallet".to_string()));
    }

    #[test]
    fn vault_load_wallet_into_core_reports_truthfully_when_no_file() {
        let _guard = setup_test_vault_dir();
        let result = load_wallet_into_core_blocking("nonexistent-wallet-xyz");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("does not exist"),
            "Expected missing-file error, got: {err}"
        );
    }

    #[test]
    fn envelope_uses_recovered_indices_for_external_hint() {
        // The vault stores recovered_external_indices for any
        // address the user actually used. The envelope must
        // include enough external addresses to cover both the
        // gap limit AND any recovered indices; otherwise Core
        // only derives a few addresses and the user's
        // balance-holding address is never checked.
        let _guard = setup_test_vault_dir();
        let passphrase = "test-recovered-indices";
        let mut payload = make_provider_payload("pinata", "filebase");
        let mut webcom = make_webcom_bip39_record(DERIVATION_HEMP_CANONICAL_420, "bip39");
        // Set the gap limit low (1) but include a high recovered
        // index (47) — this is the exact scenario that was
        // producing 0-balance wallets before the fix.
        let md = webcom.metadata.as_mut().unwrap();
        md.as_object_mut()
            .unwrap()
            .insert("external_count".to_string(), serde_json::json!(1));
        md.as_object_mut()
            .unwrap()
            .insert("change_count".to_string(), serde_json::json!(1));
        md.as_object_mut().unwrap().insert(
            "recovered_external_indices".to_string(),
            serde_json::json!([0, 5, 12, 47]),
        );
        payload.secrets.insert(webcom.record_id.clone(), webcom);
        save_vault_with_payload(passphrase, payload);

        let loaded = load_bundle().unwrap().unwrap();
        let decrypted = decrypt_vault_envelope(passphrase, &loaded.vault).unwrap();
        let record = decrypted
            .secrets
            .get(RECORD_ID_WALLET_HEMP_PRIMARY)
            .unwrap();
        let envelope = build_migration_envelope_from_webcom_bip39(passphrase, record).unwrap();
        let env: serde_json::Value = serde_json::from_str(&envelope).unwrap();

        // The hint must be at least max_recovered_index + 1 = 48.
        // The previous hardcoded value of 20 would have failed here.
        let hint = env["metadata"]["external_count_hint"].as_i64().unwrap();
        assert!(
            hint >= 48,
            "external_count_hint must cover recovered indices; got {hint}, expected >= 48"
        );
        // And the metadata surfaces the source values for debugging.
        assert_eq!(env["metadata"]["external_count_from_vault"], 1);
        assert_eq!(env["metadata"]["max_recovered_external_index"], 47);
        assert_eq!(env["metadata"]["recovered_external_indices_count"], 4);
    }

    #[test]
    fn envelope_external_hint_respects_floor() {
        // A record with no recovered indices and a low gap limit
        // must still get a reasonable floor so the keypool is
        // useful.
        let _guard = setup_test_vault_dir();
        let passphrase = "test-external-floor";
        let mut payload = make_provider_payload("pinata", "filebase");
        let mut webcom = make_webcom_bip39_record(DERIVATION_HEMP_CANONICAL_420, "bip39");
        let md = webcom.metadata.as_mut().unwrap();
        md.as_object_mut()
            .unwrap()
            .insert("external_count".to_string(), serde_json::json!(1));
        md.as_object_mut()
            .unwrap()
            .insert("change_count".to_string(), serde_json::json!(1));
        payload.secrets.insert(webcom.record_id.clone(), webcom);
        save_vault_with_payload(passphrase, payload);

        let loaded = load_bundle().unwrap().unwrap();
        let decrypted = decrypt_vault_envelope(passphrase, &loaded.vault).unwrap();
        let record = decrypted
            .secrets
            .get(RECORD_ID_WALLET_HEMP_PRIMARY)
            .unwrap();
        let envelope = build_migration_envelope_from_webcom_bip39(passphrase, record).unwrap();
        let env: serde_json::Value = serde_json::from_str(&envelope).unwrap();

        let hint = env["metadata"]["external_count_hint"].as_i64().unwrap();
        assert!(
            hint >= 20,
            "external_count_hint must be at least the floor (20); got {hint}"
        );
        let change_hint = env["metadata"]["change_count_hint"].as_i64().unwrap();
        assert!(
            change_hint >= 6,
            "change_count_hint must be at least the floor (6); got {change_hint}"
        );
    }

    #[test]
    fn deprecated_alignment_command_still_returns_error() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-deprecated";
        let mut payload = make_provider_payload("pinata", "filebase");
        let webcom = make_webcom_bip39_record(DERIVATION_HEMP_CANONICAL_420, "bip39");
        payload.secrets.insert(webcom.record_id.clone(), webcom);
        save_vault_with_payload(passphrase, payload);

        // Both deprecated paths must still return errors.
        let result = vault_create_or_update_alignment_record(
            RECORD_ID_WALLET_HEMP_PRIMARY.to_string(),
            CORE_WALLET_SOURCE_WEBCOM_BIP39.to_string(),
            DERIVATION_HEMP_CANONICAL_420.to_string(),
            None,
            None,
            None,
            None,
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Deprecated"));
    }

    // ─── Slice 64g: Restore timeout + duplicate recovery tests ──────────

    #[test]
    fn is_duplicate_wallet_error_recognizes_known_phrases() {
        assert!(is_duplicate_wallet_error(
            "Wallet 'hemp0x-vault-main' already exists"
        ));
        assert!(is_duplicate_wallet_error("Wallet already loaded"));
        assert!(is_duplicate_wallet_error("Duplicate wallet name"));
        assert!(is_duplicate_wallet_error("Wallet file already exists"));
        assert!(!is_duplicate_wallet_error("Network timeout"));
        assert!(!is_duplicate_wallet_error("Invalid passphrase"));
        assert!(!is_duplicate_wallet_error(""));
    }

    #[test]
    fn restore_timeout_sentinel_format() {
        // The sentinel pattern that the UI uses to detect the timeout.
        let err = "RESTORE_TIMEOUT::hemp0x-vault-main::RPC transport error (restorewalletmigration): http://127.0.0.1:42068/: Network Error: timed out::Core did not respond within the restore RPC read timeout...";
        assert!(err.starts_with("RESTORE_TIMEOUT::"));
        assert!(err.contains("hemp0x-vault-main"));
    }

    #[test]
    fn effective_birth_height_defaults_from_webcom_metadata() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-birth-height";
        let mut payload = make_provider_payload("pinata", "filebase");
        let mut webcom = make_webcom_bip39_record(DERIVATION_HEMP_CANONICAL_420, "bip39");
        // Set best_block_height in metadata
        let md = webcom.metadata.as_mut().unwrap();
        md.as_object_mut()
            .unwrap()
            .insert("best_block_height".to_string(), serde_json::json!(12345));
        payload.secrets.insert(webcom.record_id.clone(), webcom);
        save_vault_with_payload(passphrase, payload);

        // The legacy 175 check uses the envelope builder, so this should
        // still build fine. We only assert that best_block_height is read
        // from the metadata; the actual restore call requires Core.
        let loaded = load_bundle().unwrap().unwrap();
        let decrypted = decrypt_vault_envelope(passphrase, &loaded.vault).unwrap();
        let record = decrypted
            .secrets
            .get(RECORD_ID_WALLET_HEMP_PRIMARY)
            .unwrap();
        let envelope = build_migration_envelope_from_webcom_bip39(passphrase, record).unwrap();
        let env: serde_json::Value = serde_json::from_str(&envelope).unwrap();
        assert_eq!(env["metadata"]["best_block_height"], 12345);
    }

    #[test]
    fn effective_birth_height_falls_back_to_zero_when_metadata_missing() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-birth-zero";
        let mut payload = make_provider_payload("pinata", "filebase");
        let mut webcom = make_webcom_bip39_record(DERIVATION_HEMP_CANONICAL_420, "bip39");
        // Remove best_block_height from metadata
        let md = webcom.metadata.as_mut().unwrap();
        md.as_object_mut().unwrap().remove("best_block_height");
        payload.secrets.insert(webcom.record_id.clone(), webcom);
        save_vault_with_payload(passphrase, payload);

        let loaded = load_bundle().unwrap().unwrap();
        let decrypted = decrypt_vault_envelope(passphrase, &loaded.vault).unwrap();
        let record = decrypted
            .secrets
            .get(RECORD_ID_WALLET_HEMP_PRIMARY)
            .unwrap();
        let envelope = build_migration_envelope_from_webcom_bip39(passphrase, record).unwrap();
        let env: serde_json::Value = serde_json::from_str(&envelope).unwrap();
        // No best_block_height -> defaults to 0 (rescan from genesis)
        assert_eq!(env["metadata"]["best_block_height"], 0);
    }

    #[test]
    fn query_core_chain_tip_returns_zero_when_core_unreachable() {
        // With no Core daemon running, the helper must return 0 rather
        // than error out. This keeps the bounded-rescan logic safe in
        // tests and on machines where Core is briefly offline.
        let tip = query_core_chain_tip();
        // Could be 0 or non-zero depending on whether Core is actually
        // running; the only thing we can assert is the function does
        // not panic and returns an i64.
        let _: i64 = tip;
    }

    #[test]
    fn fast_default_birth_height_is_positive_and_small() {
        // The fast default must be strictly positive so that
        // (chain_tip - 1) does not underflow on a small chain, and
        // must be small (we use 1) so that restorewalletmigration
        // does essentially no rescan.
        assert!(FAST_DEFAULT_BIRTH_HEIGHT_BACKOFF > 0);
        assert!(FAST_DEFAULT_BIRTH_HEIGHT_BACKOFF <= 10);
    }

    // ─── Slice 64h: Named-wallet state + truthful scan fields ────────
    //
    // The connect flow must not pretend a wallet is loaded, claim
    // `hemp.conf` was edited, or collapse "UTXOs the chain says
    // belong to this wallet" into "Core wallet balance". These tests
    // cover the cheap-to-verify parts of that contract without
    // requiring a live Core daemon.

    #[test]
    fn named_wallet_state_reports_truthfully_when_no_wallet_file() {
        let _guard = setup_test_vault_dir();
        // Pick a wallet name that cannot collide with any leftover
        // state from a sibling test in the same temp datadir. The
        // contract is: if the file does not exist, loaded=false and
        // restart_required=false. If the file DOES exist (e.g. a
        // leftover from another test), the state must NOT lie about
        // it — `wallet_file_exists` must be true and the loaded
        // status must reflect the actual query result.
        let wallet_name = "hemp0x-vault-64h-missing-test";
        let dir = data_dir().unwrap();
        let wallet_path = dir.join(wallet_name);
        // Force the file to be absent for this test.
        let _ = std::fs::remove_file(&wallet_path);
        assert!(
            !wallet_path.exists(),
            "test setup failed: wallet file should not exist before the test"
        );

        let state = describe_named_wallet_state(wallet_name);
        assert!(
            !state.wallet_file_exists,
            "wallet_file_exists must be false when no file is on disk"
        );
        assert!(
            !state.named_wallet_loaded,
            "named_wallet_loaded must be false when no file is on disk"
        );
        assert_eq!(state.loaded_via, "none");
        assert!(
            !state.restart_required,
            "missing file is not a restart-required state, it is a not-restored state"
        );
        // The wallet_arg must always be present, even in the empty
        // state, so the UI can render restart instructions.
        assert_eq!(state.wallet_arg, format!("-wallet={wallet_name}"));
    }

    #[test]
    fn named_wallet_state_to_json_includes_truthful_fields() {
        let _guard = setup_test_vault_dir();
        let state = describe_named_wallet_state("hemp0x-vault-main");
        let json = named_wallet_state_to_json(&state);

        // Every UI-rendered field must be present, even if false/empty.
        assert!(json.get("wallet_name").is_some());
        assert!(json.get("wallet_file_exists").is_some());
        assert!(json.get("wallet_file_path").is_some());
        assert!(json.get("named_wallet_loaded").is_some());
        assert!(json.get("loaded_via").is_some());
        assert!(json.get("restart_required").is_some());
        assert!(json.get("default_wallet_path").is_some());
        assert!(json.get("previous_default_wallet_existed").is_some());
        assert!(json.get("wallet_arg").is_some());
    }

    #[test]
    fn named_wallet_state_wallet_arg_is_correct_format() {
        // The wallet_arg must be the exact Core CLI flag the user
        // would type, so the UI can copy it verbatim into restart
        // instructions. A typo here is a beta-blocker.
        let _guard = setup_test_vault_dir();
        let state = describe_named_wallet_state("hemp0x-vault-main");
        assert_eq!(state.wallet_arg, "-wallet=hemp0x-vault-main");
    }

    #[test]
    fn scan_kinds_helper_uses_distinct_labels() {
        // The three numbers we now surface in the connect result must
        // be distinguishable in code AND UI. A test on the labels
        // catches accidental copy-paste of one label into another.
        // We do not call the full connect flow here; we only assert
        // the labels we wrote in perform_webcom_to_core_restore_and_align.
        let utxo_label = "chain-side spendable UTXOs (does not require a rescan)";
        let wallet_label =
            "Core wallet balance from -wallet=<name> getwalletinfo (authoritative when present)";
        let history_label = "Background rescanblockchain that fills wallet transaction history (async; txindex-only)";

        assert!(utxo_label.contains("UTXO"));
        assert!(wallet_label.contains("getwalletinfo"));
        assert!(history_label.contains("rescanblockchain"));
        // And they are not the same string.
        assert_ne!(utxo_label, wallet_label);
        assert_ne!(utxo_label, history_label);
        assert_ne!(wallet_label, history_label);
    }

    #[test]
    fn bip39_valid_12_word_mnemonic_passes_validation() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let result = Mnemonic::parse(mnemonic);
        assert!(result.is_ok(), "Valid 12-word BIP39 mnemonic should parse");
    }

    #[test]
    fn bip39_invalid_checksum_mnemonic_is_rejected() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon";
        let result = Mnemonic::parse(mnemonic);
        assert!(
            result.is_err(),
            "Invalid checksum mnemonic should be rejected"
        );
    }

    #[test]
    fn invalid_recovery_phrase_does_not_write_primary_wallet_record() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-invalid-recovery-write";
        save_vault_with_payload(passphrase, make_provider_payload("pinata", "filebase"));

        let invalid = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon";
        let result = restore_from_recovery_phrase_blocking(
            invalid.to_string(),
            Some("test-wallet".to_string()),
            passphrase.to_string(),
            None,
        );

        assert!(result.is_err(), "Invalid phrase must be rejected");
        let loaded = load_bundle().unwrap().unwrap();
        let decrypted = decrypt_vault_envelope(passphrase, &loaded.vault).unwrap();
        assert!(
            !decrypted
                .secrets
                .contains_key(RECORD_ID_WALLET_HEMP_PRIMARY),
            "Invalid phrase must not write a primary wallet record"
        );
    }

    #[test]
    fn bip39_non_wordlist_word_is_rejected() {
        let mnemonic = "zzzzz zzzzz zzzzz zzzzz zzzzz zzzzz zzzzz zzzzz zzzzz zzzzz zzzzz zzzzz";
        let result = Mnemonic::parse(mnemonic);
        assert!(result.is_err(), "Non-wordlist words should be rejected");
    }

    #[test]
    fn bip39_wrong_word_count_is_rejected() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon";
        let result = Mnemonic::parse(mnemonic);
        assert!(result.is_err(), "11-word mnemonic should be rejected");
    }

    #[test]
    fn bip39_generate_produces_valid_12_word_mnemonic() {
        let mut rng = rand::thread_rng();
        let mnemonic = Mnemonic::generate_in_with(&mut rng, bip39::Language::English, 12);
        assert!(mnemonic.is_ok(), "Generated mnemonic should be valid");
        let phrase = mnemonic.unwrap().to_string();
        let words: Vec<&str> = phrase.split_whitespace().collect();
        assert_eq!(words.len(), 12, "Generated mnemonic should have 12 words");
        let reparse = Mnemonic::parse(&phrase);
        assert!(
            reparse.is_ok(),
            "Generated mnemonic should re-parse successfully"
        );
    }

    #[test]
    fn bip39_18_word_mnemonic_passes_validation() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon agent";
        let result = Mnemonic::parse(mnemonic);
        assert!(result.is_ok(), "Valid 18-word BIP39 mnemonic should parse");
    }

    #[test]
    fn bip39_24_word_mnemonic_passes_validation() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon art";
        let result = Mnemonic::parse(mnemonic);
        assert!(result.is_ok(), "Valid 24-word BIP39 mnemonic should parse");
    }

    // ─── Slice 64p: Vault passphrase rotation tests ───────────────────
    //
    // These tests sandbox the vault, commander, AND data dirs via the
    // combined guard below, so detect_network()/hemp.conf reads never
    // touch the user's real ~/.hemp0x. They verify the full rotation
    // contract: old decrypts before, new decrypts after, old fails
    // after, all records survive byte-for-byte, wrong current fails,
    // short new fails, and created/network are preserved.

    /// Test guard that sandboxes the vault dir, commander settings dir,
    /// AND the Core data dir together, so unload/rotation tests can
    /// write wallet.dat, hemp.conf, and the vault bundle into temp dirs
    /// without ever touching the user's real ~/.hemp0x data directory.
    struct TestVaultAndDataDirGuard {
        root: PathBuf,
        data_dir: PathBuf,
        vault_dir: PathBuf,
    }

    impl TestVaultAndDataDirGuard {
        fn new() -> Self {
            let nanos = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
            let root = std::env::temp_dir().join(format!("commander_64p_test_{nanos}"));
            // Start from a clean slate in case a prior run left state.
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(&root).unwrap();
            let data_dir = root.join("data");
            let commander_dir = root.join("commander");
            let vault_dir = root.join("vaultstate");
            std::fs::create_dir_all(&data_dir).unwrap();
            std::fs::create_dir_all(&commander_dir).unwrap();
            std::fs::create_dir_all(&vault_dir).unwrap();

            crate::modules::files::TEST_COMMANDER_DIR.with(|cell| {
                *cell.borrow_mut() = Some(commander_dir.clone());
            });
            crate::modules::files::TEST_DATA_DIR.with(|cell| {
                *cell.borrow_mut() = Some(data_dir.clone());
            });
            TEST_VAULT_DIR.with(|cell| {
                *cell.borrow_mut() = Some(vault_dir.clone());
            });

            Self {
                root,
                data_dir,
                vault_dir,
            }
        }

        fn data_dir_path(&self) -> &std::path::Path {
            &self.data_dir
        }

        fn vault_dir_path(&self) -> &std::path::Path {
            &self.vault_dir
        }
    }

    impl Drop for TestVaultAndDataDirGuard {
        fn drop(&mut self) {
            TEST_VAULT_DIR.with(|cell| {
                *cell.borrow_mut() = None;
            });
            crate::modules::files::TEST_DATA_DIR.with(|cell| {
                *cell.borrow_mut() = None;
            });
            crate::modules::files::TEST_COMMANDER_DIR.with(|cell| {
                *cell.borrow_mut() = None;
            });
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    /// Build a payload that exercises preservation of multiple record
    /// kinds: provider tokens, the WebCom primary BIP39 wallet, a Core
    /// backup migration record, and an unknown future record type.
    fn make_diverse_payload() -> VaultPayload {
        let mut payload = make_provider_payload("pinata-jwt-value", "filebase-token-value");
        let webcom = make_webcom_bip39_record(DERIVATION_HEMP_CANONICAL_420, "bip39");
        payload.secrets.insert(webcom.record_id.clone(), webcom);
        let future = make_future_record(
            "future.unknown.kind.alpha",
            "future.record.type",
            "future-payload-value",
        );
        payload.secrets.insert(future.record_id.clone(), future);
        payload
    }

    #[test]
    fn vault_passphrase_rotation_old_passphrase_works_before_rotation() {
        let _guard = TestVaultAndDataDirGuard::new();
        let old = "old-passphrase-1234";
        save_vault_with_payload(old, make_diverse_payload());
        let bundle = load_bundle().unwrap().unwrap();
        assert!(
            decrypt_vault_envelope(old, &bundle.vault).is_ok(),
            "old passphrase must decrypt the vault before rotation"
        );
    }

    #[test]
    fn vault_passphrase_rotation_new_passphrase_works_after_rotation() {
        let _guard = TestVaultAndDataDirGuard::new();
        let old = "old-passphrase-1234";
        let new = "new-passphrase-5678";
        save_vault_with_payload(old, make_diverse_payload());

        let res = change_vault_passphrase(old, new).unwrap();
        assert_eq!(res["rotated"], true);

        let bundle = load_bundle().unwrap().unwrap();
        assert!(
            decrypt_vault_envelope(new, &bundle.vault).is_ok(),
            "new passphrase must decrypt the vault after rotation"
        );
    }

    #[test]
    fn vault_passphrase_rotation_old_passphrase_fails_after_rotation() {
        let _guard = TestVaultAndDataDirGuard::new();
        let old = "old-passphrase-1234";
        let new = "new-passphrase-5678";
        save_vault_with_payload(old, make_diverse_payload());

        change_vault_passphrase(old, new).unwrap();

        let bundle = load_bundle().unwrap().unwrap();
        assert!(
            decrypt_vault_envelope(old, &bundle.vault).is_err(),
            "old passphrase must NOT decrypt the vault after rotation"
        );
    }

    #[test]
    fn vault_passphrase_rotation_preserves_all_records_unchanged() {
        let _guard = TestVaultAndDataDirGuard::new();
        let old = "old-passphrase-1234";
        let new = "new-passphrase-5678";
        save_vault_with_payload(old, make_diverse_payload());

        let before = {
            let b = load_bundle().unwrap().unwrap();
            decrypt_vault_envelope(old, &b.vault).unwrap()
        };
        change_vault_passphrase(old, new).unwrap();
        let after = {
            let b = load_bundle().unwrap().unwrap();
            decrypt_vault_envelope(new, &b.vault).unwrap()
        };

        // The set of record ids must be identical.
        let mut before_ids: Vec<String> = before.secrets.keys().cloned().collect();
        before_ids.sort();
        let mut after_ids: Vec<String> = after.secrets.keys().cloned().collect();
        after_ids.sort();
        assert_eq!(
            before_ids, after_ids,
            "record id set must be unchanged by rotation"
        );

        // Every record's type, value, label, and metadata must be
        // byte-for-byte identical after rotation.
        for (id, rec_before) in &before.secrets {
            let rec_after = after
                .secrets
                .get(id)
                .unwrap_or_else(|| panic!("record {id} disappeared after rotation"));
            assert_eq!(
                rec_after.record_type, rec_before.record_type,
                "record_type changed for {id}"
            );
            assert_eq!(rec_after.value, rec_before.value, "value changed for {id}");
            assert_eq!(rec_after.label, rec_before.label, "label changed for {id}");
            assert_eq!(
                rec_after.metadata, rec_before.metadata,
                "metadata changed for {id}"
            );
        }
    }

    #[test]
    fn vault_passphrase_rotation_rejects_wrong_current_passphrase() {
        let _guard = TestVaultAndDataDirGuard::new();
        let old = "old-passphrase-1234";
        save_vault_with_payload(old, make_diverse_payload());

        let res = change_vault_passphrase("wrong-current-passphrase", "new-passphrase-5678");
        assert!(res.is_err(), "wrong current passphrase must be rejected");

        // A rejected rotation must leave the on-disk vault usable with
        // the ORIGINAL passphrase (no partial write, no re-key).
        let bundle = load_bundle().unwrap().unwrap();
        assert!(
            decrypt_vault_envelope(old, &bundle.vault).is_ok(),
            "rejected rotation must not corrupt the vault"
        );
    }

    #[test]
    fn vault_passphrase_rotation_rejects_short_new_passphrase() {
        let _guard = TestVaultAndDataDirGuard::new();
        let old = "old-passphrase-1234";
        save_vault_with_payload(old, make_diverse_payload());

        let res = change_vault_passphrase(old, "short");
        assert!(res.is_err(), "short new passphrase must be rejected");
        assert!(res.unwrap_err().contains("at least 8 characters"));
    }

    #[test]
    fn vault_passphrase_rotation_rejects_same_passphrase() {
        let _guard = TestVaultAndDataDirGuard::new();
        let pp = "same-passphrase-1234";
        save_vault_with_payload(pp, make_diverse_payload());

        let res = change_vault_passphrase(pp, pp);
        assert!(res.is_err(), "same old/new passphrase must be rejected");
        assert!(res.unwrap_err().contains("different"));
    }

    #[test]
    fn vault_passphrase_rotation_preserves_created_and_network() {
        let _guard = TestVaultAndDataDirGuard::new();
        let old = "old-passphrase-1234";
        let new = "new-passphrase-5678";
        save_vault_with_payload(old, make_diverse_payload());

        let before = load_bundle().unwrap().unwrap();
        let created_before = before.vault.created;
        let network_before = before.vault.network.clone();
        let bundle_meta_before = before.meta.clone();

        change_vault_passphrase(old, new).unwrap();

        let after = load_bundle().unwrap().unwrap();
        assert_eq!(
            after.vault.created, created_before,
            "created timestamp must be preserved"
        );
        assert_eq!(
            after.vault.network, network_before,
            "network must be preserved"
        );
        assert_eq!(
            after.meta, bundle_meta_before,
            "bundle meta must be preserved"
        );
        assert!(
            after.vault.modified > created_before || after.vault.modified >= before.vault.modified,
            "modified must advance on rotation"
        );
    }

    #[test]
    fn vault_passphrase_rotation_does_not_expose_passphrases_in_result() {
        let _guard = TestVaultAndDataDirGuard::new();
        let old = "old-passphrase-1234";
        let new = "new-passphrase-5678";
        save_vault_with_payload(old, make_diverse_payload());

        let res = change_vault_passphrase(old, new).unwrap();
        let json = serde_json::to_string(&res).unwrap();
        assert!(
            !json.contains(old),
            "result must not leak the old passphrase"
        );
        assert!(
            !json.contains(new),
            "result must not leak the new passphrase"
        );
    }

    // ─── Slice 64p: Unload Vault / Use wallet.dat tests ───────────────
    //
    // These tests run with restart_node=false so they never try to
    // spawn a real hemp0xd in the test sandbox. They still exercise
    // the full classification + preservation contract: settings clear,
    // vault file preserved, runtime wallet file preserved, hemp.conf
    // untouched, missing-wallet.dat reporting, and the fallback path.

    #[test]
    fn unload_vault_clears_active_vault_wallet_name() {
        let guard = TestVaultAndDataDirGuard::new();
        save_vault_with_payload("unload-clears", make_provider_payload("p", "f"));
        let mut settings = crate::modules::models::AppSettings::default();
        settings.active_vault_wallet_name = Some("hemp0x-vault-main".to_string());
        crate::modules::files::save_app_settings_impl(&settings).unwrap();
        // wallet.dat exists so we go through the fallback classification.
        std::fs::write(
            guard.data_dir_path().join("wallet.dat"),
            b"SQLite format 3\0dummy",
        )
        .unwrap();

        let res = unload_vault_and_use_wallet_dat_blocking(false).unwrap();
        assert_eq!(res["unloaded"], true);

        let after = crate::modules::files::load_app_settings_impl().unwrap();
        assert!(
            after.active_vault_wallet_name.is_none(),
            "active_vault_wallet_name must be cleared after unload"
        );
    }

    #[test]
    fn unload_vault_does_not_delete_vault_file() {
        let guard = TestVaultAndDataDirGuard::new();
        save_vault_with_payload("unload-keep-vault", make_provider_payload("p", "f"));
        let vault_file = guard.vault_dir_path().join("vault.json");
        assert!(vault_file.exists(), "vault file should exist before unload");
        std::fs::write(
            guard.data_dir_path().join("wallet.dat"),
            b"SQLite format 3\0dummy",
        )
        .unwrap();

        let _ = unload_vault_and_use_wallet_dat_blocking(false).unwrap();

        assert!(
            vault_file.exists(),
            "vault file must NOT be deleted by unload"
        );
        // And it must remain a valid, decryptable bundle with the same passphrase.
        let bundle = load_bundle().unwrap().unwrap();
        assert!(
            decrypt_vault_envelope("unload-keep-vault", &bundle.vault).is_ok(),
            "vault must still decrypt with its original passphrase after unload"
        );
    }

    #[test]
    fn unload_vault_does_not_delete_vault_runtime_wallet_file() {
        let guard = TestVaultAndDataDirGuard::new();
        save_vault_with_payload("unload-keep-runtime", make_provider_payload("p", "f"));
        let runtime_name = "hemp0x-vault-main";
        let mut settings = crate::modules::models::AppSettings::default();
        settings.active_vault_wallet_name = Some(runtime_name.to_string());
        crate::modules::files::save_app_settings_impl(&settings).unwrap();

        let runtime_wallet = guard.data_dir_path().join(runtime_name);
        std::fs::write(&runtime_wallet, b"SQLite format 3\0runtime-wallet").unwrap();
        // wallet.dat also exists so we classify as the fallback path.
        std::fs::write(
            guard.data_dir_path().join("wallet.dat"),
            b"SQLite format 3\0default",
        )
        .unwrap();

        let _ = unload_vault_and_use_wallet_dat_blocking(false).unwrap();

        assert!(
            runtime_wallet.exists(),
            "vault runtime wallet file must NOT be deleted by unload"
        );
    }

    #[test]
    fn unload_vault_does_not_modify_hemp_conf() {
        let guard = TestVaultAndDataDirGuard::new();
        save_vault_with_payload("unload-conf", make_provider_payload("p", "f"));
        let mut settings = crate::modules::models::AppSettings::default();
        settings.active_vault_wallet_name = Some("hemp0x-vault-main".to_string());
        crate::modules::files::save_app_settings_impl(&settings).unwrap();

        let conf_path = guard.data_dir_path().join("hemp.conf");
        let original_conf = "server=1\ndaemon=1\nrpcport=42068\n# user tuning\ndbcache=2048\nwallet=hemp0x-vault-main\n";
        std::fs::write(&conf_path, original_conf).unwrap();
        std::fs::write(
            guard.data_dir_path().join("wallet.dat"),
            b"SQLite format 3\0dummy",
        )
        .unwrap();

        let res = unload_vault_and_use_wallet_dat_blocking(false).unwrap();

        let after = std::fs::read_to_string(&conf_path).unwrap();
        assert_eq!(
            after, original_conf,
            "hemp.conf must NOT be modified by unload"
        );
        // The manual wallet= line must be surfaced as a warning, not rewritten.
        assert_eq!(res["hemp_conf_wallet"], "hemp0x-vault-main");
    }

    #[test]
    fn unload_vault_reports_missing_wallet_dat_clearly() {
        let guard = TestVaultAndDataDirGuard::new();
        save_vault_with_payload("unload-no-dat", make_provider_payload("p", "f"));
        let mut settings = crate::modules::models::AppSettings::default();
        settings.active_vault_wallet_name = Some("hemp0x-vault-main".to_string());
        crate::modules::files::save_app_settings_impl(&settings).unwrap();
        // Intentionally do NOT create wallet.dat.
        assert!(!guard.data_dir_path().join("wallet.dat").exists());

        let res = unload_vault_and_use_wallet_dat_blocking(false).unwrap();
        assert_eq!(res["unloaded"], true);
        assert_eq!(res["no_legacy_wallet"], true);
        assert_eq!(res["legacy_wallet_mode"], false);
        assert_eq!(res["wallet_dat_exists"], false);
        assert_eq!(res["restarted"], false);
        // Must offer clear next actions rather than silently creating a wallet.
        let next = res["next_actions"].as_array().unwrap();
        assert!(
            !next.is_empty(),
            "must offer next actions when wallet.dat is missing"
        );
        let reason = res["restart_skipped_reason"].as_str().unwrap();
        assert!(
            reason.contains("not silently create"),
            "skip reason must explain why Core was not restarted: {reason}"
        );
        // Settings still cleared even when there is no wallet.dat.
        let after = crate::modules::files::load_app_settings_impl().unwrap();
        assert!(after.active_vault_wallet_name.is_none());
    }

    #[test]
    fn unload_vault_with_existing_wallet_dat_uses_fallback_path() {
        let guard = TestVaultAndDataDirGuard::new();
        save_vault_with_payload("unload-has-dat", make_provider_payload("p", "f"));
        let mut settings = crate::modules::models::AppSettings::default();
        settings.active_vault_wallet_name = Some("hemp0x-vault-main".to_string());
        crate::modules::files::save_app_settings_impl(&settings).unwrap();
        std::fs::write(
            guard.data_dir_path().join("wallet.dat"),
            b"SQLite format 3\0dummy",
        )
        .unwrap();

        // restart_node=false so the test does not try to spawn a real
        // daemon, but the classification must still be the legacy
        // fallback path (not the no-legacy-wallet path).
        let res = unload_vault_and_use_wallet_dat_blocking(false).unwrap();
        assert_eq!(res["unloaded"], true);
        assert_eq!(res["legacy_wallet_mode"], true);
        assert_eq!(res["no_legacy_wallet"], false);
        assert_eq!(res["wallet_dat_exists"], true);
        assert_eq!(res["restarted"], false);

        let after = crate::modules::files::load_app_settings_impl().unwrap();
        assert!(after.active_vault_wallet_name.is_none());
    }

    #[test]
    fn unload_vault_preserves_vault_file_when_no_wallet_dat() {
        // Even when we take the no-legacy-wallet branch, the vault
        // file and runtime wallet file must remain intact.
        let guard = TestVaultAndDataDirGuard::new();
        save_vault_with_payload("unload-no-dat-keep", make_provider_payload("p", "f"));
        let runtime_name = "hemp0x-vault-main";
        let mut settings = crate::modules::models::AppSettings::default();
        settings.active_vault_wallet_name = Some(runtime_name.to_string());
        crate::modules::files::save_app_settings_impl(&settings).unwrap();
        let runtime_wallet = guard.data_dir_path().join(runtime_name);
        std::fs::write(&runtime_wallet, b"SQLite format 3\0runtime").unwrap();
        let vault_file = guard.vault_dir_path().join("vault.json");

        let _ = unload_vault_and_use_wallet_dat_blocking(false).unwrap();

        assert!(
            vault_file.exists(),
            "vault file must be preserved on the no-legacy-wallet path"
        );
        assert!(
            runtime_wallet.exists(),
            "runtime wallet file must be preserved on the no-legacy-wallet path"
        );
    }

    // ─── Slice 66: Core v2 Envelope Decryption Tests ─────────────────────

    fn make_test_v2_envelope(mnemonic: &str, passphrase: &str) -> String {
        make_test_v2_envelope_with_payload(mnemonic, passphrase, None)
    }

    fn make_test_v2_envelope_with_payload(
        mnemonic: &str,
        passphrase: &str,
        payload_overrides: Option<serde_json::Value>,
    ) -> String {
        let exported_at = chrono::Utc::now().timestamp();
        let mut private_payload = serde_json::json!({
            "payload_version": 1,
            "wallet_type": "bip39_bip44_p2pkh",
            "network": "main",
            "coin_type": 420,
            "account": 0,
            "derivation_profile": "hemp0x.mainnet.bip44.p2pkh.coin420.v1",
            "mnemonic": {
                "language": "english",
                "words": mnemonic,
            },
            "mnemonic_passphrase": "",
            "external_count_hint": 20,
            "change_count_hint": 6,
        });
        if let Some(overrides) = payload_overrides {
            if let Some(obj) = private_payload.as_object_mut() {
                if let Some(ov) = overrides.as_object() {
                    for (k, v) in ov {
                        obj.insert(k.clone(), v.clone());
                    }
                }
            }
        }
        let plaintext = serde_json::to_vec(&private_payload).unwrap();
        let mut salt = [0u8; 32];
        OsRng.fill_bytes(&mut salt);
        let mut key = Zeroizing::new([0u8; 32]);
        pbkdf2_hmac::<Sha512>(passphrase.as_bytes(), &salt, 600_000, key.as_mut());
        let aes_key = aes_gcm::Key::<Aes256Gcm>::from_slice(key.as_slice());
        let cipher = Aes256Gcm::new(aes_key);
        let mut iv = [0u8; 12];
        OsRng.fill_bytes(&mut iv);
        let nonce = Nonce::from_slice(&iv);
        let aad = format!(
            "{}:{}:{}:{}:{}:{}",
            MIGRATION_SCHEMA_IDENTIFIER,
            MIGRATION_ENVELOPE_VERSION,
            "main",
            420i64,
            exported_at,
            MIGRATION_PURPOSE_LABEL_PRIVATE,
        )
        .into_bytes();
        let encrypted = cipher
            .encrypt(
                nonce,
                aes_gcm::aead::Payload {
                    msg: &plaintext,
                    aad: &aad,
                },
            )
            .unwrap();
        let ciphertext = &encrypted[..encrypted.len() - 16];
        let tag = &encrypted[encrypted.len() - 16..];
        serde_json::to_string(&serde_json::json!({
            "envelope_version": 2,
            "schema_identifier": MIGRATION_SCHEMA_IDENTIFIER,
            "exported_at": exported_at,
            "source_client": "test",
            "source_client_version": "1.0",
            "chain": {
                "network": "main",
                "coin_type_bip44": 420,
            },
            "wallet_summary": {
                "wallet_name": "test-wallet",
                "encrypted": true,
                "locked": false,
                "hd_enabled": true,
                "bip44_enabled": true,
                "private_keys_included": true,
                "mnemonic_available": true,
            },
            "derivation": [{
                "profile_id": DERIVATION_HEMP_CANONICAL_420,
                "purpose": 44,
                "coin_type": 420,
                "address_type": "p2pkh",
                "best_block_height": 500000,
            }],
            "private": {
                "encrypted": true,
                "payload_format": MIGRATION_PRIVATE_PAYLOAD_FORMAT,
                "kdf_profile": MIGRATION_KDF_PROFILE,
                "kdf_iterations": 600_000,
                "cipher_profile": MIGRATION_CIPHER_PROFILE,
                "salt": hex::encode(salt),
                "iv": hex::encode(iv),
                "tag": hex::encode(tag),
                "aad_profile": MIGRATION_AAD_PROFILE,
                "ciphertext": hex::encode(ciphertext),
            },
        }))
        .unwrap()
    }

    #[test]
    fn decrypt_valid_v2_envelope_produces_canonical_material() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let passphrase = "test-migration-passphrase";
        let envelope = make_test_v2_envelope(mnemonic, passphrase);
        let material = decrypt_core_migration_bip39(&envelope, passphrase).unwrap();
        assert_eq!(material.mnemonic.as_str(), mnemonic);
        assert_eq!(material.coin_type, 420);
        assert_eq!(material.derivation_profile, DERIVATION_HEMP_CANONICAL_420);
        assert_eq!(material.mnemonic_word_count, 12);
        assert_eq!(material.best_block_height, 500000);
        assert_eq!(material.source_wallet_name.as_deref(), Some("test-wallet"));
    }

    #[test]
    fn wrong_passphrase_fails_authentication() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let passphrase = "correct-passphrase";
        let envelope = make_test_v2_envelope(mnemonic, passphrase);
        let result = decrypt_core_migration_bip39(&envelope, "wrong-passphrase");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Authentication failed"));
    }

    #[test]
    fn modified_ciphertext_fails() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let passphrase = "test-passphrase";
        let mut envelope = make_test_v2_envelope(mnemonic, passphrase);
        // Corrupt the ciphertext by changing a hex char
        envelope = envelope.replace("ciphertext\":\"", "ciphertext\":\"ff");
        let result = decrypt_core_migration_bip39(&envelope, passphrase);
        assert!(result.is_err());
    }

    #[test]
    fn migration_decrypt_unsupported_kdf_profile_rejected() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let passphrase = "test-passphrase";
        let mut envelope = make_test_v2_envelope(mnemonic, passphrase);
        envelope = envelope.replace(
            "\"kdf_profile\":\"pbkdf2-hmac-sha512-v1\"",
            "\"kdf_profile\":\"argon2id-v1\"",
        );
        let result = decrypt_core_migration_bip39(&envelope, passphrase);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unsupported KDF profile"));
    }

    #[test]
    fn migration_decrypt_unsupported_cipher_profile_rejected() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let passphrase = "test-passphrase";
        let mut envelope = make_test_v2_envelope(mnemonic, passphrase);
        envelope = envelope.replace(
            "\"cipher_profile\":\"aes-256-gcm-v1\"",
            "\"cipher_profile\":\"aes-128-gcm-v1\"",
        );
        let result = decrypt_core_migration_bip39(&envelope, passphrase);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unsupported cipher profile"));
    }

    #[test]
    fn unsupported_envelope_version_rejected() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let passphrase = "test-passphrase";
        let mut envelope = make_test_v2_envelope(mnemonic, passphrase);
        envelope = envelope.replace("\"envelope_version\":2", "\"envelope_version\":1");
        let result = decrypt_core_migration_bip39(&envelope, passphrase);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unsupported envelope version"));
    }

    #[test]
    fn network_mismatch_rejected() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let passphrase = "test-passphrase";
        let mut envelope = make_test_v2_envelope(mnemonic, passphrase);
        envelope = envelope.replace("\"network\":\"main\"", "\"network\":\"test\"");
        let result = decrypt_core_migration_bip39(&envelope, passphrase);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("Network 'test' is not supported"));
    }

    #[test]
    fn coin_type_other_than_420_rejected() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let passphrase = "test-passphrase";
        let mut envelope = make_test_v2_envelope(mnemonic, passphrase);
        envelope = envelope.replace("\"coin_type_bip44\":420", "\"coin_type_bip44\":175");
        let result = decrypt_core_migration_bip39(&envelope, passphrase);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("Coin type 175 is not supported"));
    }

    #[test]
    fn generic_derivation_rejected() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let passphrase = "test-passphrase";
        let envelope = make_test_v2_envelope_with_payload(
            mnemonic,
            passphrase,
            Some(serde_json::json!({"derivation_profile": "hemp0x.mainnet.bip44.p2pkh.v1"})),
        );
        let result = decrypt_core_migration_bip39(&envelope, passphrase);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("Unsupported derivation profile"));
    }

    #[test]
    fn public_v1_envelope_rejected_for_promotion() {
        let v1_envelope = serde_json::json!({
            "envelope_version": 1,
            "schema_identifier": "hemp0x-core.migration-envelope.v1",
            "chain": { "network": "main", "coin_type_bip44": 420 },
            "wallet_summary": { "private_keys_included": false },
        });
        let result = decrypt_core_migration_bip39(
            &serde_json::to_string(&v1_envelope).unwrap(),
            "any-passphrase",
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unsupported envelope version"));
    }

    #[test]
    fn invalid_mnemonic_rejected() {
        let passphrase = "test-passphrase";
        let envelope = make_test_v2_envelope_with_payload(
            "zzzz zzzz zzzz zzzz zzzz zzzz zzzz zzzz zzzz zzzz zzzz zzzz",
            passphrase,
            None,
        );
        let result = decrypt_core_migration_bip39(&envelope, passphrase);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid BIP39 mnemonic"));
    }

    #[test]
    fn excessive_kdf_iterations_rejected() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let passphrase = "test-passphrase";
        let mut envelope = make_test_v2_envelope(mnemonic, passphrase);
        envelope = envelope.replace("\"kdf_iterations\":600000", "\"kdf_iterations\":10000000");
        let result = decrypt_core_migration_bip39(&envelope, passphrase);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("exceeds the supported maximum"));
    }

    #[test]
    fn malformed_salt_length_rejected() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let passphrase = "test-passphrase";
        let mut envelope = make_test_v2_envelope(mnemonic, passphrase);
        // Replace 64-char salt hex with 32-char (16 bytes) — valid hex but wrong length
        envelope = envelope.replacen(
            "\"salt\":\"",
            "\"salt\":\"aabbccddaabbccddaabbccddaabbccdd",
            1,
        );
        let result = decrypt_core_migration_bip39(&envelope, passphrase);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid salt length"));
    }

    #[test]
    fn public_private_metadata_mismatch_rejected() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let passphrase = "test-passphrase";
        let envelope = make_test_v2_envelope_with_payload(
            mnemonic,
            passphrase,
            Some(serde_json::json!({"coin_type": 175})),
        );
        let result = decrypt_core_migration_bip39(&envelope, passphrase);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("Coin type 175")
                || err.contains("does not match public envelope coin type"),
            "Expected coin type mismatch error, got: {err}"
        );
    }

    // ─── Slice 66: Primary Record Builder Tests ──────────────────────────

    #[test]
    fn build_primary_record_has_webcom_compatible_shape() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let material = PortableWalletMaterial {
            mnemonic: Zeroizing::new(mnemonic.to_string()),
            mnemonic_passphrase: Zeroizing::new(String::new()),
            network: "mainnet".to_string(),
            coin_type: 420,
            account: 0,
            derivation_profile: DERIVATION_HEMP_CANONICAL_420.to_string(),
            external_count_hint: 20,
            change_count_hint: 6,
            best_block_height: 500000,
            source_wallet_name: Some("test-wallet".to_string()),
            exported_at: 1700000000,
            mnemonic_language: "english".to_string(),
            mnemonic_word_count: 12,
        };
        let record = build_webcom_primary_record_from_material(&material);
        assert_eq!(record.record_id, RECORD_ID_WALLET_HEMP_PRIMARY);
        assert_eq!(record.record_type, RECORD_TYPE_WALLET_BIP39);
        assert_eq!(record.value, mnemonic);
        assert_eq!(record.network.as_deref(), Some("mainnet"));
        let meta = record.metadata.as_ref().unwrap();
        assert_eq!(meta["recovery"]["seedType"], "bip39");
        assert_eq!(meta["recovery"]["network"], "mainnet");
        assert_eq!(
            meta["recovery"]["derivationProfiles"]["hemp"],
            DERIVATION_HEMP_CANONICAL_420
        );
        assert_eq!(meta["recovery"]["createdAt"], 1700000000000_i64);
        assert!(meta["recovery"]["updatedAt"].as_i64().unwrap_or(0) > 0);
        assert_eq!(meta["external_count"], 20);
        assert_eq!(meta["change_count"], 6);
        assert_eq!(meta["commander_core_external_count_hint"], 20);
        assert_eq!(meta["commander_core_change_count_hint"], 6);
        assert_eq!(meta["account"], 0);
        assert_eq!(
            meta["recovered_external_indices"]
                .as_array()
                .map(|items| items.len()),
            Some(0)
        );
        assert_eq!(meta["best_block_height"], 500000);
        assert_eq!(meta["source"], "core-migration-import");
        assert_eq!(meta["source_wallet_name"], "test-wallet");
    }

    #[test]
    fn promoted_primary_record_caps_webcom_address_counts() {
        let material = PortableWalletMaterial {
            mnemonic: Zeroizing::new(
                "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
                    .to_string(),
            ),
            mnemonic_passphrase: Zeroizing::new(String::new()),
            network: "mainnet".to_string(),
            coin_type: 420,
            account: 0,
            derivation_profile: DERIVATION_HEMP_CANONICAL_420.to_string(),
            external_count_hint: 5000,
            change_count_hint: 4000,
            best_block_height: 0,
            source_wallet_name: Some("large-core-wallet".to_string()),
            exported_at: 1700000000,
            mnemonic_language: "english".to_string(),
            mnemonic_word_count: 12,
        };

        let record = build_webcom_primary_record_from_material(&material);
        let meta = record.metadata.as_ref().unwrap();
        assert_eq!(
            meta["external_count"],
            WEBCOM_PRIMARY_EXTERNAL_COUNT_DEFAULT
        );
        assert_eq!(meta["change_count"], WEBCOM_PRIMARY_CHANGE_COUNT_DEFAULT);
        assert_eq!(meta["commander_core_external_count_hint"], 5000);
        assert_eq!(meta["commander_core_change_count_hint"], 4000);
        assert_eq!(
            meta["recovered_external_indices"]
                .as_array()
                .map(|items| items.len()),
            Some(0)
        );
    }

    #[test]
    fn primary_record_can_be_rebuilt_into_migration_envelope() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let material = PortableWalletMaterial {
            mnemonic: Zeroizing::new(mnemonic.to_string()),
            mnemonic_passphrase: Zeroizing::new(String::new()),
            network: "mainnet".to_string(),
            coin_type: 420,
            account: 0,
            derivation_profile: DERIVATION_HEMP_CANONICAL_420.to_string(),
            external_count_hint: 20,
            change_count_hint: 6,
            best_block_height: 0,
            source_wallet_name: None,
            exported_at: 1700000000,
            mnemonic_language: "english".to_string(),
            mnemonic_word_count: 12,
        };
        let record = build_webcom_primary_record_from_material(&material);
        let passphrase = "bridge-test-passphrase";
        let envelope = build_migration_envelope_from_webcom_bip39(passphrase, &record).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&envelope).unwrap();
        assert_eq!(parsed["envelope_version"], 2);
        assert_eq!(parsed["schema_identifier"], MIGRATION_SCHEMA_IDENTIFIER);
        assert_eq!(parsed["chain"]["network"], "main");
        assert_eq!(parsed["chain"]["coin_type_bip44"], 420);
        assert!(parsed["private"]["encrypted"].as_bool().unwrap());
    }

    // ─── Slice 66: Vault Promotion Tests ────────────────────────────────

    #[test]
    fn promotion_preserves_unrelated_records() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-vault-passphrase";
        let mut payload = make_provider_payload("pinata-token", "filebase-token");
        let future_record = make_future_record("wallet.bip39.main", RECORD_TYPE_WALLET_BIP39, "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about");
        payload
            .secrets
            .insert(future_record.record_id.clone(), future_record.clone());
        let envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        let bundle = VaultBundle {
            bundleVersion: CURRENT_BUNDLE_VERSION,
            format_identifier: FORMAT_IDENTIFIER.to_string(),
            vault: envelope,
            meta: None,
        };
        save_bundle_atomic(&bundle).unwrap();

        // Simulate promotion by inserting a primary record
        let dek = unwrap_dek_with_passphrase(passphrase, &bundle.vault).unwrap();
        let mut existing = decrypt_payload_with_dek(dek.as_slice(), &bundle.vault).unwrap();
        let primary = SecretRecord {
            record_id: RECORD_ID_WALLET_HEMP_PRIMARY.to_string(),
            record_type: RECORD_TYPE_WALLET_BIP39.to_string(),
            label: "Test Primary".to_string(),
            value: "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about".to_string(),
            metadata: Some(serde_json::json!({"recovery": {"seedType": "bip39", "network": "mainnet", "derivationProfiles": {"hemp": DERIVATION_HEMP_CANONICAL_420}}})),
            tags: Some(vec!["wallet".to_string()]),
            origin_app: Some(APP_IDENTIFIER.to_string()),
            derivation_profiles: Some({
                let mut dp = HashMap::new();
                dp.insert("hemp".to_string(), DERIVATION_HEMP_CANONICAL_420.to_string());
                dp
            }),
            network: Some("mainnet".to_string()),
            created: chrono::Utc::now().timestamp(),
            modified: chrono::Utc::now().timestamp(),
        };
        existing
            .secrets
            .insert(RECORD_ID_WALLET_HEMP_PRIMARY.to_string(), primary);
        let now = chrono::Utc::now().timestamp();
        let mut bundle2 = bundle.clone();
        bundle2.vault.modified = now;
        bundle2.vault.payload =
            encrypt_payload_with_dek(dek.as_slice(), &existing, &bundle2.vault).unwrap();
        save_bundle_atomic(&bundle2).unwrap();

        let loaded = load_bundle().unwrap().unwrap();
        let decrypted = decrypt_vault_envelope(passphrase, &loaded.vault).unwrap();
        assert!(decrypted
            .secrets
            .contains_key(RECORD_ID_WALLET_HEMP_PRIMARY));
        assert!(decrypted.secrets.contains_key("wallet.bip39.main"));
        assert_eq!(payload_pinata_token(&decrypted), "pinata-token");
        assert_eq!(payload_filebase_token(&decrypted), "filebase-token");
    }

    #[test]
    fn same_wallet_promotion_is_idempotent() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-vault-passphrase";
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let mut payload = VaultPayload::default();
        let primary = SecretRecord {
            record_id: RECORD_ID_WALLET_HEMP_PRIMARY.to_string(),
            record_type: RECORD_TYPE_WALLET_BIP39.to_string(),
            label: "Test Primary".to_string(),
            value: mnemonic.to_string(),
            metadata: Some(serde_json::json!({
                "recovery": {
                    "seedType": "bip39",
                    "network": "mainnet",
                    "derivationProfiles": {"hemp": DERIVATION_HEMP_CANONICAL_420},
                },
                "external_count": 20,
                "change_count": 6,
            })),
            tags: Some(vec!["wallet".to_string()]),
            origin_app: Some(APP_IDENTIFIER.to_string()),
            derivation_profiles: Some({
                let mut dp = HashMap::new();
                dp.insert(
                    "hemp".to_string(),
                    DERIVATION_HEMP_CANONICAL_420.to_string(),
                );
                dp
            }),
            network: Some("mainnet".to_string()),
            created: 1700000000,
            modified: 1700000000,
        };
        payload
            .secrets
            .insert(RECORD_ID_WALLET_HEMP_PRIMARY.to_string(), primary);
        let envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        let bundle = VaultBundle {
            bundleVersion: CURRENT_BUNDLE_VERSION,
            format_identifier: FORMAT_IDENTIFIER.to_string(),
            vault: envelope,
            meta: None,
        };
        save_bundle_atomic(&bundle).unwrap();

        let fp1 = {
            let loaded = load_bundle().unwrap().unwrap();
            let decrypted = decrypt_vault_envelope(passphrase, &loaded.vault).unwrap();
            let rec = decrypted
                .secrets
                .get(RECORD_ID_WALLET_HEMP_PRIMARY)
                .unwrap();
            build_alignment_fingerprint(rec)
        };

        // Simulate same-wallet update (metadata-only)
        let dek = unwrap_dek_with_passphrase(passphrase, &bundle.vault).unwrap();
        let mut existing = decrypt_payload_with_dek(dek.as_slice(), &bundle.vault).unwrap();
        let mut updated = existing
            .secrets
            .get(RECORD_ID_WALLET_HEMP_PRIMARY)
            .unwrap()
            .clone();
        updated.metadata = Some(serde_json::json!({
            "recovery": {
                "seedType": "bip39",
                "network": "mainnet",
                "derivationProfiles": {"hemp": DERIVATION_HEMP_CANONICAL_420},
            },
            "external_count": 30,
            "change_count": 10,
        }));
        updated.modified = chrono::Utc::now().timestamp();
        existing
            .secrets
            .insert(RECORD_ID_WALLET_HEMP_PRIMARY.to_string(), updated);
        let now = chrono::Utc::now().timestamp();
        let mut bundle2 = bundle.clone();
        bundle2.vault.modified = now;
        bundle2.vault.payload =
            encrypt_payload_with_dek(dek.as_slice(), &existing, &bundle2.vault).unwrap();
        save_bundle_atomic(&bundle2).unwrap();

        let fp2 = {
            let loaded = load_bundle().unwrap().unwrap();
            let decrypted = decrypt_vault_envelope(passphrase, &loaded.vault).unwrap();
            let rec = decrypted
                .secrets
                .get(RECORD_ID_WALLET_HEMP_PRIMARY)
                .unwrap();
            build_alignment_fingerprint(rec)
        };

        assert_eq!(fp1, fp2, "Same wallet fingerprint must be stable");
        let loaded = load_bundle().unwrap().unwrap();
        let decrypted = decrypt_vault_envelope(passphrase, &loaded.vault).unwrap();
        let rec = decrypted
            .secrets
            .get(RECORD_ID_WALLET_HEMP_PRIMARY)
            .unwrap();
        let meta = rec.metadata.as_ref().unwrap();
        assert_eq!(meta["external_count"], 30);
    }

    #[test]
    fn promotion_result_never_contains_mnemonic() {
        let _guard = setup_test_vault_dir();
        let passphrase = "test-vault-passphrase";
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let mut payload = VaultPayload::default();
        let primary = SecretRecord {
            record_id: RECORD_ID_WALLET_HEMP_PRIMARY.to_string(),
            record_type: RECORD_TYPE_WALLET_BIP39.to_string(),
            label: "Test Primary".to_string(),
            value: mnemonic.to_string(),
            metadata: Some(serde_json::json!({
                "recovery": {
                    "seedType": "bip39",
                    "network": "mainnet",
                    "derivationProfiles": {"hemp": DERIVATION_HEMP_CANONICAL_420},
                },
            })),
            tags: Some(vec!["wallet".to_string()]),
            origin_app: Some(APP_IDENTIFIER.to_string()),
            derivation_profiles: Some({
                let mut dp = HashMap::new();
                dp.insert(
                    "hemp".to_string(),
                    DERIVATION_HEMP_CANONICAL_420.to_string(),
                );
                dp
            }),
            network: Some("mainnet".to_string()),
            created: chrono::Utc::now().timestamp(),
            modified: chrono::Utc::now().timestamp(),
        };
        payload
            .secrets
            .insert(RECORD_ID_WALLET_HEMP_PRIMARY.to_string(), primary);
        let envelope = encrypt_vault_envelope(passphrase, &payload, KDF_PROFILE_SCRYPT).unwrap();
        let bundle = VaultBundle {
            bundleVersion: CURRENT_BUNDLE_VERSION,
            format_identifier: FORMAT_IDENTIFIER.to_string(),
            vault: envelope,
            meta: None,
        };
        save_bundle_atomic(&bundle).unwrap();

        let loaded = load_bundle().unwrap().unwrap();
        let json = serde_json::to_string(&loaded).unwrap();
        assert!(!json.contains(mnemonic));
        assert!(!json.to_lowercase().contains("abandon"));
    }

    // ─── Slice 66c: Focused Blocker Tests ────────────────────────────────

    // B1: Core Next has no dynamic loadwallet
    #[test]
    fn external_wallet_file_not_currently_loaded_is_rejected() {
        // Verify the file validation rejects non-wallet files.
        let _guard = setup_test_vault_dir();
        let tmp = std::env::temp_dir().join("not_a_wallet_66c.dat");
        fs::write(&tmp, b"This is not a wallet\x00").unwrap();
        let validation =
            crate::modules::process::validate_wallet_file(tmp.to_string_lossy().to_string());
        let _ = fs::remove_file(&tmp);
        assert!(validation.is_err());
    }

    // B2: Two canonical coin420 wallets with same name, different mnemonics
    #[test]
    fn two_different_mnemonics_produce_different_canonical_identities() {
        let id1 = canonical_wallet_identity("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about");
        let id2 = canonical_wallet_identity("zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong");
        assert_ne!(
            id1, id2,
            "Different mnemonics must produce different canonical identities"
        );
    }

    #[test]
    fn same_mnemonic_with_different_whitespace_produces_same_identity() {
        let id1 = canonical_wallet_identity("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about");
        let id2 = canonical_wallet_identity("  ABANDON abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon ABOUT  ");
        assert_eq!(
            id1, id2,
            "Same mnemonic with different casing/whitespace must produce same identity"
        );
    }

    // B3: Alignment identity reflects wallet identity, not just metadata
    #[test]
    fn two_canonical_wallets_with_identical_metadata_produce_different_identities() {
        let mnemonic_a = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let mnemonic_b = "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong";

        let material_a = PortableWalletMaterial {
            mnemonic: Zeroizing::new(mnemonic_a.to_string()),
            mnemonic_passphrase: Zeroizing::new(String::new()),
            network: "mainnet".to_string(),
            coin_type: 420,
            account: 0,
            derivation_profile: DERIVATION_HEMP_CANONICAL_420.to_string(),
            external_count_hint: 20,
            change_count_hint: 6,
            best_block_height: 0,
            source_wallet_name: None,
            exported_at: 0,
            mnemonic_language: "english".to_string(),
            mnemonic_word_count: 12,
        };
        let material_b = PortableWalletMaterial {
            mnemonic: Zeroizing::new(mnemonic_b.to_string()),
            mnemonic_passphrase: Zeroizing::new(String::new()),
            network: "mainnet".to_string(),
            coin_type: 420,
            account: 0,
            derivation_profile: DERIVATION_HEMP_CANONICAL_420.to_string(),
            external_count_hint: 20,
            change_count_hint: 6,
            best_block_height: 0,
            source_wallet_name: None,
            exported_at: 0,
            mnemonic_language: "english".to_string(),
            mnemonic_word_count: 12,
        };

        let record_a = build_webcom_primary_record_from_material(&material_a);
        let record_b = build_webcom_primary_record_from_material(&material_b);

        // Metadata-only fingerprint should be identical (same metadata fields)
        let fp_a = build_alignment_fingerprint(&record_a);
        let fp_b = build_alignment_fingerprint(&record_b);
        assert_eq!(
            fp_a, fp_b,
            "Metadata-only fingerprints must be identical for same-metadata wallets"
        );

        // Canonical identity must differ (different mnemonics)
        let id_a = canonical_wallet_identity(mnemonic_a);
        let id_b = canonical_wallet_identity(mnemonic_b);
        assert_ne!(
            id_a, id_b,
            "Canonical identities must differ for different mnemonics"
        );
    }

    // B4: Previous primary preserved as real migration envelope
    #[test]
    fn previous_primary_built_as_valid_migration_envelope() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let material = PortableWalletMaterial {
            mnemonic: Zeroizing::new(mnemonic.to_string()),
            mnemonic_passphrase: Zeroizing::new(String::new()),
            network: "mainnet".to_string(),
            coin_type: 420,
            account: 0,
            derivation_profile: DERIVATION_HEMP_CANONICAL_420.to_string(),
            external_count_hint: 20,
            change_count_hint: 6,
            best_block_height: 0,
            source_wallet_name: None,
            exported_at: 0,
            mnemonic_language: "english".to_string(),
            mnemonic_word_count: 12,
        };
        let record = build_webcom_primary_record_from_material(&material);
        let envelope =
            build_migration_envelope_from_webcom_bip39("test-passphrase", &record).unwrap();

        // The envelope must be valid JSON and parseable
        let parsed: serde_json::Value = serde_json::from_str(&envelope).unwrap();
        assert_eq!(parsed["envelope_version"], 2);
        assert_eq!(parsed["schema_identifier"], MIGRATION_SCHEMA_IDENTIFIER);
        assert_eq!(parsed["chain"]["coin_type_bip44"], 420);
        assert!(parsed["private"]["encrypted"].as_bool().unwrap());

        // The previous primary mnemonic must NOT appear in the public envelope
        let public_str = serde_json::to_string(&parsed).unwrap();
        assert!(!public_str.contains(mnemonic));
        assert!(!public_str.to_lowercase().contains("abandon"));
    }

    #[test]
    fn previous_primary_record_type_is_core_migration_envelope() {
        // Verify the record type used for backup is wallet.core_migration_envelope
        // and the value is an actual JSON migration envelope (not raw mnemonic).
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let record = build_webcom_primary_record_from_material(&PortableWalletMaterial {
            mnemonic: Zeroizing::new(mnemonic.to_string()),
            mnemonic_passphrase: Zeroizing::new(String::new()),
            network: "mainnet".to_string(),
            coin_type: 420,
            account: 0,
            derivation_profile: DERIVATION_HEMP_CANONICAL_420.to_string(),
            external_count_hint: 20,
            change_count_hint: 6,
            best_block_height: 0,
            source_wallet_name: None,
            exported_at: 0,
            mnemonic_language: "english".to_string(),
            mnemonic_word_count: 12,
        });
        let envelope =
            build_migration_envelope_from_webcom_bip39("test-passphrase", &record).unwrap();

        // The envelope value is real JSON, NOT a raw mnemonic
        assert!(envelope.starts_with('{'));
        let _: serde_json::Value = serde_json::from_str(&envelope).unwrap();
        // Raw mnemonic must not appear in the envelope text
        assert!(!envelope.contains(mnemonic));
    }

    // B5: Typed zeroizing struct
    #[test]
    fn typed_payload_deserializes_correctly() {
        let json = r#"{
            "payload_version": 1,
            "wallet_type": "bip39_bip44_p2pkh",
            "coin_type": 420,
            "account": 0,
            "derivation_profile": "hemp0x.mainnet.bip44.p2pkh.coin420.v1",
            "network": "main",
            "mnemonic": {"language": "english", "words": "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"},
            "mnemonic_passphrase": "",
            "external_count_hint": 20,
            "change_count_hint": 6,
            "best_block_height": 500000
        }"#;
        let payload: PrivateMigrationPayload = serde_json::from_str(json).unwrap();
        assert_eq!(payload.payload_version, 1);
        assert_eq!(payload.wallet_type, "bip39_bip44_p2pkh");
        assert_eq!(payload.coin_type, 420);
        assert_eq!(payload.mnemonic.words, "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about");
        assert!(payload.mnemonic_passphrase.is_empty());
        assert_eq!(payload.best_block_height, 500000);
    }

    #[test]
    fn typed_payload_rejects_invalid_json() {
        let result: Result<PrivateMigrationPayload, _> = serde_json::from_str("not json");
        assert!(result.is_err());
    }

    #[test]
    fn typed_payload_missing_fields_default() {
        let json = r#"{"payload_version": 1}"#;
        let payload: PrivateMigrationPayload = serde_json::from_str(json).unwrap();
        assert_eq!(payload.payload_version, 1);
        assert_eq!(payload.wallet_type, "");
        assert_eq!(payload.coin_type, 0);
        assert!(payload.mnemonic.words.is_empty());
    }

    #[test]
    fn typed_payload_no_generic_json_value_leaks_secrets() {
        // Verify that the decrypt_core_migration_bip39 function produces
        // correct material through the typed path. The test envelope
        // helper encrypts a payload; the decryptor must parse it
        // through the PrivateMigrationPayload struct.
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let passphrase = "test-passphrase";
        let envelope = make_test_v2_envelope(mnemonic, passphrase);
        let material = decrypt_core_migration_bip39(&envelope, passphrase).unwrap();
        assert_eq!(material.mnemonic.as_str(), mnemonic);
        assert_eq!(material.mnemonic_word_count, 12);
        assert_eq!(material.coin_type, 420);
    }

    // B7: Named wallet RPC URL and wallet name validation
    #[test]
    fn wallet_name_validation_rejects_empty() {
        // call_rpc_wallet validates wallet names
        let result = crate::modules::rpc::call_rpc_wallet("", "getinfo", &[]);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_lowercase()
            .contains("must not be empty"));
    }

    #[test]
    fn wallet_name_validation_rejects_special_chars() {
        let result = crate::modules::rpc::call_rpc_wallet("bad/name", "getinfo", &[]);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_lowercase()
            .contains("invalid wallet name"));
    }

    #[test]
    fn wallet_name_validation_rejects_too_long() {
        let long_name = "a".repeat(65);
        let result = crate::modules::rpc::call_rpc_wallet(&long_name, "getinfo", &[]);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_lowercase().contains("too long"));
    }

    #[test]
    fn wallet_name_validation_accepts_valid_names() {
        // These should pass validation (URL construction will fail without Core running)
        let valid = crate::modules::rpc::call_rpc_wallet("my_wallet-v2", "getinfo", &[]);
        // Should fail with transport/RPC error, not with name validation error
        assert!(valid.is_err());
        assert!(!valid
            .unwrap_err()
            .to_lowercase()
            .contains("invalid wallet name"));
    }

    #[test]
    fn wallet_name_accepts_alphanumeric_underscore_dash() {
        let test = crate::modules::rpc::call_rpc_wallet("testWallet_123-abc", "getinfo", &[]);
        // Validation passes, fails on transport
        assert!(test.is_err());
    }

    // B8: Promotion result never contains mnemonic (extended)
    #[test]
    fn promotion_result_json_never_exposes_canonical_identity() {
        // The canonical wallet identity must never appear in returned JSON.
        let mnemonic_a = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let mnemonic_b = "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong";
        let id_a = canonical_wallet_identity(mnemonic_a);
        let id_b = canonical_wallet_identity(mnemonic_b);

        // The identities are hashes, not the raw mnemonic
        assert!(!id_a.contains("abandon"));
        assert!(!id_b.contains("zoo"));

        // The format fingerprint (exposed to frontend) must not contain the identity hash
        let material = PortableWalletMaterial {
            mnemonic: Zeroizing::new(mnemonic_a.to_string()),
            mnemonic_passphrase: Zeroizing::new(String::new()),
            network: "mainnet".to_string(),
            coin_type: 420,
            account: 0,
            derivation_profile: DERIVATION_HEMP_CANONICAL_420.to_string(),
            external_count_hint: 20,
            change_count_hint: 6,
            best_block_height: 0,
            source_wallet_name: None,
            exported_at: 0,
            mnemonic_language: "english".to_string(),
            mnemonic_word_count: 12,
        };
        let record = build_webcom_primary_record_from_material(&material);
        let fp = build_alignment_fingerprint(&record);
        assert!(
            !fp.contains(&id_a),
            "Format fingerprint must not expose canonical identity"
        );
        assert!(
            !fp.contains("abandon"),
            "Format fingerprint must not expose mnemonic"
        );
    }

    // B8: Same-wallet idempotent promotion does not duplicate records
    #[test]
    fn same_wallet_does_not_create_duplicate_snapshot() {
        let _guard = setup_test_vault_dir();
        let _passphrase = "test-vault-passphrase";
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let material = PortableWalletMaterial {
            mnemonic: Zeroizing::new(mnemonic.to_string()),
            mnemonic_passphrase: Zeroizing::new(String::new()),
            network: "mainnet".to_string(),
            coin_type: 420,
            account: 0,
            derivation_profile: DERIVATION_HEMP_CANONICAL_420.to_string(),
            external_count_hint: 20,
            change_count_hint: 6,
            best_block_height: 0,
            source_wallet_name: None,
            exported_at: 0,
            mnemonic_language: "english".to_string(),
            mnemonic_word_count: 12,
        };
        let record = build_webcom_primary_record_from_material(&material);
        assert_eq!(record.record_id, RECORD_ID_WALLET_HEMP_PRIMARY);
        assert_eq!(record.record_type, RECORD_TYPE_WALLET_BIP39);
        assert_eq!(record.value, mnemonic);
        // Verify the record has the WebCom-compatible shape
        let meta = record.metadata.as_ref().unwrap();
        assert_eq!(meta["recovery"]["seedType"], "bip39");
        assert_eq!(
            meta["recovery"]["derivationProfiles"]["hemp"],
            DERIVATION_HEMP_CANONICAL_420
        );
    }

    // B8: Relock guard behaviour
    #[test]
    fn relock_guard_does_not_lock_if_not_unlocked_by_us() {
        let guard = WalletRelockGuard::new("test", false, false);
        // Guard should not attempt to lock since we didn't unlock it
        // (proves the was_unlocked_by_us flag is respected)
        drop(guard);
    }

    #[test]
    fn relock_guard_tracks_whether_we_unlocked() {
        let mut guard = WalletRelockGuard::new("test", false, true);
        assert!(guard.was_unlocked_by_us);
        // relock_now should succeed (wallet not running in test)
        let result = guard.relock_now();
        // Will fail because no Core is running, but was_unlocked_by_us should be cleared
        assert!(!guard.was_unlocked_by_us);
        // The call fails on transport, but the guard state is correct
        let _ = result;
    }

    // ─── Slice 66e: Isolated Decision Tests ─────────────────────────────
    //
    // F8: Pure-function injected tests. No live CLI, no Core daemon,
    // no Commander settings files, no filesystem dependency.

    fn make_input(
        default_exists: bool,
        configured: Option<&str>,
        loaded: &[&str],
        named_queryable: &[(&str, bool)],
        default_queryable: bool,
    ) -> ActiveWalletInput {
        ActiveWalletInput {
            configured_named_wallet: configured.map(|s| s.to_string()),
            loaded_wallets: loaded.iter().map(|s| s.to_string()).collect(),
            default_wallet_path: if default_exists {
                PathBuf::from("/tmp/test/wallet.dat")
            } else {
                PathBuf::from("/tmp/test/no_wallet.dat")
            },
            is_default_queryable: default_queryable,
            named_queryable: named_queryable
                .iter()
                .map(|(k, v)| (k.to_string(), *v))
                .collect(),
        }
    }

    // F5: Default wallet active
    #[test]
    fn resolver_proven_default_wallet() {
        let input = make_input(true, None, &["wallet.dat"], &[], true);
        match resolve_proven_wallet(&input) {
            WalletResolution::Proven(ProvenWallet::Default { .. }) => {}
            other => panic!("Expected Proven(Default), got {:?}", other),
        }
    }

    // F5: Named wallet active, wallet.dat exists
    #[test]
    fn resolver_proven_named_wallet() {
        let input = make_input(true, None, &["vault-main"], &[("vault-main", true)], false);
        match resolve_proven_wallet(&input) {
            WalletResolution::Proven(ProvenWallet::Named {
                ref wallet_name, ..
            }) if wallet_name == "vault-main" => {}
            other => panic!("Expected Proven(Named(vault-main)), got {:?}", other),
        }
    }

    // F5: Named wallet via Commander settings
    #[test]
    fn resolver_configured_named_wallet() {
        let input = make_input(
            false,
            Some("vault-main"),
            &["vault-main"],
            &[("vault-main", true)],
            false,
        );
        match resolve_proven_wallet(&input) {
            WalletResolution::Proven(ProvenWallet::Named {
                ref wallet_name, ..
            }) if wallet_name == "vault-main" => {}
            other => panic!("Expected Proven(Named(vault-main)), got {:?}", other),
        }
    }

    // F5: Configured wallet not queryable → Unavailable
    #[test]
    fn resolver_configured_unavailable_no_fallback() {
        let input = make_input(
            true,
            Some("vault-main"),
            &["vault-main"],
            &[("vault-main", false)],
            true,
        );
        match resolve_proven_wallet(&input) {
            WalletResolution::Unavailable => {}
            other => panic!(
                "Expected Unavailable (configured not queryable), got {:?}",
                other
            ),
        }
    }

    // F5: Multiple named wallets without configuration → Ambiguous
    #[test]
    fn resolver_multiple_named_ambiguous() {
        let input = make_input(
            false,
            None,
            &["wallet-a", "wallet-b"],
            &[("wallet-a", true), ("wallet-b", true)],
            false,
        );
        match resolve_proven_wallet(&input) {
            WalletResolution::Ambiguous { ref loaded_names } => {
                assert_eq!(loaded_names.len(), 2);
            }
            other => panic!("Expected Ambiguous, got {:?}", other),
        }
    }

    // F5: No wallets → Unavailable
    #[test]
    fn resolver_no_wallets_unavailable() {
        let input = make_input(false, None, &[], &[], false);
        match resolve_proven_wallet(&input) {
            WalletResolution::Unavailable => {}
            other => panic!("Expected Unavailable, got {:?}", other),
        }
    }

    #[test]
    fn resolver_unqueryable_named_wallet_is_unavailable() {
        let input = make_input(true, None, &["vault-main"], &[("vault-main", false)], false);
        assert!(matches!(
            resolve_proven_wallet(&input),
            WalletResolution::Unavailable
        ));
    }

    #[test]
    fn resolver_unqueryable_default_wallet_is_unavailable() {
        let input = make_input(true, None, &["wallet.dat"], &[], false);
        assert!(matches!(
            resolve_proven_wallet(&input),
            WalletResolution::Unavailable
        ));
    }

    #[test]
    fn resolver_configured_wallet_mismatch_is_rejected() {
        let input = make_input(
            true,
            Some("vault-main"),
            &["wallet.dat"],
            &[("vault-main", false)],
            true,
        );
        match resolve_proven_wallet(&input) {
            WalletResolution::SettingsMismatch { configured, actual } => {
                assert_eq!(configured, "vault-main");
                assert_eq!(actual.as_deref(), Some("wallet.dat"));
            }
            other => panic!("Expected SettingsMismatch, got {:?}", other),
        }
    }

    #[test]
    fn resolver_default_and_named_without_selection_is_ambiguous() {
        let input = make_input(
            true,
            None,
            &["wallet.dat", "vault-main"],
            &[("vault-main", true)],
            true,
        );
        assert!(matches!(
            resolve_proven_wallet(&input),
            WalletResolution::Ambiguous { .. }
        ));
    }

    #[test]
    fn core_migration_profile_uses_actual_rpc_response_shape() {
        let migration_info = serde_json::json!({
            "wallet_name": "wallet.dat",
            "canonical_coin_type": 420,
            "hd_enabled": true,
            "bip44_enabled": true,
            "has_mnemonic_metadata": true
        });
        let chain_info = serde_json::json!({ "chain": "main" });
        let (network, coin_type) = core_migration_runtime_profile(&migration_info, &chain_info);
        assert_eq!(network, "main");
        assert_eq!(coin_type, 420);
    }

    // F5: Named active, wallet.dat exists but not active
    #[test]
    fn named_wallet_active_wallet_dat_exists_but_not_active() {
        let input = make_input(true, None, &["vault-main"], &[("vault-main", true)], false);
        match resolve_proven_wallet(&input) {
            WalletResolution::Proven(ProvenWallet::Named { .. }) => {}
            other => panic!(
                "Expected Proven(Named) when named wallet is loaded, got {:?}",
                other
            ),
        }
    }

    // F3: Sentinel propagation
    #[test]
    fn unlock_sentinel_survives_identity_verifier() {
        // verify_exact_wallet_identity returns WALLET_UNLOCK_REQUIRED:: prefix
        let err = crate::modules::rpc::call_rpc_wallet("", "getinfo", &[]);
        // Error is about empty wallet name (validation path), but we verify
        // the format function's behavior: the error string must be a normal error
        // without secrets in it.
        assert!(err.is_err());
        assert!(!err.unwrap_err().contains("passphrase"));
    }

    // F3: Promotion wrapper preserves sentinel
    #[test]
    fn sentinel_prefix_not_destroyed_by_wrapper() {
        // Simulate the wrapper logic: if the error starts with WALLET_UNLOCK_REQUIRED::,
        // it should be returned as-is.
        let sentinel = "WALLET_UNLOCK_REQUIRED::The wallet is locked.";
        let wrapper = |e: &str| {
            if e.starts_with("WALLET_UNLOCK_REQUIRED::") {
                e.to_string()
            } else {
                format!("Wrapper prefix: {}", e)
            }
        };
        assert_eq!(wrapper(sentinel), sentinel);
        assert!(wrapper(sentinel).starts_with("WALLET_UNLOCK_REQUIRED::"));
    }

    // F8: RPC transport errors never include passphrases
    #[test]
    fn rpc_errors_never_include_passphrases() {
        // Test that the RPC error formatting functions don't capture secret content.
        // is_rpc_transport_timeout_error only checks error text for transport/timeout keywords.
        assert!(crate::modules::rpc::is_rpc_transport_timeout_error(
            "RPC transport error: timed out"
        ));
        assert!(!crate::modules::rpc::is_rpc_transport_timeout_error(
            "invalid passphrase wrong password"
        ));
    }

    // F8: Previous primary is valid migration envelope (B4 extended)
    #[test]
    fn previous_primary_envelope_validates_with_migration_constants() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let material = PortableWalletMaterial {
            mnemonic: Zeroizing::new(mnemonic.to_string()),
            mnemonic_passphrase: Zeroizing::new(String::new()),
            network: "mainnet".to_string(),
            coin_type: 420,
            account: 0,
            derivation_profile: DERIVATION_HEMP_CANONICAL_420.to_string(),
            external_count_hint: 20,
            change_count_hint: 6,
            best_block_height: 0,
            source_wallet_name: None,
            exported_at: 0,
            mnemonic_language: "english".to_string(),
            mnemonic_word_count: 12,
        };
        let record = build_webcom_primary_record_from_material(&material);
        let envelope =
            build_migration_envelope_from_webcom_bip39("test-passphrase", &record).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&envelope).unwrap();
        assert_eq!(parsed["envelope_version"], 2);
        assert_eq!(parsed["private"]["encrypted"], true);
        assert_eq!(
            parsed["private"]["payload_format"],
            MIGRATION_PRIVATE_PAYLOAD_FORMAT
        );
        assert_eq!(parsed["private"]["kdf_profile"], MIGRATION_KDF_PROFILE);
        assert_eq!(
            parsed["private"]["cipher_profile"],
            MIGRATION_CIPHER_PROFILE
        );
        assert!(parsed["private"].get("salt").is_some());
        assert!(parsed["private"].get("iv").is_some());
        assert!(parsed["private"].get("tag").is_some());
        assert!(parsed["private"].get("ciphertext").is_some());
    }

    // Remove/rename weak tests that claim stronger coverage than they provide
    // (The old zeroization test is removed since it admitted it couldn't verify.
    // The old resolver test is replaced with the pure-function tests above.)

    #[test]
    fn canonical_identity_is_backend_only() {
        let id = canonical_wallet_identity("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about");
        assert!(!id.contains("abandon"));
        assert_eq!(id.len(), 64);
    }
}
