#!/usr/bin/env bash
set -euo pipefail

BINARY="${1:?usage: smoke.sh BINARY_PATH}"

if [ ! -x "$BINARY" ]; then
	echo "❌ Binary not executable: $BINARY"
	exit 1
fi

echo "✅ Binary exists and is executable: $BINARY"
