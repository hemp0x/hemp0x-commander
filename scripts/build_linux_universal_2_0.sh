#!/bin/bash
# =============================================================================
# Hemp0x Commander 2.0.0 - Universal Linux AppImage builder (release eng.)
# =============================================================================
# Release-engineering only. Does NOT modify application source code.
# Builds Commander inside an Ubuntu 22.04 (glibc 2.35) container for broad
# Linux compatibility, then repacks the payload with zstd-22 and the known-good
# legacy 1.2 AppImage runtime header so the artifact runs without libfuse2.
#
# Replaces the 1.3-era scripts/build_linux_universal.sh assumptions:
#   - version 2.0.0
#   - all three Linux sidecars (hemp0xd, hemp0x-cli, hemp0x-tx) staged via
#     ./scripts/stage_core_next.sh before running this script
#   - NO `cargo update` (preserves the committed Cargo.lock)
#
# Prerequisites (verified by the preflight block below):
#   - podman (preferred) or docker
#   - local image: hemp0x-builder-2204 (Ubuntu 22.04 + Rust + Node + webkit2gtk-4.1)
#   - mksquashfs on host
#   - a known-good legacy Commander 1.2 universal AppImage (LEGACY_APPIMAGE_PATH)
#   - Core Next sidecars already staged in src-tauri/binaries/
# =============================================================================
set -euo pipefail

PROJECT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_DIR"

VERSION="2.0.0"
APP_NAME="Hemp0x_Commander"
ENGINE="${ENGINE:-podman}"
if ! command -v "$ENGINE" >/dev/null 2>&1; then
  ENGINE="docker"
fi
DOCKER_IMAGE_NAME="${DOCKER_IMAGE_NAME:-hemp0x-builder-2204}"
LEGACY_APPIMAGE_PATH="${LEGACY_APPIMAGE_PATH:-$PROJECT_DIR/Hemp0x_Commander_1.2.0_Universal_Fixed.AppImage}"
OUTPUT_DIR="$PROJECT_DIR/untracked/release-candidates"
INTERMEDIATE_APPIMAGE="$OUTPUT_DIR/_internal_universal_intermediate.AppImage"
FINAL_APPIMAGE="$OUTPUT_DIR/${APP_NAME}_${VERSION}_Universal_Linux.AppImage"
FINAL_SHA256="$FINAL_APPIMAGE.sha256"

mkdir -p "$OUTPUT_DIR"

# ---------------------------------------------------------------------------
# Preflight
# ---------------------------------------------------------------------------
echo "== Universal AppImage build: preflight =="
command -v mksquashfs >/dev/null 2>&1 || { echo "BLOCKER: mksquashfs not found on host"; exit 2; }
command -v "$ENGINE" >/dev/null 2>&1 || { echo "BLOCKER: neither podman nor docker found"; exit 2; }
"$ENGINE" image exists "$DOCKER_IMAGE_NAME" 2>/dev/null || {
  echo "BLOCKER: container image '$DOCKER_IMAGE_NAME' not found. Build or pull it first."
  echo "  hint: $ENGINE pull $DOCKER_IMAGE_NAME  (or build the 2204 builder image)"
  exit 2
}
[ -f "$LEGACY_APPIMAGE_PATH" ] || { echo "BLOCKER: legacy 1.2 AppImage not found at $LEGACY_APPIMAGE_PATH"; exit 2; }
for b in hemp0xd-x86_64-unknown-linux-gnu hemp0x-cli-x86_64-unknown-linux-gnu hemp0x-tx-x86_64-unknown-linux-gnu; do
  [ -f "$PROJECT_DIR/src-tauri/binaries/$b" ] || { echo "BLOCKER: sidecar $b not staged. Run ./scripts/stage_core_next.sh first."; exit 2; }
done
echo "  engine              : $ENGINE"
echo "  image               : $DOCKER_IMAGE_NAME"
echo "  legacy runtime src  : $LEGACY_APPIMAGE_PATH"
echo "  output              : $FINAL_APPIMAGE"

# ---------------------------------------------------------------------------
# Tree-cleanliness guard: snapshot tracked lockfiles so we can restore them
# if the container build mutates anything. No source edits are intended.
# ---------------------------------------------------------------------------
LOCKF_CARGO="$PROJECT_DIR/src-tauri/Cargo.lock"
LOCKF_NPM="$PROJECT_DIR/package-lock.json"
hash_before_cargo="$(sha256sum "$LOCKF_CARGO" 2>/dev/null | awk '{print $1}')"
hash_before_npm="$(sha256sum "$LOCKF_NPM" 2>/dev/null | awk '{print $1}')"
restore_lockfiles() {
  local changed=0
  local h
  h="$(sha256sum "$LOCKF_CARGO" 2>/dev/null | awk '{print $1}')"
  if [ "$h" != "$hash_before_cargo" ]; then
    git checkout -- "$LOCKF_CARGO" 2>/dev/null || true
    changed=1
    echo "  [guard] restored src-tauri/Cargo.lock"
  fi
  h="$(sha256sum "$LOCKF_NPM" 2>/dev/null | awk '{print $1}')"
  if [ "$h" != "$hash_before_npm" ]; then
    git checkout -- "$LOCKF_NPM" 2>/dev/null || true
    changed=1
    echo "  [guard] restored package-lock.json"
  fi
  return $changed
}
trap 'restore_lockfiles || true' EXIT

# ---------------------------------------------------------------------------
# Inner container build script. Written to a temp path inside untracked/ (not
# tracked). Runs npm ci (respects lockfile, never modifies it) and tauri
# build, then prunes/strips the AppDir and produces an intermediate AppImage
# via the image's own /build_tools/appimagetool.
# ---------------------------------------------------------------------------
INNER="$OUTPUT_DIR/_universal_inner_build.sh"
cat > "$INNER" <<'INNEREOF'
#!/bin/bash
set -euo pipefail
shopt -s nullglob
export APPIMAGE_EXTRACT_AND_RUN=1
cd /app
source "$HOME/.cargo/env" || true

echo "[inner] npm ci"
npm ci

echo "[inner] tauri build (appimage, no-default-features)"
./node_modules/.bin/tauri build -b appimage -- --no-default-features || {
  echo "[inner] tauri appimage bundling returned non-zero; will locate AppDir manually"
}

APPDIR="$(find /app/src-tauri/target/release/bundle/appimage -maxdepth 1 -name "*.AppDir" 2>/dev/null | head -n1)"
if [ -z "$APPDIR" ]; then
  echo "BLOCKER: no AppDir produced at src-tauri/target/release/bundle/appimage/"
  ls -F /app/src-tauri/target/release/bundle/appimage/ 2>/dev/null || true
  exit 1
fi
echo "[inner] APPDIR=$APPDIR"
ls -la "$APPDIR/usr/bin/"

# Prune bloat
echo "[inner] pruning bloat"
rm -rf "$APPDIR/usr/lib/libLLVM"*
rm -rf "$APPDIR/usr/lib/dri" "$APPDIR/usr/lib_bak"
rm -rf "$APPDIR/usr/share/doc" "$APPDIR/usr/share/man" "$APPDIR/usr/share/gir"* "$APPDIR/usr/lib/girepository-1.0"
if [ -d "$APPDIR/usr/lib/lib" ]; then rm -rf "$APPDIR/usr/lib/lib"; fi

# Strip. ONLY strip shared libraries and the Commander main binary.
# The bundled Core Next sidecars (hemp0xd, hemp0x-cli, hemp0x-tx) and other
# /usr/bin binaries ship PRE-STRIPPED from the Core build; re-stripping them
# with the container's older binutils corrupts them (hemp0x-tx segfaults),
# so they MUST be left untouched.
echo "[inner] stripping shared libs + Commander main binary only"
find "$APPDIR/usr/lib" -type f -name "*.so*" -exec strip --strip-unneeded {} \; 2>/dev/null || true
strip --strip-unneeded "$APPDIR/usr/bin/hemp0x-commander" 2>/dev/null || true

# Intermediate AppImage via the image's bundled appimagetool
command -v file >/dev/null 2>&1 || (apt-get update && apt-get install -y file)
ARCH=x86_64 /build_tools/appimagetool-x86_64.AppImage --no-appstream "$APPDIR" /app/_internal.AppImage
mkdir -p /app/untracked/release-candidates
mv /app/_internal.AppImage /app/untracked/release-candidates/_internal_universal_intermediate.AppImage
echo "[inner] intermediate AppImage written"
INNEREOF
chmod +x "$INNER"

echo "== Running container build ($ENGINE $DOCKER_IMAGE_NAME) =="
SELINUX_FLAG=""
[ "$ENGINE" = "podman" ] && SELINUX_FLAG=":Z"
"$ENGINE" run --rm \
  -v "$PROJECT_DIR:/app$SELINUX_FLAG" \
  -w /app \
  "$DOCKER_IMAGE_NAME" \
  /bin/bash /app/untracked/release-candidates/_universal_inner_build.sh

[ -f "$INTERMEDIATE_APPIMAGE" ] || { echo "BLOCKER: intermediate AppImage not produced"; exit 1; }

# ---------------------------------------------------------------------------
# Host: repack with zstd-22 and fuse with the legacy 1.2 runtime header.
# ---------------------------------------------------------------------------

WORKDIR="$OUTPUT_DIR/_repack_work"
rm -rf "$WORKDIR"
mkdir -p "$WORKDIR"

echo "== Extracting intermediate payload (host has no libfuse2; using --appimage-extract) =="
chmod +x "$INTERMEDIATE_APPIMAGE"
export APPIMAGE_EXTRACT_AND_RUN=1
( cd "$WORKDIR" && "$INTERMEDIATE_APPIMAGE" --appimage-extract >/tmp/hemp0x-universal-extract.log 2>&1 )
[ -d "$WORKDIR/squashfs-root" ] || { echo "BLOCKER: extraction did not produce squashfs-root"; tail -40 /tmp/hemp0x-universal-extract.log; exit 1; }

echo "== Repacking payload (zstd-22) =="
mksquashfs "$WORKDIR/squashfs-root" "$WORKDIR/payload.sqfs" \
  -root-owned -noappend -comp zstd -Xcompression-level 22 -quiet

echo "== Fusing legacy 1.2 runtime header + payload =="
chmod +x "$LEGACY_APPIMAGE_PATH"
OFFSET="$("$LEGACY_APPIMAGE_PATH" --appimage-offset)"
dd if="$LEGACY_APPIMAGE_PATH" of="$WORKDIR/runtime_header.bin" bs=1 count="$OFFSET" status=none
cat "$WORKDIR/runtime_header.bin" "$WORKDIR/payload.sqfs" > "$FINAL_APPIMAGE"
chmod +x "$FINAL_APPIMAGE"

echo "== Checksum =="
( cd "$OUTPUT_DIR" && sha256sum "$(basename "$FINAL_APPIMAGE")" > "$(basename "$FINAL_SHA256")" )

# Cleanup intermediates (keep final AppImage + sha256)
rm -rf "$WORKDIR" "$INTERMEDIATE_APPIMAGE" "$INNER"

echo "== Done =="
ls -lh "$FINAL_APPIMAGE"
cat "$FINAL_SHA256"

# Final tree-cleanliness verification
restore_lockfiles || true