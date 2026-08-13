# Hemp0x Commander 2.0.1

Hemp0x Commander 2.0.1 is an emergency maintenance release for the coordinated Core Next KAWPOW safety update. Users and operators should update before block `3,894,000`.

Back up `wallet.dat`, Hemp0x Vault files, and any important `hemp.conf` changes before wallet migration, repair, reindex, or snapshot work.

## Bundled Core Next

- Core Next version: `v4.8.1.0-3e061497a`
- Bundled binaries:
  - `hemp0xd`
  - `hemp0x-cli`
  - `hemp0x-tx`
- Commander validates the bundled Core Next build and warns if a running daemon does not match the expected release build.
- Core Next v4.8.1.0 adds the KAWPOW header-height safety update at block `3,894,000`.

## Downloads

- Windows: `Hemp0x_Commander_2.0.1_Windows_x64_Portable.zip`
- Linux: `Hemp0x_Commander_2.0.1_Universal_Linux_x86_64.AppImage`

## Checksums

Final SHA256 checksums are published with the release artifacts in `SHA256SUMS.txt`. Verify the checksum before running.

Windows PowerShell:

```powershell
Get-FileHash .\Hemp0x_Commander_2.0.1_Windows_x64_Portable.zip -Algorithm SHA256
```

Linux:

```bash
sha256sum Hemp0x_Commander_2.0.1_Universal_Linux_x86_64.AppImage
```

## Windows Portable

1. Extract `Hemp0x_Commander_2.0.1_Windows_x64_Portable.zip` to a writable folder.
2. Run `hemp0x-commander.exe`.
3. Microsoft Edge WebView2 Runtime is required. Most Windows 10 and Windows 11 systems already include it.
4. The Windows build is unsigned. SmartScreen or antivirus products may warn on first launch. Verify the checksum before allowing the app.

## Linux AppImage

```bash
chmod +x Hemp0x_Commander_2.0.1_Universal_Linux_x86_64.AppImage
./Hemp0x_Commander_2.0.1_Universal_Linux_x86_64.AppImage
```

If your distribution blocks AppImage mounting, run:

```bash
APPIMAGE_EXTRACT_AND_RUN=1 ./Hemp0x_Commander_2.0.1_Universal_Linux_x86_64.AppImage
```

## What Changed Since 2.0.0

- Updated bundled Core Next sidecars from `v4.8.0.0-fed84d517` to `v4.8.1.0-3e061497a`.
- Updated Commander runtime validation so the top status bar and startup checks require the new Core build.
- Kept wallet, vault, H0XC, messageindex, asset, console, explorer, and solo mining behavior compatible with Commander 2.0.0.
- Kept Peer Guard behavior compatible with current Hemp0x 4.7+ peers and protected addnode peers.

## Operator Verification

After updating, start the daemon and verify:

```bash
hemp0x-cli getnetworkinfo
hemp0x-cli getblockchaininfo
hemp0x-cli getblockhash 3887915
```

Expected checkpoint hash for block `3887915`:

```text
0000000b68a76df70e3a0451ec1deb9959c3df733968970ed7b8769ce365c11e
```

`getnetworkinfo` should report a subversion or build commit containing:

```text
v4.8.1.0-3e061497a
```

## Notes

- Update before block `3,894,000`.
- Do not run older Core Next binaries after activation unless instructed by the project maintainers.
- Pools, public services, WebCom nodes, and Commander users should all move to this release.

## License

Hemp0x Commander is released under the MIT License.

Copyright (c) 2026 Hemp0x Devs

Bundled Core Next binaries, platform runtimes, and third-party dependencies keep their own licenses. See [Third-Party Notices](../THIRD_PARTY_NOTICES.md).
