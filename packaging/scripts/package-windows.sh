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

# Use PowerShell's Compress-Archive on Windows (zip not available by default)
if command -v powershell.exe &> /dev/null; then
	powershell.exe -Command "Compress-Archive -Path '$WIN_DIR' -DestinationPath '$DIST/ApaKan-$VERSION-windows-x64.zip' -Force"
else
	# Fallback to zip if available (e.g., in Git Bash with zip installed)
	cd "$DIST"
	zip -r "ApaKan-$VERSION-windows-x64.zip" "ApaKan-$VERSION-windows-x64"
fi

echo "✅ Windows package created: $DIST/ApaKan-$VERSION-windows-x64.zip"
