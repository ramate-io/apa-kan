# Apa Kan for Android

Bevy owns the entire game UI. This directory is a **thin GameActivity wrapper**:
splash, adaptive icon, deep-link intent filters, and packaging. There is no
Compose or Dioxus shell. Payments and other external pages open in the **default
browser** (`ACTION_VIEW`); there is no Custom Tab or in-app WebView.

**Application id:** `io.ramate.apakan` (no underscore). iOS should use the same
id (`io.ramate.apakan`, not `io.ramate.apa-kan` / `io.ramate.apa_kan`).

**SDKs:** min 26, compile/target **36** (Play’s current requirement as of
2026-08-31). ABI in CI: `arm64-v8a` only.

## Why Gradle, not cargo-apk

`cargo-apk` can still emit an APK, but it cannot:

- set `android:autoVerify` for [App Links](https://developer.android.com/training/app-links)
- produce a Play **Android App Bundle** (`.aab`)
- plug into Play App Signing / CI keystore secrets as a first-class `signingConfig`

Bevy 0.19’s own mobile example uses **GameActivity + cargo-ndk + Gradle**. That
is the path this repo follows. The Rust crate still builds as `cdylib`
(`libapa_kan.so`); Gradle only packages it.

## Debug install

Prerequisites: JDK 17+, Android SDK + NDK `26.1.10909125`, `cargo-ndk`,
`aarch64-linux-android` target, `ANDROID_HOME` set.

```bash
rustup target add aarch64-linux-android
cargo install cargo-ndk
export ANDROID_NDK_HOME="$ANDROID_HOME/ndk/26.1.10909125"

# Device or emulator over adb:
packaging/scripts/build-android.sh
adb install -r android/app/build/outputs/apk/debug/app-debug.apk
```

Or, after the `.so` is in `android/app/src/main/jniLibs`:

```bash
cd android && ./gradlew :app:installDebug
```

Custom-scheme smoke test:

```bash
adb shell am start -a android.intent.action.VIEW -d "apakan://settings"
```

The URI is forwarded into Bevy as an `IncomingUrl` message
(`apa-kan/game/src/platform.rs`). Home / H2H routing is still a later Bevy UI
task; the event is wired so iOS and web can match the same name.

## Release signing

Do **not** commit a keystore. Create one once and store it in a password manager:

```bash
keytool -genkey -v -keystore apa-kan-release.keystore \
  -alias apa-kan -keyalg RSA -keysize 2048 -validity 10000
```

GitHub Actions secrets (used by `.github/workflows/mobile-android.yml` on
release / `workflow_dispatch`):

| Secret | Meaning |
|---|---|
| `ANDROID_KEYSTORE` | Base64 of the `.keystore` file |
| `ANDROID_KEYSTORE_PASSWORD` | Keystore password |
| `ANDROID_KEY_ALIAS` | e.g. `apa-kan` |
| `ANDROID_KEY_PASSWORD` | Key password |

Local signed bundle:

```bash
export ANDROID_KEYSTORE_FILE=/absolute/path/apa-kan-release.keystore
export ANDROID_KEYSTORE_PASSWORD=…
export ANDROID_KEY_ALIAS=apa-kan
export ANDROID_KEY_PASSWORD=…
packaging/scripts/build-android.sh
(cd android && ./gradlew :app:bundleRelease)
```

Upload `app-release.aab` to Play. Prefer Play App Signing (Google holds the
app-signing key; the upload key is this keystore).

Fingerprint for Digital Asset Links:

```bash
keytool -list -v -keystore apa-kan-release.keystore
# copy SHA-256 into android/assetlinks.json.example and host it at
# https://apakan.ramate.io/.well-known/assetlinks.json
```

Until that file is served, `https://apakan.ramate.io/…` links will not
auto-open the app. `apakan://…` still works.

## Wrapper contents

| Piece | Role |
|---|---|
| `MainActivity` (`GameActivity`) | Load `libapa_kan.so`, hide system bars, forward VIEW intents |
| Splash | Android 12 SplashScreen API, background `#f4eee2` / dark `#1b1a18` |
| Adaptive icon | Same palette; 8-petal mark in `#6e5883` |
| Expletus Sans (OFL) | Wrapper chrome `android:fontFamily` only. Berkeley Mono is not shipped. |
| `apakan://` intent filter | Custom scheme |
| `https://apakan.ramate.io` | App Links scaffolding (`autoVerify`) |
| `webbrowser` / `open_url` | Default browser for payments and legal |

The `apa-kan-account` `KeyStore` trait is **not** implemented here. Android
Keystore is the intended native backend (#49); first-run key generation still
belongs with the Bevy home UI.
