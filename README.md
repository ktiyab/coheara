# Coheara

Your Personal MedAI

You collect prescriptions, lab reports, discharge summaries, and medical letters from every doctor you see. They pile up: paper in folders, PDFs in downloads, photos on your phone. No single person sees the complete picture. Not even your doctor.

When things fall through the cracks (a medication conflict between two specialists, a lab result that contradicts a diagnosis, a dosage change you were never told about) you are the one who pays.

**Coheara changes that.** It is a personal medical AI that runs entirely on your computer. Import your documents. Coheara reads them, structures them, and helps you understand your care.

- Ask questions in your own words and get cited, grounded answers
- Spot inconsistencies across doctors, medications, and lab results, turning confusion into clear questions for your next appointment
- Prepare for appointments with organized summaries and the right questions
- Track medications, symptoms, and health events over time

When you walk into the doctor's office, your smartphone MedAI companion carries your personal Vault with you: medications, alerts, summaries, all synchronized over local WiFi. No cloud. No account. Just your personal devices talking to each other.

**What Coheara never does:** diagnose, prescribe, or give medical advice. It has comprehension authority, not clinical authority. When it finds something worth discussing, it says: *"Ask your doctor about this."*

Your professionals bring clinical judgment. You bring understanding and the right questions. Coheara is the bridge that makes the conversation productive for both sides.

```
                 Health Professional
                  clinical judgment
                 ▲                 ▲
                /                   \
        better /                     \ better
     encounter/                       \encounter
              /                         \
         You ◄───────────────────────────► Coheara
    your questions            comprehension + preparation
              \                         /
               \       personal        /
                └──────► Vault ◄──────┘
```

---

## Who It's For

### Managing your own health

Whether you take 7 medications from 3 doctors and forget why you take half of them, or you track a chronic condition and want full data visibility, Coheara works for you. Take a photo of your prescription and ask *"what does this mean?"* or dig into medication timelines, lab trends, and document search. Simple by default, detailed on demand. At the pharmacy or the clinic, pull up your full medication list on your phone instead of trying to remember 7 drug names.

### Caring for someone else

If you coordinate a parent's care across multiple specialists, you know what happens when two doctors prescribe conflicting medications without knowing about each other. Coheara lets you manage another person's documents in a separate encrypted profile, detect conflicts, and print appointment summaries to bring to every visit. When you are with the person you care for, your phone gives you instant access to their alerts, medication history, and what changed since the last appointment.

### Privacy without compromise

Coheara runs offline, encrypts each profile with AES-256-GCM, stores nothing in the cloud, makes zero network calls, and collects zero telemetry. Every privacy claim is architecturally enforced and verifiable: no accounts, no tracking, no phone-home. The phone syncs over local WiFi only, locks behind biometrics, and if you revoke the pairing, all cached data is erased.

The desktop is where you prepare. The phone is where you show up informed.

---

## How It Works

Coheara is a **two-app system**: a desktop application that does the heavy lifting (AI, OCR, storage) and a mobile companion that puts your health data in your pocket.

```
┌──────────────────────────┐         WiFi         ┌──────────────────────────┐
│   Desktop (Tauri)        │◄═══════════════════►  │   Phone (PWA/Capacitor)  │
│                          │   encrypted sync      │                          │
│  Import documents        │                       │  View medications        │
│  OCR + AI structuring    │   REST + WebSocket    │  Check lab results       │
│  RAG chat (MedGemma)     │   X25519 key exchange │  Read alerts             │
│  Coherence detection     │   Token rotation      │  Log symptoms            │
│  Encrypted SQLite store  │                       │  Prepare for appointments│
│  Vector search (ONNX)    │   ┌──────────────┐    │  Capture documents       │
│  Distribution server ────│──►│ Install page  │    │                          │
└──────────────────────────┘   │ QR code scan  │    └──────────────────────────┘
     Everything computed       │ APK / PWA     │       Reads cached data
     and stored here           └──────────────┘        Works offline too
```

The desktop is the brain. The phone is the window. Install the companion by scanning a QR code from the desktop, no app store needed. Pair once, then they sync automatically whenever they share a network.

---

## What You Can Do

- **Import documents**: prescriptions, lab reports, medical letters (PDF, images, photos)
- **AI structuring**: MedGemma extracts medications, labs, diagnoses, and professionals automatically
- **Ask questions**: RAG chat with cited, safety-filtered answers grounded in your documents
- **Spot problems**: automatic conflict, duplicate, gap, and critical value detection
- **Track medications**: current and historical medications with dose and schedule
- **Log symptoms**: OLDCARTS-guided journal with temporal correlation to medications
- **Prepare for appointments**: auto-generated summaries with PDF export for your doctor
- **Browse your timeline**: interactive SVG timeline across all health events
- **Capture from phone**: photograph documents with your phone camera, send to desktop
- **Install from desktop**: serve the phone companion directly over WiFi (QR code, APK or PWA, no app store)
- **Back up everything**: encrypted backup files with cryptographic erasure

---

## Stack

### Desktop

| Layer | Technology |
|-------|-----------|
| Shell | Tauri 2.10 (Rust + WebView) |
| Frontend | Svelte 5, SvelteKit 2, TailwindCSS 4 |
| Backend | Rust 1.80+ (975 tests, 0 warnings) |
| Database | SQLite (bundled via rusqlite, WAL mode) |
| Vectors | SQLite-backed cosine similarity search |
| Encryption | AES-256-GCM, PBKDF2 600K iterations, BIP39 recovery |
| AI | MedGemma 1.5 4B via Ollama (runs locally) |
| Embeddings | all-MiniLM-L6-v2 via ONNX Runtime |
| OCR | Tesseract (bundled) |
| Phone API | axum REST + WebSocket server on local WiFi |
| Distribution | HTTP server for companion app install (APK + PWA) |

### Mobile

| Layer | Technology |
|-------|-----------|
| Shell | Capacitor 8 (iOS + Android) or PWA (any browser) |
| Frontend | Svelte 5, SvelteKit 2, TailwindCSS 4 (481 tests) |
| Auth | Face ID / fingerprint via NativeBiometric |
| Storage | Capacitor Preferences (Keychain / Keystore) |
| Camera | Capacitor Camera (document capture) |
| Privacy | PrivacyScreen (FLAG_SECURE / view hiding) |
| Integrity | Root/jailbreak detection (warning, not blocking) |
| Sync | REST + WebSocket over local WiFi |
| PWA | Service worker, offline cache, manifest |

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
| Windows | [Visual Studio Build Tools 2022](https://visualstudio.microsoft.com/visual-cpp-build-tools/): "Desktop development with C++" workload. WebView2 is pre-installed on Windows 10 (21H2+) and 11. |
| macOS | `xcode-select --install` |
| Linux | `sudo apt-get install build-essential pkg-config libgtk-3-dev libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf libsoup-3.0-dev libjavascriptcoregtk-4.1-dev` |

**For AI features** (optional, everything else works without it):

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

### Install the phone companion

Open Settings, then Companion Setup in the desktop app. Tap "Start Distribution Server", then scan the QR code with your phone.

- **Android**: downloads and installs the APK directly (enable "Install unknown apps" when prompted)
- **iOS**: opens a PWA that installs to the home screen (Safari, Share, Add to Home Screen)

No app store account required.

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

### Mobile: Direct Install (recommended)

```bash
cd mobile
npm run build                           # Build PWA (output: build/)
```

Place `build/` contents at `~/Coheara/mobile-pwa/` and (optionally) a signed APK at `~/Coheara/mobile-apk/coheara.apk`. The desktop app's distribution server serves these to phones on the local network.

### Android (Google Play, optional)

```bash
cd mobile
npm run cap:android                     # Build web + sync
cd android && ./gradlew bundleRelease   # Signed AAB for Play Store
```

Signing requires either `android/keystore.properties` (local) or CI environment variables (`ANDROID_KEYSTORE_FILE`, `ANDROID_KEYSTORE_PASSWORD`, `ANDROID_KEY_ALIAS`, `ANDROID_KEY_PASSWORD`). See `android/keystore.properties.example`.

### iOS (App Store, optional)

```bash
cd mobile
npm run cap:ios                         # Build web + sync
npm run cap:open:ios                    # Open in Xcode
# Xcode > Product > Archive > Distribute App > App Store Connect
```

Requires an Apple Developer account ($99/year) and signing identity. `ios/exportOptions.plist` is preconfigured for App Store distribution.

For most users, the direct install via QR code from the desktop is the simplest path.

---

## Releasing

### Desktop: automated via GitHub Actions

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

### Mobile: direct install (primary) or store submission

| Channel | Build | Distribute |
|---------|-------|------------|
| Direct install | `npm run build` in `mobile/` | Desktop serves via QR code |
| Google Play | `./gradlew bundleRelease`, signed `.aab` | Upload via [Play Console](https://play.google.com/console) |
| Apple App Store | Xcode archive, `.ipa` | Upload via App Store Connect |

Direct install is the recommended path: no developer accounts, no store review, no internet required. Privacy policy and accessibility documentation are in `mobile/PRIVACY-POLICY.md` and `mobile/ACCESSIBILITY-CHECKLIST.md`.

---

## CI/CD

### On every push / PR (`.github/workflows/ci.yml`)

- `npm run check`: Svelte/TypeScript type checking
- `npm run build`: frontend build verification
- `cargo clippy -- -D warnings`: zero-warning Rust lint
- `cargo test`: 975 backend tests

### On version tag (`.github/workflows/release.yml`)

Builds all 4 desktop platforms and publishes a draft release. Optional GitHub Secrets for signing:

| Secret | Purpose |
|--------|---------|
| `TAURI_SIGNING_PRIVATE_KEY` | Sign auto-update bundles |
| `APPLE_CERTIFICATE` + `APPLE_SIGNING_IDENTITY` | macOS code signing |
| `APPLE_API_KEY` + `APPLE_API_ISSUER` | macOS notarization (Gatekeeper) |

Without secrets, everything still builds, just unsigned.

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
│   │   ├── pipeline/                 #   Import > OCR > Structure > Embed > Store
│   │   ├── intelligence/             #   8 coherence detectors, alert lifecycle
│   │   ├── commands/                 #   65 Tauri IPC commands
│   │   ├── api/                      #   axum REST + WebSocket for phone sync
│   │   ├── distribution.rs           #   App Distribution Server (APK + PWA over WiFi)
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
│   ├── src/service-worker.ts          #   PWA service worker (offline cache)
│   ├── static/manifest.json          #   PWA manifest
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

1,456 tests across desktop and mobile:

| Suite | Tests | Scope |
|-------|-------|-------|
| Desktop (Rust) | 975 | Encryption, data model, import, OCR, structuring, storage, RAG, safety, coherence, sync, pairing, WebSocket, distribution, commands |
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
- `database/coheara.db`: SQLite (encrypted at application level)
- `originals/`: imported document files
- `markdown/`: extracted structured documents
- `verification.enc`: password verification token

The phone caches a read-only snapshot of the active profile. Revoking the device pairing clears all cached data.

---

## Security

**Encryption:** AES-256-GCM with random 12-byte nonces per operation.
**Key derivation:** PBKDF2 with 600,000 iterations (SHA-256).
**Key storage:** Never written to disk. Derived from password on each unlock.
**Memory safety:** `Zeroize` + `ZeroizeOnDrop` on all key material.
**Recovery:** 24-word BIP39 mnemonic (generated at profile creation).
**Device pairing:** X25519 ECDH key exchange, one-time WebSocket tickets (30s TTL), token rotation with 30s grace period.
**Phone privacy:** Face ID / fingerprint gating, screenshot prevention on sensitive screens, session timeout (5 min), root/jailbreak warning.
**Network:** Zero internet access. Desktop-to-phone sync over local WiFi only.
**App distribution:** HTTP (not HTTPS) on an ephemeral port. The distribution server only serves public install artifacts (APK, PWA assets), never patient data. Local WiFi with WPA2/WPA3 provides transport encryption. Per-IP rate limiting prevents abuse.
**Telemetry:** None. No analytics, no crash reporting, no phone-home.

---

## License

Proprietary. All rights reserved.
