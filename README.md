# Coheara

Patient's Personal MedAI — a local, offline, encrypted health data system that runs on your desktop and syncs to your phone over WiFi.

Your health data never leaves your devices. No cloud. No accounts. No tracking.

## How It Works

Coheara is a **two-app system**: a desktop application that does the heavy lifting (AI, OCR, storage) and a mobile companion that puts your health data in your pocket.

```
┌──────────────────────────┐         WiFi         ┌──────────────────────────┐
│   Desktop (Tauri)        │◄═══════════════════►  │   Phone (Capacitor)      │
│                          │   encrypted sync      │                          │
│  Import documents        │                       │  View medications        │
│  OCR + AI structuring    │   REST + WebSocket    │  Check lab results       │
│  RAG chat (MedGemma)     │   X25519 key exchange │  Read alerts             │
│  Coherence detection     │   Token rotation      │  Log symptoms            │
│  Encrypted SQLite store  │                       │  Prepare for appointments│
│  Vector search (ONNX)    │                       │  Capture documents       │
└──────────────────────────┘                       └──────────────────────────┘
     Everything computed                              Reads cached data
     and stored here                                  Works offline too
```

The desktop is the brain. The phone is the window. Pair them once via QR code, then they sync automatically whenever they're on the same network.

---

## What You Can Do

- **Import documents** — prescriptions, lab reports, medical letters (PDF, images, photos)
- **AI structuring** — MedGemma extracts medications, labs, diagnoses, professionals automatically
- **Ask questions** — RAG chat with cited, safety-filtered answers grounded in your documents
- **Spot problems** — automatic conflict, duplicate, gap, and critical value detection
- **Track medications** — current and historical medications with dose and schedule
- **Log symptoms** — OLDCARTS-guided journal with temporal correlation to medications
- **Prepare for appointments** — auto-generated summaries with PDF export for your doctor
- **Browse your timeline** — interactive SVG timeline across all health events
- **Capture from phone** — photograph documents with your phone camera, send to desktop
- **Back up everything** — encrypted backup files with cryptographic erasure

---

## Stack

### Desktop

| Layer | Technology |
|-------|-----------|
| Shell | Tauri 2.10 (Rust + WebView) |
| Frontend | Svelte 5, SvelteKit 2, TailwindCSS 4 |
| Backend | Rust 1.80+ (953 tests, 0 warnings) |
| Database | SQLite (bundled via rusqlite, WAL mode) |
| Vectors | SQLite-backed cosine similarity search |
| Encryption | AES-256-GCM, PBKDF2 600K iterations, BIP39 recovery |
| AI | MedGemma 1.5 4B via Ollama (runs locally) |
| Embeddings | all-MiniLM-L6-v2 via ONNX Runtime |
| OCR | Tesseract (bundled) |
| Phone API | axum REST + WebSocket server on local WiFi |

### Mobile

| Layer | Technology |
|-------|-----------|
| Shell | Capacitor 8 (iOS + Android) |
| Frontend | Svelte 5, SvelteKit 2, TailwindCSS 4 (481 tests) |
| Auth | Face ID / fingerprint via NativeBiometric |
| Storage | Capacitor Preferences (Keychain / Keystore) |
| Camera | Capacitor Camera (document capture) |
| Privacy | PrivacyScreen (FLAG_SECURE / view hiding) |
| Integrity | Root/jailbreak detection (warning, not blocking) |
| Sync | REST + WebSocket over local WiFi |

---

## Getting Started

### Prerequisites

| Tool | Version | Install |
|------|---------|---------|
| Node.js | >= 22 | [nodejs.org](https://nodejs.org/) |
| Rust | >= 1.80 | [rustup.rs](https://rustup.rs/) |

**Platform-specific:**

| Platform | Extra dependencies |
|----------|-------------------|
| Windows | [Visual Studio Build Tools 2022](https://visualstudio.microsoft.com/visual-cpp-build-tools/) — "Desktop development with C++" workload. WebView2 is pre-installed on Windows 10 (21H2+) and 11. |
| macOS | `xcode-select --install` |
| Linux | `sudo apt-get install build-essential pkg-config libgtk-3-dev libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf libsoup-3.0-dev libjavascriptcoregtk-4.1-dev` |

**For AI features** (optional — everything else works without it):

```bash
# Install Ollama: https://ollama.com/download
ollama pull medgemma:4b
```

### Run the desktop app

```bash
git clone https://github.com/ktiyab/coheara.git
cd coheara
npm install
npm run tauri dev
```

### Run the mobile app (development)

```bash
cd mobile
npm install
npm run dev          # Web preview at localhost:1421
npm run cap:android  # Build + sync to Android Studio
npm run cap:ios      # Build + sync to Xcode
```

---

## Building for Production

### Desktop installers

```bash
npm run tauri build
```

| Platform | Output | Location |
|----------|--------|----------|
| Windows | `Coheara_0.1.0_x64-setup.exe` (NSIS) | `src-tauri/target/release/bundle/nsis/` |
| Windows | `Coheara_0.1.0_x64_en-US.msi` | `src-tauri/target/release/bundle/msi/` |
| macOS | `Coheara_0.1.0_aarch64.dmg` | `src-tauri/target/release/bundle/dmg/` |
| Linux | `coheara_0.1.0_amd64.deb` | `src-tauri/target/release/bundle/deb/` |
| Linux | `coheara_0.1.0_amd64.AppImage` | `src-tauri/target/release/bundle/appimage/` |

Build a specific format: `npm run tauri build -- --bundles nsis` (or `dmg`, `deb`, `appimage`).

### Android (Google Play)

```bash
cd mobile
npm run cap:android                     # Build web + sync
cd android && ./gradlew bundleRelease   # Signed AAB for Play Store
```

Signing requires either `android/keystore.properties` (local) or CI environment variables (`ANDROID_KEYSTORE_FILE`, `ANDROID_KEYSTORE_PASSWORD`, `ANDROID_KEY_ALIAS`, `ANDROID_KEY_PASSWORD`). See `android/keystore.properties.example`.

### iOS (App Store)

```bash
cd mobile
npm run cap:ios                         # Build web + sync
npm run cap:open:ios                    # Open in Xcode
# Xcode → Product → Archive → Distribute App → App Store Connect
```

Requires an Apple Developer account ($99/year) and signing identity. `ios/exportOptions.plist` is preconfigured for App Store distribution.

---

## Releasing

### Desktop — automated via GitHub Actions

```bash
# 1. Bump version in: tauri.conf.json, Cargo.toml, package.json, config.rs
# 2. Commit and tag
git commit -am "Release v0.2.0"
git tag v0.2.0
git push origin main --tags
```

This triggers `.github/workflows/release.yml` which builds 4 targets in parallel:

| Runner | Target | Installers |
|--------|--------|------------|
| `windows-latest` | x86_64-pc-windows-msvc | NSIS + MSI |
| `macos-latest` | aarch64-apple-darwin | DMG (Apple Silicon) |
| `macos-13` | x86_64-apple-darwin | DMG (Intel) |
| `ubuntu-22.04` | x86_64-unknown-linux-gnu | deb + AppImage |

A draft GitHub Release is created with all installers attached. The in-app auto-updater checks `latest.json` from the latest release.

### Mobile — manual store submission

| Store | Build | Submit |
|-------|-------|--------|
| Google Play | `./gradlew bundleRelease` → signed `.aab` | Upload via [Play Console](https://play.google.com/console) |
| Apple App Store | Xcode archive → `.ipa` | Upload via App Store Connect |

No CI/CD for mobile stores yet. Privacy policy and accessibility documentation are in `mobile/PRIVACY-POLICY.md` and `mobile/ACCESSIBILITY-CHECKLIST.md`.

---

## CI/CD

### On every push / PR (`.github/workflows/ci.yml`)

- `npm run check` — Svelte/TypeScript type checking
- `npm run build` — frontend build verification
- `cargo clippy -- -D warnings` — zero-warning Rust lint
- `cargo test` — 953 backend tests

### On version tag (`.github/workflows/release.yml`)

Builds all 4 desktop platforms and publishes a draft release. Optional GitHub Secrets for signing:

| Secret | Purpose |
|--------|---------|
| `TAURI_SIGNING_PRIVATE_KEY` | Sign auto-update bundles |
| `APPLE_CERTIFICATE` + `APPLE_SIGNING_IDENTITY` | macOS code signing |
| `APPLE_API_KEY` + `APPLE_API_ISSUER` | macOS notarization (Gatekeeper) |

Without secrets, everything still builds — just unsigned.

---

## Project Structure

```
coheara/
├── src/                              # Desktop frontend (Svelte 5)
│   ├── routes/                       #   SvelteKit pages
│   ├── lib/
│   │   ├── components/               #   67 components across 12 domains
│   │   ├── api/                      #   Tauri IPC wrappers
│   │   └── types/                    #   TypeScript type definitions
│   └── app.css                       #   TailwindCSS entry
├── src-tauri/                        # Desktop backend (Rust)
│   ├── src/
│   │   ├── models/                   #   Data model (18 tables, 16 enums)
│   │   ├── db/                       #   SQLite schema + repository functions
│   │   ├── crypto/                   #   AES-256-GCM, PBKDF2, BIP39 recovery
│   │   ├── pipeline/                 #   Import → OCR → Structure → Embed → Store
│   │   ├── intelligence/             #   8 coherence detectors, alert lifecycle
│   │   ├── commands/                 #   61 Tauri IPC commands
│   │   ├── api/                      #   axum REST + WebSocket for phone sync
│   │   └── sync.rs                   #   Version-based delta sync engine
│   ├── migrations/                   #   SQLite schema (6 migrations)
│   └── tauri.conf.json               #   App config + updater + bundle settings
├── mobile/                           # Phone companion (Capacitor 8)
│   ├── src/
│   │   ├── routes/                   #   8 screens (home, chat, meds, journal, ...)
│   │   ├── lib/
│   │   │   ├── components/           #   44 Svelte components
│   │   │   ├── stores/               #   11 state stores
│   │   │   ├── api/                  #   REST + WebSocket clients
│   │   │   ├── types/                #   TypeScript type definitions
│   │   │   └── utils/                #   Native providers + helpers
│   │   │       ├── capacitor-*.ts    #   6 Capacitor native bridges
│   │   │       ├── biometric.ts      #   BiometricProvider interface
│   │   │       ├── secure-storage.ts #   SecureStorageProvider interface
│   │   │       └── ...               #   Lifecycle, screenshot, integrity, camera
│   ├── android/                      #   Android platform (Gradle)
│   ├── ios/                          #   iOS platform (Xcode)
│   └── capacitor.config.ts           #   Capacitor configuration
├── .github/workflows/                # CI/CD
│   ├── ci.yml                        #   Tests on push/PR
│   └── release.yml                   #   Build + release on tag
└── package.json                      # Root dependencies
```

---

## Development Commands

### Desktop

```bash
npm run tauri dev                  # Hot-reload dev mode
npm run check                      # Svelte/TypeScript type check
npm run build                      # Build frontend (required before cargo commands)
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml -- pipeline::safety  # Specific module
```

### Mobile

```bash
cd mobile
npm run dev                        # Web preview (port 1421)
npm run check                      # Svelte/TypeScript type check
npm test                           # 481 Vitest tests
npm run cap:sync                   # Sync web assets to native platforms
npm run cap:open:android           # Open Android Studio
npm run cap:open:ios               # Open Xcode
```

### Test suite

1,434 tests across desktop and mobile:

| Suite | Tests | Scope |
|-------|-------|-------|
| Desktop (Rust) | 953 | Encryption, data model, import, OCR, structuring, storage, RAG, safety, coherence, sync, pairing, WebSocket, commands |
| Mobile (Vitest) | 481 | Stores, API clients, biometric, lifecycle, screenshot, integrity, cache, sync, safety filter, accessibility |

---

## Data Storage

All data stays on the user's machine. Nothing is sent anywhere.

| Platform | Desktop data | Mobile cache |
|----------|-------------|--------------|
| Windows | `%LOCALAPPDATA%\com.coheara.app\profiles\` | N/A |
| macOS | `~/Library/Application Support/com.coheara.app/profiles/` | N/A |
| Linux | `~/.local/share/com.coheara.app/profiles/` | N/A |
| Android | N/A | App-private Preferences (Keystore) |
| iOS | N/A | App-private Preferences (Keychain) |

Each desktop profile is an isolated encrypted directory:
- `database/coheara.db` — SQLite (encrypted at application level)
- `originals/` — imported document files
- `markdown/` — extracted structured documents
- `verification.enc` — password verification token

The phone caches a read-only snapshot of the active profile. Revoking the device pairing clears all cached data.

---

## Security

**Encryption:** AES-256-GCM with random 12-byte nonces per operation.
**Key derivation:** PBKDF2 with 600,000 iterations (SHA-256).
**Key storage:** Never written to disk — derived from password on each unlock.
**Memory safety:** `Zeroize` + `ZeroizeOnDrop` on all key material.
**Recovery:** 24-word BIP39 mnemonic (generated at profile creation).
**Device pairing:** X25519 ECDH key exchange, one-time WebSocket tickets (30s TTL), token rotation with 30s grace period.
**Phone privacy:** Face ID / fingerprint gating, screenshot prevention on sensitive screens, session timeout (5 min), root/jailbreak warning.
**Network:** Zero internet access. Desktop-to-phone sync over local WiFi only.
**Telemetry:** None. No analytics, no crash reporting, no phone-home.

---

## License

Proprietary. All rights reserved.
