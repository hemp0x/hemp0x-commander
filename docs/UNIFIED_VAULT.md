# Hemp0x Unified Vault

This document summarizes the Commander/WebCom/Core Next vault direction used by Hemp0x Commander.

The vault is a local encrypted bundle for application secrets. In Commander v2 it is used for IPFS provider tokens. The format is designed to later carry wallet migration artifacts, WebCom-compatible wallet records, watch-only metadata, hardware-wallet metadata, and other app secrets without changing the outer contract.

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

## Current Deferred Work

These are intentional future slices, not missing pieces in the current vault foundation:

- vault backup/export and restore/import UI
- provider-token remove UI
- passphrase change
- DEK rotation
- KDF upgrade migration
- PIN/biometric/multi-slot unlock
- hardware-wallet and watch-only vault UX
- deeper plaintext payload zeroization

