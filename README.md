# Hemp0x Commander

<div align="center">
  <a href="https://hemp0x.com">
    <img src="src/assets/hemp0xgit.png" alt="Hemp0x Logo" width="160" />
  </a>

  <h3>Interface for the Hemp0x Blockchain</h3>

  <p>
    A secure, non-custodial, and cross-platform dashboard for managing your node and assets.
  </p>

  <br />

  [![Website](https://img.shields.io/badge/WEBSITE-hemp0x.com-000000?style=for-the-badge&logo=globe&logoColor=00ff00&labelColor=000000)](https://hemp0x.com)
  [![Download](https://img.shields.io/badge/DOWNLOAD-v2.0.0-000000?style=for-the-badge&logo=windows&logoColor=00ff00&labelColor=000000)](https://github.com/beyondcr/hemp0x-commander/releases)
  [![Discord](https://img.shields.io/badge/DISCORD-JOIN_US-000000?style=for-the-badge&logo=discord&logoColor=00ff00&labelColor=000000)](https://discord.gg/FMEKJUwcsu)
  [![Build Guides](https://img.shields.io/badge/DOCS-BUILD_GUIDES-000000?style=for-the-badge&logo=rust&logoColor=00ff00&labelColor=000000)](docs/BUILDING.md)
</div>

<br />

![Dashboard Preview](screenshots/dashboard.png)

---

## ⚡ Overview

**Hemp0x Commander** brings the power of the Hemp0x blockchain to your desktop. Built with **Tauri v2** (Rust) and **Svelte 5**, it combines the security of a local full node with the ease of use of a modern web application.

> [!WARNING]
> ### ⚠️ STATE: ACTIVE DEVELOPMENT
> **This software is currently in Beta (v2.0). While functional, it is evolving.**
>
> *   🐛 **Expect bugs.**
> *   🎨 **Expect UI changes.**
> *   � **Use at your own risk.** We are not responsible for lost funds or data.
> *   💾 **Always backup your `wallet.dat`.**

### ✨ Key Features

| Feature | Description | Status |
| :--- | :--- | :--- |
| **🪙 Asset Management** | Issue, reissue, and transfer unique Hemp0x assets directly from the UI. Visualize your portfolio with rich metadata. | 🚧 **In Testing** |
| **🛡️ Privacy First** | Connects to your own local `hemp0xd` node via authenticated RPC. No third-party servers track your transactions. | ✅ **Stable** |
| **🔧 Coin Control** | (Advanced) Manually select which UTXOs to spend. Optimize privacy or reduce transaction fees. | ✅ **Stable** |
| **🖥️ Node Control** | Start, stop, and monitor your blockchain daemon seamlessly. View sync progress, peer count, and network difficulty. | ✅ **Stable** |
| **🔒 Encryption** | Secures your wallet with AES-256 encryption. Unlock only when necessary to sign transactions. | ✅ **Stable** |

---

## 📸 Gallery

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

## 📦 Installation

**Windows** & **Linux** supported.

### Windows
1.  Navigate to the **[Releases Page](https://github.com/beyondcr/hemp0x-commander/releases)**.
2.  Download the **Portable Version** (`.zip`).
3.  Extract and launch `Hemp0x Commander.exe`.

### Linux
1.  Download the **AppImage** from **[Releases](https://github.com/beyondcr/hemp0x-commander/releases)**.
2.  `chmod +x Hemp0x_Commander_*.AppImage`
3.  ./Launch

---

## 💀 Contributing & Bugs

This application is merely a **Visual Shell** interacting with the core `hemp0xd` binaries. If the shell breaks, your coins are safe in the daemon.

**Found a glitch? Want a feature?**
We value your feedback.
*   **Report it** on the [Hemp0x Discord](https://discord.gg/FMEKJUwcsu).
*   **Fix it** and submit a PR.

*Together we build the future of Hemp0x.*

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
