#!/usr/bin/env bash
set -euo pipefail

VERSION="${VERSION:?VERSION env var required}"
DIST="dist"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

mkdir -p "$DIST"

DEBUG_APK="android/app/build/outputs/apk/debug/app-debug.apk"
RELEASE_APK="android/app/build/outputs/apk/release/app-release.apk"
RELEASE_AAB="android/app/build/outputs/bundle/release/app-release.aab"

copied=0
if [[ -f "$RELEASE_AAB" ]]; then
	cp "$RELEASE_AAB" "$DIST/ApaKan-$VERSION-android.aab"
	echo "✅ Android App Bundle: $DIST/ApaKan-$VERSION-android.aab"
	copied=1
fi
if [[ -f "$RELEASE_APK" ]]; then
	cp "$RELEASE_APK" "$DIST/ApaKan-$VERSION-android.apk"
	echo "✅ Android APK (release): $DIST/ApaKan-$VERSION-android.apk"
	copied=1
elif [[ -f "$DEBUG_APK" ]]; then
	cp "$DEBUG_APK" "$DIST/ApaKan-$VERSION-android-debug.apk"
	echo "✅ Android APK (debug): $DIST/ApaKan-$VERSION-android-debug.apk"
	copied=1
fi

if [[ "$copied" -eq 0 ]]; then
	echo "❌ No APK/AAB found. Run packaging/scripts/build-android.sh first." >&2
	exit 1
fi
