#!/usr/bin/env bash
#
# Package each theme under themes/ into a .zip archive with the theme's
# contents (templates/, skel/, pages/) at the archive root, which is the
# layout `bckt themes install` and `bckt init --theme` expect.
#
# Usage: scripts/package-themes.sh [output-dir]
#   output-dir defaults to target/themes
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
THEMES_DIR="$ROOT/themes"
OUT_DIR="${1:-$ROOT/target/themes}"

mkdir -p "$OUT_DIR"

for dir in "$THEMES_DIR"/*/; do
  name="$(basename "$dir")"
  out="$OUT_DIR/$name.zip"
  rm -f "$out"
  # Zip from inside the theme dir so paths are relative to the theme root.
  ( cd "$dir" && zip -qr "$out" . -x '.*' -x '*/.*' '*.DS_Store' )
  echo "packaged $name -> $out"
done
