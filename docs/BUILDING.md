# Build Guide for Hemp0x Commander

> **Note:** This is a legacy v1.x cross-compilation guide. For current Hemp0x Commander 2.0.0 builds, use the staging-based flow in [`RELEASE_BUILDING.md`](RELEASE_BUILDING.md): run `npm run stage:core-next` to stage Core Next 4.8.0.0 binaries into `src-tauri/binaries/` (with target-triple suffixes), then `npm run tauri build`. Do not place binaries directly into `src-tauri/`. The manual placement steps below are retained as historical reference only.

This generic guide outlines the process for building the Hemp0x Commander application from source. It covers setting up the unique cross-compilation environment required to produce stable binaries for both Windows and Linux platforms.

## Prerequisites

Ensure the following tools are installed on your workstation:

### Generic Requirements
- Node.js (Version 18 or higher)
- npm (Node Package Manager)
- Rust (Stable toolchain via rustup)
- Git

### Core Binaries (Critical)
Since binaries are excluded from Git, stage them before building using the Core Next release artifacts and the staging script, which verifies SHA256 checksums and writes them to `src-tauri/binaries/` with the required target-triple suffixes:

```bash
npm run stage:core-next
```

Override the artifact source directory with `CORE_NEXT_ARTIFACT_DIR=/path/to/artifacts npm run stage:core-next` if needed. See [`RELEASE_BUILDING.md`](RELEASE_BUILDING.md) for details.

### Linux Build Host Requirements
To produce widely compatible binaries, we recommend using a Linux environment (specifically Nobara or Fedora) for the compilation process. This host performs two critical functions:
1. Cross-compiling the Windows binaries using MinGW.
2. Repacking the Linux AppImage using an older runtime to ensure compatibility with legacy GLIBC versions (e.g., Ubuntu).

The Linux host must have the following directories and tools available:
- Source Code: Clone of the `hemp0x-commander` repository.
- **Base AppImage**: A working AppImage skeleton.
  - *Strategy:* The "Base" is created once using an **Ubuntu 20.04 Docker container** (to ensure legacy GLIBC compatibility).
  - *Workflow:* Generate the Base on Docker -> Move to Nobara -> Use `build_linux_appimage.sh` to inject new binaries (Repack).
- **AppImageTool**: The `appimagetool-x86_64.AppImage` executable (required for repacking).

---

## 1. Building Windows Binaries (Cross-Compilation)

We do not build the Windows binaries directly on Windows because the Linux-based cross-compilation pipeline provides more deterministic results and easier dependency management for the C++ `hemp0xd` core.

### Procedure

1.  **Sync Source Code**
    Transfer your latest local source code to your Linux build host.
    
    ```bash
    scp -r src/ user@build-host:~/projects/hemp0x-commander/src/
    ```

2.  **Execute Build Script**
    SSH into the Linux host and run the provided helper script. This script handles cloning the core, patching dependencies, and compiling for `x86_64-w64-mingw32`.

    ```bash
    ssh user@build-host "~/projects/hemp0x-commander/scripts/build_windows_binaries_on_linux.sh"
    ```

3.  **Retrieve Artifacts**
    Once compilation completes, download the resulting zip archive to your local machine.

    ```bash
    scp user@build-host:~/hemp0x-win-build/hemp0x-win64.zip .
    ```

4.  **Install Binaries**
    Extract `hemp0xd.exe` and `hemp0x-cli.exe` from the zip file and place them into the `src-tauri/` directory of your project.

---

## 2. Building Linux AppImage (Cross-Distro Universal)

Building a Linux AppImage natively on a modern distribution (like Nobara or Arch) often creates binaries that fail on older systems (like Ubuntu 20.04) due to GLIBC version mismatches. To solve this, we use a "Cross-Distro Repack" method.

### The Repack Method (Recommended)
This method is the most reliable for updates. It allows you to build the new binaries on a modern OS (like Nobara) while keeping the compatible runtime environment of an older release.

**How it works (The "Binary Swap"):**
1.  **Extract** a known-good AppImage (the "Base").
2.  **Compile** the new `hemp0x-commander` binary and `hemp0xd` sidecars on your host.
3.  **Swap** the old binaries in the extracted folder with the new ones.
4.  **Repack** the folder into a new AppImage.

**Step-by-Step Instructions:**
1.  **Locate Base:** Ensure `Hemp0x Commander_1.1.0_x64-portable.appimage` is in your `Hemp_Commander_V1.1_Build` folder.
2.  **Compile New Code:**
    ```bash
    npm install && npm run tauri build
    ```
3.  **Run Repack Script:**
    ```bash
    ~/projects/hemp0x-commander/scripts/build_linux_appimage.sh
    ```
    *This script automatically handles the extraction, binary swapping, stripping, and repacking.*

### Option B: Full Docker Build (If Base is Lost)
If you lose the v1.1 Base AppImage, you must generate a new one using an **Ubuntu 20.04 Docker Container** to ensure the core libraries (GLIBC) are old enough to run everywhere. Once generated, use that as your new Base.

3.  **Retrieve Artifact**
    Download the final Universal AppImage.

    ```bash
    scp user@build-host:~/Hemp_Commander_V1.2_Build/Hemp0x_Commander_1.2.0_Universal.AppImage .
    ```

---

## 3. Building the Windows Installer

Once the correct `hemp0xd.exe` and `hemp0x-cli.exe` binaries are placed in `src-tauri/` (Step 1), you can build the final Windows installer directly on your Windows machine.

### Command

```powershell
npm run tauri build
```

### Outputs
The build process will generate:
- Installer: `src-tauri/target/release/bundle/nsis/Hemp0x Commander_x.x.x_x64-setup.exe`
- Portable Zip (if configured): `src-tauri/target/release/bundle/nsis/*.zip`

---

## Troubleshooting

### "execv error: No such file or directory" on Linux
This error occurs if you try to run a natively built AppImage on a system missing specific loaders.
**Solution:** Ensure you are using the `build_linux_appimage.sh` script (The Repack Method) rather than a raw native build.

### GLIBC Errors
If the application crashes immediately on Ubuntu with GLIBC errors, it means the binary was linked against a newer C library than the target system supports.
**Solution:** Rebuild using the Repack Method to inherit the older, compatible runtime from the Base AppImage.
