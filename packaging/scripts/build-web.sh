#!/usr/bin/env bash
# Build the web bundle (dist/web): wasm32 release build + wasm-bindgen + optional wasm-opt.
#
# wasm-bindgen-cli must match the `wasm-bindgen` crate version in Cargo.lock
# exactly (the bindgen schema is unstable). This script reads that version and
# uses a matching `wasm-bindgen` from PATH, or installs exactly that version
# into `.tools/wasm-bindgen-<version>/` (git-ignored) and uses it from there.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

OUT="${OUT:-dist/web}"
VERSION="${VERSION:-$(packaging/version.sh)}"
TOOLS="${APAKAN_TOOLS_DIR:-$ROOT/.tools}"

die() {
	echo "error: $*" >&2
	exit 1
}

# Exact wasm-bindgen version from Cargo.lock (the source of truth).
LOCK_VERSION="$(
	awk '
		$0 == "name = \"wasm-bindgen\"" { found=1; next }
		found && $1 == "version" { gsub(/"/, "", $3); print $3; exit }
	' Cargo.lock
)"
[ -n "$LOCK_VERSION" ] || die "could not read the wasm-bindgen version from Cargo.lock"
if [ -n "${WASM_BINDGEN_VERSION:-}" ] && [ "$WASM_BINDGEN_VERSION" != "$LOCK_VERSION" ]; then
	die "WASM_BINDGEN_VERSION=$WASM_BINDGEN_VERSION but Cargo.lock pins wasm-bindgen $LOCK_VERSION; unset it"
fi

bindgen_version() {
	"$1" --version 2>/dev/null | awk '{ print $2 }'
}

# Pick a wasm-bindgen binary whose version equals LOCK_VERSION.
WASM_BINDGEN=""
LOCAL_BIN="$TOOLS/wasm-bindgen-$LOCK_VERSION/bin/wasm-bindgen"
if [ -x "$LOCAL_BIN" ] && [ "$(bindgen_version "$LOCAL_BIN")" = "$LOCK_VERSION" ]; then
	WASM_BINDGEN="$LOCAL_BIN"
elif command -v wasm-bindgen >/dev/null 2>&1 &&
	[ "$(bindgen_version "$(command -v wasm-bindgen)")" = "$LOCK_VERSION" ]; then
	WASM_BINDGEN="$(command -v wasm-bindgen)"
else
	if command -v wasm-bindgen >/dev/null 2>&1; then
		echo "wasm-bindgen on PATH is $(bindgen_version "$(command -v wasm-bindgen)"), Cargo.lock needs $LOCK_VERSION; installing a matching copy under $TOOLS"
	else
		echo "Installing wasm-bindgen-cli $LOCK_VERSION under $TOOLS"
	fi
	if ! cargo install wasm-bindgen-cli --version "$LOCK_VERSION" --locked \
		--root "$TOOLS/wasm-bindgen-$LOCK_VERSION"; then
		die "could not install wasm-bindgen-cli $LOCK_VERSION. Fix: cargo install -f wasm-bindgen-cli --version $LOCK_VERSION --locked (then re-run this script)"
	fi
	WASM_BINDGEN="$LOCAL_BIN"
fi
got="$(bindgen_version "$WASM_BINDGEN")"
[ "$got" = "$LOCK_VERSION" ] ||
	die "$WASM_BINDGEN is $got, need $LOCK_VERSION. Fix: cargo install -f wasm-bindgen-cli --version $LOCK_VERSION --locked"
echo "Using wasm-bindgen $got ($WASM_BINDGEN)"

rustup target add wasm32-unknown-unknown >/dev/null

echo "Building apa-kan for wasm32-unknown-unknown (WebGL2)"
cargo build -p apa-kan --bin apa-kan --release --locked --target wasm32-unknown-unknown --features desktop

WASM_IN="target/wasm32-unknown-unknown/release/apa-kan.wasm"
[ -f "$WASM_IN" ] || die "cargo did not produce $WASM_IN"
rm -rf "$OUT"
mkdir -p "$OUT"

"$WASM_BINDGEN" "$WASM_IN" \
	--target web \
	--no-typescript \
	--out-dir "$OUT" \
	--out-name apa-kan ||
	die "wasm-bindgen failed; no bundle was written"
[ -f "$OUT/apa-kan_bg.wasm" ] && [ -f "$OUT/apa-kan.js" ] ||
	die "wasm-bindgen did not write $OUT/apa-kan.js and $OUT/apa-kan_bg.wasm"

# wasm-opt must understand the reference types wasm-bindgen emits (externref
# table). Old binaryen (e.g. Ubuntu 22.04's 105) "optimizes" the bundle into
# one that dies at init with "WebAssembly.Table.grow(): failed to grow table",
# so older versions are skipped instead of trusted.
WASM_OPT_MIN=116
WASM_OPT_FEATURES=(--enable-bulk-memory --enable-reference-types --enable-multivalue
	--enable-mutable-globals --enable-nontrapping-float-to-int --enable-sign-ext)
WASM_OPT="${WASM_OPT:-$(command -v wasm-opt || true)}"
if [ -n "$WASM_OPT" ]; then
	opt_version="$("$WASM_OPT" --version 2>/dev/null | awk '{ for (i = 1; i <= NF; i++) if ($i ~ /^[0-9]+$/) { print $i; exit } }')"
	if [ -z "$opt_version" ] || [ "$opt_version" -lt "$WASM_OPT_MIN" ]; then
		echo "wasm-opt ${opt_version:-?} at $WASM_OPT is older than $WASM_OPT_MIN (breaks externref tables); skipping. Install a current binaryen for smaller bundles."
	else
		echo "Running wasm-opt $opt_version -Oz"
		tmp="$OUT/apa-kan_bg.opt.wasm"
		if "$WASM_OPT" -Oz "${WASM_OPT_FEATURES[@]}" -o "$tmp" "$OUT/apa-kan_bg.wasm"; then
			mv "$tmp" "$OUT/apa-kan_bg.wasm"
		else
			echo "wasm-opt failed; keeping wasm-bindgen output"
			rm -f "$tmp"
		fi
	fi
else
	echo "wasm-opt not found; skipping (install binaryen >= $WASM_OPT_MIN for smaller bundles)"
fi

packaging/scripts/configure-web-host.sh apa-kan/game/web/index.html "$OUT/index.html"
if [ -d apa-kan/game/assets ]; then
	mkdir -p "$OUT/assets"
	cp -R apa-kan/game/assets/. "$OUT/assets/"
fi

wasm_bytes=$(wc -c < "$OUT/apa-kan_bg.wasm" | tr -d ' ')
js_bytes=$(wc -c < "$OUT/apa-kan.js" | tr -d ' ')
gz_bytes=$(gzip -c "$OUT/apa-kan_bg.wasm" | wc -c | tr -d ' ')
{
	echo "apa-kan web ${VERSION}"
	echo "wasm_bindgen=${LOCK_VERSION}"
	echo "wasm_bytes=${wasm_bytes}"
	echo "wasm_gzip_bytes=${gz_bytes}"
	echo "js_bytes=${js_bytes}"
} | tee "$OUT/size.txt"

echo "Bundle written to ${OUT}"
echo "Serve: packaging/scripts/serve-web.sh"
