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

Vault file management UI lives on the **Wallet page** (Tools →
Wallet). IPFS Settings only stores and uses provider credentials
inside the active vault. The two pages share the same underlying
`vault.json` file and the same cached unlock session.

| Control | Location |
|---------|----------|
| Vault setup (create) | Wallet page → Recommended Backup (inline, when no vault exists) |
| Vault unlock / lock | Wallet page → Wallet Status header (inline) and Recommended Backup (inline) |
| Vault file backup (export bundle) | Wallet page → Vault File area → Back Up Current Vault |
| Vault file restore (import bundle) | Wallet page → Vault File area → Switch / Import Vault |
| Vault file label / archive / known vaults | Wallet page → Vault File area |
| Back up active wallet to vault (recommended) | Wallet page → Recommended Backup |
| List stored wallet backups | Wallet page → Restore / Recover Wallet |
| Restore wallet from vault backup | Wallet page → Restore / Recover Wallet |
| `wallet.dat` file backup/restore (legacy) | Wallet page → Advanced → Wallet.dat (Legacy / Compatibility) |
| Core migration envelope export/validate/restore (low-level) | Wallet page → Advanced → Core Wallet Migration |
| Vault wallet record import (from migration file) | Wallet page → Advanced → Advanced Vault Records |
| Vault wallet record remove | Wallet page → Advanced → Advanced Vault Records |
| Provider token entry / update | IPFS Settings → Publishing provider card |
| Provider token status (stored / missing) | IPFS Settings → Vault status panel |
| Vault unlock for IPFS | IPFS Settings → UNLOCK VAULT button (opens unified modal) |
| Provider token removal | IPFS Settings → per-provider REMOVE (only when unlocked and stored) |
| Vault status bar | App trust strip → VAULT (LOCKED / UNLOCKED / NONE) |
| Unified vault unlock modal | App header VAULT click or `commander-open-vault-unlock` event |

The Wallet page Recommended Backup section can drive the user all
the way through setup, unlock, backup, list, and restore without
leaving the page. IPFS Settings no longer manages the vault *file*
itself; it only stores and uses provider credentials inside the
active vault, and reuses the cached unlock session set by the
Wallet page.

## Wallet Page Vault-First Flow (60q)

The Wallet page uses a **vault-first** model for user-facing wallet
backup and recovery, while keeping `wallet.dat` as the live Core
wallet internally. The flow on the Wallet page is:

1. **Wallet Status header** (no passphrase required)
   - Shows Core `wallet.dat` name + encrypted / locked / unlocked
     state via `rpc_get_wallet_info`.
   - Shows vault existence, KDF profile, and lock state via
     `vault_get_vault_overview` and `ipfs_vault_unlock_status`.
   - Shows whether `wallet.dat` exists via `get_data_folder_info`
     and surfaces a recovery banner if it is missing.
   - Surfaces a dismissible `Finish vault backup` banner after a
     successful `create_new_wallet`.
2. **Recommended Backup** section
   - If no vault exists: inline `Create Vault` card (passphrase +
     confirm) using `ipfs_vault_setup_and_unlock`.
   - If vault exists but locked: inline `Unlock Vault` row using
     `ipfs_unlock_vault`.
   - If vault is unlocked: just label + wallet backup passphrase +
     one `Back Up Wallet To Vault` button. The cached vault session
     is reused; no per-call vault passphrase is required.
3. **Restore / Recover Wallet** section
   - If vault exists and unlocked, the stored backup list is
     populated and refreshes on demand.
   - If vault exists but locked, the user is prompted to unlock
     inline.
   - Restore form asks only for the wallet backup passphrase and a
     typed confirmation when the vault is unlocked. The cached
     vault session is reused; no per-call vault passphrase is
     required.
   - Birth height is hidden behind an `Advanced restore options`
     disclosure.
4. **Unified Vault** informational section
   - Explains that the vault is a general encrypted container
     holding wallet backup records and optional app-secret records
     such as provider tokens.
   - States that Core still uses `wallet.dat` internally today.
5. **Advanced** (collapsed by default)
   - Legacy `wallet.dat` file backup, restore, and new.
   - Private-key export/import.
   - Wallet encryption change.
   - Low-level Core migration envelope export / validate / restore.
   - Vault record import-from-file and remove.

### Cached Unlock Session

The Wallet page reuses the cached vault unlock session set by
`ipfs_unlock_vault` (or `ipfs_vault_setup_and_unlock`). The cached
passphrase:

- Is held in a process-local `Zeroizing<String>` inside a `Mutex`.
  It is never persisted, never logged, and never sent back over IPC.
- Is cleared automatically on `ipfs_lock_vault`,
  `ipfs_vault_import_bundle_replace`, and on a vault-decrypt error
  inside `load_provider_tokens`.
- Is used by the new `ipfs_vault_list_wallet_migration_records`,
  `ipfs_vault_export_current_wallet_migration_record`,
  `ipfs_vault_restore_wallet_migration_record`,
  `ipfs_vault_remove_wallet_migration_record`, and
  `ipfs_vault_import_wallet_migration_record_from_path` wrappers
  when the Wallet page does not pass an explicit per-call vault
  passphrase. Users who want to keep the vault locked can still
  pass the passphrase explicitly to one operation.

The cached-session design is opt-in per operation: the frontend
falls back to the explicit `vaultPassphrase` field whenever the
vault is not unlocked, so users who never unlock the vault on the
Wallet page continue to work.

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
| `recovery_mode` | Set by Commander at backup time | `"vault_passphrase"` or `"separate_passphrase"` (see Recovery Mode below) |

### Recovery Mode (Slice 60v)

New wallet backup records include a `recovery_mode` metadata field:

- **`vault_passphrase`** (default): The vault passphrase doubles as the migration envelope passphrase. When restoring, Commander automatically uses the cached vault passphrase — no second password prompt.
- **`separate_passphrase`**: A distinct backup recovery password protects the migration envelope. The user must provide it at restore time.
- **Legacy / unknown** (no `recovery_mode` key): Treated as "separate". The user must provide an explicit backup recovery password.

The live Core wallet unlock password is **never** stored in the vault. The vault is never used as a password manager for the runtime Core wallet.

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
- For `vault_passphrase` records, the vault passphrase is used as the migration envelope passphrase automatically (server-side only; never returned to frontend).
- For `separate_passphrase` and legacy records, the user must provide the explicit backup recovery password.
- The live Core wallet unlock password is never stored in the vault.

### Transition Path

- **Today**: `wallet.dat` is the live Core wallet. Vault wallet records are portable encrypted backups.
  - Commander **can** restore a Core runtime wallet from a vault backup record without a pre-existing `wallet.dat`. Core's `restorewalletmigration` RPC creates the necessary wallet files on disk.
  - Commander **cannot** run Core directly from a vault without writing wallet files to disk. Core has no RPC to load a migration envelope as the active runtime wallet without file-system persistence.
- **Future blocker for truly wallet.dat-free operation**: Core needs a new RPC (e.g., `loadwalletmigration`) that accepts a migration envelope and loads it as the active runtime wallet without writing wallet files to disk. The vault could then serve as the primary wallet backing store.
- **Best currently possible**: Commander creates/imports a vault first, then provisions or restores the Core runtime wallet from vault records. `wallet.dat` is treated as a compatibility/runtime detail, not the primary user-facing backup model.

## What to Back Up

Commander uses a vault-first backup model:

| File | Purpose | Backup command |
|------|---------|---------------|
| `vault.json` | Commander's portable encrypted wallet container + app secrets | Wallet page → Save Vault |
| Runtime wallet files (`wallet.dat` / wallet directories) | Core's internal wallet files (compatibility) | Wallet page → Advanced → Backup wallet.dat |

The **vault** is the recommended portable backup. The vault's wallet backup records are the primary recovery path. The runtime wallet files are compatibility plumbing; backing them up directly is an advanced operation.

If you have vault backup records (created via Backup tab), you can restore a runtime wallet without a pre-existing `wallet.dat`. Core still requires the runtime wallet files to exist on disk during operation, but Commander can provision them from the vault automatically.

## Vault File Manager (60s)

The Wallet page exposes a small **Vault File** area that gives users a normal UI path to identify, back up, switch, import, and label their vault file. The vault crypto, KDF, AAD, envelope version, payload schema, record shape, and bundle format are unchanged. No passphrase handling is weakened.

### Active Vault File

The active vault is the file at:

```text
<active Hemp0x data dir>/commander/vault.json
```

The Wallet page reads non-secret on-disk metadata only (file path, size, modified time, bundle version, vault version, network, envelope created/modified timestamps) via `vault_get_vault_overview`. It does not decrypt the vault for this overview.

### Display Label (Sidecar)

A human-friendly label such as `Main Commander Vault`, `Testing Vault`, or `WebCom Import` is stored in a non-secret local sidecar at:

```text
<active Hemp0x data dir>/commander/vault_index.json
```

The sidecar contains only:

- vault path
- display name
- last selected timestamp
- archived/known vault list

It **must never** contain passphrases, passphrase hints, private keys, mnemonics, provider tokens, or any decrypted vault content. Storing the label outside the encrypted payload is intentional for this release; no envelope format change is required.

### Archive

`Back Up Current Vault` moves the current `vault.json` to:

```text
<active Hemp0x data dir>/commander/vaults/archive/vault-YYYYMMDD-HHMMSS.json
```

The file is preserved verbatim on disk and the cached unlock session is cleared. The operation is a rename (with copy-then-delete fallback for cross-filesystem paths). The active path `vault.json` is no longer present after a successful backup, so the Wallet page falls back to the **No vault / First setup** state.

Confirmation requires typing the phrase `BACK UP VAULT`. This is the UI replacement for manually running:

```bash
mv ~/.hemp0x/commander/vault.json ~/.hemp0x/commander/vault.json.bak
```

Backed-up vaults are kept on disk and can be imported later via `Switch / Import Vault`. Internally, `vaults/archive/` is the backup/archive storage folder.

### Switch / Import Vault

`Switch / Import Vault` opens a file picker, validates the selected bundle (header, format identifier, supported bundle version) before changing anything, and then:

1. If an active vault exists, backs it up first (as above) so it is preserved.
2. Replaces the active `vault.json` with the imported bundle.
3. Clears the cached unlock session.
4. Re-renders the Wallet page in the locked state for the new vault.

### Create New Vault When One Already Exists

If a locked vault already exists and the user chooses `Create New Vault`:

1. The user must confirm that the existing vault will be backed up first.
2. The old vault is moved into `commander/vaults/archive/`.
3. A new active vault is created with the user's new passphrase and unlocked for the session.

### Lost Passphrase Behavior

Commander does not attempt to recover or brute-force a lost vault passphrase. The recovery path is:

- The file is still on disk; it was not deleted.
- The user can `Back Up Current Vault` and then `Create New Vault` to start fresh.
- Backed-up vaults can be re-imported via `Switch / Import Vault` and unlocked with their original passphrase if it is still known.

### Sidecar Metadata Rules

`vault_index.json` is the single source of truth for local, non-secret UI metadata. It is allowed to contain:

- vault path
- display name
- last selected timestamp
- archived/known vault list

It is not allowed to contain: passphrases, hints, private keys, mnemonics, provider tokens, decrypted vault contents, or any other credential material. The sidecar is best-effort local metadata; the encrypted vault remains authoritative for any sensitive value.

## IPFS Settings And The Vault (60w / 60x)

The IPFS Settings page is intentionally narrow now that the unified
vault exists:

- It only stores and uses provider API credentials (Pinata, Filebase,
  future providers) inside the **active** Commander vault.
- It does **not** create, back up, restore, switch, or import the
  vault file. Those actions live on the Wallet page.
- The vault must be unlocked for the current Commander session
  before saved provider tokens can be used. IPFS Settings shows an
  **UNLOCK VAULT** button that opens the app-wide unified vault
  unlock modal.
- The IPFS Settings status panel reports three states for provider
  tokens: `Stored safely` (encrypted in the vault),
  `No token stored`, and `Unlock vault to check saved tokens`.
- No provider token secrets, passphrases, or decrypted vault
  content are returned to the frontend. The non-secret helper
  `ipfs_provider_token_presence` reports per-provider presence
  and the storage source without decrypting the vault or exposing
  token values.

If an IPFS action that needs a saved provider token is attempted
while the vault is locked, the UI shows `Unlock your vault to use
saved provider tokens.` instead of a raw backend error.

## Global Vault Status Bar (60x)

The app-wide trust strip (status bar) at the top of every view
shows vault lock state between the Wallet and Network items:

- **VAULT NONE** -- No vault configured. Clicking navigates to
  Tools → Wallet where the vault can be created.
- **VAULT LOCKED** -- A vault exists but is not unlocked. Clicking
  opens the unified vault unlock modal.
- **VAULT UNLOCKED** -- The vault session is active. Clicking locks
  the vault (clears the cached passphrase).

The status bar refreshes on the normal polling interval. After any
unlock or lock action, the bar updates immediately.

## Unified Vault Unlock Modal (60x)

A single reusable vault unlock prompt is used app-wide:

- Opens from the app header (click VAULT LOCKED) or from any feature
  that needs vault access (IPFS Settings UNLOCK VAULT button).
- Accepts the vault passphrase and calls `ipfs_unlock_vault`.
- On success, updates global vault state and dismisses.
- On failure, shows a clear error message.
- The passphrase is never logged, never persisted in the frontend,
  and only held in the backend's process-local `Zeroizing<String>`
  cache during the session.

Any feature that needs the vault should dispatch a
`commander-open-vault-unlock` window event to trigger the modal,
rather than embedding its own vault unlock form.

## IPFS Settings (60x): No Legacy Plaintext-Token UX

IPFS Settings no longer shows:

- Inline vault passphrase input fields.
- Legacy plaintext migration cards or terminology.
- "Legacy plaintext token detected" status messages.

The user-facing model is simplified:

- Provider tokens are stored in the vault.
- If no provider token exists, the status shows `No token stored`.
- If the vault is locked and token presence cannot be checked,
  the status shows `Unlock vault to check saved tokens`.
- If tokens exist and the vault is unlocked, the status shows
  `Stored safely`.

Old plaintext tokens in `provider_settings.json` are treated
defensively by the backend but are not surfaced as a normal UX
state. On the next successful save with the vault unlocked,
provider settings are saved without real token values.

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

## App Secret Records

The vault is a **general encrypted container**, not a wallet-only
construct. Commander currently stores the following *optional*
app-secret record types in the vault:

- `provider.api_token` (current examples: Pinata, Filebase).

These records are *examples* of a general `provider.*` namespace,
not a permanent requirement of the wallet-vault design. Future
sensitive app settings can be added as new record types under
`app_setting.*` or similar without changing the envelope format,
the KDF, or the key slots.

Rules for app-secret records:

- They are independent of wallet records. Removing a provider
  token does not affect any wallet backup record, and removing a
  wallet record does not affect any provider token.
- They are *optional*. A user can have a vault with only wallet
  records, only app-secret records, both, or neither.
- They must continue to be stored only as encrypted
  `SecretRecord` entries inside the encrypted payload, never in
  `meta` (which is unauthenticated and non-secret).
- Their presence does not imply they are mandatory for wallet
  backups, and their absence does not block wallet backups.
- New record types should follow the existing record ID
  conventions (`<namespace>.<provider-or-feature>.<key>`) and the
  existing record shape.

Commander and WebCom share this container concept, but the set
of app-secret record types each app uses is a per-app
configuration detail, not a property of the bundle or envelope
format. See the Interop Notes below for WebCom-specific guidance.

## WebCom Interop Notes

WebCom currently has its own native vault format. The unified bridge should import/export `bundleVersion: 3` bundles without breaking existing WebCom users.

Commander and WebCom are intended to converge on this unified
vault format. The bundle, envelope, KDF, AAD, payload schema, and
record namespace language are designed to be neutral:

- `wallet.*` for wallet / backup / derivation records.
- `provider.*` for provider API credentials / settings.
- `app_setting.*` (reserved) for future sensitive app settings.
- Unknown / future record types must be preserved by any
  implementation when a vault is re-saved.

Important bridge rules:

- Do not put secrets in `meta`.
- Treat `meta` as advisory and unauthenticated. WebCom and
  Commander must not use `meta` for trust decisions, key
  derivation, record authorization, wallet identity, or any other
  security-sensitive behavior.
- Preserve legacy derivation profile strings.
- Do not silently convert legacy coin 175 records to canonical coin 420 records.
- WebCom PBKDF2-SHA256 16-byte salts are accepted for bridge import.
- WebCom must reproduce Commander's AAD field order exactly when decrypting a Commander-compatible bundle.
- Do not assume Commander-specific app-secret record types
  (`provider.pinata.api_token`, `provider.filebase.token`,
  `provider.kubo_endpoint`, etc.) exist in every WebCom bundle, or
  vice versa. Each app stores the app-secret record types it
  uses; the absence of a particular record is not an error.
- Do not assume every bundle contains wallet records. A bundle
  with only app-secret records is valid; a bundle with only wallet
  records is valid; a bundle with no records at all is valid
  (just an empty / freshly-created vault).
- Commander's IPC surface and provider-token commands
  (`ipfs_*_provider_token*`) are Commander-local. WebCom has its
  own provider/settings concepts and may later adopt this format.
  No Commander-only required field is added to the generic vault
  shape.

This section is documentation only. No WebCom code is changed in
this slice.

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
