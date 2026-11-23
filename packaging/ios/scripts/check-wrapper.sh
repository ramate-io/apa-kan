#!/usr/bin/env bash
# Linux-friendly sanity checks for the iOS wrapper files (no Xcode required).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
fail=0

check() {
	local file="$1"
	local pattern="$2"
	if ! grep -qE "$pattern" "$ROOT/$file"; then
		echo "missing /$pattern/ in $file" >&2
		fail=1
	fi
}

check packaging/ios/project.yml 'io\.ramate\.apakan'
check packaging/ios/Info.plist 'apakan'
check packaging/ios/Info.plist 'UIInterfaceOrientationLandscapeLeft'
check packaging/ios/Info.plist 'UIInterfaceOrientationLandscapeRight'
check packaging/ios/ApaKan.entitlements 'applinks:apakan\.ramate\.io'
check packaging/ios/native/ApaKanAppDelegate.m 'apa_kan_push_deep_link'
check packaging/ios/native/ApaKanAppDelegate.m 'continueUserActivity'
check packaging/ios/Info.plist 'ExpletusSans.ttf'
check packaging/ios/Fonts/OFL.txt 'SIL OPEN FONT LICENSE'
test -f "$ROOT/packaging/ios/Fonts/ExpletusSans.ttf" || {
	echo "missing ExpletusSans.ttf" >&2
	fail=1
}
test -f "$ROOT/packaging/ios/Assets.xcassets/AppIcon.appiconset/AppIcon.png" || {
	echo "missing AppIcon.png" >&2
	fail=1
}

if find "$ROOT/packaging/ios" -iname '*berkeley*' \( -name '*.ttf' -o -name '*.otf' -o -name '*.woff*' \) | grep -q .; then
	echo "must not ship the commercial Berkeley typeface" >&2
	fail=1
fi

if [[ "$fail" -ne 0 ]]; then
	exit 1
fi
echo "✅ iOS wrapper files look consistent"
