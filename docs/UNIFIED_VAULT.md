# Hemp0x Vault

The **Hemp0x Vault** is the portable encrypted wallet and app-secret
container shared by Hemp0x Commander and Hemp0x WebCom.

User-facing name:

```text
Hemp0x Vault
```

Wire-format identifier:

```text
hemp0x-unified-vault-bundle
```

The wire-format identifier is part of compatibility and must not be renamed
for branding changes.

## Current Scope

The vault stores encrypted records such as:

- Core Next wallet migration envelopes
- WebCom wallet records
- IPFS provider API tokens
- shared Hemp0x address book data
- future app-secret and wallet metadata records

The vault is the recommended portable backup file. Core still uses runtime
wallet files internally today. Commander can restore a runtime wallet from a
vault-stored Core migration record, but Core cannot yet run directly from an
in-memory vault record without writing wallet files.

## File Location

Commander stores its active vault here:

```text
<active Hemp0x data dir>/commander/vault.json
```

Default Linux mainnet path:

```text
~/.hemp0x/commander/vault.json
```

Local non-secret vault labels and known-file metadata are stored separately:

```text
<active Hemp0x data dir>/commander/vault_index.json
```

`vault_index.json` must never contain passphrases, passphrase hints, private
keys, mnemonics, provider tokens, decrypted vault contents, or other secrets.

## Bundle Format

New vault files use a bundle wrapper:

```json
{
  "bundleVersion": 3,
  "format_identifier": "hemp0x-unified-vault-bundle",
  "vault": {},
  "meta": null
}
```

`meta` is public, optional, advisory metadata. It is not authenticated and must
never be used for trust decisions. Do not store secrets in `meta`.

For development compatibility, Commander can read older wrapper names:

- `bundle_version`
- `public_meta`

New writes use `bundleVersion` and `meta`.

## Envelope Format

The encrypted `vault` envelope contains:

| Field | Value |
|---|---|
| `version` | `1` |
| `schema_identifier` | `hemp0x-commander-vault` |
| `app_identifier` | writer app, usually `hemp0x-commander` or `hemp0x-webcom` |
| `network` | `mainnet`, `testnet`, or `regtest` |
| `cipher_profile` | `aes-256-gcm-v1` |
| `aad_profile` | `commander-envelope-v1` |
| `payload` | encrypted payload block |
| `key_slots` | one or more encrypted DEK slots |
| `created` / `modified` | Unix timestamps in seconds |

Network validation is local metadata validation only. Unlocking the vault does
not require internet access, RPC access, daemon availability, or node sync.

## Crypto Model

The vault uses a DEK/key-slot model:

1. Generate a random 32-byte data encryption key.
2. Encrypt the payload with AES-256-GCM.
3. Derive a wrapping key from the user passphrase.
4. Wrap the DEK into one or more key slots with AES-256-GCM.

Supported KDF profiles:

| Profile | Purpose |
|---|---|
| `scrypt-v1` | default for new Commander/WebCom vaults |
| `pbkdf2-hmac-sha512-v1` | Core Next compatibility |
| `pbkdf2-sha256-v1` | WebCom bridge compatibility |

All current profiles use `kdf_dklen: 32`.

## AAD Contract

The authenticated-data field order is part of the format. Changing it breaks
interop.

Payload AAD:

```text
schema_identifier:version:app_identifier:network:cipher_profile:aad_profile:payload_schema:created:modified
```

Key-slot wrap AAD:

```text
schema_identifier:version:app_identifier:network:slot_id:slot_type:kdf_profile:kdf_params:kdf_dklen:wrap_cipher_profile:created
```

KDF parameter string:

- PBKDF2: decimal iteration count
- scrypt: `log_n/r/p`

## Payload Format

The decrypted payload is a map of `SecretRecord` values:

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
        "endpoint": "https://api.pinata.cloud",
        "token_kind": "jwt"
      },
      "tags": ["provider", "ipfs"],
      "origin_app": "hemp0x-commander",
      "network": "mainnet",
      "created": 1718136000,
      "modified": 1718136000
    }
  }
}
```

`value` contains the protected record content. Do not return `value` to the
frontend in metadata/list APIs for wallet keys, provider tokens, swap secrets,
or other sensitive records.

Unknown record IDs and record types must be preserved when an app updates the
vault.

## Record Namespaces

Current and reserved namespaces:

| Namespace | Use |
|---|---|
| `wallet.*` | wallet records, migration envelopes, derivation metadata |
| `provider.*` | provider credentials and provider-specific secrets |
| `app_setting.*` | portable app settings that may contain sensitive values |
| `protocol.*` | protocol-specific keys such as future Nostr records |
| `note.*` | future secure notes |

Implemented record types:

- `provider.api_token`
- `wallet.bip39` (Commander creates `wallet.webcom.hemp.primary` records)
- `wallet.core_migration_envelope`
- `app_setting.hemp0x.address_book.v1`
- `app_setting.commander.wallet_alignment`

Reserved record types:

- `wallet.bip39`
- `wallet.wif`
- `wallet.hardware_metadata`
- `wallet.watch_only`
- `protocol.nostr_key`
- `app.secret`
- `note.secure`

## Core Wallet Migration Records

Commander stores Core Next migration envelopes as records whose IDs start with:

```text
wallet.core_migration_envelope.
```

Example:

```json
{
  "record_id": "wallet.core_migration_envelope.export-1718136000",
  "record_type": "wallet.core_migration_envelope",
  "label": "Main wallet backup",
  "value": "{...encrypted Core migration envelope JSON...}",
  "metadata": {
    "value_kind": "embedded_encrypted_json",
    "source": "core-next-exportwalletmigration",
    "restorable": true,
    "private_keys_included": true,
    "envelope_kdf_profile": "pbkdf2-hmac-sha512-v1",
    "envelope_cipher_profile": "aes-256-gcm-v1",
    "envelope_aad_profile": "commander-envelope-v1",
    "envelope_coin_type": 420,
    "recovery_mode": "vault_passphrase"
  },
  "tags": ["wallet", "migration"],
  "origin_app": "hemp0x-commander",
  "network": "mainnet",
  "created": 1718136000,
  "modified": 1718136000
}
```

Recovery modes:

| Mode | Meaning |
|---|---|
| `vault_passphrase` | migration envelope passphrase is the vault passphrase |
| `separate_passphrase` | migration envelope has its own recovery passphrase |
| missing / unknown | treat as `separate_passphrase` |

The live Core wallet unlock password is never stored in the vault.

## WebCom Wallet Records

WebCom writes one primary Hemp wallet record:

```text
record_id: wallet.webcom.hemp.primary
record_type: wallet.bip39
```

The `value` is sensitive. It may contain a BIP39 mnemonic or a WebCom WIF marker
for a single-key wallet.

Important metadata:

```json
{
  "account": 0,
  "external_count": 20,
  "change_count": 6,
  "recovered_external_indices": [],
  "recovery": {
    "schemaVersion": 1,
    "seedType": "bip39",
    "network": "mainnet",
    "derivationProfiles": {
      "hemp": "hemp0x.mainnet.bip44.p2pkh.coin420.v1",
      "btc": "btc.mainnet.bip84.p2wpkh.v1"
    }
  }
}
```

Supported Hemp derivation profiles:

- `hemp0x.mainnet.bip44.p2pkh.coin420.v1`
- `hemp0x.webcom.legacy.bip44.p2pkh.coin175.v1`
- `hemp0x.mainnet.bip44.p2pkh.v1`
- `hemp0x.mainnet.wif.single.v1`

Rules:

- Do not silently convert legacy coin type 175 records to coin type 420.
- Do not trust balances or asset holdings from vault metadata.
- Recalculate balances from chain/index data.
- Do not expose mnemonic, WIF, or private-key values through metadata APIs.

WebCom may also write:

```text
wallet.webcom.btc_lite.primary
app_setting.webcom.swap_secrets
```

Commander currently preserves those records. BTC Lite is metadata-only in
Commander. WebCom swap secrets are opaque and must not be parsed or modified by
Commander.

## Shared Address Book Record

Commander and WebCom share this record:

```text
record_id: app_setting.hemp0x.address_book
record_type: app_setting.hemp0x.address_book.v1
```

The `value` is a JSON string:

```json
{
  "schema": "hemp0x.address_book",
  "schema_version": 1,
  "exported_at": 1718136000,
  "entries": [
    {
      "chain": "hemp0x",
      "label": "Alice",
      "address": "R...",
      "locked": true
    },
    {
      "chain": "bitcoin",
      "label": "Bob",
      "address": "bc1q...",
      "locked": false
    }
  ]
}
```

Valid `chain` values:

- `hemp0x`
- `bitcoin`

Commander imports only `chain: "hemp0x"` entries into its local send-page
address book today. Bitcoin entries are preserved in the vault record.

Import behavior:

- merge by normalized address
- prefer the existing local label unless it is blank
- preserve `locked` with OR semantics
- do not delete local entries that are missing from the vault
- malformed address-book records fail closed and do not overwrite local data

Export behavior:

- convert local Commander address book entries to `chain: "hemp0x"`
- preserve existing `chain: "bitcoin"` entries from the vault record
- preserve unrelated vault records

## Provider Tokens

Provider tokens are optional app-secret records. Current records:

```text
provider.pinata.api_token
provider.filebase.token
```

Record type:

```text
provider.api_token
```

Provider settings such as endpoints may remain outside the secret value when
they are not sensitive. Token values must live only inside encrypted
`SecretRecord.value`.

Blank token updates preserve existing records. Removing a token requires an
explicit remove operation.

## UI Entry Points

The Wallet page is organized around three primary portable-wallet actions,
with legacy compatibility tooling hidden behind an Advanced / Legacy Backups
section:

1. **Create New Hemp0x Vault / Wallet** — create a vault and generate a
   12- or 24-word BIP39 recovery phrase.
2. **Import Hemp0x Vault** — import a vault file from WebCom or another
   Commander install.
3. **Import Legacy Wallet File** — advanced recovery from a Core
   `wallet.dat` / named wallet file.

| Feature | Location |
|---|---|
| create/unlock/lock vault | Tools -> Wallet |
| create new wallet (generate BIP39, 12 or 24 words) | Tools -> Wallet -> Create New Wallet |
| create new wallet from recovery phrase | Tools -> Wallet -> Restore from Recovery Phrase |
| import vault file from WebCom/Commander | Tools -> Wallet -> Import Hemp0x Vault |
| save/export Hemp0x Vault file | Tools -> Wallet -> Save Hemp0x Vault |
| connect vault wallet to Core | Tools -> Wallet -> Connect Vault Wallet |
| encrypt runtime wallet | Tools -> Wallet -> Encrypt Wallet |
| import legacy wallet.dat file | Tools -> Wallet -> Advanced / Legacy Backups |
| back up Core runtime wallet into vault (legacy record) | Tools -> Wallet -> Advanced / Legacy Backups -> Add Current Core Wallet To Vault |
| restore Core runtime wallet from vault (legacy record) | Tools -> Wallet -> Advanced / Legacy Backups -> Restore From Backup |
| export/import private keys | Tools -> Wallet -> Advanced / Legacy Backups |
| Core migration file tools | Tools -> Wallet -> Advanced / Legacy Backups -> Legacy Migration Files |
| IPFS provider tokens | Tools -> IPFS -> Settings |
| global vault status | app trust strip |

The normal user path does not expose migration-envelope records, `wallet.dat`,
`wallet=`, BDB/SQLite, or raw Core RPC names. Those details live only inside
the collapsed Advanced / Legacy Backups section.

Features that need vault access should use the shared vault unlock modal by
dispatching:

```text
commander-open-vault-unlock
```

Do not add separate vault passphrase forms to feature pages.

## Import And Export Behavior

Exporting a vault file copies the encrypted bundle as-is. It does not decrypt
the vault and does not expose secrets through IPC.

Importing a vault file replaces the active vault. It does not merge records.
Before replacement, Commander validates:

- valid JSON
- supported `bundleVersion`
- `format_identifier: "hemp0x-unified-vault-bundle"`
- supported network
- supported cipher profile
- supported AAD profile

After import, Commander clears the cached unlock session. The imported vault
starts locked.

## Cached Unlock Session

Commander can cache the vault passphrase in backend memory for the current app
session.

Properties:

- stored in a process-local `Zeroizing<String>`
- guarded by a mutex
- never persisted
- never logged
- never returned to the frontend
- cleared on lock, vault replacement, and vault decrypt failure

The cache is convenience only. The vault file remains encrypted at rest.

## Security Rules

- Do not store secrets in `meta` or `vault_index.json`.
- Do not return `SecretRecord.value` through list/summary APIs.
- Treat public metadata as advisory.
- Preserve unknown records when updating known records.
- Fail closed on malformed encrypted payloads or malformed sensitive records.
- Do not store Core runtime wallet unlock passwords in the vault.
- Do not export PIN, biometric, WebAuthn, hardware session, or local unlock
  convenience material in the portable vault.
- Do not claim a `wallet.dat` is WebCom-portable unless actual BIP39 or WIF
  recovery material is exported in a WebCom-supported record.

## Version Map

Do not confuse these layers:

| Layer | Current Version | Profile/Identifier |
|---|---|---|
| Hemp0x Vault bundle | `bundleVersion: 3` | `hemp0x-unified-vault-bundle` |
| Commander vault envelope | `version: 1` | `hemp0x-commander-vault` |
| Core Next migration envelope | `envelope_version: 2` | `hemp0x-core.migration-envelope.v2` |
| Core private migration payload | `payload_version: 1` | `hemp0x-core.private-migration-payload.v1` |

## Current Limitations

- Commander can restore Core runtime wallet files from vault-stored Core
  migration records.
- Commander can connect a WebCom BIP39 vault wallet to Core via the
  migration envelope bridge (see Portable Wallet Flow below).
- Commander can create new BIP39 wallets and restore from recovery phrases
  directly into the vault and Core.
- Commander cannot load Core directly from an in-memory vault record.
- WebCom wallet records are metadata-preview/preservation for non-BIP39
  coin420 records.
- Commander cannot export the active `wallet.dat` contents as
  `wallet.webcom.hemp.primary`. The Core `getmywords` RPC is not wrapped.
  This is deferred.
- BTC Lite records are preserved but not managed by Commander.
- Post-restore encryption is a user action; Core restore creates an
  unencrypted runtime wallet. Use the Encrypt Wallet button on the Wallet page.
- Post-restore Core wallet backup is deferred; the RPC wrapper cannot
  reliably target named wallets.

## Portable Wallet Model

This section records the stable model decisions for portable wallet records.
It is the only authority for these IDs and namespaces; do not invent new ones
without updating this section.

### WebCom Primary Wallet Record

The WebCom-compatible primary Hemp wallet record is:

```text
record_id: wallet.webcom.hemp.primary
record_type: wallet.bip39
```

WebCom loads exactly this record ID. Commander exposes it as the WebCom
interface slot. The supported `value_kind` is a BIP39 mnemonic (12 or 24
words) or a WebCom WIF marker for a single-key wallet.

Required metadata for Commander to safely preview/restore:

```json
{
  "account": 0,
  "external_count": 20,
  "change_count": 6,
  "recovery": {
    "schemaVersion": 1,
    "seedType": "bip39" | "wif",
    "network": "mainnet" | "testnet" | "regtest",
    "derivationProfiles": {
      "hemp": "hemp0x.mainnet.bip44.p2pkh.coin420.v1"
    }
  }
}
```

Rules:

- Do not silently convert legacy `coin175` profiles to `coin420`.
- Do not trust balances or asset holdings from vault metadata.
- Do not return the `value` field through metadata APIs.
- Do not write to `wallet.webcom.hemp.primary` without all safety
  requirements in this document being met.

### Commander-Owned Candidate Records (reserved)

For future multi-wallet vaults, Commander-owned wallet records use:

```text
wallet.commander.hemp.<stable-id>
```

These records are Commander-internal. They must not be loaded by WebCom and
must not be written into `wallet.webcom.hemp.primary` automatically.

### WIF / Key-Set Records

A WIF single-key wallet uses `record_type: wallet.wif` under the WebCom
primary record ID. A multi-key/key-set record is not yet defined; if Core
exposes loose imported keys that are not covered by a single mnemonic, they
must be exported as individual WIFs and re-imported per address.

WebCom import of WIF records requires WebCom-side support and is marked as
"requires WebCom support" until that lands.

### Core Migration Records

Core Next migration envelopes remain under the existing namespace:

```text
record_id: wallet.core_migration_envelope.*
record_type: wallet.core_migration_envelope
```

These are Commander/Core recovery records. They are not WebCom wallet records
and must not be presented to WebCom as a wallet record.

## Portable Wallet Metadata-Only API

Commander exposes three metadata-only backend commands for the Wallet
page. They never return `value` content and never write to
`wallet.webcom.hemp.primary`:

| Command | Purpose |
|---|---|
| `vault_list_portable_wallet_records` | List all wallet-like vault records with metadata only. |
| `vault_preview_webcom_primary_restore` | Validate `wallet.webcom.hemp.primary` metadata and return a restore-compatibility preview. |
| `vault_preview_core_wallet_export_capability` | Honest read-only preview of what Core Next can export/import for the active wallet. |

Cached-session wrappers (`ipfs_vault_*`) follow the existing vault unlock
pattern. These commands are backend building blocks for the guided wallet
alignment flow; they are not intended to be exposed as a diagnostics-heavy
user interface.

These helpers do not implement a "promote Commander record to WebCom primary"
operation. That operation is intentionally deferred.

## Wallet Alignment Model

Commander records alignment metadata in the vault so that future imports
of the same vault, or a re-opened vault session, do not repeat wallet setup.
The alignment record tracks the relationship between the active Core runtime
wallet and the vault's WebCom primary wallet record.

### Alignment Record

```text
record_id: app_setting.commander.wallet_alignment
record_type: app_setting.commander.wallet_alignment
```

The record `value` is always empty — alignment metadata is non-secret
public-info only. All meaningful data lives in `metadata`:

```json
{
  "schema": "hemp0x.commander.wallet_alignment",
  "schema_version": 1,
  "active_wallet_record_id": "wallet.webcom.hemp.primary",
  "active_wallet_fingerprint": "...",
  "core_wallet_name": "hemp0x-vault-main",
  "core_wallet_source": "webcom_bip39 | commander_bip39 | core_migration | wallet_dat_legacy | wif_keyset",
  "derivation_profile": "hemp0x.mainnet.bip44.p2pkh.coin420.v1",
  "network": "mainnet",
  "created_at": 0,
  "updated_at": 0,
  "last_verified_at": 0,
  "core_migration_backup_record_id": "",
  "post_connect_backup_record_id": "",
  "verification_method": "core_restorewalletmigration_from_generated_envelope",
  "connection_state": "verified_aligned",
  "notes": []
}
```

An alignment record without `verification_method` (old slice-64c records)
is treated as `connection_state: stale_unverified_alignment` and is not
considered aligned. Only the `vault_connect_webcom_primary_wallet_to_core`
flow can write a verified alignment record.

### Safety Rules

- The alignment record is non-secret; it contains only public metadata.
- The fingerprint is a hash of public derivation metadata only.
- Only the Core restore/connect flow writes verified alignment records.
  Manual metadata-only alignment (`vault_create_or_update_alignment_record`)
  is deprecated and returns an error.
- The alignment record preserves unknown vault records.
- WebCom primary is never overwritten by the alignment flow.
- Temp migration envelope files are cleaned up after use.

### Alignment API

| Command | Purpose |
|---|---|
| `vault_get_wallet_alignment_status_v2` | Return vault, WebCom primary, Core wallet, and alignment record status with hardened connection_state, verification_status, and recommended_next_action. Never returns secret values. |
| `vault_preview_connect_webcom_primary_to_core` | Plan/preview the WebCom vault BIP39 wallet → Core runtime wallet connection flow. Returns can_execute: true when conditions are met. |
| `vault_preview_export_core_wallet_to_webcom_primary` | Plan/preview whether the active Core runtime wallet can be exported as a WebCom-compatible BIP39 wallet record. |
| `vault_connect_webcom_primary_wallet_to_core` | **Verified execute**: bridges a WebCom BIP39 vault record into a Core runtime wallet via Commander-generated migration envelope → validatewalletmigration → restorewalletmigration → verified alignment record. Mnemonic never leaves the backend. |
| `vault_backup_current_core_wallet_before_alignment` | Safe execute: export the current Core runtime wallet into the vault before any alignment operation. |

`vault_create_or_update_alignment_record` is **deprecated**. Manual metadata-only alignment is no longer supported. Verified alignment records can only be written by the `vault_connect_webcom_primary_wallet_to_core` restore flow.

Recommended next actions returned by the v2 status command:

- `unlock_vault` — vault is locked
- `create_vault` — no vault or no WebCom primary record
- `connect_webcom_wallet` — Core is not reachable
- `needs_backup` — WebCom primary detected, Core wallet exists, backup required before connect
- `restore_available` — ready to connect vault wallet to Core
- `already_aligned` — verified alignment record exists

### Portable Wallet Flow

Commander now supports the full bridge from WebCom `wallet.bip39` vault record
to a Core runtime wallet:

1. User imports/loads a Hemp0x Vault from WebCom.
2. Commander detects `wallet.webcom.hemp.primary` (record_type: wallet.bip39).
3. Commander builds a Core v2 encrypted migration envelope from the WebCom
   BIP39 record in backend memory. The mnemonic never leaves the backend.
4. Commander validates the envelope with Core `validatewalletmigration`.
5. Commander restores it into a new named Core wallet with
   `restorewalletmigration`.
6. Commander verifies the restore result matches the canonical coin420 profile.
7. Commander writes a verified alignment record with
   `connection_state: verified_aligned` and
   `verification_method: core_restorewalletmigration_from_generated_envelope`.
8. Future vault loads detect verified alignment and do not repeat setup.

The reverse direction (Commander → WebCom primary export) is deferred.
Existing `exportwalletmigration` backups continue to work for Core recovery.

### Pre-Connect Backup Gate

If a current Core runtime wallet exists, Commander requires a pre-connect
backup before connecting a vault wallet. Without a backup, the connect
command returns a clear error. The user must create a backup via the
Backup tab first.

## Compatibility Checklist

Any Commander or WebCom change that writes vault files must verify:

1. AAD field order is unchanged.
2. Unknown records survive a read-modify-write cycle.
3. Provider token records survive wallet/address-book operations.
4. WebCom wallet records survive Commander operations.
5. Commander wallet migration records survive WebCom operations.
6. Address book records round-trip through both apps.
7. Secret values are not returned in metadata/list APIs.
8. Wrong passphrase, tampered AAD, and tampered ciphertext fail closed.
