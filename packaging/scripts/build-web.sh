#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

OUT="${OUT:-dist/web}"
VERSION="${VERSION:-$(packaging/version.sh)}"
if [ -z "${WASM_BINDGEN_VERSION:-}" ]; then
	WASM_BINDGEN_VERSION="$(
		awk '
			$0 == "name = \"wasm-bindgen\"" { found=1; next }
			found && $1 == "version" { gsub(/"/, "", $3); print $3; exit }
		' Cargo.lock
	)"
	WASM_BINDGEN_VERSION="${WASM_BINDGEN_VERSION:-0.2.129}"
fi

rustup target add wasm32-unknown-unknown >/dev/null

if ! command -v wasm-bindgen >/dev/null 2>&1; then
	echo "Installing wasm-bindgen-cli ${WASM_BINDGEN_VERSION}"
	cargo install wasm-bindgen-cli --version "$WASM_BINDGEN_VERSION" --locked || \
		cargo install wasm-bindgen-cli --version "$WASM_BINDGEN_VERSION"
fi

echo "Building apa-kan for wasm32-unknown-unknown (WebGL2)"
cargo build -p apa-kan --bin apa-kan --release --locked --target wasm32-unknown-unknown --features desktop

WASM_IN="target/wasm32-unknown-unknown/release/apa-kan.wasm"
rm -rf "$OUT"
mkdir -p "$OUT"

wasm-bindgen "$WASM_IN" \
	--target web \
	--no-typescript \
	--out-dir "$OUT" \
	--out-name apa-kan

if command -v wasm-opt >/dev/null 2>&1; then
	echo "Running wasm-opt -Oz"
	tmp="$OUT/apa-kan_bg.opt.wasm"
	if wasm-opt -Oz --enable-bulk-memory -o "$tmp" "$OUT/apa-kan_bg.wasm"; then
		mv "$tmp" "$OUT/apa-kan_bg.wasm"
	else
		echo "wasm-opt failed; keeping wasm-bindgen output"
		rm -f "$tmp"
	fi
else
	echo "wasm-opt not found; skipping (install binaryen for smaller bundles)"
fi

cp apa-kan/game/web/index.html "$OUT/index.html"
if [ -d apa-kan/game/assets ]; then
	mkdir -p "$OUT/assets"
	cp -R apa-kan/game/assets/. "$OUT/assets/"
fi

wasm_bytes=$(wc -c < "$OUT/apa-kan_bg.wasm" | tr -d ' ')
js_bytes=$(wc -c < "$OUT/apa-kan.js" | tr -d ' ')
gz_bytes=$(gzip -c "$OUT/apa-kan_bg.wasm" | wc -c | tr -d ' ')
{
	echo "apa-kan web ${VERSION}"
	echo "wasm_bytes=${wasm_bytes}"
	echo "wasm_gzip_bytes=${gz_bytes}"
	echo "js_bytes=${js_bytes}"
} | tee "$OUT/size.txt"

echo "Bundle written to ${OUT}"
echo "Serve: packaging/scripts/serve-web.sh"
