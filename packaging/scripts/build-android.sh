#!/usr/bin/env bash
# Build the Rust cdylib for Android and assemble a debug APK.
#
# Prerequisites: JDK 17+, Android SDK (ANDROID_HOME), NDK 26.1, cargo-ndk,
# rustup target aarch64-linux-android.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

ABI="${ANDROID_ABI:-arm64-v8a}"
PROFILE="${ANDROID_CARGO_PROFILE:-release}"
OUT_JNI="$ROOT/android/app/src/main/jniLibs"

if [[ -z "${ANDROID_HOME:-${ANDROID_SDK_ROOT:-}}" ]]; then
	echo "ANDROID_HOME or ANDROID_SDK_ROOT must be set" >&2
	exit 1
fi

export ANDROID_HOME="${ANDROID_HOME:-$ANDROID_SDK_ROOT}"
export ANDROID_SDK_ROOT="${ANDROID_SDK_ROOT:-$ANDROID_HOME}"
export ANDROID_NDK_ROOT="${ANDROID_NDK_ROOT:-${ANDROID_NDK_HOME:-$ANDROID_HOME/ndk/26.1.10909125}}"
export ANDROID_NDK_HOME="${ANDROID_NDK_HOME:-$ANDROID_NDK_ROOT}"

if [[ ! -d "$ANDROID_NDK_HOME" ]]; then
	echo "Android NDK not found at $ANDROID_NDK_HOME" >&2
	exit 1
fi

CARGO_NDK_ARGS=(ndk -t "$ABI" -P 26 -o "$OUT_JNI" build -p apa-kan --lib)
if [[ "$PROFILE" == "release" ]]; then
	CARGO_NDK_ARGS+=(--release)
fi

echo "==> cargo ${CARGO_NDK_ARGS[*]}"
cargo "${CARGO_NDK_ARGS[@]}"

echo "==> gradle :app:assembleDebug"
(cd "$ROOT/android" && ./gradlew :app:assembleDebug --no-daemon)

APK="$ROOT/android/app/build/outputs/apk/debug/app-debug.apk"
echo "✅ Debug APK: $APK"
echo "   adb install -r \"$APK\""
