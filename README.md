# Hemp0x Commander

<div align="center">
  <a href="https://hemp0x.com">
    <img src="src/assets/hemp0xgit.png" alt="Hemp0x Logo" width="160" />
  </a>

  <h3>All-In-One Manager for the Hemp0x Blockchain</h3>

  <p>
    A secure, non-custodial, cross-platform desktop app for managing your Hemp0x node, wallet, assets, vaults, and H0XC community chat.
  </p>

  <br />

  [![Website](https://img.shields.io/badge/WEBSITE-hemp0x.com-000000?style=for-the-badge&logo=globe&logoColor=00ff00&labelColor=000000)](https://hemp0x.com)
  [![Download](https://img.shields.io/badge/DOWNLOAD-v2.0.0-000000?style=for-the-badge&logo=windows&logoColor=00ff00&labelColor=000000)](https://github.com/hemp0x/hemp0x-commander/releases/tag/v2.0.0)
  [![Discord](https://img.shields.io/badge/DISCORD-JOIN_US-000000?style=for-the-badge&logo=discord&logoColor=00ff00&labelColor=000000)](https://discord.gg/FMEKJUwcsu)
  [![Build Guides](https://img.shields.io/badge/DOCS-BUILD_GUIDES-000000?style=for-the-badge&logo=rust&logoColor=00ff00&labelColor=000000)](docs/BUILDING.md)
</div>

<br />

![Dashboard Preview](screenshots/dashboard.png)

---

## Overview

**Hemp0x Commander** brings Hemp0x Core Next to the desktop. Built with **Tauri v2** (Rust) and **Svelte 5**, it combines local-node security with a modern interface for wallet, asset, vault, message, and node operations.

> [!WARNING]
> ### Release Notice
> **Hemp0x Commander 2.0.0 is a major release and should still be treated as early production software.**
>
> * Verify release checksums before running downloaded artifacts.
> * Back up `wallet.dat` and Hemp0x Vault files before wallet, vault, or node maintenance.
> * Unsigned Windows builds may trigger SmartScreen or antivirus warnings.
> * Use at your own risk. We are not responsible for lost funds or data.

### Key Features

| Feature | Description | Status |
| :--- | :--- | :--- |
| **Node Control** | Start, stop, configure, repair, and monitor bundled Hemp0x Core Next from Commander. | **Ready** |
| **Wallet & Vaults** | Use legacy `wallet.dat`, portable Hemp0x Vault wallets, recovery phrases, wallet imports, and local PIN unlock. | **Ready** |
| **Asset Management** | Issue, reissue, transfer, browse, and inspect Hemp0x assets through your local node. | **Ready** |
| **H0XC Community Chat** | Use message-index backed on-chain community chat with local moderation and history recovery. | **Ready** |
| **Local Explorer** | Inspect local transactions, addresses, assets, UTXOs, and wallet history without third-party explorers. | **Ready** |
| **Privacy First** | Uses authenticated local RPC. No hosted wallet service is required. | **Ready** |

---

## Gallery

<div align="center">

### Dashboard
<img src="screenshots/dashboard.png" width="100%" alt="Dashboard" />
<br/><br/>

### Asset Management
<img src="screenshots/assets.png?v=2.0" width="100%" alt="Asset Creation" />
<br/><br/>

### Tools & System
<div style="display: flex; flex-wrap: wrap; gap: 10px; justify-content: center;">
  <img src="screenshots/network.png" width="48%" alt="Network Tools" />
  <img src="screenshots/system.png" width="48%" alt="System Status" />
</div>
<br/>

### About
<img src="screenshots/about.png" width="100%" alt="About" />

</div>

---

## Installation

Windows and Linux builds are provided on the GitHub release page.

### Windows
1. Download `Hemp0x_Commander_2.0.0_Windows_x64_Portable.zip` from the **[v2.0.0 release](https://github.com/hemp0x/hemp0x-commander/releases/tag/v2.0.0)**.
2. Verify the SHA256 checksum from the release notes.
3. Extract the zip to a writable folder.
4. Launch `hemp0x-commander.exe`.

### Linux
1. Download `Hemp0x_Commander_2.0.0_Universal_Linux_x86_64.AppImage` from the **[v2.0.0 release](https://github.com/hemp0x/hemp0x-commander/releases/tag/v2.0.0)**.
2. Verify the SHA256 checksum from the release notes.
3. Make it executable and run it:

```bash
chmod +x Hemp0x_Commander_2.0.0_Universal_Linux_x86_64.AppImage
./Hemp0x_Commander_2.0.0_Universal_Linux_x86_64.AppImage
```

If your distribution blocks AppImage mounting, use:

```bash
APPIMAGE_EXTRACT_AND_RUN=1 ./Hemp0x_Commander_2.0.0_Universal_Linux_x86_64.AppImage
```

---

## Contributing & Bugs

This application is merely a **Visual Shell** interacting with the core `hemp0xd` binaries. If the shell breaks, your coins are safe in the daemon.

**Found a glitch? Want a feature?**
We value your feedback.
*   **Report it** on the [Hemp0x Discord](https://discord.gg/FMEKJUwcsu).
*   **Fix it** and submit a PR.

Together we build the future of Hemp0x.

---

## 🛠️ Building from Source

**Prerequisites:** Node.js v18+, Rust Stable.

Commander bundles Core Next `hemp0xd`, `hemp0x-cli`, and `hemp0x-tx` binaries as Tauri sidecars. Stage them from the Core Next 4.8.0.0 release artifacts before building. The staging script verifies SHA256 checksums and places the binaries into `src-tauri/binaries/` with the required target-triple suffixes.

```bash
npm install
npm run stage:core-next
npm run tauri build
```

See [`docs/RELEASE_BUILDING.md`](docs/RELEASE_BUILDING.md) for the full repeatable build flow (Linux AppImage and Windows portable EXE).

---

<div align="center">
  <p>Powered by the <b>Hemp0x Blockchain</b>.</p>
  <a href="https://hemp0x.com">hemp0x.com</a>
</div>
