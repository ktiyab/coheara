# Building Coheara from Source

This guide walks you through building Coheara locally on Linux/WSL2. Every step has been tested end-to-end from a clean environment.

---

## Quick Start (first build)

Your first build should be **unsigned** — no keys, no `.env`, no setup script needed:

```bash
git clone https://github.com/ktiyab/coheara.git
cd coheara
./build.sh desktop --no-sign
```

This builds the full desktop installer (~30 minutes on first run). Artifacts appear in `./package/`:

```
package/
├── Coheara_X.Y.Z_amd64.deb        (11 MB)
├── Coheara_X.Y.Z_amd64.AppImage   (85 MB)
└── coheara-X.Y.Z.apk              (2.2 MB)
```

Once the unsigned build works, set up signing keys for production builds (see [Signing Keys](#signing-keys)).

---

## Prerequisites

### 1. Node.js and Rust

| Tool | Version | Install |
|------|---------|---------|
| Node.js | >= 20 | [nodejs.org](https://nodejs.org/) or `nvm install 20` |
| Rust | >= 1.80 | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| Perl | >= 5 | Required by OpenSSL compilation (see below) |
| Tauri CLI | >= 2 | Installed automatically via `npm ci` (devDependency) |

**After installing Rust**, ensure cargo is in your PATH. Add this to your `~/.bashrc` or `~/.zshrc`:

```bash
export PATH="$HOME/.cargo/bin:$PATH"
```

Then reload: `source ~/.bashrc`

> `build.sh` will find cargo at `~/.cargo/bin/cargo` automatically, but other tools (like `npx tauri build`) also need it in PATH.

### 2. Linux system libraries (Ubuntu/Debian/WSL2)

```bash
sudo apt-get update
sudo apt-get install -y \
    build-essential \
    pkg-config \
    perl \
    libgtk-3-dev \
    libwebkit2gtk-4.1-dev \
    libappindicator3-dev \
    librsvg2-dev \
    patchelf \
    libsoup-3.0-dev \
    libjavascriptcoregtk-4.1-dev \
    tesseract-ocr \
    libtesseract-dev \
    libleptonica-dev \
    libclang-dev \
    libssl-dev \
    unzip
```

> `perl` and `libssl-dev` are needed by the `openssl-sys` Rust crate to configure and compile OpenSSL. `unzip` is needed by the Android SDK manager. All three may already be installed on your system.

### 3. Android SDK (for APK builds)

The desktop build bundles an Android APK inside the installer so that paired phones can receive the companion app directly over local WiFi — no app store or internet needed. This means you need the Android SDK even for `./build.sh desktop`.

#### Option A: Android Studio (easiest)

Install [Android Studio](https://developer.android.com/studio), then from **SDK Manager** install:
- Android SDK Platform 36
- Android SDK Build-Tools 36.0.0
- Android SDK Platform-Tools

Then set `ANDROID_HOME`:

```bash
export ANDROID_HOME=~/Android/Sdk
```

#### Option B: Command-line only (headless / WSL2)

This is what we tested. Step by step:

```bash
# Create SDK directory
mkdir -p ~/Android/Sdk/cmdline-tools

# Download command-line tools
cd /tmp
wget https://dl.google.com/android/repository/commandlinetools-linux-11076708_latest.zip
unzip commandlinetools-linux-11076708_latest.zip
mv cmdline-tools ~/Android/Sdk/cmdline-tools/latest

# Set environment (add to ~/.bashrc)
export ANDROID_HOME=~/Android/Sdk
export PATH="$ANDROID_HOME/cmdline-tools/latest/bin:$ANDROID_HOME/platform-tools:$PATH"

# Install required SDK components
sdkmanager "platforms;android-36" "build-tools;36.0.0" "platform-tools"

# Accept licenses
sdkmanager --licenses
```

> You will be prompted to accept several licenses. Type `y` for each.

### 4. Java JDK 21 (for Android builds)

```bash
sudo apt-get install -y openjdk-21-jdk
```

Verify: `java -version` should show version 21+.

### 5. macOS

```bash
xcode-select --install
brew install tesseract leptonica pkg-config
```

You also need the **Android SDK** and **JDK 21** (see sections 3 and 4 above). The desktop build bundles an APK, so macOS builders need the same Android toolchain as Linux. Use [Android Studio](https://developer.android.com/studio) for the SDK, and:

```bash
brew install openjdk@21
```

### 6. Windows (native .msi + .exe builds)

For Windows installers, build natively on Windows (not WSL2):

1. **Visual Studio Build Tools** with the **C++ desktop development** workload ([download](https://visualstudio.microsoft.com/downloads/#build-tools))
2. **Node.js >= 20** ([nodejs.org](https://nodejs.org/))
3. **Rust >= 1.80** (`winget install Rustlang.Rustup`)
4. **Perl** (`winget install StrawberryPerl.StrawberryPerl`) — required by `openssl-sys` to configure OpenSSL during Rust compilation
5. **JDK 21** (`winget install EclipseAdoptium.Temurin.21.JDK`)
6. **Android SDK** via [Android Studio](https://developer.android.com/studio) — install Platform 36, Build-Tools 36.0.0, Platform-Tools
7. **Tesseract** via vcpkg:
   ```powershell
   vcpkg install tesseract:x64-windows-static-md
   ```

Then build:

```powershell
.\build.ps1 desktop -NoSign
```

> WSL2 produces Linux packages (`.deb`, `.AppImage`), not Windows ones. Use `build.ps1` on native Windows for `.msi` and `.exe` installers.
>
> **Lighter setup**: Items 5-6 (JDK, Android SDK) are only needed if you build mobile from scratch. If you first run `./build.sh desktop` on WSL2, the pre-staged mobile artifacts are auto-detected and items 5-6 can be skipped. Item 7 (Tesseract) is still required — the desktop Rust build links against it for OCR. See [Skipping the mobile build](#skipping-the-mobile-build---skip-mobile).

---

## Development (Fast Iteration)

For day-to-day development, use the dev scripts instead of the build pipeline. No signing keys, no installer packaging — just code and see results.

### Dev commands

**Linux / macOS:**

| Command | What it does | Speed |
|---------|-------------|-------|
| `./dev.sh` | Full stack: Svelte HMR + Rust backend | Frontend <1s, Rust ~10-30s |
| `./dev.sh frontend` | Frontend only (no Rust compilation) | <1s (HMR) |
| `./dev.sh check` | Type-check Svelte + Rust (parallel) | ~25s |
| `./dev.sh test` | Run all tests (Vitest + cargo test) | ~3 min |
| `./dev.sh test:watch` | Watch mode for frontend tests | Instant re-run on save |

**Windows:**

| Command | What it does | Speed |
|---------|-------------|-------|
| `.\dev.ps1` | Full stack: Svelte HMR + Rust backend | Frontend <1s, Rust ~10-30s |
| `.\dev.ps1 frontend` | Frontend only (no Rust compilation) | <1s (HMR) |
| `.\dev.ps1 check` | Type-check Svelte + Rust | ~25s |
| `.\dev.ps1 test` | Run all tests (Vitest + cargo test) | ~3 min |
| `.\dev.ps1 test:watch` | Watch mode for frontend tests | Instant re-run on save |

### Choosing the right mode

| I'm working on... | Use | Why |
|--------------------|-----|-----|
| Svelte components, styling, i18n | `./dev.sh frontend` | No Rust needed — instant feedback |
| Frontend + Rust IPC commands | `./dev.sh` (full) | Need the backend for `invoke()` calls |
| Rust logic only | `cargo check` + `cargo test` | Skip frontend entirely |
| Quick sanity check before commit | `./dev.sh check` | Catches type errors in both layers |
| Pre-push validation | `./dev.sh test` | Runs all 1,597 tests |

### First-time dev setup

```bash
git clone https://github.com/ktiyab/coheara.git
cd coheara
./dev.sh          # Installs deps, builds i18n, starts full-stack dev server
```

The dev scripts auto-detect missing `node_modules` and i18n generated files and bootstrap them before starting.

---

## Build Commands (Production)

**Linux / macOS** (bash):

| Command | What it does | Keys required? |
|---------|-------------|----------------|
| `./build.sh desktop --no-sign` | `.deb` + `.AppImage` (Linux) or `.dmg` (macOS) | No |
| `./build.sh desktop` | Same, signed | Yes |
| `./build.sh desktop --skip-mobile` | Desktop only, use pre-staged mobile artifacts | No |
| `./build.sh android --no-sign` | Android APK only (unsigned) | No |
| `./build.sh android` | Android APK only (signed) | Yes |
| `./build.sh all` | Desktop installer + standalone APK | Yes |
| `./build.sh clean` | Remove build intermediates (see below) | No |

**Windows** (PowerShell):

| Command | What it does | Keys required? |
|---------|-------------|----------------|
| `.\build.ps1 desktop -NoSign` | `.msi` + `.exe` (NSIS) installers | No |
| `.\build.ps1 desktop` | Same, signed | Yes |
| `.\build.ps1 desktop -SkipMobile` | Desktop only, use pre-staged mobile artifacts | No |
| `.\build.ps1 android -NoSign` | Android APK only (unsigned) | No |
| `.\build.ps1 android` | Android APK only (signed) | Yes |
| `.\build.ps1 all` | Desktop installer + standalone APK | Yes |
| `.\build.ps1 clean` | Remove build intermediates (see below) | No |

**`desktop` vs `all`**: The `desktop` command already bundles an APK *inside* the desktop installer (for phone pairing). The `all` command does the same but also copies the APK to `package/` as a standalone file — useful when you want to distribute the APK separately (e.g., direct sideload without the desktop app).

### What `desktop` builds (in order)

1. **SvelteKit frontend** — `npm ci && npm run build` (~8 min)
2. **Mobile PWA** — `cd mobile && npm ci && npm run build` (~2 min)
3. **Android APK** — [Capacitor](https://capacitorjs.com/) sync + Gradle assembleRelease (~10 min first, ~2 min cached)
4. **Stage mobile resources** — copies PWA + APK into `src-tauri/resources/`
5. **Tauri desktop build** — Rust compilation + bundling (~7 min first, ~1 min cached)
6. **Collect artifacts** — copies everything to `./package/`

> Capacitor (step 3) bridges the mobile PWA into native Android/iOS projects. It's installed automatically via `npm ci` in `mobile/` — no separate install needed. The Gradle wrapper (`mobile/android/gradlew`) is committed in the repo, so you don't need a standalone Gradle installation either.

**First build takes ~30 minutes** (Rust compiles 400+ crates from scratch). Subsequent builds are much faster since Rust and Gradle cache their outputs.

### Expected output

**Linux:**
```
package/
├── Coheara_0.2.0_amd64.deb          # Debian/Ubuntu installer
├── Coheara_0.2.0_amd64.AppImage     # Portable Linux app
└── coheara-0.2.0.apk                # Android APK (only with `all`)
```

**macOS:**
```
package/
├── Coheara_0.2.0_aarch64.dmg        # macOS disk image
└── coheara-0.2.0.apk                # Android APK (only with `all`)
```

**Windows:**
```
package/
├── Coheara_0.2.0_x64-setup.exe      # NSIS installer
├── Coheara_0.2.0_x64_en-US.msi      # MSI installer
└── coheara-0.2.0.apk                # Android APK (only with `all`)
```

### Skipping the mobile build (`--skip-mobile`)

The `desktop` command normally builds mobile artifacts (PWA + APK) and bundles them inside the installer. This requires JDK 21 and the Android SDK. Use `--skip-mobile` (bash) or `-SkipMobile` (PowerShell) to skip the mobile build entirely and use **pre-staged artifacts** instead.

**Auto-detection**: If both `src-tauri/resources/mobile-pwa/` (with files) and `src-tauri/resources/mobile-apk/coheara.apk` already exist, the mobile build is skipped automatically — no flag needed.

**Cross-platform workflow (WSL2 → Windows)**:

This is the primary use case. WSL2 has the Android toolchain but produces Linux packages. Windows has MSVC but lacks JDK/Android SDK. Solution: build mobile on WSL2 first, then build the Windows installer using those pre-staged artifacts.

```bash
# Step 1: On WSL2 — build everything (stages mobile artifacts)
./build.sh desktop --no-sign

# Step 2: On PowerShell — build Windows installer (reuses staged mobile)
.\build.ps1 desktop -NoSign
# Auto-detects pre-staged artifacts, skips JDK/Android SDK requirement
```

Both environments share the same filesystem (`C:\` = `/mnt/c/`), so the staged resources from step 1 are immediately available in step 2.

---

## Security Audit

Coheara includes dependency vulnerability scanning for all three component layers: Rust crates, frontend npm packages, and mobile npm packages.

### Running the audit

**Linux / macOS:**

| Command | What it does |
|---------|-------------|
| `./audit.sh` | Full audit, writes report to `AUDIT.txt` |
| `./audit.sh --ci` | Same, exits 1 if critical/high vulnerabilities found (for CI pipelines) |
| `./audit.sh --fix` | Attempts `npm audit fix` for auto-resolvable issues |

**Windows:**

| Command | What it does |
|---------|-------------|
| `.\audit.ps1` | Full audit, writes report to `AUDIT.txt` |
| `.\audit.ps1 -CI` | Same, exits 1 if critical/high vulnerabilities found (for CI pipelines) |
| `.\audit.ps1 -Fix` | Attempts `npm audit fix` for auto-resolvable issues |

### What gets scanned

| Component | Tool | Database |
|-----------|------|----------|
| Rust crates (`src-tauri/Cargo.lock`) | `cargo audit` | [RustSec Advisory DB](https://rustsec.org/) |
| Frontend npm (`package-lock.json`) | `npm audit` | npm Registry |
| Mobile npm (`mobile/package-lock.json`) | `npm audit` | npm Registry |

`cargo-audit` is auto-installed if missing. The report is written to `AUDIT.txt` (git-ignored).

### Severity classification

| Severity | CI behavior | Action |
|----------|-------------|--------|
| Critical / High | `--ci` exits 1 | Must fix before release |
| Moderate | Warning | Review recommended |
| Low | Informational | Monitor, fix when convenient |

### Example output (AUDIT.txt)

```
SUMMARY
  Critical:  0
  High:      1
  Moderate:  2
  Low:       6
  Total:     9

VERDICT: FAIL — 0 critical + 1 high vulnerabilities require attention.
```

---

## Verifying the Build

After `./build.sh desktop --no-sign` completes, verify the artifacts work:

**Linux (.deb):**
```bash
sudo dpkg -i package/Coheara_0.2.0_amd64.deb
coheara    # or find it in your application menu
```

**Linux (.AppImage):**
```bash
chmod +x package/Coheara_0.2.0_amd64.AppImage
./package/Coheara_0.2.0_amd64.AppImage
```

**macOS (.dmg):**
Double-click the `.dmg`, drag Coheara to Applications, launch from Applications.

**Windows (.exe / .msi):**
Double-click the `.exe` (NSIS) or `.msi` installer, follow the wizard, then launch Coheara from the Start menu.

The app should open a window and display the profile picker. If it does, the build is good.

---

## Signing Keys

Signing is needed for production builds and auto-updates. For development and testing, use `--no-sign`.

### Option A: Automated (recommended)

```bash
./setup-keys.sh
```

This script:
1. Verifies `.gitignore` protects `Specs/` and `.env` (refuses to run if not)
2. Creates `Specs/build/keys/` with a defense-layer `.gitignore`
3. Generates a Tauri updater signing keypair (password-protected)
4. Generates an Android release keystore (PKCS12, RSA 2048-bit)
5. Generates an iOS distribution key + CSR
6. Writes all passwords to `.env`
7. Updates `src-tauri/tauri.conf.json` with the new public key

Safe to re-run — existing keys are never overwritten. Use `--force` to regenerate all keys (e.g., after a password compromise or if a key was generated with an empty password).

**Requirements**: `openssl`, `keytool` (from JDK 21), and `npx tauri` (available after `npm ci` in the project root).

### Option B: Manual

If you prefer to generate keys yourself:

#### 1. Create the protected directory

```bash
mkdir -p Specs/build/keys
```

Then create `Specs/build/.gitignore`:

```gitignore
# Defense layer 2 — prevent accidental commit of signing secrets
keys/
*.jks
*.keystore
*.key
*.p12
*.pem
*.b64
*.cer
*.csr
*.mobileprovision
CREDENTIALS*
.DS_Store
```

#### 2. Tauri updater signing key

```bash
npx tauri signer generate \
    -w Specs/build/keys/coheara-tauri.key \
    -p "YOUR_PASSWORD_HERE" \
    --ci
```

**Important**: Use a real password, not an empty string. Empty passwords cause signing failures at build time.

This produces:
- `coheara-tauri.key` — private key (keep secret)
- `coheara-tauri.key.pub` — public key

Update the `pubkey` field in `src-tauri/tauri.conf.json` with the content of `.key.pub`.

#### 3. Android release keystore

```bash
keytool -genkey -v \
    -keystore Specs/build/keys/coheara-release.jks \
    -storetype PKCS12 \
    -keyalg RSA \
    -keysize 2048 \
    -validity 10000 \
    -alias coheara \
    -storepass "YOUR_PASSWORD_HERE" \
    -keypass "YOUR_PASSWORD_HERE" \
    -dname "CN=Coheara, OU=Mobile, O=Coheara, L=Paris, ST=IDF, C=FR"
```

PKCS12 requires identical store and key passwords.

#### 4. iOS distribution key (optional)

```bash
openssl genrsa -out Specs/build/keys/coheara-ios-dist.key 2048

openssl req -new \
    -key Specs/build/keys/coheara-ios-dist.key \
    -out Specs/build/keys/coheara-ios-csr.pem \
    -subj "/CN=Coheara Distribution/O=Coheara/C=FR"
```

Upload the CSR to the [Apple Developer portal](https://developer.apple.com/account/resources/certificates/add) to receive a `.cer` certificate.

#### 5. Create `.env`

```bash
cat > .env << 'EOF'
# Coheara build signing passwords
TAURI_SIGNING_PRIVATE_KEY_PASSWORD=YOUR_TAURI_PASSWORD
ANDROID_KEYSTORE_PASSWORD=YOUR_ANDROID_PASSWORD
ANDROID_KEY_PASSWORD=YOUR_ANDROID_PASSWORD
ANDROID_KEY_ALIAS=coheara
EOF
```

---

## File Structure

After setup, your local-only files look like this:

```
coheara/
├── .env                          # Passwords (git-ignored)
├── dev.sh                        # Dev server — Linux/macOS (committed)
├── dev.ps1                       # Dev server — Windows (committed)
├── build.sh                      # Build script — Linux/macOS (committed)
├── build.ps1                     # Build script — Windows (committed)
├── audit.sh                      # Security audit — Linux/macOS (committed)
├── audit.ps1                     # Security audit — Windows (committed)
├── setup-keys.sh                 # Key generator (committed)
├── BUILD.md                      # This file (committed)
├── AUDIT.txt                     # Vulnerability report (git-ignored, generated)
├── package/                      # Build output (git-ignored)
│   ├── *.deb / *.AppImage / *.dmg
│   └── *.apk
└── Specs/                        # Entire directory is git-ignored
    └── build/
        ├── .gitignore            # Defense layer 2
        └── keys/
            ├── coheara-tauri.key
            ├── coheara-tauri.key.pub
            ├── coheara-release.jks
            ├── coheara-release.jks.b64
            ├── coheara-ios-dist.key
            └── coheara-ios-csr.pem
```

### Security Layers

| Layer | Mechanism | Protects |
|-------|-----------|----------|
| 1 | Root `.gitignore` excludes `Specs/` | All key files, credentials docs |
| 2 | `Specs/build/.gitignore` blocks `*.jks`, `*.key`, `*.p12`, etc. | Individual file patterns (redundant safety) |
| 3 | Root `.gitignore` excludes `.env` | Passwords |

Both layers 1 and 2 must be defeated for a key leak to occur.

---

## Credential Priority

`build.sh` loads signing passwords in this order:

1. **Environment variables** — `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`, `ANDROID_KEYSTORE_PASSWORD`, etc.
2. **`.env` file** — in the project root (written by `setup-keys.sh`)
3. **Interactive prompt** — asks for missing passwords at runtime

Key files (`*.key`, `*.jks`) are always read directly from `Specs/build/keys/`.

---

## CI/CD

The same signing keys can be used in GitHub Actions. Upload the key values as repository secrets:

| Secret | Source |
|--------|--------|
| `TAURI_SIGNING_PRIVATE_KEY` | Content of `Specs/build/keys/coheara-tauri.key` |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` from `.env` |
| `ANDROID_KEYSTORE_FILE` | Content of `Specs/build/keys/coheara-release.jks.b64` |
| `ANDROID_KEYSTORE_PASSWORD` | `ANDROID_KEYSTORE_PASSWORD` from `.env` |
| `ANDROID_KEY_ALIAS` | `coheara` |
| `ANDROID_KEY_PASSWORD` | Same as `ANDROID_KEYSTORE_PASSWORD` |

See `.github/workflows/release.yml` for the full CI pipeline.

---

## Troubleshooting

### `SDK location not found. Define a valid SDK location with ANDROID_HOME`

The Android SDK is not found. Set `ANDROID_HOME` explicitly:

```bash
export ANDROID_HOME=~/Android/Sdk
```

If you don't have the SDK installed, follow [Android SDK setup](#option-b-command-line-only-headless--wsl2) above.

You can also pass it directly to the build:

```bash
ANDROID_HOME=~/Android/Sdk ./build.sh desktop --no-sign
```

### `cargo: command not found` or `cargo metadata: No such file or directory`

Cargo is not in your PATH. This happens frequently on WSL2. Fix:

```bash
export PATH="$HOME/.cargo/bin:$PATH"
```

Add this to your `~/.bashrc` to make it permanent. `build.sh` auto-detects cargo at `~/.cargo/bin/cargo` and adds it to PATH, but you should still set it in your shell profile for other tools.

### `failed to decode secret key: incorrect updater private key password`

The Tauri signing key password doesn't match. This happens when the key was generated with an empty password. Regenerate:

```bash
./setup-keys.sh --force
```

Then update `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` in `.env` and GitHub secrets.

### `A public key has been found, but no private key`

This is expected when building with `--no-sign`. The desktop bundles (`.deb`, `.AppImage`) are still created — only the updater `.sig` files are skipped. `build.sh` handles this automatically.

### `Error configuring OpenSSL build: Command 'perl' not found`

The `openssl-sys` Rust crate requires Perl to configure OpenSSL during compilation.

**Linux/WSL2:**
```bash
sudo apt-get install -y perl libssl-dev
```

**Windows:**
```powershell
winget install StrawberryPerl.StrawberryPerl
```

> Restart your terminal after installing Perl so it appears in PATH. `build.ps1` auto-detects Strawberry Perl at `C:\Strawberry\perl\bin\` and offers to install it via winget if missing. `build.sh` includes `perl` in the auto-install package list.

### Missing system libraries on Linux

If you see errors about missing packages during the Rust build:

```bash
sudo apt-get install -y build-essential pkg-config perl \
    libgtk-3-dev libwebkit2gtk-4.1-dev \
    libappindicator3-dev librsvg2-dev patchelf libsoup-3.0-dev \
    libjavascriptcoregtk-4.1-dev tesseract-ocr libtesseract-dev \
    libleptonica-dev libclang-dev libssl-dev unzip
```

### Build seems stuck

The first Rust compilation takes ~7 minutes and shows hundreds of `Compiling ...` lines. This is normal. The Gradle build also takes ~10 minutes on first run. Subsequent builds are much faster due to caching.

### Cleaning up

If a build gets into a bad state:

```bash
./build.sh clean                  # Remove build intermediates
./build.sh desktop --no-sign      # Rebuild from scratch
```

`build.sh clean` removes:
- `build/` and `mobile/build/` (frontend outputs)
- `.svelte-kit/` and `mobile/.svelte-kit/` (SvelteKit caches)
- Gradle build outputs (`mobile/android/app/build/`)
- Staged resources (`src-tauri/resources/mobile-pwa/*`, `mobile-apk/*`)
- `package/` (collected artifacts)
- Copied keystore (`mobile/android/app/release-key.jks`)

`build.sh clean` does **NOT** remove:
- `node_modules/` — npm dependency caches (saves ~8 min on reinstall)
- `src-tauri/target/` — Rust compilation cache (saves ~7 min on recompile)

For a full clean slate:

```bash
./build.sh clean
rm -rf node_modules mobile/node_modules src-tauri/target
./build.sh desktop --no-sign
```

This forces everything to rebuild from scratch (~30 minutes).
