# Hemp0x Commander 2.0.0

Hemp0x Commander 2.0.0 is the desktop manager for Hemp0x Core Next. It bundles the Core Next daemon, CLI, and transaction utility so users can run, manage, and recover their Hemp0x node and wallet from one interface.

## Bundled Core Next

- Core Next version: `v4.8.0.0-6c18fe5a2`
- Bundled binaries:
  - `hemp0xd`
  - `hemp0x-cli`
  - `hemp0x-tx`

## Downloads

- Windows: `Hemp0x_Commander_2.0.0_Windows_x64_Portable.zip`
- Linux: `Hemp0x_Commander_2.0.0_Universal_Linux_x86_64.AppImage`

## Checksums

```text
18381b8fd9d835518ff1057df3a36a32bd84ec389932b06217f6aada1d5ccce9  Hemp0x_Commander_2.0.0_Universal_Linux_x86_64.AppImage
79e3f66a06ed38f1eec0de6d484c810ecfb5744c00f3881e92b347b434297a96  Hemp0x_Commander_2.0.0_Windows_x64_Portable.zip
```

Verify before running.

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
3. If SmartScreen or antivirus warns on first launch, verify the checksum before allowing the app. The Windows build is unsigned.
4. Microsoft Edge WebView2 Runtime is required. Most Windows 10/11 systems already include it.

## Linux AppImage

```bash
chmod +x Hemp0x_Commander_2.0.0_Universal_Linux_x86_64.AppImage
./Hemp0x_Commander_2.0.0_Universal_Linux_x86_64.AppImage
```

The AppImage uses a universal runtime path and is built to run without `libfuse2` on systems where AppImage FUSE mounting is unavailable. If a distribution blocks direct AppImage mounting, run with:

```bash
APPIMAGE_EXTRACT_AND_RUN=1 ./Hemp0x_Commander_2.0.0_Universal_Linux_x86_64.AppImage
```

## Highlights

- Core Next v4.8.0.0 packaging with message-index support.
- H0XC community chat message-index integration and history recovery flow.
- Hemp0x Vault wallet flow for portable vault-backed wallets.
- Runtime wallet PIN unlock support for local convenience.
- Improved daemon start/stop responsiveness and shutdown handling.
- Smart node configuration helpers, including message index, prune, RPC, and repair guidance.
- Local explorer, receive-address tools, asset tools, wallet recovery, and solo mining UI refinements.
- Windows portable package with static Core sidecars.
- Universal Linux AppImage with bundled Core sidecars.

## Notes

- Back up `wallet.dat` and any Hemp0x Vault files before major wallet or node maintenance.
- Enabling or changing some indexes can require reindexing. Commander will guide this from the System tools.
- H0XC full history requires `messageindex=1` and a completed message backfill on nodes that synced before enabling the index.
- Pruned nodes may not be able to recover all historical wallet, asset, or message data.
- These release artifacts are unsigned. Checksum verification is required for trust.
