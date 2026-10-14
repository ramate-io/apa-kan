#!/usr/bin/env bash
set -euo pipefail

VERSION="${VERSION:?VERSION env var required}"
DIST="dist"
NAME="apa-kan"
BINARY="target/release/$NAME"

mkdir -p "$DIST"

APP_DIR="$DIST/ApaKan.app"
mkdir -p "$APP_DIR/Contents/MacOS"
mkdir -p "$APP_DIR/Contents/Resources"

cp "$BINARY" "$APP_DIR/Contents/MacOS/apa-kan"
chmod +x "$APP_DIR/Contents/MacOS/apa-kan"

cat > "$APP_DIR/Contents/Info.plist" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleExecutable</key>
	<string>apa-kan</string>
	<key>CFBundleIdentifier</key>
	<string>io.ramate.apa-kan</string>
	<key>CFBundleName</key>
	<string>ApaKan</string>
	<key>CFBundleVersion</key>
	<string>$VERSION</string>
	<key>CFBundleShortVersionString</key>
	<string>$VERSION</string>
	<key>CFBundlePackageType</key>
	<string>APPL</string>
</dict>
</plist>
EOF

hdiutil create -volname "ApaKan" -srcfolder "$APP_DIR" -ov -format UDZO "$DIST/ApaKan-$VERSION-macos-arm64.dmg"

echo "✅ macOS package created: $DIST/ApaKan-$VERSION-macos-arm64.dmg"
