#!/usr/bin/env bash
set -euo pipefail

VERSION="${VERSION:?VERSION env var required}"
DIST="dist"
NAME="apa-kan"
BINARY="target/aarch64-apple-ios/release/$NAME"

mkdir -p "$DIST"

APP_DIR="$DIST/Payload/ApaKan.app"
mkdir -p "$APP_DIR"

cp "$BINARY" "$APP_DIR/apa-kan"
chmod +x "$APP_DIR/apa-kan"

cat > "$APP_DIR/Info.plist" << EOF
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
	<key>UIRequiredDeviceCapabilities</key>
	<array>
		<string>metal</string>
	</array>
	<key>UISupportedInterfaceOrientations</key>
	<array>
		<string>UIInterfaceOrientationPortrait</string>
		<string>UIInterfaceOrientationLandscapeLeft</string>
		<string>UIInterfaceOrientationLandscapeRight</string>
	</array>
</dict>
</plist>
EOF

cd "$DIST"
zip -r "ApaKan-$VERSION-ios.ipa" Payload

echo "✅ iOS package created: $DIST/ApaKan-$VERSION-ios.ipa"
echo "⚠️  NOTE: This IPA is unsigned. To publish:"
echo "   1. Sign with Apple Developer certificate using Xcode or fastlane"
echo "   2. Configure App Store Connect account"
echo "   3. Set up provisioning profiles"
echo "   4. Add signing certificate secrets to CI/CD"
