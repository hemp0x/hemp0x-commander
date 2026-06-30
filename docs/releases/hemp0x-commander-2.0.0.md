# Hemp0x Commander 2.0.0

Hemp0x Commander 2.0.0 is the desktop manager for Hemp0x Core Next. It bundles the Core daemon, CLI, and transaction utility and gives users one local interface for node control, wallets, vaults, assets, recovery, chain inspection, and H0XC community chat.

This is a major release. Back up `wallet.dat`, Hemp0x Vault files, and any important `hemp.conf` changes before wallet migration, repair, reindex, or snapshot work.

## Bundled Core Next

- Core Next version: `v4.8.0.0-fed84d517`
- Bundled binaries:
  - `hemp0xd`
  - `hemp0x-cli`
  - `hemp0x-tx`
- Commander validates the bundled Core Next build and warns if a running daemon does not match the expected release build.
- Core Next v4.8.0.0 was refreshed to build `fed84d517` with asset amount validation hardening.

## Downloads

- Windows: `Hemp0x_Commander_2.0.0_Windows_x64_Portable.zip`
- Linux: `Hemp0x_Commander_2.0.0_Universal_Linux_x86_64.AppImage`

## Checksums

Final SHA256 checksums are published with the release artifacts in `SHA256SUMS.txt`. Verify the checksum before running.

Windows PowerShell:

```powershell
Get-FileHash .\Hemp0x_Commander_2.0.0_Windows_x64_Portable.zip -Algorithm SHA256
```

Linux:

```bash
sha256sum Hemp0x_Commander_2.0.0_Universal_Linux_x86_64.AppImage
```

## Windows Portable

1. Extract `Hemp0x_Commander_2.0.0_Windows_x64_Portable.zip` to a writable folder.
2. Run `hemp0x-commander.exe`.
3. Microsoft Edge WebView2 Runtime is required. Most Windows 10 and Windows 11 systems already include it.
4. The Windows build is unsigned. SmartScreen or antivirus products may warn on first launch. Verify the checksum before allowing the app.

## Linux AppImage

```bash
chmod +x Hemp0x_Commander_2.0.0_Universal_Linux_x86_64.AppImage
./Hemp0x_Commander_2.0.0_Universal_Linux_x86_64.AppImage
```

If your distribution blocks direct AppImage mounting, run:

```bash
APPIMAGE_EXTRACT_AND_RUN=1 ./Hemp0x_Commander_2.0.0_Universal_Linux_x86_64.AppImage
```

## Major Changes

### Hemp0x Vault wallet system

- Added portable Hemp0x Vault support for WebCom-compatible BIP39 wallet records.
- Added create-new-vault-wallet flow with 12 or 24 word recovery phrase generation.
- Added recovery phrase confirmation before leaving the wallet creation screen.
- Added import and promotion paths for:
  - legacy `wallet.dat`
  - Core runtime wallet files
  - Core migration envelopes
  - portable Hemp0x Vault records
- Added guided connect flow that restores the vault wallet into Core and verifies identity before writing alignment.
- Added recovery history flow for vault wallets.
- Added safer unload and fallback behavior when switching between vault wallets and legacy `wallet.dat`.
- Added automatic vault save/update behavior after important wallet changes.

### Wallet unlock and local PIN

- Added local runtime-wallet PIN unlock for convenience on trusted devices.
- PIN records are local to the machine and invalidated when the active runtime wallet changes.
- Wallet passphrases continue to work even when a PIN is set.
- Added wallet lock, unlock, encryption, and restart handling improvements across wallet flows.

### Core migration and recovery

- Added Core migration wallet export/import tooling.
- Added support for extensionless migration files.
- Added migration-envelope validation and restore routes through base Core RPC.
- Added safeguards so stale active-wallet routing does not affect newly generated vault wallet restores.
- Added recovery snapshot support inside the vault.

### H0XC community chat and message indexing

- Added Core message-index integration for H0XC community chat.
- Added `messageindex` support in the System config page.
- Added H0XC history recovery via `rescanmessages`.
- Added message-index status handling for nodes that are still catching up.
- Added guest read mode for public H0XC messages.
- Added local chat settings for history window, expired messages, polling, and discovery scan limits.
- Added message filtering so H0XC chat frames stay out of the general asset message inbox.
- Added support for the current H0XSHT message table format and H0XC magic-byte filtering.
- Added local handling for H0XC control frames such as moderation, delete, report, leave, status, expiry, and display policy.
- Added local-only moderation behavior. Mute, block, tag handling, and expired-message display affect the local view only.

### Node configuration and repair

- Added a smarter System config page for:
  - server
  - listen
  - daemon mode
  - messageindex
  - txindex
  - addressindex
  - assetindex
  - timestampindex
  - spentindex
  - dbcache
  - max connections
  - prune target
  - ZMQ raw transaction publishing
  - addnode entries
- Added full-feature and storage-saver presets.
- Added restart and reindex guidance when config changes require Core maintenance.
- Added reindex and chainstate repair status handling.
- Added snapshot install flow improvements.
- Added data directory switching improvements.
- Added RPC cookie-auth guidance for users moving away from static `rpcuser` and `rpcpassword`.

### Dashboard and daemon control

- Improved daemon start, stop, restart, and busy-state handling.
- Added better handling for Core startup, Core shutdown, RPC warmup, and stuck `.lock` or `.cookie` cases.
- Added daemon state indicators for sync, reindex, wallet lock state, vault lock state, RPC auth, Stratum, and Core build match.
- Added single-instance behavior and safer close handling on Windows.

### Assets and transactions

- Added updated asset dashboard and owned-asset views.
- Added network asset browsing through local Core data.
- Added message-aware asset details.
- Added wallet consolidation tools.
- Added raw transaction decode, build, mempool check, and inspection tools.
- Added destination address picker improvements.
- Added local explorer icons and copy actions in more transaction and address views.

### Receive, history, and local explorer

- Added receive-address recovery and funded-address visibility improvements.
- Added receive page action buttons for copy, label edit, and explorer lookup.
- Added local transaction and address explorer.
- Added paged transaction loading for large addresses.
- Added history and journal navigation improvements.
- Added local explorer handling for pruned nodes and unavailable previous input values.

### Solo mining and Stratum

- Added a new solo mining workspace for running a local Stratum server from Commander.
- Added worker address selection for solo mining payouts.
- Added Stratum connection details for miner setup.
- Added Stratum status reporting in the app header and solo mining page.
- Added Core RPC readiness checks for solo mining startup.
- Preserved solo-only behavior. Commander does not add pool payout, ledger, confirmation, or consolidation logic to solo mining.

### Performance and responsiveness

- Moved expensive Core and CLI work off the UI thread.
- Reworked polling and daemon operations to avoid renderer stalls.
- Improved Windows responsiveness when Core is busy, starting, stopping, rescanning, or reindexing.
- Added timeouts and progress states for long-running operations.
- Reduced cases where repeated clicks could stack daemon operations.
- Improved AppImage launch behavior on Linux systems without standard AppImage FUSE support.

### UI and release polish

- Reworked the main navigation and Tools submenus.
- Added a cleaner dark interface across dashboard, wallet, assets, system, explorer, console, and advanced tools.
- Improved modal sizing for smaller windows.
- Improved number input styling and app-wide button rendering.
- Updated app icons and release packaging.
- Added release build documentation for Windows portable and Linux AppImage artifacts.

## License

Hemp0x Commander 2.0.0 is released under the MIT License.

Copyright (c) 2026 Hemp0x Devs

The MIT License applies to the Commander codebase, including the H0XC and H0XSHT message handling code, encoder and decoder logic, and tables included in this repository.

Bundled Core Next binaries, platform runtimes, and third-party dependencies keep their own licenses. See `docs/THIRD_PARTY_NOTICES.md`.

## Known Notes

- H0XC full history requires `messageindex=1` and a completed message backfill on nodes that synced before enabling the index.
- Pruned nodes may not be able to recover all historical wallet, asset, or message data.
- Some Core index changes require reindexing or chainstate rebuilds.
- Unsigned Windows artifacts can trigger SmartScreen or antivirus warnings.
- Commander is non-custodial. It cannot recover lost wallet passphrases, lost recovery phrases, or lost vault passphrases.

## Safety Checklist

Before upgrading or testing:

- Back up `wallet.dat`.
- Back up Hemp0x Vault files.
- Back up important `hemp.conf` changes.
- Verify release checksums.
- Keep a copy of any old Commander release you still depend on until the new one is tested with your data.
