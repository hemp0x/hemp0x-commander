# Release Building - Hemp0x Commander 2.0.0

This document covers repeatable local build steps for Hemp0x Commander 2.0.0.

## Prerequisites

- Node.js 18+
- Rust stable (via rustup, 1.77.2+)
- Git
- Core Next release artifacts (acquired separately from the Hemp0x Core Next repository)
- Linux host for AppImage builds; Windows host for portable EXE builds

## 1. Stage Core Next Binaries

Commander bundles Core Next daemon, CLI, and tx binaries. Before building, stage them from the Core Next release artifacts directory.

```bash
npm run stage:core-next
```

Override the artifact source directory if needed:

```bash
CORE_NEXT_ARTIFACT_DIR=/path/to/artifacts npm run stage:core-next
```

This script:
- Verifies SHA256 checksums against `SHA256SUMS`
- Extracts `hemp0xd`, `hemp0x-cli`, `hemp0x-tx` for both Linux and Windows
- Places them into `src-tauri/binaries/` with target-triple suffixes

The Core Next release archives are expected to have a flat top-level layout with
the binaries at the archive root.

Staged binary files are gitignored. They are not intended to be committed to the repository.

## 2. Build

### Linux AppImage

```bash
npm install
npm run tauri build -- -b appimage
```

The `-b appimage` flag builds only the AppImage bundle (skipping deb/rpm).

Output: `src-tauri/target/release/bundle/appimage/Hemp0x Commander_2.0.0_amd64.AppImage`

The AppImage bundles the Core Next binaries as Tauri `externalBin` resources. At runtime, Commander resolves them relative to the AppImage extraction path.

**Known issue**: On newer Linux distributions (glibc >= 2.39), the linuxdeploy bundling step may fail during the library `strip` phase due to `.relr.dyn` section support in system libraries that the bundled linuxdeploy `strip` doesn't recognize. Workaround: manually run linuxdeploy with a newer version, or build inside a Docker container with an older glibc.

### Windows Portable EXE

On Windows, with the binaries staged:

```powershell
npm install
npm run tauri build -- --no-bundle
```

The `--no-bundle` flag skips installer generation (no NSIS/MSI), producing only the raw portable EXE and sidecar binaries at `src-tauri/target/release/`.

For distribution, zip the following files together:
- `hemp0x-commander.exe`
- `hemp0xd-x86_64-pc-windows-msvc.exe`
- `hemp0x-cli-x86_64-pc-windows-msvc.exe`
- `hemp0x-tx-x86_64-pc-windows-msvc.exe`

Do not publish NSIS/MSI installer artifacts for Commander 2.0.0 unless the release
scope changes.

## 3. Runtime Binary Resolution

At runtime, Commander resolves Core Next binaries (`hemp0xd`, `hemp0x-cli`, `hemp0x-tx`) using the `resolve_bin` function in `src-tauri/src/modules/utils.rs`. The resolution order is:

1. Adjacent to the running executable (extracted AppImage / portable EXE directory)
2. `resources/` subdirectory adjacent to the executable
3. Current working directory and parent walk (development mode fallback)
4. `CARGO_MANIFEST_DIR` parent walk (development mode fallback)
5. `~/hemp0x-deploy/hemp0x-core/src/` (developer override, Linux only)

In bundled AppImage builds, `tauri.conf.json`'s `externalBin` entries cause Tauri to place the binaries into the AppImage's `usr/bin/` or extraction root, which is adjacent to the executable at runtime.

In Windows portable builds, `externalBin` places `.exe` files adjacent to the main executable.

## 4. Extract Binaries Feature

Commander's **Extract Binaries** button (SYSTEM tab) copies the resolved `hemp0xd`, `hemp0x-cli`, and `hemp0x-tx` binaries from the bundled location to a user-chosen directory. This is useful for advanced users who want to run the daemon outside Commander while still using the bundled Core Next build.

## 5. Intentionally NOT Done for 2.0.0

- No auto-updater mechanism
- No installer (NSIS/MSI on Windows, deb/rpm on Linux)
- No release signing or notarization
- No remote checksum verification service
- No bundled Core Next data directory (users maintain their own `~/.hemp0x`)
