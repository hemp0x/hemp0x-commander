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
- `wallet.core_migration_envelope`
- `app_setting.hemp0x.address_book.v1`

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

| Feature | Location |
|---|---|
| create/unlock/lock vault | Tools -> Wallet |
| back up current vault file | Tools -> Wallet -> Vault File |
| switch/import vault file | Tools -> Wallet -> Vault File |
| back up Core runtime wallet into vault | Tools -> Wallet -> Backup |
| restore Core runtime wallet from vault | Tools -> Wallet -> Restore |
| WebCom interop preview/address book sync | Tools -> Wallet -> Advanced |
| legacy wallet.dat actions | Tools -> Wallet -> Advanced |
| IPFS provider tokens | Tools -> IPFS -> Settings |
| global vault status | app trust strip |

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

## Current Limitations

- Commander can restore Core runtime wallet files from vault-stored Core
  migration records.
- Commander does not yet load Core directly from an in-memory vault record.
- WebCom wallet records are currently metadata-preview/preservation records in
  Commander.
- Commander does not yet export active `wallet.dat` contents as
  `wallet.webcom.hemp.primary`.
- BTC Lite records are preserved but not managed by Commander.

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
