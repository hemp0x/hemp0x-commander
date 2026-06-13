# Hemp0x Unified Vault

This document summarizes the Commander/WebCom/Core Next vault direction used by Hemp0x Commander.

The vault is a local encrypted bundle for application secrets and wallet migration records. In Commander v2 it is used for IPFS provider tokens and encrypted Core Next wallet migration envelopes. The format is designed to later carry WebCom-compatible wallet records, watch-only metadata, hardware-wallet metadata, and other app secrets without changing the outer contract.

## File Location

Commander stores the vault at:

```text
<active Hemp0x data dir>/commander/vault.json
```

For a default Linux mainnet setup this is usually:

```text
~/.hemp0x/commander/vault.json
```

The vault is separate from Core's `wallet.dat`. Backing up `wallet.dat` does not back up `vault.json`, and the two passphrases may be different.

## Bundle Shape

Commander writes unified vault bundles using:

```json
{
  "bundleVersion": 3,
  "format_identifier": "hemp0x-unified-vault-bundle",
  "vault": {},
  "meta": null
}
```

`meta` is advisory, unauthenticated, and non-secret. It must never contain passphrases, PIN material, private keys, mnemonics, provider tokens, encrypted offer records, or other credential material.

For development compatibility, Commander can read the earlier uncommitted snake_case wrapper names:

- `bundle_version`
- `public_meta`

New writes should use `bundleVersion` and `meta`.

## Envelope

The encrypted vault envelope contains:

- `version: 1`
- `schema_identifier: "hemp0x-commander-vault"`
- `app_identifier: "hemp0x-commander"`
- `network: "mainnet" | "testnet" | "regtest"`
- `cipher_profile: "aes-256-gcm-v1"`
- `aad_profile: "commander-envelope-v1"`
- encrypted `payload`
- one or more `key_slots`
- `created` and `modified` Unix timestamps in seconds

Network validation is local metadata validation only. Unlocking the vault does not require internet access, RPC, daemon availability, or node sync.

## Crypto Model

The vault uses a DEK/key-slot model:

1. A random 32-byte data encryption key encrypts the payload with AES-256-GCM.
2. Each key slot wraps that DEK with a key derived from the user passphrase.
3. New Commander vaults use `scrypt-v1` by default.

Supported KDF profiles:

- `scrypt-v1`: default for new Commander vaults, 32-byte salt
- `pbkdf2-hmac-sha512-v1`: Core Next compatibility, 32-byte salt
- `pbkdf2-sha256-v1`: WebCom bridge compatibility, accepts 16-byte legacy salts and 32-byte normalized salts

All current profiles require `kdf_dklen: 32`.

## AAD

Payload AAD for `commander-envelope-v1`:

```text
schema_identifier:version:app_identifier:network:cipher_profile:aad_profile:payload_schema:created:modified
```

Key-slot wrap AAD:

```text
schema_identifier:version:app_identifier:network:slot_id:slot_type:kdf_profile:kdf_params:kdf_dklen:wrap_cipher_profile:created
```

Implementers must preserve field order exactly.

## Payload

The decrypted payload uses `SecretRecord` entries as the source of truth:

```json
{
  "payload_version": 1,
  "secrets": {
    "provider.pinata.api_token": {
      "record_id": "provider.pinata.api_token",
      "record_type": "provider.api_token",
      "label": "Pinata API Token",
      "value": "...",
      "metadata": {
        "provider_id": "pinata",
        "provider_name": "Pinata",
        "endpoint": "https://api.pinata.cloud",
        "token_kind": "jwt"
      },
      "created": 1718136000,
      "modified": 1718136000
    }
  }
}
```

Provider records are intentionally generic. Future providers should use:

```text
provider.<provider_id>.api_token
```

Blank provider-token updates preserve the existing record. Removing a provider token requires an explicit remove flow.

## Backup and Export

Commander can export the vault bundle to a user-chosen destination file.

Behavior:

- Copies the exact `vault.json` bundle as-is (encrypted).
- Does not decrypt the vault for backup.
- Does not expose secrets through IPC.
- If no vault exists, returns an error.
- Uses the Tauri command `vault_export_bundle_to_path(path)`.
- The frontend shows a save dialog; the user picks a destination.

The exported file is a standard `vault.json` bundle and can be restored on the same or another Commander installation.

## Restore and Import

Commander can import a vault bundle from a selected file, replacing the current vault.

Validation rules:

- The imported file must be valid JSON.
- It must parse as a `VaultBundle` (with `bundleVersion` and `format_identifier`) or as a raw `VaultEnvelope` (legacy compatibility).
- `bundleVersion` must be within the supported range (1–3).
- `format_identifier` must match `"hemp0x-unified-vault-bundle"`.
- `network` must be `"mainnet"`, `"testnet"`, or `"regtest"`.
- `cipher_profile` must be `"aes-256-gcm-v1"`.
- `aad_profile` must be `"commander-envelope-v1"`.

Import flow:

1. The frontend shows a file-open dialog.
2. The selected file is validated using `vault_validate_import_bundle(path)`.
3. If validation passes and a vault already exists, the UI shows a replace warning with bundle metadata.
4. On confirmation, `vault_import_bundle_replace(path, passphrase?)` atomically replaces the current vault.
5. If a passphrase is provided, it is verified against the imported bundle before replacement.
6. The imported vault is written atomically (temp + rename).

Import does not merge secrets. It is a full replacement.

### Session behavior after restore

When a vault is restored/imported, the cached unlock session is cleared automatically. The restored vault starts in a **locked** state. The user must unlock the restored vault with the restored vault's passphrase before provider tokens become available. This prevents stale session state from a previous vault from being used with a different vault.

The backend wrapper `ipfs_vault_import_bundle_replace` clears the cached passphrase after writing the imported bundle. Any caller (frontend or future IPC consumer) that uses this command gets safe session cleanup without needing to call `ipfs_lock_vault` separately.

## Provider Token Removal

Commander can remove individual provider tokens from the vault through an explicit UI flow.

Behavior:

- Only known provider IDs are accepted: `pinata`, `filebase`.
- Requires an unlocked vault (uses the cached passphrase from the current session).
- Removes the specified `SecretRecord` from the encrypted payload.
- Does not affect other provider tokens.
- The UI shows a confirmation dialog before removal.
- After removal, the provider's publish operations will fail until a new token is stored.

The Tauri command is `ipfs_vault_remove_provider_token(provider_id)`, which uses the cached passphrase from the current unlock session.

## UI Control Locations

Vault management UI lives in two places:

| Control | Location |
|---------|----------|
| Vault setup (create) | IPFS Settings → Vault section |
| Vault unlock / lock | IPFS Settings → Vault section |
| Vault backup (export) | IPFS Settings → Vault section → Back Up Vault |
| Vault restore (import) | IPFS Settings → Vault section → Restore Vault |
| Provider token migration | IPFS Settings → Vault section → Migrate Tokens |
| Provider token status | IPFS Settings → Vault section (when unlocked) |
| Provider token removal | IPFS Settings → Vault section → Remove (per provider) |
| Core wallet backup | Wallet page → Wallet Management |
| Core wallet migration | Wallet page → Core Wallet Migration |
| Vault vs wallet distinction | Wallet page → App Secrets Vault |
| Vault wallet record import | Wallet page → Unified Vault Wallet Records |
| Vault wallet record export (active) | Wallet page → Unified Vault Wallet Records |
| Vault wallet record list | Wallet page → Unified Vault Wallet Records |
| Vault wallet record restore | Wallet page → Unified Vault Wallet Records |
| Vault wallet record remove | Wallet page → Unified Vault Wallet Records |

The Wallet page includes an **App Secrets Vault** section that explains the `wallet.dat` / `vault.json` distinction and directs users to IPFS Settings for vault operations. The **Unified Vault Wallet Records** section manages wallet migration records stored in the vault.

## Wallet Migration Records

Commander can store encrypted Core Next wallet migration envelopes inside `vault.json` as `wallet.core_migration_envelope` records. This is the transition path toward unified wallet vault storage.

### Record Shape

```json
{
  "record_id": "wallet.core_migration_envelope.export-1718136000",
  "record_type": "wallet.core_migration_envelope",
  "label": "Main wallet backup",
  "value": "{...encrypted migration envelope JSON...}",
  "metadata": {
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
  },
  "tags": ["wallet", "migration"],
  "origin_app": "hemp0x-commander",
  "network": "mainnet",
  "created": 1718136000,
  "modified": 1718136000
}
```

### Record ID Convention

Record IDs use the prefix `wallet.core_migration_envelope.` followed by a stable suffix (alphanumeric, underscore, or hyphen, max 64 chars). Examples:

- `wallet.core_migration_envelope.export-1718136000`
- `wallet.core_migration_envelope.import-1718136000`
- `wallet.core_migration_envelope.main-backup-1`

### Value Content

The `value` field contains the full encrypted Core Next migration envelope JSON string. This is the same content that would be written to a `.json` file by `export_wallet_migration`. The envelope is already encrypted with its own passphrase — the vault provides a second layer of encryption.

### Metadata Fields

| Field | Source | Notes |
|-------|--------|-------|
| `value_kind` | Always `"embedded_encrypted_json"` | Self-contained vault record |
| `source` | `"core-next-exportwalletmigration"` or `"file-import"` | Origin of the record |
| `restorable` | From migration validation | Whether the envelope can restore a wallet |
| `private_keys_included` | From migration export | Whether private keys are in the envelope |
| `envelope_kdf_profile` | From migration validation | KDF used by the migration envelope |
| `envelope_cipher_profile` | From migration validation | Cipher used by the migration envelope |
| `envelope_aad_profile` | From migration validation | AAD profile of the migration envelope |
| `envelope_coin_type` | From migration validation | Coin type (420 for Hemp0x) |
| `label` | User-provided | Human-readable label |
| `wallet_name_hint` | User-provided or `"default"` | Suggested wallet name for restore |

### Commands

| Command | Purpose |
|---------|---------|
| `vault_import_wallet_migration_record_from_path` | Import an existing migration envelope file into the vault |
| `vault_export_current_wallet_migration_record` | Export the active Core wallet migration envelope directly into the vault |
| `vault_restore_wallet_migration_record` | Restore a Core wallet from a vault-stored migration record |
| `vault_list_wallet_migration_records` | List wallet migration records (metadata only, no values) |
| `vault_remove_wallet_migration_record` | Remove a wallet migration record from the vault |

### Security

- Listing returns metadata only — the embedded envelope JSON is never returned.
- Export to temp file writes the exact stored envelope for restore operations.
- Temp files are deleted best-effort after restore.
- All commands require a vault passphrase (unlocked vault or explicit passphrase).
- Restore requires the migration envelope passphrase in addition to the vault passphrase.

### Transition Path

- **Today**: `wallet.dat` is the live Core wallet. Vault wallet records are portable encrypted backups.
- **Future**: The vault can become the primary wallet storage, with `wallet.dat` as a fallback during transition.
- A vault containing a private/restorable migration envelope can restore wallet keys if the user knows both the vault passphrase and the migration envelope passphrase.

## What to Back Up

A full Commander setup restore requires two separate files:

| File | Purpose | Backup command |
|------|---------|---------------|
| `wallet.dat` | Core wallet keys and funds | Wallet page → Back Up Wallet |
| `vault.json` | App/provider secrets + wallet migration records | IPFS Settings → Back Up Vault |

These are independent. Backing up `wallet.dat` does **not** back up `vault.json`, and the two may use different passphrases. For a full Commander restore, back up both.

The wallet migration tools (Export/Validate/Restore Migration Package in the Wallet page) back up wallet keys in a portable format. They do **not** include vault secrets. The **Unified Vault Wallet Records** section can store encrypted migration envelopes inside the vault for a self-contained backup.

## Current Deferred Work

These are intentional future slices, not missing pieces in the current vault foundation:

- passphrase change
- DEK rotation
- KDF upgrade migration
- PIN/biometric/multi-slot unlock
- hardware-wallet and watch-only vault UX
- deeper plaintext payload zeroization
- auto-migration of plaintext tokens on unlock
- notice bar color differentiation (green/orange/red by state)
- direct active-wallet export to vault without temp file (currently uses temp file)
- vault wallet record metadata enrichment from migration validation output

## Reserved Record Types

Implemented:

- `provider.api_token`

Reserved for future use:

- `wallet.bip39`
- `wallet.wif`
- `wallet.core_migration_envelope`
- `wallet.hardware_metadata`
- `wallet.watch_only`
- `protocol.nostr_key`
- `app.secret`
- `note.secure`

Unknown records must be preserved when Commander updates provider tokens.

## WebCom Interop Notes

WebCom currently has its own native vault format. The unified bridge should import/export `bundleVersion: 3` bundles without breaking existing WebCom users.

Important bridge rules:

- Do not put secrets in `meta`.
- Preserve legacy derivation profile strings.
- Do not silently convert legacy coin 175 records to canonical coin 420 records.
- WebCom PBKDF2-SHA256 16-byte salts are accepted for bridge import.
- WebCom must reproduce Commander's AAD field order exactly when decrypting a Commander-compatible bundle.

## Core Next Notes

Core Next migration artifacts should use `wallet.core_migration_envelope` records when implemented.

Recommended metadata includes:

- `value_kind`: `embedded_encrypted_json`, `embedded_public_json`, `cid`, or `file_path`
- `core_version`
- `migration_format`
- `envelope_kdf_profile`
- `envelope_cipher_profile`
- `envelope_aad_profile`
- `envelope_coin_type`
- `envelope_schema_identifier`
- `restorable`

