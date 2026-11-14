#!/usr/bin/env bash
# Lay out the Go SDK's windows-x64 runtime bundle in $BUNDLE_DIR from what the
# kusa-windows workflow built. Run from the repository root (Git Bash).
#
#   smol_go.dll, smol-vmm.exe, krun.dll  $CARGO_TARGET_DIR/release
#   libkrunfw.dll                        lib/windows-x86_64 (upstream binary)
#   agent-rootfs.tar, *-template.ext4.zst  guest/ (the guest job's artifact)
#
# The SDK finds the boot helper as smol-vmm.exe beside smol_go.dll; it is the
# smolvm binary renamed, as in the upstream npm and zip distributions.
set -euo pipefail

: "${BUNDLE_DIR:?}"
REL="${CARGO_TARGET_DIR:-target}/release"
GUEST="${GUEST_DIR:-guest}"

rm -rf "$BUNDLE_DIR"
mkdir -p "$BUNDLE_DIR"
install -m 0755 "$REL/smol_go.dll" "$BUNDLE_DIR/smol_go.dll"
install -m 0755 "$REL/smolvm.exe" "$BUNDLE_DIR/smol-vmm.exe"
install -m 0755 "$REL/krun.dll" "$BUNDLE_DIR/krun.dll"
install -m 0755 lib/windows-x86_64/libkrunfw.dll "$BUNDLE_DIR/libkrunfw.dll"
install -m 0644 "$GUEST/agent-rootfs.tar" "$BUNDLE_DIR/agent-rootfs.tar"
install -m 0644 "$GUEST/storage-template.ext4.zst" "$GUEST/overlay-template.ext4.zst" "$BUNDLE_DIR/"

# What went in, for the bundle's own record and the workflow summary.
{
  echo "smolvm=$(git rev-parse HEAD)"
  echo "libkrun=$(git -C libkrun rev-parse HEAD)"
  echo "libkrunfw=$(sed -n 's/^libkrunfw=//p' lib/windows-x86_64/libkrun.provenance)"
  echo "smol=$(git -C smol rev-parse HEAD)"
} > "$BUNDLE_DIR/PROVENANCE"
cat "$BUNDLE_DIR/PROVENANCE"
(cd "$BUNDLE_DIR" && sha256sum -- *) | tee -a "${GITHUB_STEP_SUMMARY:-/dev/null}"
