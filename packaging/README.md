# Apa Kan Packaging

This directory contains packaging scripts and configuration for building and distributing Apa Kan across multiple platforms.

## Supported Platforms

### Desktop
- **Linux** (x64)
- **macOS** (Apple Silicon)
- **Windows** (x64)

### Mobile
- **iOS** (arm64)
- **Android** (arm64-v8a, armeabi-v7a)

## Building Packages

### Desktop

Build packages for all desktop platforms via GitHub Actions by:
- Pushing to `main` branch
- Creating a release
- Running workflow manually
- Including `ci-action::package` in commit message

### Mobile

Mobile builds are handled by separate workflows:
- `mobile-ios.yml` for iOS
- `mobile-android.yml` for Android

## Manual Setup Required

### iOS Publishing

To publish to the App Store, you need to:

1. **Apple Developer Account**
   - Enroll in Apple Developer Program ($99/year)
   - Create App ID: `io.ramate.apa-kan`

2. **Code Signing**
   - Generate signing certificate in Xcode or Apple Developer portal
   - Create provisioning profiles (development and distribution)
   - Add certificate and profiles to CI/CD secrets

3. **CI/CD Secrets** (add to GitHub repository)
   - `IOS_CERTIFICATE` - Base64-encoded .p12 certificate
   - `IOS_CERTIFICATE_PASSWORD` - Certificate password
   - `IOS_PROVISIONING_PROFILE` - Base64-encoded provisioning profile
   - `IOS_TEAM_ID` - Apple Developer Team ID

4. **App Store Connect**
   - Create app listing in App Store Connect
   - Configure app metadata, screenshots, descriptions
   - Set up TestFlight for beta testing

### Android Publishing

To publish to Google Play, you need to:

1. **Google Play Developer Account**
   - Register for Google Play Console ($25 one-time fee)
   - Create app listing with package name: `io.ramate.apa_kan`

2. **Release Signing**
   - Generate release keystore:
     ```bash
     keytool -genkey -v -keystore apa-kan-release.keystore \
       -alias apa-kan -keyalg RSA -keysize 2048 -validity 10000
     ```
   - Store keystore securely (do NOT commit to repository)

3. **CI/CD Secrets** (add to GitHub repository)
   - `ANDROID_KEYSTORE` - Base64-encoded keystore file
   - `ANDROID_KEYSTORE_PASSWORD` - Keystore password
   - `ANDROID_KEY_ALIAS` - Key alias (e.g., "apa-kan")
   - `ANDROID_KEY_PASSWORD` - Key password

4. **Google Play Console**
   - Complete app listing (title, description, screenshots, etc.)
   - Set up internal/alpha/beta testing tracks
   - Configure app content rating and privacy policy

## Local Builds

### Prerequisites

- Rust toolchain (see `rust-toolchain.toml`)
- Nix (with flakes enabled)
- Platform-specific tools:
  - macOS: Xcode Command Line Tools
  - Windows: Visual Studio Build Tools
  - Android: Android SDK and NDK
  - iOS: Xcode

### Building Locally

Enter the Nix development shell:
```bash
nix develop
```

Build for your platform:
```bash
cargo build -p apa-kan --release
```

Run packaging script for your platform:
```bash
export VERSION=$(packaging/version.sh)
./packaging/scripts/package-{linux,macos,windows}.sh
```

## Testing Packages

Smoke test binary:
```bash
packaging/smoke.sh path/to/binary
```

## Version Management

Version is managed in `Cargo.toml` workspace section:
```toml
[workspace.package]
version = "0.1.0"
```

The `packaging/version.sh` script extracts the version for use in workflows and scripts.
