# Coheara

Patient's Personal MedAI — a local, offline, encrypted desktop application that helps patients understand and manage their health documents.

All data stays on the patient's computer. No cloud. No tracking. No internet required.

## Stack

| Layer | Technology |
|-------|-----------|
| Desktop shell | Tauri 2.10 |
| Frontend | Svelte 5, SvelteKit 2, TailwindCSS 4 |
| Backend | Rust 1.80+ |
| Structured data | SQLite (bundled via rusqlite) |
| Vector search | LanceDB |
| Encryption | AES-256-GCM, PBKDF2 600K iterations |
| AI inference | MedGemma 1.5 4B via Ollama (local) |
| Embeddings | all-MiniLM-L6-v2 via ONNX Runtime |

## Features

- **Document import** — Load prescriptions, lab reports, medical letters (PDF, images)
- **OCR & structuring** — Extract medications, labs, diagnoses, professionals via MedGemma
- **RAG chat** — Ask questions about your health data with grounded, cited, safe answers
- **Coherence detection** — Automatic conflict, duplicate, gap, and critical value alerts
- **Medication list** — Current and historical medications with dose tracking
- **Symptom journal** — OLDCARTS-guided recording with temporal correlation
- **Appointment prep** — Auto-generated summaries for doctor visits with PDF export
- **Timeline view** — SVG-based interactive timeline across all health events
- **WiFi transfer** — Phone-to-desktop document transfer via QR code and PIN
- **Backup & restore** — Encrypted backup files with cryptographic erasure
- **Privacy verification** — Inspectable proof that everything runs offline

---

## Prerequisites

### All platforms

| Tool | Version | Install |
|------|---------|---------|
| Rust | >= 1.80 | [rustup.rs](https://rustup.rs/) |
| Node.js | >= 20 | [nodejs.org](https://nodejs.org/) |

### Windows

Install [Visual Studio Build Tools 2022](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with the "Desktop development with C++" workload.

WebView2 is pre-installed on Windows 10 (21H2+) and Windows 11.

### macOS

```bash
xcode-select --install
```

### Linux (Ubuntu/Debian)

```bash
sudo apt-get update
sudo apt-get install -y \
  build-essential pkg-config \
  libgtk-3-dev libwebkit2gtk-4.1-dev libappindicator3-dev \
  librsvg2-dev patchelf libsoup-3.0-dev libjavascriptcoregtk-4.1-dev
```

### AI features (optional)

Install [Ollama](https://ollama.com/download), then pull the model:

```bash
ollama pull medgemma:4b
```

Without Ollama, document storage, viewing, timeline, journal, and medications still work. Only AI chat, medical structuring, and coherence detection require the model.

---

## Setup

```bash
# Clone the repository
git clone https://github.com/ktiyab/coheara.git
cd coheara

# Install frontend dependencies
npm install

# Run in development mode (hot-reload)
npm run tauri dev
```

This starts the Vite dev server, compiles the Rust backend, and opens the app window.

---

## Development

### Project structure

```
coheara/
├── src/                          # Svelte frontend
│   ├── routes/                   #   SvelteKit pages
│   ├── lib/
│   │   ├── components/           #   67 Svelte components across 12 domains
│   │   ├── api/                  #   10 Tauri invoke wrapper modules
│   │   ├── types/                #   10 TypeScript type modules
│   │   └── utils/                #   Shared utilities
│   └── app.css                   #   TailwindCSS entry
├── src-tauri/                    # Rust backend
│   ├── src/
│   │   ├── lib.rs                #   Module declarations + Tauri handler registration
│   │   ├── config.rs             #   App configuration
│   │   ├── models/               #   Data model (enums, entities)
│   │   ├── db/                   #   SQLite (schema, repository functions)
│   │   ├── crypto/               #   Encryption (AES-GCM, PBKDF2, recovery)
│   │   ├── pipeline/             #   Import, OCR, structuring, storage, RAG, safety
│   │   ├── intelligence/         #   Coherence engine (8 detectors, alert store)
│   │   ├── commands/             #   61 Tauri IPC command handlers
│   │   └── {feature}.rs          #   Feature modules (home, chat, review, etc.)
│   ├── migrations/               #   SQLite schema (001_initial.sql)
│   ├── Cargo.toml                #   Rust dependencies
│   └── tauri.conf.json           #   Tauri configuration
├── .github/workflows/            # CI/CD
│   ├── ci.yml                    #   Tests on push/PR
│   └── release.yml               #   Build + release on tag
└── package.json                  # Frontend dependencies
```

### Commands

```bash
# Development
npm run tauri dev              # Hot-reload dev mode

# Frontend checks
npm run check                  # Svelte/TypeScript type check
npm run build                  # Build frontend to build/

# Backend checks (run npm run build first)
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml

# Run specific test module
cargo test --manifest-path src-tauri/Cargo.toml -- trust::tests

# Production build (current platform)
npm run tauri build            # Builds installer for your OS
```

### Test suite

633 tests covering all 20 components:

| Layer | Tests | Coverage |
|-------|-------|----------|
| L0 Foundation | 56 | Scaffold, data model, encryption |
| L1 Pipeline | 30+ | Import, OCR, structuring, storage |
| L2 Intelligence | 154 | RAG, safety filter, coherence engine |
| L3 Interface | 42 | Review screen, medication list |
| L4 Value | 102 | Journal, appointment, WiFi transfer, timeline |
| L5 Trust | 27 | Emergency protocol, backup, dose plausibility |

### Architecture conventions

- **Rust errors:** `thiserror` enums, `.map_err(|e| e.to_string())` at IPC boundary
- **Database:** Function-based repository (free functions taking `&Connection`), WAL mode
- **Encryption:** All profile data encrypted at rest, `ProfileKey` with `ZeroizeOnDrop`
- **Svelte 5:** `$props()`, `$state()`, `$derived()` — no legacy `export let`
- **Components:** `onNavigate` callback pattern, local state only (no global stores)
- **IPC:** `invoke<T>()` wrappers in `src/lib/api/`, typed with `src/lib/types/`

---

## Build & Distribution

### Production build

```bash
npm run tauri build
```

Outputs per platform:

| Platform | Installer | Location |
|----------|-----------|----------|
| Windows | `Coheara_0.1.0_x64-setup.exe` (NSIS) | `src-tauri/target/release/bundle/nsis/` |
| Windows | `Coheara_0.1.0_x64_en-US.msi` | `src-tauri/target/release/bundle/msi/` |
| macOS | `Coheara_0.1.0_aarch64.dmg` | `src-tauri/target/release/bundle/dmg/` |
| Linux | `coheara_0.1.0_amd64.deb` | `src-tauri/target/release/bundle/deb/` |
| Linux | `coheara_0.1.0_amd64.AppImage` | `src-tauri/target/release/bundle/appimage/` |

### Build specific target

```bash
npm run tauri build -- --bundles nsis       # Windows NSIS only
npm run tauri build -- --bundles dmg        # macOS DMG only
npm run tauri build -- --bundles deb        # Linux deb only
npm run tauri build -- --bundles appimage   # Linux AppImage only
```

### Release

```bash
# Update version in: tauri.conf.json, Cargo.toml, package.json, config.rs
git commit -am "Release v0.2.0"
git tag v0.2.0
git push origin main --tags
# GitHub Actions builds all platforms and creates a draft release
```

---

## CI/CD

### Continuous integration (`.github/workflows/ci.yml`)

Runs on every push to `main` and on pull requests:

- **Frontend:** `npm run check` (svelte-check) + `npm run build`
- **Backend:** `cargo clippy -D warnings` + `cargo test` (633 tests)

### Release pipeline (`.github/workflows/release.yml`)

Triggered by pushing a `v*` tag. Builds in parallel:

| Runner | Target | Output |
|--------|--------|--------|
| `windows-latest` | `x86_64-pc-windows-msvc` | NSIS + MSI installers |
| `macos-latest` | `aarch64-apple-darwin` | DMG (Apple Silicon) |
| `macos-13` | `x86_64-apple-darwin` | DMG (Intel) |
| `ubuntu-22.04` | `x86_64-unknown-linux-gnu` | deb + AppImage |

Creates a draft GitHub Release with all installers attached.

### Optional secrets (GitHub repo settings)

| Secret | Purpose |
|--------|---------|
| `TAURI_SIGNING_PRIVATE_KEY` | Sign update bundles (for auto-updater) |
| `APPLE_CERTIFICATE` | macOS code signing |
| `APPLE_SIGNING_IDENTITY` | macOS Developer ID |

No secrets are required for unsigned builds.

---

## Data Storage

| Platform | Location |
|----------|----------|
| Windows | `%LOCALAPPDATA%\com.coheara.app\profiles\` |
| macOS | `~/Library/Application Support/com.coheara.app/profiles/` |
| Linux | `~/.local/share/com.coheara.app/profiles/` |

Each profile is an isolated encrypted directory containing:
- `database/coheara.db` — SQLite database (encrypted at application level)
- `originals/` — Imported document files
- `markdown/` — Extracted structured documents
- `lancedb/` — Vector embeddings for semantic search
- `verification.enc` — Password verification token

---

## Security

- **Encryption:** AES-256-GCM with random 12-byte nonces per operation
- **Key derivation:** PBKDF2 with 600,000 iterations (SHA-256)
- **Key storage:** Never written to disk — derived from password on each unlock
- **Memory safety:** `Zeroize` + `ZeroizeOnDrop` on all key material
- **Recovery:** 24-word BIP39 mnemonic phrase (generated at profile creation)
- **Erasure:** Cryptographic erasure — delete encrypted files, zero key in memory
- **Network:** Zero network access. Verify by enabling airplane mode.
- **Telemetry:** None. No analytics, no crash reporting, no phone-home.

---

## License

Proprietary. All rights reserved.
