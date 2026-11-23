#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
OUT="${OUT:-$ROOT/dist/web}"
PORT="${PORT:-8080}"

if [ ! -f "$OUT/index.html" ]; then
	echo "No bundle at $OUT. Run packaging/scripts/build-web.sh first."
	exit 1
fi

cd "$OUT"
echo "Apa Kan web client at http://127.0.0.1:${PORT}/"
echo "Deep links: http://127.0.0.1:${PORT}/#/h2h  http://127.0.0.1:${PORT}/#/game/demo"
if command -v python3 >/dev/null 2>&1; then
	exec python3 -m http.server "$PORT" --bind 127.0.0.1
fi
exec python -m http.server "$PORT" --bind 127.0.0.1
