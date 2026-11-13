#!/usr/bin/env bash
# Pack $BUNDLE_DIR as the Go SDK expects to download it: a flat tar.gz named
# smol-go-runtime-<sdk version>-windows-x64.tar.gz (smol/sdk/go/scripts/
# bundle-runtime.sh does the packing and the no-symlinks check), plus its
# sha256 in sha256sum format. kusa-connect does not download from GitHub: the
# hash goes into the desktop tool manifest and the file onto our server.
set -euo pipefail

: "${BUNDLE_DIR:?}"
OUT="${OUT_DIR:-dist}"
mkdir -p "$OUT"

# bundle-runtime.sh copies <native-dir> and adds the library itself.
NATIVE="$(mktemp -d)"
cp -R "$BUNDLE_DIR"/. "$NATIVE"/
rm -f "$NATIVE/smol_go.dll"
smol/sdk/go/scripts/bundle-runtime.sh "$NATIVE" "$BUNDLE_DIR/smol_go.dll" windows-x64 "$OUT"

archive="$(ls "$OUT"/smol-go-runtime-*-windows-x64.tar.gz)"
(cd "$OUT" && sha256sum "$(basename "$archive")" > "$(basename "$archive").sha256")
cat "$archive.sha256" | tee -a "${GITHUB_STEP_SUMMARY:-/dev/null}"
echo "archive=$archive" >> "${GITHUB_OUTPUT:-/dev/null}"
