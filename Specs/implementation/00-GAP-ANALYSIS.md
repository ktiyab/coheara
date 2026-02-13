# Coheara — Production Gap Analysis

> **Purpose**: Master inventory of gaps between current codebase and shippable desktop + mobile apps.
> **Recovery**: After compression, read THIS FIRST → find first `PENDING` → resume.
> **Cross-refs**: Specs (components/), Reviews (review/), Resolution (resolution/)

---

## TOC

| Section | Lines | Offset |
|---------|-------|--------|
| Executive Summary | 25-50 | `offset=20 limit=30` |
| Gap Registry | 52-200 | `offset=47 limit=155` |
| Phase Plan | 202-260 | `offset=197 limit=63` |
| Implementation Log | 262-320 | `offset=257 limit=63` |

---

## Executive Summary

**Current state**: 35/35 components implemented, 917 Rust + 481 mobile tests, 0 warnings.
**Gap**: Code-complete but not shippable. Desktop AI pipeline uses mocks; mobile has no native framework.

### Verified Reality (Code Inspection, Not Review Docs)

| Area | Verified State | Impact |
|------|---------------|--------|
| LLM (Ollama) | REAL — OllamaClient via HTTP | RAG works when Ollama running |
| Embeddings | MOCK ONLY — MockEmbedder | No semantic search capability |
| Vector Store | MOCK ONLY — InMemoryVectorStore | Chunks lost on restart |
| OCR | REAL — behind `ocr` feature flag | Works when flag enabled |
| Mobile Framework | NO CAPACITOR — web SPA only | Cannot install on phone |
| Native Providers | 5 MOCK interfaces, 0 real | No biometric/camera/storage |
| Alert Sync | BUG — returns dismissed, not active | Phone misses active alerts |
| Audit Persistence | BUG — flush_to_db never called | Audit data lost on crash |
| Pairing Flow | FIXED — split-approval pattern | No race condition |
| Auth Lockout | FIXED — 10/15min per-source | Working |
| Rate Limiting | FIXED — 5 req/min on pairing | Working |
| Lab Trends | FIXED — trend_direction field present | Working |
| ProfileChanged | FIXED — triggered on profile ops | Working |
| Icons | READY — full suite present | No action needed |
| CI/CD | READY — release.yml for 4 platforms | Desktop builds configured |

---

## Gap Registry

### P0: Critical Code Bugs (Fix Immediately)

| ID | Title | Domain | File | Impact | Status |
|----|-------|--------|------|--------|--------|
| IMP-001 | Alert assembly returns dismissed instead of active | sync | `src-tauri/src/sync.rs:374-444` | Phone misses active health alerts | **RESOLVED** |
| IMP-002 | Audit log flush_to_db never called | audit | `src-tauri/src/core_state.rs` | Audit data lost on crash; compliance gap | **RESOLVED** |

### P1: Production Pipeline (Desktop AI)

| ID | Title | Domain | File | Impact | Status |
|----|-------|--------|------|--------|--------|
| IMP-003 | Implement ONNX embedding model | embedding | `src-tauri/src/pipeline/storage/embedder.rs` | No semantic search; RAG returns only structured data | **RESOLVED** |
| IMP-004 | Implement persistent vector store | vectordb | `src-tauri/src/pipeline/storage/vectordb.rs` | Chunks stored in memory only; lost on restart | **RESOLVED** |
| IMP-005 | Enable OCR feature by default | ocr | `src-tauri/Cargo.toml` | OCR disabled unless explicit `--features ocr` | **RESOLVED** |
| IMP-006 | Wire production pipeline in chat | pipeline | `src-tauri/src/commands/chat.rs` | Chat uses mock pipeline; no real RAG | **RESOLVED** |

### P2: Mobile Native Foundation

| ID | Title | Domain | File | Impact | Status |
|----|-------|--------|------|--------|--------|
| IMP-007 | Initialize Capacitor framework | mobile-infra | `mobile/` | No native deployment possible | PENDING |
| IMP-008 | Implement CapacitorBiometricProvider | mobile-native | `mobile/src/lib/utils/biometric.ts` | No Face ID / fingerprint auth | PENDING |
| IMP-009 | Implement CapacitorSecureStorage | mobile-native | `mobile/src/lib/utils/secure-storage.ts` | Tokens stored in memory; lost on kill | PENDING |
| IMP-010 | Implement CapacitorLifecycleListener | mobile-native | `mobile/src/lib/utils/lifecycle.ts` | No foreground/background/network detection | PENDING |
| IMP-011 | Implement CapacitorCamera | mobile-native | `mobile/src/routes/more/capture/+page.svelte` | No document photography | PENDING |
| IMP-012 | Implement ScreenshotPrevention plugin | mobile-native | `mobile/src/lib/utils/screenshot.ts` | Health data screenshottable | PENDING |
| IMP-013 | Implement DeviceIntegrity plugin | mobile-native | `mobile/src/lib/utils/device-integrity.ts` | No root/jailbreak detection | PENDING |

### P3: Desktop Polish

| ID | Title | Domain | File | Impact | Status |
|----|-------|--------|------|--------|--------|
| IMP-014 | First-run onboarding flow | ux | `src/routes/+page.svelte` | No guidance for new users | PENDING |
| IMP-015 | Graceful degradation when Ollama missing | ux | `src-tauri/src/commands/chat.rs` | Hard error if Ollama not running | PENDING |
| IMP-016 | Auto-updater plugin | infra | `src-tauri/Cargo.toml` | No update mechanism | PENDING |
| IMP-017 | Tested installer builds | qa | `.github/workflows/release.yml` | Untested installers | PENDING |

### P4: Cross-Device Integration

| ID | Title | Domain | File | Impact | Status |
|----|-------|--------|------|--------|--------|
| IMP-018 | End-to-end pairing over real WiFi | integration | Desktop + Mobile | Never tested cross-device | PENDING |
| IMP-019 | Sync reliability under real data | integration | `src-tauri/src/sync.rs` | Only tested with fixtures | PENDING |
| IMP-020 | Reconnection resilience | integration | `src-tauri/src/api/websocket.rs` | No backoff/retry on disconnect | PENDING |

### P5: App Store Readiness

| ID | Title | Domain | File | Impact | Status |
|----|-------|--------|------|--------|--------|
| IMP-021 | Android app signing (keystore) | security | `mobile/android/` | Cannot publish to Play Store | PENDING |
| IMP-022 | iOS provisioning profiles | security | `mobile/ios/` | Cannot publish to App Store | PENDING |
| IMP-023 | Privacy policy & data handling | compliance | N/A | Required for store submission | PENDING |
| IMP-024 | Accessibility verification on devices | qa | Both apps | WCAG AAA unverified on real hardware | PENDING |

---

## Phase Plan

### Phase 1: Critical Fixes (IMP-001, IMP-002)
**Risk**: Health-safety (alerts) + compliance (audit). Fix before any other work.
**Effort**: ~2 hours. Pure code fixes with tests.
**Dependencies**: None.

### Phase 2: Production Pipeline (IMP-003 through IMP-006)
**Risk**: External native dependencies (ort, lancedb) may have build complexity on WSL2.
**Strategy**: Implement behind feature flags; graceful fallback if deps unavailable.
**Effort**: ~3-5 days. Heavy native integration.
**Dependencies**: ort, tokenizers, lancedb, arrow crates.

### Phase 3: Mobile Foundation (IMP-007 through IMP-013)
**Risk**: Requires Node.js toolchain + Android SDK + iOS SDK (Xcode on macOS only).
**Strategy**: Start with Capacitor init + Android (testable on WSL2 via ADB). iOS requires macOS.
**Effort**: ~3-5 days.
**Dependencies**: Capacitor 6, Android SDK, Xcode (iOS only).

### Phase 4: Desktop Polish (IMP-014 through IMP-017)
**Risk**: Low. Incremental UX improvements.
**Effort**: ~2-3 days.
**Dependencies**: Phase 2 (graceful degradation depends on knowing what's available).

### Phase 5: Cross-Device Integration (IMP-018 through IMP-020)
**Risk**: Requires two physical devices on same network.
**Effort**: ~2-3 days.
**Dependencies**: Phases 1-3 complete.

### Phase 6: Store Submission (IMP-021 through IMP-024)
**Risk**: External process (Apple review, Google review).
**Effort**: ~1-2 days technical + review wait time.
**Dependencies**: All prior phases.

---

## Implementation Log

| Date | IMP-IDs | Tests Added | Total Tests | Notes |
|------|---------|-------------|-------------|-------|
| 2026-02-13 | IMP-005, IMP-006 | +2 | 933 Rust / 481 mobile | OCR default; chat wired to SqliteVectorStore + conditional OnnxEmbedder + Box<dyn> blanket impl |
| 2026-02-12 | IMP-004 | +8 | 931 Rust / 481 mobile | SQLite vector store: persistent chunks + search + migration 006 |
| 2026-02-12 | IMP-003 | +0 | 923 Rust / 481 mobile | ONNX embedder: ort v2 + tokenizers behind onnx-embeddings flag |
| 2026-02-12 | IMP-002 | +1 | 923 Rust / 481 mobile | Audit auto-flush: threshold-based + auto-lock + manual lock |
| 2026-02-12 | IMP-001 | +5 | 922 Rust / 481 mobile | Fixed alert assembly: queries coherence_alerts (active) + dismissed_alerts (history) |
| (starting) | — | — | 917 Rust / 481 mobile | Gap analysis complete |

---

## Progress

```
Total gaps: 24 (P0: 2, P1: 4, P2: 7, P3: 4, P4: 3, P5: 4)
Resolved: 6/24 (IMP-001, IMP-002, IMP-003, IMP-004, IMP-005, IMP-006)
In Progress: 0/24
Phase 2 (Production Pipeline): COMPLETE ✓
```
