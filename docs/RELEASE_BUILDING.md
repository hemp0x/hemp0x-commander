# Release Building - Hemp0x Commander 2.0.0

This document covers repeatable local build steps for Hemp0x Commander 2.0.0.

## Prerequisites

- Node.js 18+
- Rust stable (via rustup, 1.77.2+)
- Git
- Core Next release artifacts (acquired separately from the Hemp0x Core Next repository)
- Linux host for AppImage builds
- Linux host with the staged MSVC cross-build toolchain for Windows portable
  prerelease builds, or a Windows host for native portable EXE builds

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

#### Standard Tauri AppImage

```bash
npm install
npm run tauri build -- -b appimage
```

The `-b appimage` flag builds only the AppImage bundle (skipping deb/rpm).

Output: `src-tauri/target/release/bundle/appimage/Hemp0x Commander_2.0.0_amd64.AppImage`

The AppImage bundles the Core Next binaries as Tauri `externalBin` resources. At runtime, Commander resolves them relative to the AppImage extraction path.

**Known issue**: On newer Linux distributions (glibc >= 2.39), the linuxdeploy bundling step may fail during the library `strip` phase due to `.relr.dyn` section support in system libraries that the bundled linuxdeploy `strip` doesn't recognize. Workaround: manually run linuxdeploy with a newer version, or build inside a Docker container with an older glibc.

#### Universal AppImage Path

Commander 1.2 and 1.3 used a smaller universal AppImage path for broad Linux
compatibility. Keep this path available for the 2.0 release if the standard
Tauri AppImage is too large or fails on older distributions.

The working model is:

1. Build the app in a controlled Ubuntu container rather than on a rolling
   distro host.
2. Let Tauri assemble an AppDir with the Linux Core Next sidecars in
   `usr/bin`.
3. Remove duplicate or unneeded payload from the AppDir, such as docs,
   bundled driver directories, `libLLVM*`, nested duplicate `lib/` trees, and
   generated introspection data.
4. Strip ELF binaries and shared libraries with `strip --strip-unneeded`.
5. Repack the payload with `mksquashfs` using zstd compression level 22.
6. Reuse the known-good older AppImage runtime header from the 1.2 universal
   AppImage so the final artifact works on systems without `libfuse2`.

For Commander 2.0.0, use `scripts/build_linux_universal_2_0.sh`. It stages
against the current `src-tauri/binaries/` layout, includes all three Linux Core
Next sidecars (`hemp0xd`, `hemp0x-cli`, and `hemp0x-tx`), preserves committed
lockfiles, and avoids re-stripping the Core sidecars.

Before running it:

- Run `npm run stage:core-next`.
- Confirm `LEGACY_APPIMAGE_PATH` points to the known-good 1.2 universal
  AppImage runtime source.
- Confirm the `hemp0x-builder-2204` container image exists.
- Test the output on at least one modern distro and one older LTS distro.

The older `scripts/build_linux_universal.sh` remains as the 1.3-era reference
script only.

Expected output shape:

```text
release/Hemp0x_Commander_2.0.0_Universal_Linux.AppImage
```

This universal path is release engineering, not app logic. Do not replace it
with packers, binary shielding, or installer wrappers to reduce size.

### Windows Portable EXE

#### Native Windows Build

On Windows, with the binaries staged:

```powershell
npm install
npm run tauri build -- --no-bundle
```

The `--no-bundle` flag skips installer generation (no NSIS/MSI), producing only the raw portable EXE and sidecar binaries at `src-tauri/target/release/`.

For distribution, zip the following files together:
- `hemp0x-commander.exe`
- `hemp0xd.exe`
- `hemp0x-cli.exe`
- `hemp0x-tx.exe`

Do not publish NSIS/MSI installer artifacts for Commander 2.0.0 unless the release
scope changes.

#### Linux-to-Windows MSVC Cross-Build

The prerelease Windows portable build can also be produced from the Linux
workspace using the staged `clang`/`xwin` toolchain under `untracked/`. This
keeps the source tree in one place while producing MSVC-ABI Windows binaries.

Preconditions:

- `src-tauri/binaries/` contains the Windows Core Next sidecars with the
  Tauri target suffix:
  - `hemp0xd-x86_64-pc-windows-msvc.exe`
  - `hemp0x-cli-x86_64-pc-windows-msvc.exe`
  - `hemp0x-tx-x86_64-pc-windows-msvc.exe`
  These sidecars may be static MinGW-w64 Core builds. Commander does not link
  against them; it launches them as external executables.
- `untracked/bin/cargo-xwin-runner` exists.
- `untracked/toolchains/clang+llvm-18.1.8-x86_64-linux-gnu-ubuntu-18.04`
  exists.

Build command:

```bash
set -euo pipefail
export LLVM_DIR="$PWD/untracked/toolchains/clang+llvm-18.1.8-x86_64-linux-gnu-ubuntu-18.04"
export PATH="$LLVM_DIR/bin:$PATH"
export LIBCLANG_PATH="$LLVM_DIR/lib"
export LD_LIBRARY_PATH="$PWD/untracked/toolchains/compat-libs:${LD_LIBRARY_PATH:-}"

npm run tauri build -- \
  --runner "$PWD/untracked/bin/cargo-xwin-runner" \
  --target x86_64-pc-windows-msvc \
  --no-bundle \
  -- --no-default-features
```

Output:

```text
src-tauri/target/x86_64-pc-windows-msvc/release/hemp0x-commander.exe
```

Package the portable prerelease as a zip with clean filenames:

- `hemp0x-commander.exe`
- `hemp0xd.exe`
- `hemp0x-cli.exe`
- `hemp0x-tx.exe`
- `README-PRERELEASE.txt`

Also publish a `.zip.sha256` file. The prerelease README should include the
build commit, build timestamp, bundled Core Next version, SHA256 verification
steps, the Microsoft WebView2 runtime requirement, and a clear note that
unsigned prerelease builds can trigger SmartScreen or antivirus false
positives.

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

## 6. Release Trust and Antivirus Notes

Commander is a Tauri 2 / WebView2 application. On Windows it launches the
system `msedgewebview2.exe` runtime and writes its cache, Crashpad, and
BrowserMetrics data under
`%LOCALAPPDATA%\io.hemp0x.commander\EBWebView\`. The following are expected,
normal WebView2/Edge runtime behavior and are not Commander network
activity: Edge/WebView2 child processes, `EdgeUpdate`, EBWebView cache and
Crashpad files, Edge registry reads, the `ChromeProcessSingletonStartup`
and `OneSettingQueryMutex` mutexes, and Edge runtime background DNS/TLS
such as `edge.microsoft.com`, DigiCert OCSP/CRL (`*.akahost.net`), and the
`easyauth.edgebrowser.microsoft-*-falcon.io` endpoints. Commander's own
outbound network is restricted to local Core RPC, local Tauri IPC, and the
user-configured IPFS gateways / pinning providers; no other external
domains are contacted by the app, and the frontend CSP forbids direct
outbound fetch.

Windows builds pass conservative WebView2 startup flags in
`src-tauri/tauri.conf.json` to reduce runtime background networking and
component-update noise while keeping GPU acceleration enabled for UI
responsiveness. These flags do not replace code signing and cannot guarantee
that a system WebView2 runtime will never touch Microsoft-owned services, but
they keep Commander's own webview surface focused on the local app.

Static strings that appear in `hemp0x-commander.exe` (such as
`html4/loose.dtd`, `digicert.com`, and `docs.rs`) come from reputable
Rust crate data embedded in the binary: the Brotli static dictionary, the
`rustls`/`webpki-roots` CA root metadata, and `rustls`/`getrandom` error
doc URLs. They are not executable callbacks or network targets.

For the final public release:

- Publish SHA256 checksums for every distributed archive alongside the
  download (the release-candidate workflow already produces `*.zip.sha256`
  files).
- Code-sign the Windows portable EXE and any installer artifacts with an
  EV or OV certificate if at all possible; this is the most effective
  false-positive reducer for SmartScreen and ML-based antivirus engines.
- Keep release metadata (`Cargo.toml` and `tauri.conf.json` `bundle`
  block) populated and consistent so the binary is not clustered with
  unsigned, unconfigured Tauri/Electron templates.
- If antivirus detections remain after signing, submit false-positive
  reports to the detecting vendors (e.g. Trapmine, Acronis) with the
  signed binary, checksum, and a link to the public source repository.
- Do not add packers, obfuscation, anti-debugging, anti-analysis, or
  binary shielding to reduce detections; these reliably increase
  detections and are explicitly avoided.
