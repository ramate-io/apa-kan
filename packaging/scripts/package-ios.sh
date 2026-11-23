#!/usr/bin/env bash
set -euo pipefail

VERSION="${VERSION:?VERSION env var required}"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
DIST="$ROOT/dist"
NAME="apa-kan"
IOS="$ROOT/packaging/ios"
BINARY="${BINARY:-$ROOT/target/aarch64-apple-ios/release/$NAME}"

if [[ ! -x "$BINARY" ]]; then
	echo "missing iOS binary at $BINARY" >&2
	echo "build with: cargo build -p apa-kan --bin apa-kan --release --target aarch64-apple-ios" >&2
	exit 1
fi

mkdir -p "$DIST"
APP_DIR="$DIST/Payload/ApaKan.app"
rm -rf "$DIST/Payload"
mkdir -p "$APP_DIR"

cp "$BINARY" "$APP_DIR/ApaKan"
chmod +x "$APP_DIR/ApaKan"

# Materialize Info.plist so the IPA does not depend on Xcode substitutions.
python3 - "$IOS/Info.plist" "$APP_DIR/Info.plist" "$VERSION" <<'PY'
import pathlib, sys
src, dest, version = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2]), sys.argv[3]
text = src.read_text()
text = text.replace("$(EXECUTABLE_NAME)", "ApaKan")
text = text.replace("$(PRODUCT_BUNDLE_IDENTIFIER)", "io.ramate.apakan")
text = text.replace("$(MARKETING_VERSION)", version)
text = text.replace("$(CURRENT_PROJECT_VERSION)", version)
dest.write_text(text)
PY

mkdir -p "$APP_DIR/Assets.car" 2>/dev/null || true
cp "$IOS/LaunchScreen.storyboard" "$APP_DIR/"
mkdir -p "$APP_DIR/Fonts"
cp "$IOS/Fonts/ExpletusSans.ttf" "$APP_DIR/Fonts/"
cp "$IOS/Fonts/ExpletusSans.ttf" "$APP_DIR/"
cp "$IOS/Assets.xcassets/AppIcon.appiconset/AppIcon.png" "$APP_DIR/AppIcon.png"
cp "$IOS/Assets.xcassets/LaunchMark.imageset/LaunchMark.png" "$APP_DIR/LaunchMark.png"

# Minimal PkgInfo
printf 'APPL????' > "$APP_DIR/PkgInfo"

(
	cd "$DIST"
	rm -f "ApaKan-$VERSION-ios.ipa"
	zip -r "ApaKan-$VERSION-ios.ipa" Payload
)

echo "✅ iOS package created: $DIST/ApaKan-$VERSION-ios.ipa"
echo "⚠️  Unsigned. Simulator: packaging/ios/scripts/ci-simulator.sh"
echo "    Device signing: packaging/ios/README.md"
