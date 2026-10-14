#!/usr/bin/env bash
set -euo pipefail

VERSION="${VERSION:?VERSION env var required}"
DIST="dist"
NAME="apa-kan"
BINARY="target/x86_64-pc-windows-msvc/release/$NAME.exe"

mkdir -p "$DIST"

WIN_DIR="$DIST/ApaKan-$VERSION-windows-x64"
mkdir -p "$WIN_DIR"

cp "$BINARY" "$WIN_DIR/apa-kan.exe"

cd "$DIST"
zip -r "ApaKan-$VERSION-windows-x64.zip" "ApaKan-$VERSION-windows-x64"

echo "✅ Windows package created: $DIST/ApaKan-$VERSION-windows-x64.zip"
