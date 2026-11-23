# Apa Kan iOS wrapper

Thin native shell around the Bevy 0.19 client. **Bevy owns the entire game UI** (home, Head-to-Head configurator, and match). There is no Dioxus or SwiftUI UI. This supersedes the SwiftUI/Dioxus shell in [issue #55](https://github.com/ramate-io/apa-kan/issues/55).

Payments, legal, and other external pages open in **Safari** (`UIApplication.open` / the `webbrowser` crate). There is no in-app browser.

## Bundle id

**`io.ramate.apakan`**

| Surface | Id today | Notes |
|---|---|---|
| iOS (this wrapper) | `io.ramate.apakan` | Applied here |
| Old iOS packaging | `io.ramate.apa-kan` | Hyphen; replaced |
| Android `cargo-apk` | `io.ramate.apa_kan` | Underscore; **not** changed in this PR |

`io.ramate.apakan` is valid on both stores (iOS rejects `_` in bundle ids; a shared id should also avoid `-`). The Android agent should switch `package.metadata.android.package` to the same string when that crate is touched.

## Orientation

**Landscape only** (left and right), iPhone and iPad.

The board HUD is laid out for a wide window (desktop default 1280×900; Android metadata is already `orientation = "landscape"`). Portrait would crush the rosette and the two public benches. `UIRequiresFullScreen` is set so iPad does not split-view the Metal surface. Revisit when the Bevy layout is size-class aware.

## What the wrapper contains

| Path | Role |
|---|---|
| `project.yml` | xcodegen source of truth (CI generates `ApaKan.xcodeproj`, not committed) |
| `Info.plist` | Bundle id, `apakan://` URL scheme, landscape, launch storyboard, Expletus Sans |
| `ApaKan.entitlements` | Associated-domains scaffolding: `applinks:apakan.ramate.io` (not attached to unsigned simulator builds) |
| `apple-app-site-association.example` | AASA template; serve at `https://apakan.ramate.io/.well-known/apple-app-site-association` |
| `native/ApaKanAppDelegate.m` | Thin AppDelegate compiled **into** the Rust binary (`game/build.rs`). Forwards custom-scheme and universal-link URLs to `push_incoming_url` via `apa_kan_push_deep_link` |
| `Sources/dummy_main.c` | Linkable `_main` so Xcode 26 Debug can finish linking; the post-build script then replaces the executable with `apa-kan` |
| `scripts/build-rust.sh` | `cargo build -p apa-kan --bin apa-kan` for `aarch64-apple-ios` / `aarch64-apple-ios-sim` |
| `LaunchScreen.storyboard` | Cream `#f4eee2` + launch mark (no game UI) |
| `Assets.xcassets` | App icon and launch mark on `#f4eee2` |
| `Fonts/ExpletusSans.ttf` | OFL display face for wrapper chrome. **Do not ship Berkeley Mono** |

Deep links arrive in Bevy as `apa_kan::platform::IncomingUrl` (`uri: String`), the same name as Android PR #95. The AppDelegate calls `apa_kan_push_deep_link` → `push_incoming_url`. External http(s)/mailto URLs go through `open_url` (Safari / `UIApplication.open`; no in-app browser).

## Simulator (unsigned)

Needs a Mac with Xcode, Rust (`rust-toolchain.toml`), and [xcodegen](https://github.com/yonaskolb/XcodeGen).

```bash
rustup target add aarch64-apple-ios-sim
brew install xcodegen
packaging/ios/scripts/ci-simulator.sh
```

Or by hand:

```bash
cd packaging/ios
xcodegen generate --spec project.yml
xcodebuild -project ApaKan.xcodeproj -scheme ApaKan -configuration Debug \
  -sdk iphonesimulator -destination 'generic/platform=iOS Simulator' \
  CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO CODE_SIGN_IDENTITY= build
```

Install and launch on a booted simulator:

```bash
APP=$(find packaging/ios/build -name 'ApaKan.app' | head -1)
DEVICE=$(xcrun simctl list devices booted | awk -F '[()]' '/Booted/{print $2; exit}')
xcrun simctl install "$DEVICE" "$APP"
xcrun simctl launch --console "$DEVICE" io.ramate.apakan
xcrun simctl openurl "$DEVICE" 'apakan://home'
```

CI runs that unsigned simulator build on **every pull request** (workflow `mobile-ios.yml`). No Apple Developer account is required.

## Device

```bash
rustup target add aarch64-apple-ios
export VERSION=$(packaging/version.sh)
cargo build -p apa-kan --bin apa-kan --release --target aarch64-apple-ios
packaging/scripts/package-ios.sh   # unsigned .ipa in dist/
```

Open `packaging/ios` in Xcode (`xcodegen generate` first) to run on a plugged-in iPhone.

### Signing (not done in CI)

1. Enroll in the Apple Developer Program.
2. App ID `io.ramate.apakan` with Associated Domains.
3. In `packaging/ios/project.yml`, set `DEVELOPMENT_TEAM`, `CODE_SIGNING_ALLOWED: true`, `CODE_SIGNING_REQUIRED: true`, and point `CODE_SIGN_ENTITLEMENTS` at `ApaKan.entitlements`.
4. Xcode → Signing & Capabilities: Automatic signing for a development profile; Distribution + App Store Connect for TestFlight.
5. Optional GitHub secrets for a later signed workflow: `IOS_CERTIFICATE`, `IOS_CERTIFICATE_PASSWORD`, `IOS_PROVISIONING_PROFILE`, `IOS_TEAM_ID`.

Universal links stay dark until the AASA file is live on `apakan.ramate.io` (or whichever host Liam picks) with the real Team ID. Custom scheme `apakan://` works without that.

## Keychain

[Issue #49](https://github.com/ramate-io/apa-kan/issues/49) / RFC 002-001-002: native seeds live in the OS keychain. `apa-kan-account::KeyStore` is the trait. This bring-up does **not** wire first-run key generation into Bevy (no home/account UI yet). An iOS `SecItem` backend belongs next to that first-run flow, not in this wrapper.

## Palette (wrapper chrome only)

From symbolic.software: light bg `#f4eee2`, ink `#1a1a1a`, purple `#6e5883`, green `#3d6652`, orange `#a35c3a`, yellow `#ecc368`, red `#a03d33`. Dark bg `#1b1a18`, ink `#eee8dc`. Launch screen and icon use the light set.
