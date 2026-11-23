#!/usr/bin/env bash
# Build the Bevy `apa-kan` binary for the active Xcode destination and install
# it as the app executable. Based on Bevy 0.19's examples/mobile/build_rust_deps.sh.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
cd "$ROOT"

PATH="$PATH:$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin"
# Xcode prepends its toolchain usr/bin, which makes rustc's linker fail with
# `ld: library 'System' not found`. Prefer the system clang.
export PATH="/usr/bin:/bin:/usr/sbin:/sbin:$PATH"

PROFILE=debug
RELFLAG=()
if [[ "${CONFIGURATION:-Debug}" != "Debug" ]]; then
	PROFILE=release
	RELFLAG=(--release)
fi

export IPHONEOS_DEPLOYMENT_TARGET="${IPHONEOS_DEPLOYMENT_TARGET:-16.0}"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"

IS_SIMULATOR=0
if [[ "${LLVM_TARGET_TRIPLE_SUFFIX:-}" == "-simulator" ]]; then
	IS_SIMULATOR=1
fi

EXECUTABLES=()
ARCHS_LIST="${ARCHS:-arm64}"
for arch in $ARCHS_LIST; do
	case "$arch" in
	x86_64)
		if [[ "$IS_SIMULATOR" -eq 0 ]]; then
			echo "x86_64 is only valid for the iOS simulator" >&2
			exit 2
		fi
		TARGET=x86_64-apple-ios
		;;
	arm64)
		if [[ "$IS_SIMULATOR" -eq 0 ]]; then
			TARGET=aarch64-apple-ios
		else
			TARGET=aarch64-apple-ios-sim
		fi
		;;
	*)
		echo "unsupported ARCH=$arch" >&2
		exit 2
		;;
	esac

	echo "cargo build -p apa-kan --bin apa-kan --target $TARGET ${RELFLAG[*]:-}"
	cargo build -p apa-kan --bin apa-kan --features desktop "${RELFLAG[@]}" --target "$TARGET"
	EXECUTABLES+=("$CARGO_TARGET_DIR/$TARGET/$PROFILE/apa-kan")
done

DEST="${TARGET_BUILD_DIR:-.}/${EXECUTABLE_PATH:-apa-kan}"
mkdir -p "$(dirname "$DEST")"
if [[ ${#EXECUTABLES[@]} -eq 1 ]]; then
	cp "${EXECUTABLES[0]}" "$DEST"
else
	lipo -create -output "$DEST" "${EXECUTABLES[@]}"
fi
chmod +x "$DEST"
echo "installed $DEST"
