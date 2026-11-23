#!/usr/bin/env bash
# Unsigned iOS Simulator build for pull requests. Requires macOS + Xcode + xcodegen.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
cd "$ROOT"

IOS_DIR="$ROOT/packaging/ios"
VERSION="$(packaging/version.sh)"
RUST="$(packaging/version.sh --rust)"

echo "Apa Kan iOS simulator build version=$VERSION rust=$RUST"

if ! command -v xcodegen >/dev/null 2>&1; then
	echo "xcodegen is required (brew install xcodegen)" >&2
	exit 1
fi

command -v cargo >/dev/null
rustup target add aarch64-apple-ios-sim >/dev/null

(cd "$IOS_DIR" && xcodegen generate --spec project.yml --use-cache)

DEST="${DEST:-generic/platform=iOS Simulator}"
DERIVED="${DERIVED:-$ROOT/packaging/ios/build}"

xcodebuild \
	-project "$IOS_DIR/ApaKan.xcodeproj" \
	-scheme ApaKan \
	-configuration Debug \
	-sdk iphonesimulator \
	-destination "$DEST" \
	-derivedDataPath "$DERIVED" \
	ARCHS=arm64 \
	ONLY_ACTIVE_ARCH=YES \
	EXCLUDED_ARCHS=x86_64 \
	ENABLE_DEBUG_DYLIB=NO \
	CODE_SIGNING_ALLOWED=NO \
	CODE_SIGNING_REQUIRED=NO \
	CODE_SIGN_IDENTITY= \
	build

echo "✅ iOS Simulator build succeeded (unsigned)"
