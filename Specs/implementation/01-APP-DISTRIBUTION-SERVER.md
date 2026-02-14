# App Distribution Server — Gap Analysis & Architecture

> **Purpose**: Eliminate app store dependency for the mobile companion by making the desktop app serve installation, updates, and PWA access over the local network.
> **Recovery**: After compression, read THIS FIRST. Find first `PENDING` section and resume.
> **Cross-refs**: `00-GAP-ANALYSIS.md`, `Specs/components/M0-01-API-ROUTER.md`, `Specs/components/L4-03-WIFI-TRANSFER.md`

---

## TOC

| Section | Lines | Offset |
|---------|-------|--------|
| Restatement | 25-60 | `offset=20 limit=45` |
| Why It Matters | 62-110 | `offset=57 limit=55` |
| Current State Inventory | 112-180 | `offset=107 limit=73` |
| Gap Registry | 182-280 | `offset=177 limit=103` |
| Architecture | 282-420 | `offset=277 limit=143` |
| Platform Realities | 422-510 | `offset=417 limit=93` |
| Security Considerations | 512-580 | `offset=507 limit=73` |
| Endpoint Specifications | 582-720 | `offset=577 limit=143` |
| Implementation Plan | 722-800 | `offset=717 limit=83` |

---

## Restatement — What This Is

Coheara's core security promise is **local network isolation**: health data never leaves the patient's devices, never touches the internet, no cloud, no accounts. Yet the current system breaks this promise at the very first step — to install the mobile companion, the user must:

1. Visit Google Play Store or Apple App Store (internet required)
2. Create/use a store account (email, identity linkage)
3. Download from a centralized server (metadata exposure)
4. Trust a third-party distribution channel with a health app binary

This is a contradiction. The desktop app is the single trust anchor — the patient downloaded and installed it. **The desktop should be sufficient to bootstrap the entire ecosystem**, including serving the mobile companion directly.

The **App Distribution Server** makes the desktop the distribution point:

```
┌──────────────────────────────┐
│   Desktop (Trust Anchor)     │
│                              │
│  /install         → Landing page with platform detection
│  /install/android → Serve APK + sideload instructions
│  /app             → Full PWA (works on iOS + Android)
│  /update          → Version check + delta delivery
│                              │
│  All served over local WiFi  │
│  Self-signed TLS (already)   │
│  No internet. No store.      │
└──────────────────────────────┘
```

The user installs Coheara Desktop, unlocks a profile, and opens the companion setup. The desktop shows a QR code. The phone scans it in any browser. From that browser, the phone either:
- **(Android)**: Downloads the APK and sideloads it
- **(iOS/Android)**: Installs the PWA ("Add to Home Screen") — full offline app

---

## Why It Matters

### 1. Privacy Coherence

The app stores are metadata-rich environments. Installing Coheara from Google Play tells Google that this person uses a health AI app. Apple knows the same. This is **information leakage at the distribution layer** — the very thing Coheara exists to prevent.

Serving from the desktop eliminates this. The phone connects to a local IP over WiFi. No DNS lookup, no store API call, no account linkage.

### 2. Trust Chain Integrity

Currently: User trusts Desktop (direct download) → User must ALSO trust Store → Store delivers mobile app.

With distribution server: User trusts Desktop → Desktop delivers mobile app. **Single trust anchor.**

### 3. Independence from Store Gatekeepers

- Google can remove apps. Apple can reject updates. Both charge fees.
- Health apps face heightened scrutiny — Apple's Health-related guidelines are restrictive.
- Coheara does NOT need the store's distribution network. It already has a distribution network: the local WiFi.

### 4. Simplified User Journey

Current: Install Desktop → Open Store → Search "Coheara" → Install → Open → Scan QR.
Proposed: Install Desktop → Scan QR with phone camera → Tap "Install" → Done.

One fewer app install step. No store account needed. No search. No store page.

### 5. Update Control

Store updates are asynchronous. Users may run outdated mobile versions against a newer desktop. The App Distribution Server can:
- Detect version mismatch at pairing time
- Serve the exact compatible version
- Ensure desktop ↔ mobile version parity

---

## Current State Inventory

### What We Have (Verified Against Code)

| Asset | Location | State |
|-------|----------|-------|
| **Axum router** | `src-tauri/src/api/router.rs` | 18 protected + 1 unprotected + WS routes. API-only, no static serving. |
| **TLS certificates** | `src-tauri/src/tls_cert.rs` | Self-signed ECDSA-P256, encrypted in SQLite, cert pinning via SHA-256 fingerprint. |
| **Server startup** | `src-tauri/src/commands/pairing.rs:55` | Hardcoded `https://{local_ip}:8443`. Router composable but **no persistent server bind code in production path**. |
| **WiFi Transfer server** | `src-tauri/src/wifi_transfer.rs` | Separate HTTP server on ephemeral port. Serves self-contained HTML page. PIN auth. 5-min auto-shutdown. |
| **QR code generation** | `src-tauri/src/wifi_transfer.rs` | `generate_qr_code()` — SVG rendering. Already used by both pairing and transfer. |
| **Mobile build output** | `mobile/build/` | Static SPA (SvelteKit adapter-static). index.html + _app/ (JS/CSS). ~300-400KB compressed. |
| **Capacitor config** | `mobile/capacitor.config.ts` | webDir: `build`. 11 plugins. Android + iOS platforms. |
| **PWA support** | - | **NONE**. No manifest.json, no service worker, no offline cache. |
| **CoreState** | `src-tauri/src/core_state.rs` | Shared Arc state. Has `transfer_server` field (tokio::Mutex). |
| **DeviceManager** | `src-tauri/src/device_manager.rs` | Tracks paired devices, sessions, token rotation. |
| **API middleware** | `src-tauri/src/api/middleware/` | Rate limit, nonce, auth, audit — full stack. |

### What The WiFi Transfer Server Teaches Us

The WiFi Transfer (L4-03) is a **proven pattern** for the distribution server:
- On-demand server startup with ephemeral port
- Self-contained HTML page served from Rust (no external deps)
- QR code for phone to connect
- PIN authentication
- Local network validation (rejects public IPs)
- Auto-shutdown after inactivity
- Server handle stored in `CoreState.transfer_server`

**Key difference**: WiFi Transfer uses plain HTTP (because iOS Safari rejects self-signed certs). The API server uses HTTPS with cert pinning. The distribution server needs to handle this iOS constraint.

---

## Gap Registry

### ADS-001: Static File Serving for Mobile SPA

**Current**: Axum router is API-only. No `ServeDir`, no fallback to `index.html`.
**Needed**: Serve the mobile build output (`build/` directory) as a SPA with proper MIME types and `index.html` fallback for client-side routing.
**Impact**: Without this, `/app` cannot serve the PWA.
**Domain**: server
**Status**: **RESOLVED**

### ADS-002: PWA Manifest and Service Worker

**Current**: Mobile app has zero PWA support — no `manifest.json`, no service worker.
**Needed**: Web app manifest (name, icons, display: standalone, theme), service worker with cache-first strategy for app shell + runtime caching for API calls.
**Impact**: Without this, "Add to Home Screen" won't produce an app-like experience; no offline capability.
**Domain**: mobile-pwa
**Status**: **RESOLVED**

### ADS-003: APK Serving Endpoint

**Current**: Android APK built via Capacitor (`./gradlew assembleRelease`). No mechanism to serve the APK from the desktop.
**Needed**: Endpoint that serves the pre-built APK with correct `Content-Type: application/vnd.android.package-archive`, `Content-Disposition`, and integrity hash.
**Impact**: Without this, Android users can't sideload from the desktop.
**Domain**: server
**Status**: **RESOLVED**

### ADS-004: Installation Landing Page

**Current**: No install page. WiFi Transfer has a self-contained HTML page (upload form), but no install/download page.
**Needed**: Platform-detecting landing page that routes to APK download (Android) or PWA install instructions (iOS). Must work in any mobile browser.
**Impact**: Without this, the phone has no entry point after scanning the QR code.
**Domain**: server, ux
**Status**: **RESOLVED**

### ADS-005: Version Check and Update Endpoint

**Current**: Desktop has `tauri-plugin-updater` for self-updates. No mechanism for mobile version checking.
**Needed**: `/update` endpoint that returns current version + download URL. Mobile app (PWA or native) can check compatibility.
**Impact**: Without this, desktop and mobile versions can drift apart silently.
**Domain**: server, sync
**Status**: **RESOLVED**

### ADS-006: Distribution Server Lifecycle

**Current**: WiFi Transfer has start/stop lifecycle in CoreState. No equivalent for distribution server.
**Needed**: Distribution server that starts when a profile is unlocked and stops when locked. Persistent (not ephemeral like WiFi Transfer). May share port with API server or use its own.
**Impact**: Without this, no way to start/stop/manage the distribution server.
**Domain**: server, state
**Status**: **RESOLVED**

### ADS-007: APK Build Integration

**Current**: APK built manually via `cd mobile/android && ./gradlew assembleRelease`.
**Needed**: Pre-built APK embedded in the desktop app bundle OR built at install time. The desktop must have the APK available to serve.
**Impact**: Without this, `/install/android` has nothing to serve.
**Domain**: build, packaging
**Status**: DEFERRED (requires CI/CD integration — APK path configured, endpoint works, just needs APK file)

### ADS-008: iOS Certificate Trust Handling

**Current**: WiFi Transfer uses HTTP to avoid iOS Safari cert rejection. API server uses HTTPS with cert pinning (phone app trusts the pinned cert).
**Needed**: Distribution server must work in a **browser** (not the Coheara app). iOS Safari will reject self-signed certs. Options: HTTP for install page (like WiFi Transfer), or guide user through cert trust.
**Impact**: If HTTPS with self-signed cert, iOS users see scary warnings and may not proceed.
**Domain**: security, ux
**Status**: **RESOLVED** (distribution server uses HTTP, matching WiFi Transfer pattern)

### ADS-009: Tauri IPC Commands for Distribution

**Current**: No commands for distribution server management.
**Needed**: Commands: `start_distribution`, `stop_distribution`, `get_distribution_status`, `get_install_qr`.
**Impact**: Desktop UI can't control the distribution server.
**Domain**: commands
**Status**: **RESOLVED**

### ADS-010: Desktop UI for Companion Setup

**Current**: Pairing UI exists (QR code + approve/deny). No "Install companion" UI.
**Needed**: "Set up phone" flow in desktop UI: start distribution server → show QR → instructions → detect when phone connects → transition to pairing flow.
**Impact**: No user-facing way to trigger app distribution.
**Domain**: ux, frontend
**Status**: **RESOLVED**

---

## Architecture

### Server Topology Decision

**Option A: Separate Server** (like WiFi Transfer)
- Distribution server on its own port (ephemeral)
- Completely independent from API server
- Pro: No coupling with API middleware stack
- Con: Another port to manage, another QR code, fragmented UX

**Option B: Integrated Routes on API Server** (recommended)
- Add `/install`, `/install/android`, `/app/**`, `/update` to the existing axum router
- Unprotected routes (no Bearer auth needed — browser access)
- Rate-limited (reuse existing middleware)
- Pro: Single server, single QR code, unified lifecycle
- Con: Must handle HTTP for browser access (iOS) vs HTTPS for API

**Option C: Hybrid** — HTTP distribution server that redirects to HTTPS API after install
- Distribution routes on HTTP (port 8080) for browser compatibility
- API routes on HTTPS (port 8443) for paired device security
- Pro: Solves iOS cert trust issue cleanly
- Con: Two ports, but clear separation of concerns

### Recommended: Option C (Hybrid)

```
Phone browser scans QR → http://{local_ip}:8080/install
                          │
                          ├─ /install              → Landing page (platform detect)
                          ├─ /install/android      → APK download + sideload guide
                          ├─ /app                  → PWA (full SPA + service worker)
                          ├─ /app/**               → SPA fallback to /app/index.html
                          ├─ /update               → Version check JSON
                          └─ /health               → Server health check

After install, paired device connects to:
  https://{local_ip}:8443/api/**  → Existing API (HTTPS, cert-pinned)
```

**Rationale**:
1. The install page MUST work in a raw browser (iOS Safari, Android Chrome). Self-signed HTTPS breaks this.
2. The WiFi Transfer already proves HTTP works for browser flows on local network.
3. After installation, the Coheara app (native or PWA) can pin the self-signed cert programmatically.
4. Clean separation: HTTP = browser-facing distribution. HTTPS = app-facing API.

### Distribution Server Lifecycle

```
Profile Unlock
     │
     ▼
Start Distribution Server (HTTP :8080)
     │── Bind to local IP, ephemeral or fixed port
     │── Serve: /install, /install/android, /app/**, /update
     │── Rate limit: 30 req/min per IP
     │── Local network only (reject public IPs)
     │
     ▼
Running (persistent while profile unlocked)
     │── Desktop UI: "Companion available at http://192.168.x.x:8080"
     │── QR code always accessible in Settings > Devices
     │
     ▼
Profile Lock or App Close
     │
     ▼
Stop Distribution Server
```

### PWA Architecture

The mobile SvelteKit app already builds to static HTML/CSS/JS. To make it a PWA:

```
mobile/
├── static/
│   ├── manifest.json          ← NEW: Web app manifest
│   └── icons/                 ← NEW: PWA icons (192x192, 512x512)
├── src/
│   └── service-worker.ts      ← NEW: SvelteKit service worker
├── svelte.config.js           ← MODIFY: Enable service worker
└── build/                     ← OUTPUT: Full PWA
    ├── index.html
    ├── manifest.json
    ├── service-worker.js
    ├── icons/
    └── _app/
```

**Service Worker Strategy**:
- **App shell**: Cache-first (HTML, CSS, JS) — works offline
- **API calls**: Network-first with offline fallback to cached data
- **Images/docs**: Cache on first load, serve from cache
- **Version**: Service worker version matches desktop version for compatibility

### APK Packaging

The APK needs to be available to the desktop. Options:

**Option A**: Pre-built APK embedded in the Tauri bundle
- Built during CI/CD (`./gradlew assembleRelease`)
- Included as a Tauri resource file
- Pro: Always available, no build tools needed on user machine
- Con: Increases desktop installer size by ~15-25MB

**Option B**: APK built on demand (not recommended)
- Requires Android SDK on user machine — unrealistic

**Option C**: APK downloaded from GitHub Releases on first use
- Desktop fetches the matching version APK from releases
- Cached locally for serving
- Con: Requires internet (defeats the purpose)

**Recommended: Option A** — embed the APK in the desktop bundle.

---

## Platform Realities

### Android — Full Sideloading Support

Android allows sideloading via "Install from unknown sources" (per-app permission since Android 8.0):

1. User opens browser, navigates to `http://192.168.x.x:8080/install`
2. Page detects Android, offers APK download
3. Browser downloads APK, prompts "Allow installation from this source?"
4. User enables, installs — full native Capacitor app
5. Subsequent updates: Desktop serves new APK, app checks `/update`

**Caveats**:
- Google Play Protect may scan/warn about unknown APKs (but won't block)
- Some corporate MDM policies disable sideloading — PWA fallback needed
- Samsung has additional "Smart Manager" warnings — installable but noisy

### iOS — PWA Is The Path

iOS does NOT support sideloading (except EU DMA sideloading on iOS 17.4+ with notarized apps from authorized marketplaces — too complex for v1).

**The PWA path works well on iOS**:
1. User opens Safari, navigates to `http://192.168.x.x:8080/app`
2. Full SvelteKit app loads in browser
3. User taps Share → "Add to Home Screen"
4. PWA launches as standalone app (no browser chrome)
5. Service worker caches everything — works offline
6. Updates: Service worker detects new version, prompts refresh

**iOS PWA Capabilities** (relevant to Coheara):
- Local network requests (fetch API) — works
- WebSocket connections — works
- LocalStorage / IndexedDB — works (for caching)
- Camera access — works via `<input capture>` or MediaDevices API
- Face ID/Touch ID — NOT available in PWA (Web Authentication API only)
- Background sync — NOT available
- Push notifications — Available since iOS 16.4

**What the PWA loses vs native**:
- No biometric auth (Face ID) — use PIN/password instead
- No Keychain storage — use IndexedDB with encryption
- No background execution — acceptable for a companion viewer
- Storage limit: ~50MB for service worker cache — plenty for app shell

### Both Platforms — PWA As Universal Fallback

Even on Android, the PWA serves as a fallback for users who can't/won't enable sideloading (corporate devices, cautious users). The install page offers both options on Android, PWA-only on iOS.

---

## Security Considerations

### HTTP Distribution Server — Threat Model

The distribution server runs on HTTP (no TLS) for browser compatibility. On a local WiFi network:

| Threat | Mitigation |
|--------|------------|
| **Eavesdropping** | WiFi's WPA2/WPA3 encrypts all traffic. The distribution payload (APK/PWA) is NOT secret — it's the same binary for everyone. |
| **MITM/Injection** | APK served with SHA-256 integrity hash. PWA served with subresource integrity (SRI) on scripts. Service worker hash validation. |
| **ARP spoofing** | Local network attack. Mitigated by APK hash verification on install page. |
| **Unauthorized access** | Local network only (reject public IPs). Rate limiting. No sensitive data on install endpoints. |

**Key insight**: The distribution server serves **public artifacts** (the app binary). It contains zero patient data. The sensitive data flows through the HTTPS API server after pairing. The HTTP distribution server is no riskier than a local network printer's web UI.

### APK Integrity

```
GET /install/android
→ Response includes:
  - APK binary (Content-Type: application/vnd.android.package-archive)
  - SHA-256 hash displayed on install page
  - SHA-256 hash shown on desktop UI for user to verify
```

If the APK is signed (release signing), Android's package manager validates the signature on install. The SHA-256 is an additional out-of-band verification.

### PWA Integrity

- Service worker loaded over HTTP on local network
- All cached assets have fingerprinted filenames (SvelteKit `_app/immutable/`)
- Service worker uses cache version tied to app version
- New service worker = new version = user prompted to update

### Rate Limiting

Distribution endpoints rate-limited to prevent abuse:
- `/install`: 30 req/min per IP
- `/install/android`: 5 req/min per IP (APK is large)
- `/app/**`: 60 req/min per IP (SPA needs multiple asset fetches)
- `/update`: 10 req/min per IP

---

## Endpoint Specifications

### `GET /install` — Landing Page

**Purpose**: Platform-detecting entry point. First thing the phone sees after scanning QR.

**Response**: Self-contained HTML page (all CSS/JS inline, no external deps).

**Behavior**:
1. Detect platform via User-Agent
2. Android: Show "Install Coheara Companion" button → links to `/install/android`
3. iOS: Show "Open Coheara Companion" button → links to `/app`
4. Desktop/Unknown: Show message "Open this page on your phone"

**Content**:
- App name, version, icon
- Platform-specific instructions with illustrations
- Desktop verification hash (visible on both desktop and phone for cross-check)
- Link to PWA for all platforms as alternative

### `GET /install/android` — APK Download Page

**Purpose**: Serve the Android APK with sideloading instructions.

**Response**: HTML page with download button and step-by-step guide.

**Flow**:
1. Display APK version, size, SHA-256 hash
2. "Download" button triggers APK download
3. Step-by-step sideload instructions with screenshots:
   - "Open the downloaded file"
   - "If prompted, allow installation from your browser"
   - "Tap Install"
4. After install: "Open Coheara and scan the QR code to pair"

**APK Download**:
```
GET /install/android/coheara.apk
Content-Type: application/vnd.android.package-archive
Content-Disposition: attachment; filename="Coheara-{version}.apk"
Content-Length: {size}
X-Content-Hash: SHA256:{hash}
```

### `GET /app` and `GET /app/**` — PWA

**Purpose**: Serve the full mobile SvelteKit app as a Progressive Web App.

**Behavior**:
- `/app` → `build/index.html`
- `/app/_app/**` → Static assets (JS, CSS)
- `/app/manifest.json` → Web app manifest
- `/app/service-worker.js` → Service worker
- Any other `/app/**` → Fallback to `build/index.html` (SPA routing)

**Headers**:
```
Cache-Control: public, max-age=31536000, immutable  (for _app/immutable/)
Cache-Control: no-cache                              (for index.html, manifest, sw)
Service-Worker-Allowed: /app/
```

### `GET /update` — Version Check

**Purpose**: Mobile app checks if an update is available.

**Response**:
```json
{
  "version": "0.2.0",
  "min_compatible": "0.1.0",
  "android": {
    "url": "/install/android/coheara.apk",
    "hash": "sha256:abc123...",
    "size": 15728640
  },
  "pwa": {
    "url": "/app",
    "sw_version": "0.2.0"
  },
  "desktop_version": "0.2.0"
}
```

### `GET /health` — Server Health

**Purpose**: Quick check that the distribution server is running.

**Response**:
```json
{
  "status": "ok",
  "version": "0.2.0",
  "profile_active": true
}
```

---

## Implementation Plan

### Phase 1: Distribution Server Core (Rust)

| ID | Task | Domain | Deps |
|----|------|--------|------|
| ADS-006 | Distribution server lifecycle (start/stop, CoreState integration) | server | - |
| ADS-001 | Static file serving for mobile SPA (tower-http ServeDir) | server | ADS-006 |
| ADS-004 | Install landing page (self-contained HTML, platform detection) | server | ADS-006 |
| ADS-008 | HTTP serving for browser compatibility (iOS cert trust) | security | ADS-006 |

### Phase 2: PWA Support (Mobile + Rust)

| ID | Task | Domain | Deps |
|----|------|--------|------|
| ADS-002 | PWA manifest + service worker + icons | mobile-pwa | - |
| ADS-005 | Version check endpoint | server | ADS-006 |

### Phase 3: Android APK Distribution

| ID | Task | Domain | Deps |
|----|------|--------|------|
| ADS-003 | APK serving endpoint with integrity hash | server | ADS-006 |
| ADS-007 | APK build integration (embed in Tauri bundle or CI artifact) | build | ADS-003 |

### Phase 4: Desktop Integration

| ID | Task | Domain | Deps |
|----|------|--------|------|
| ADS-009 | Tauri IPC commands for distribution server | commands | ADS-006 |
| ADS-010 | Desktop UI for companion setup flow | frontend | ADS-009 |

### Estimated Effort

| Phase | Effort | Tests |
|-------|--------|-------|
| Phase 1 | 2-3 days | ~15-20 tests |
| Phase 2 | 1-2 days | ~10-15 tests |
| Phase 3 | 1 day | ~5-8 tests |
| Phase 4 | 1-2 days | ~5-10 tests |
| **Total** | **5-8 days** | **~35-53 tests** |

---

## Open Questions

1. **Port selection**: Fixed port (e.g., 8080) or ephemeral? Fixed is more predictable for QR codes but may conflict. Ephemeral requires dynamic QR.
2. **APK signing**: Should the distribution APK be debug-signed (simpler) or release-signed (requires keystore in CI)?
3. **PWA scope**: Should the PWA be served at `/app` (under distribution server) or at root `/` with the API under `/api`?
4. **Auto-start**: Should the distribution server auto-start when the profile unlocks, or require explicit user action?
5. **Desktop server**: The axum API server startup is composable (`mobile_api_router()`) but there's no persistent bind in the production code path (only in tests). Should we formalize the API server lifecycle as part of this work?

---

## Implementation Log

| Date | IDs | Tests Added | Total Tests | Notes |
|------|-----|-------------|-------------|-------|
| 2026-02-14 | ADS-001,003,004,005,006,008 | +22 | 975 Rust / 481 mobile | distribution.rs: server lifecycle, landing page, APK serving, PWA static file serving, version check, health. 22 unit tests. Deps: tower-http 0.5, tower 0.4, mime_guess 2. |
| 2026-02-14 | ADS-009 | +0 | 975 Rust / 481 mobile | 4 Tauri IPC commands: start_distribution, stop_distribution, get_distribution_status, get_install_qr. Registered in lib.rs. |
| 2026-02-14 | ADS-010 | +0 | 975 Rust / 481 mobile | CompanionSetup.svelte (idle/serving/error states, QR, status polling). Types + API layer (TypeScript). |
| 2026-02-14 | ADS-002 | +0 | 975 Rust / 481 mobile | PWA: manifest.json, service-worker.ts (cache-first app shell, network-first API), icons (192+512px), app.html meta tags. |

---

## Progress

```
Total gaps: 10 (ADS-001 through ADS-010)
Resolved: 9/10 (ADS-001 through ADS-006, ADS-008 through ADS-010)
Deferred: 1/10 (ADS-007 — APK build integration, needs CI/CD)
```

## Decision Log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-02-14 | Option C: Hybrid HTTP + HTTPS topology | HTTP distribution server (browser-compatible) separate from HTTPS API server (cert-pinned). WiFi Transfer proves HTTP works for browser flows. |
| 2026-02-14 | Ephemeral port (not fixed 8080) | Avoids port conflicts on user machines. QR code carries the full URL. |
| 2026-02-14 | tower-http 0.5 (not 0.6) | Compatible with axum 0.7 tower 0.4 ecosystem already in use. |
| 2026-02-14 | SvelteKit native service worker (not vite-plugin-pwa) | SvelteKit detects src/service-worker.ts automatically. Simpler, no extra deps. |
| 2026-02-14 | Deferred ADS-007 | APK embedding requires CI/CD pipeline changes and Android SDK build. Endpoint works — just needs the file. |
