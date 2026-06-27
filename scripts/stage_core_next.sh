#!/usr/bin/env bash
set -euo pipefail

# Stage Core Next binaries from release artifacts into the Commander build resource directory.
#
# Usage:
#   ./scripts/stage_core_next.sh
#
# Environment overrides:
#   CORE_NEXT_ARTIFACT_DIR  Path to the directory containing the release archives and SHA256SUMS.
#                           Default: /home/bcr/projects/hemp0x-core-next/untracked/release-artifacts/final-core-next-v4.8.0.0-messageindex/
#   STAGING_DIR             Destination directory for staged binaries.
#                           Default: <repo root>/src-tauri/binaries

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

DEFAULT_ARTIFACT_DIR="/home/bcr/projects/hemp0x-core-next/untracked/release-artifacts/final-core-next-v4.8.0.0-messageindex"
ARTIFACT_DIR="${CORE_NEXT_ARTIFACT_DIR:-$DEFAULT_ARTIFACT_DIR}"
STAGING_DIR="${STAGING_DIR:-$REPO_ROOT/src-tauri/binaries}"

LINUX_ARCHIVE="hemp0x-core-next-v4.8.0.0-linux-x86_64.tar.gz"
WINDOWS_ARCHIVE="hemp0x-core-next-v4.8.0.0-win64.zip"
SUMS_FILE="SHA256SUMS"

LINUX_TRIPLE="x86_64-unknown-linux-gnu"
# Tauri names Windows externalBin sidecars with Commander's target triple.
# Core Next currently ships static MinGW-w64 executables, which are staged under
# this suffix because Commander is built as x86_64-pc-windows-msvc.
WINDOWS_TRIPLE="x86_64-pc-windows-msvc"

BINARIES=("hemp0xd" "hemp0x-cli" "hemp0x-tx")

echo "=== Hemp0x Commander - Core Next Binary Staging ==="
echo " Artifact dir : $ARTIFACT_DIR"
echo " Staging dir  : $STAGING_DIR"
echo ""

# --- Validate artifact directory ---
if [[ ! -d "$ARTIFACT_DIR" ]]; then
  echo "ERROR: Artifact directory not found: $ARTIFACT_DIR"
  echo "       Set CORE_NEXT_ARTIFACT_DIR to override."
  exit 1
fi

# --- Verify SHA256SUMS ---
echo "[1/4] Verifying SHA256 checksums..."
cd "$ARTIFACT_DIR"

if [[ ! -f "$SUMS_FILE" ]]; then
  echo "ERROR: $SUMS_FILE not found in $ARTIFACT_DIR"
  exit 1
fi

if ! sha256sum --check "$SUMS_FILE" --status; then
  echo "ERROR: SHA256 verification failed for Core Next release artifacts."
  echo "Checksum file contents:"
  cat "$SUMS_FILE"
  exit 1
fi

echo "  Checksum verification complete."

# --- Create temp extraction dir ---
WORK_DIR=$(mktemp -d /tmp/commander-stage-core-next-XXXXXX)
trap 'rm -rf "$WORK_DIR"' EXIT

LINUX_WORK="$WORK_DIR/linux"
WINDOWS_WORK="$WORK_DIR/windows"
mkdir -p "$LINUX_WORK" "$WINDOWS_WORK" "$STAGING_DIR"

# --- Extract Linux binaries ---
echo "[2/4] Extracting Linux binaries from $LINUX_ARCHIVE..."
if [[ ! -f "$LINUX_ARCHIVE" ]]; then
  echo "ERROR: Linux archive not found: $LINUX_ARCHIVE"
  exit 1
fi

tar xzf "$LINUX_ARCHIVE" -C "$LINUX_WORK"

# --- Extract Windows binaries ---
echo "[3/4] Extracting Windows binaries from $WINDOWS_ARCHIVE..."
if [[ ! -f "$WINDOWS_ARCHIVE" ]]; then
  echo "ERROR: Windows archive not found: $WINDOWS_ARCHIVE"
  exit 1
fi

unzip -qo "$WINDOWS_ARCHIVE" -d "$WINDOWS_WORK"

# --- Stage binaries ---
echo "[4/4] Staging binaries to $STAGING_DIR..."
staged_count=0

for bin in "${BINARIES[@]}"; do
  # Linux (archives may nest binaries under a top-level directory)
  linux_src="$(find "$LINUX_WORK" -type f -name "$bin" | head -n 1)"
  linux_dst="$STAGING_DIR/${bin}-${LINUX_TRIPLE}"
  if [[ -f "$linux_src" ]]; then
    cp "$linux_src" "$linux_dst"
    chmod +x "$linux_dst"
    echo "  + $linux_dst"
    staged_count=$((staged_count + 1))
  else
    echo "ERROR: Linux binary not found in archive: $bin"
    exit 1
  fi

  # Windows
  win_src="$(find "$WINDOWS_WORK" -type f -name "${bin}.exe" | head -n 1)"
  win_dst="$STAGING_DIR/${bin}-${WINDOWS_TRIPLE}.exe"
  if [[ -f "$win_src" ]]; then
    cp "$win_src" "$win_dst"
    echo "  + $win_dst"
    staged_count=$((staged_count + 1))
  else
    echo "ERROR: Windows binary not found in archive: ${bin}.exe"
    exit 1
  fi
done

echo ""
echo "=== Staging complete: $staged_count files staged ==="

# --- Determine if staging dir is tracked by git ---
cd "$REPO_ROOT"
if git ls-files -- "$STAGING_DIR" | grep -q .; then
  echo "NOTE: $STAGING_DIR is tracked by git."
  echo "      The .gitignore at the repo root should exclude these binaries."
  echo "      If they are currently tracked, consider 'git rm --cached' on the directory."
else
  echo "NOTE: $STAGING_DIR is NOT tracked by git (expected)."
fi
