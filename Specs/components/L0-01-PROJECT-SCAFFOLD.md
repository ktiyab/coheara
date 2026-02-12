# L0-01 — Project Scaffold

<!--
=============================================================================
COMPONENT SPEC — Read fully before coding. This IS the build instruction.
Engineer review: E-RS (Rust), E-DV (DevOps), E-UX (UI), E-SC (Security)
=============================================================================
-->

## Table of Contents

| Section | Offset |
|---------|--------|
| [1] Identity | `offset=22 limit=15` |
| [2] Dependencies | `offset=37 limit=12` |
| [3] Interfaces | `offset=49 limit=20` |
| [4] Implementation Steps | `offset=69 limit=120` |
| [5] Tauri Configuration | `offset=189 limit=65` |
| [6] Error Handling | `offset=254 limit=15` |
| [7] Security | `offset=269 limit=20` |
| [8] Testing | `offset=289 limit=30` |
| [9] Performance | `offset=319 limit=10` |
| [10] Open Questions | `offset=329 limit=10` |

---

## [1] Identity

**What:** Create the complete Tauri 2.x + Svelte 5 + Rust project structure from scratch. This is the skeleton that every other component builds on.

**After this session:** `cargo tauri dev` launches the application window, displays a Svelte page with "Coheara" title, and the Rust backend responds to a health-check IPC command.

**Estimated complexity:** Medium
**Dependencies:** None (this is the root)
**Enables:** Everything (L0-02, L0-03, all subsequent components)

---

## [2] Dependencies

**Incoming:** None — this is the first component.

**Outgoing:** Every other component depends on this scaffold existing.

**External tools required for development:**
- Rust toolchain (stable, latest)
- Node.js 20+ (for Svelte frontend build)
- Tauri CLI 2.x (`cargo install tauri-cli`)

---

## [3] Interfaces

This component produces no application-level interfaces. It produces the project structure and build configuration that all other components use.

**Verification interface (IPC health check):**
```rust
#[tauri::command]
fn health_check() -> String {
    "ok".to_string()
}
```

```typescript
// Frontend verification
const status = await invoke<string>('health_check');
// status === 'ok'
```

---

## [4] Implementation Steps

### Step 1: Create Tauri Project

**E-DV decision:** Use `cargo create-tauri-app` for initial scaffold, then customize. This ensures Tauri's expected structure is correct.

```bash
# From the Coheara project root
cargo create-tauri-app coheara --template svelte-ts
```

If the template doesn't support Svelte 5 + TypeScript directly, create manually:

```bash
# Alternative: manual creation
mkdir -p src-tauri/src
mkdir -p src/lib/{components,stores,api,utils}
mkdir -p src/routes
```

### Step 2: Configure Cargo.toml (Rust Backend)

**E-RS review:** Only include dependencies needed for THIS component. Other components add their own dependencies in their sessions.

```toml
# src-tauri/Cargo.toml
[package]
name = "coheara"
version = "0.1.0"
edition = "2021"
rust-version = "1.75"
description = "Coheara — Patient's Personal MedAI"

[dependencies]
tauri = { version = "2", features = ["devtools"] }
tauri-plugin-shell = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
uuid = { version = "1", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
thiserror = "2"

[build-dependencies]
tauri-build = { version = "2", features = [] }

[features]
default = ["custom-protocol"]
custom-protocol = ["tauri/custom-protocol"]

[profile.release]
strip = true
lto = true
codegen-units = 1
panic = "abort"
```

### Step 3: Create Rust Entry Point

```rust
// src-tauri/src/main.rs
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod config;

use tracing_subscriber::EnvFilter;

fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("coheara=info")),
        )
        .init();

    tracing::info!("Coheara starting");

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            commands::health_check,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Coheara");
}
```

```rust
// src-tauri/src/commands/mod.rs
#[tauri::command]
pub fn health_check() -> String {
    tracing::debug!("Health check called");
    "ok".to_string()
}
```

```rust
// src-tauri/src/config.rs
use std::path::PathBuf;

/// Application-level constants
pub const APP_NAME: &str = "Coheara";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Get the application data directory
/// ~/Coheara/ on all platforms (user-visible, per David's requirement)
pub fn app_data_dir() -> PathBuf {
    let home = dirs::home_dir().expect("Cannot determine home directory");
    home.join("Coheara")
}

/// Get the profiles directory
pub fn profiles_dir() -> PathBuf {
    app_data_dir().join("profiles")
}
```

Add `dirs` to dependencies:
```toml
dirs = "5"
```

### Step 4: Create Rust Module Stubs

Create empty module files for the project structure. These are stubs that future components will fill.

```rust
// src-tauri/src/lib.rs
pub mod commands;
pub mod config;

// These modules will be added by future component sessions:
// pub mod models;
// pub mod db;
// pub mod crypto;
// pub mod pipeline;
// pub mod intelligence;
// pub mod export;
// pub mod transfer;
```

### Step 5: Configure Svelte Frontend

```json
// package.json
{
  "name": "coheara",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "vite build",
    "preview": "vite preview",
    "check": "svelte-check --tsconfig ./tsconfig.json"
  },
  "dependencies": {
    "@tauri-apps/api": "^2"
  },
  "devDependencies": {
    "@sveltejs/vite-plugin-svelte": "^4",
    "@tailwindcss/vite": "^4",
    "svelte": "^5",
    "svelte-check": "^4",
    "tailwindcss": "^4",
    "typescript": "^5",
    "vite": "^6"
  }
}
```

### Step 6: Create Root Layout

```svelte
<!-- src/routes/+layout.svelte -->
<script lang="ts">
  // Root layout — profile guard will be added by L3-01
  // TabBar will be added by L3-02
</script>

<main class="min-h-screen bg-stone-50 text-stone-900 font-sans">
  <slot />
</main>
```

### Step 7: Create Home Page (Placeholder)

```svelte
<!-- src/routes/+page.svelte -->
<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';

  let status = 'checking...';

  onMount(async () => {
    try {
      status = await invoke<string>('health_check');
    } catch (e) {
      status = 'error: ' + String(e);
    }
  });
</script>

<div class="flex flex-col items-center justify-center min-h-screen gap-4">
  <h1 class="text-4xl font-bold text-stone-800">Coheara</h1>
  <p class="text-lg text-stone-500">Your Personal MedAI</p>
  <p class="text-sm text-stone-400">
    Backend status: <span class="font-mono">{status}</span>
  </p>
</div>
```

### Step 8: Configure TailwindCSS

```css
/* src/app.css */
@import 'tailwindcss';

/* Atkinson Hyperlegible — bundled in Phase 1 P2 (i18n)
   For now, use system font stack optimized for readability */
:root {
  --font-sans: 'Segoe UI', system-ui, -apple-system, sans-serif;
  --color-primary: #4A6FA5;    /* Calm blue — not medical-alarm blue */
  --color-surface: #FAFAF9;    /* Warm stone-50 */
  --color-text: #1C1917;       /* Stone-900 */
  --color-muted: #78716C;      /* Stone-500 */
}

/* Minimum font size enforcement (accessibility) */
html {
  font-size: 16px;
  line-height: 1.6;
}

/* Focus visible for keyboard navigation */
*:focus-visible {
  outline: 2px solid var(--color-primary);
  outline-offset: 2px;
}

/* Minimum touch target (44x44) for interactive elements */
button, a, input, select, textarea, [role="button"] {
  min-height: 44px;
  min-width: 44px;
}
```

### Step 9: Configure Vite

```typescript
// vite.config.ts
import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import tailwindcss from '@tailwindcss/vite';

const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [svelte(), tailwindcss()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? { protocol: 'ws', host, port: 1421 }
      : undefined,
  },
});
```

### Step 10: Create Build Script

```rust
// src-tauri/build.rs
fn main() {
    tauri_build::build();
}
```

---

## [5] Tauri Configuration

**E-DV + E-SC joint review:**

```json
// src-tauri/tauri.conf.json
{
  "$schema": "https://raw.githubusercontent.com/tauri-apps/tauri/dev/crates/tauri-cli/schema.json",
  "productName": "Coheara",
  "version": "0.1.0",
  "identifier": "com.coheara.app",
  "build": {
    "beforeDevCommand": "npm run dev",
    "devUrl": "http://localhost:1420",
    "beforeBuildCommand": "npm run build",
    "frontendDist": "../build"
  },
  "app": {
    "title": "Coheara",
    "windows": [
      {
        "title": "Coheara",
        "width": 1024,
        "height": 768,
        "minWidth": 800,
        "minHeight": 600,
        "resizable": true,
        "fullscreen": false
      }
    ],
    "security": {
      "csp": "default-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; connect-src 'self' http://localhost:* http://127.0.0.1:*",
      "dangerousDisableAssetCspModification": false
    }
  },
  "bundle": {
    "active": true,
    "targets": "all",
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.icns",
      "icons/icon.ico"
    ]
  }
}
```

**E-SC notes on CSP:**
- `default-src 'self'`: Only load resources from the app itself
- `connect-src` includes localhost for Ollama API and local WiFi transfer
- No external domains allowed — enforces offline-only
- `unsafe-inline` for styles is required by Svelte but limited to style tags

---

## [6] Error Handling

This component has minimal error handling — it's the scaffold. Errors at this level are fatal (app can't start).

```rust
// In main.rs: .expect() is acceptable for startup — if Tauri fails to
// initialize, there's nothing to recover to.

// The health_check command is infallible — returns a string.
// Future commands will use Result<T, String> for Tauri IPC error protocol.
```

---

## [7] Security

**E-SC review:**

| Concern | Mitigation |
|---------|-----------|
| CSP configuration | Strict CSP in tauri.conf.json. No external resources. |
| Window security | Default Tauri security model. No custom protocol vulnerabilities. |
| Dev vs Release | `devtools` feature only in dev. Release builds strip debug symbols. |
| Data directory | Created in user home (`~/Coheara/`), not system directories. No elevated permissions needed. |

---

## [8] Testing

### Acceptance Criteria

| # | Test | Expected | Status |
|---|------|----------|--------|
| T-01 | `cargo check` completes without errors | Clean compilation | |
| T-02 | `cargo clippy` reports no warnings | Clean lint | |
| T-03 | `cargo tauri dev` launches window | Window opens with Svelte content | |
| T-04 | Health check IPC works | Page shows "Backend status: ok" | |
| T-05 | `cargo test` passes | All unit tests pass | |
| T-06 | `npm run check` passes | Svelte/TypeScript type checking clean | |
| T-07 | Window meets minimum size | 800x600 minimum enforced | |
| T-08 | `config::app_data_dir()` returns valid path | Path under user home directory | |

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_check_returns_ok() {
        assert_eq!(commands::health_check(), "ok");
    }

    #[test]
    fn app_data_dir_under_home() {
        let dir = config::app_data_dir();
        let home = dirs::home_dir().unwrap();
        assert!(dir.starts_with(home));
        assert!(dir.ends_with("Coheara"));
    }

    #[test]
    fn profiles_dir_under_app_data() {
        let profiles = config::profiles_dir();
        let app = config::app_data_dir();
        assert!(profiles.starts_with(app));
        assert!(profiles.ends_with("profiles"));
    }
}
```

---

## [9] Performance

| Metric | Target |
|--------|--------|
| `cargo check` (cold) | < 60 seconds |
| `cargo tauri dev` to window | < 10 seconds |
| Binary size (debug) | < 50 MB |
| Binary size (release) | < 15 MB |

---

## [10] Open Questions

| # | Question | Status |
|---|---------|--------|
| OQ-01 | Exact Tauri 2.x version (stable vs RC?) | Check at build time |
| OQ-02 | Svelte 5 vs SvelteKit for routing | Use SvelteKit for file-based routing |
| OQ-03 | TailwindCSS v4 compatibility with Svelte 5 | Verify during setup |
