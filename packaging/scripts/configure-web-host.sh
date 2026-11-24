#!/bin/sh
# Fill the host page's `apakan:*` meta tags from the environment.
#
#   configure-web-host.sh <template.html> [out.html]
#
# Reads APAKAN_PUBLIC_URL, APAKAN_API_URL, APAKAN_MARKET_URL,
# APAKAN_PAYMENT_URL, APAKAN_LEGAL_URL. Unset variables leave the
# `%APAKAN_…%` placeholder, which the client treats as "not configured"
# (public URL falls back to window.location.origin; no API means offline).
# POSIX sh so the nginx:alpine entrypoint can run it at container start.
set -eu

IN="$1"
OUT="${2:-$1}"
tmp="${OUT}.tmp.$$"
cp "$IN" "$tmp"
for name in APAKAN_PUBLIC_URL APAKAN_API_URL APAKAN_MARKET_URL APAKAN_PAYMENT_URL APAKAN_LEGAL_URL; do
	eval "val=\${$name:-}"
	if [ -n "$val" ]; then
		case "$val" in
			*'"'*|*'<'*|*'>'*|*'&'*) echo "configure-web-host: $name has an unsafe character" >&2; rm -f "$tmp"; exit 1 ;;
		esac
		esc=$(printf '%s' "$val" | sed -e 's/[\\&|]/\\&/g')
		sed -i -e "s|%${name}%|${esc}|g" "$tmp"
		echo "configure-web-host: $name=$val"
	fi
done
mv "$tmp" "$OUT"
