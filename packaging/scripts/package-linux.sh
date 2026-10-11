#!/usr/bin/env bash
set -euo pipefail

VERSION="${VERSION:?VERSION env var required}"
DIST="dist"
NAME="apa-kan"
BINARY="target/release/$NAME"

mkdir -p "$DIST"

LINUX_DIR="$DIST/ApaKan-$VERSION-linux-x64"
mkdir -p "$LINUX_DIR"

cp "$BINARY" "$LINUX_DIR/"
chmod +x "$LINUX_DIR/$NAME"

tar -C "$DIST" -cJf "$DIST/ApaKan-$VERSION-linux-x64.tar.xz" "ApaKan-$VERSION-linux-x64"

echo "✅ Linux package created: $DIST/ApaKan-$VERSION-linux-x64.tar.xz"
