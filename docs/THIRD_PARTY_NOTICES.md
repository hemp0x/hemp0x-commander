# Third-Party Notices

Hemp0x Commander is released under the MIT License. That license applies to the Commander codebase in this repository, including the H0XC and H0XSHT message handling code, encoder and decoder logic, and tables included here.

Commander also depends on third-party software and may bundle external runtime binaries in release artifacts. Those components keep their own licenses.

## Hemp0x Core Next Binaries

Release builds can include Hemp0x Core Next sidecar binaries:

- `hemp0xd`
- `hemp0x-cli`
- `hemp0x-tx`

These binaries are part of the Hemp0x Core Next project and are not relicensed by the Commander MIT license. Hemp0x Core Next is derived from Ravencoin Core and Bitcoin Core lineage.

If you redistribute Commander builds with bundled Core Next binaries, keep the required Core Next license and notice files with the distributed package.

## JavaScript And Rust Dependencies

Commander uses npm and Cargo dependencies for the desktop app, user interface, packaging, and platform integration. Dependency metadata in this repo currently reports permissive licenses such as MIT, Apache-2.0, BSD, ISC, Zlib, CC0, Unicode, and MPL-2.0 for transitive packages.

If you ship a downstream build, keep dependency notices required by your package format and target platform.

## Platform Runtimes

Windows builds use Microsoft Edge WebView2 Runtime through the Tauri/Wry stack. WebView2 is supplied and licensed by Microsoft. It is not part of the Commander MIT license.

Linux builds use system libraries supplied by the target distribution and may package an AppImage runtime. Those components keep their own licenses.
