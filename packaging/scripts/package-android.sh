#!/usr/bin/env bash
set -euo pipefail

VERSION="${VERSION:?VERSION env var required}"
DIST="dist"

mkdir -p "$DIST"

APK_PATH=$(find target -name "*.apk" -type f | head -1)
if [ -z "$APK_PATH" ]; then
	echo "❌ No APK found in target directory"
	exit 1
fi

cp "$APK_PATH" "$DIST/ApaKan-$VERSION-android.apk"

echo "✅ Android package created: $DIST/ApaKan-$VERSION-android.apk"
echo "⚠️  NOTE: This APK is unsigned/debug-signed. To publish:"
echo "   1. Generate release keystore with keytool"
echo "   2. Sign APK with jarsigner or apksigner"
echo "   3. Align with zipalign"
echo "   4. Configure Google Play Console account"
echo "   5. Add keystore secrets to CI/CD"
