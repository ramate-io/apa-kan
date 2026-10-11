#!/usr/bin/env bash
set -euo pipefail

if [ "${1:-}" = "--rust" ]; then
	awk '/^channel/ { print $3 }' rust-toolchain.toml | tr -d '"'
else
	awk '/^version/ { print $3; exit }' Cargo.toml | tr -d '"'
fi
