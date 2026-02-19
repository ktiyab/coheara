# Coheara Security Audit

Coheara handles encrypted patient health records. A vulnerable dependency anywhere in the stack — Rust crates, frontend npm, or mobile npm — could compromise data confidentiality. These audit scripts scan all three layers for known CVEs before every release.

---

## Quick Start

```bash
# Linux / macOS
./audit.sh

# Windows (PowerShell)
.\audit.ps1
```

Results are written to `AUDIT.txt` (git-ignored). The console shows a color-coded summary.

---

## What Gets Scanned

| # | Component | Lock File | Tool | Advisory Database |
|---|-----------|-----------|------|-------------------|
| 1 | **Rust crates** | `src-tauri/Cargo.lock` | `cargo audit` | [RustSec Advisory DB](https://rustsec.org/) |
| 2 | **Frontend npm** | `package-lock.json` | `npm audit` | npm Registry |
| 3 | **Mobile npm** | `mobile/package-lock.json` | `npm audit` | npm Registry |

All three are scanned in sequence. If `cargo-audit` is not installed, it is installed automatically via `cargo install cargo-audit`.

---

## Commands

### Linux / macOS (`audit.sh`)

```bash
./audit.sh              # Full audit, report to AUDIT.txt
./audit.sh --ci         # Exit 1 on critical/high (CI gate)
./audit.sh --fix        # Attempt npm audit fix after scanning
./audit.sh --help       # Show usage
```

### Windows (`audit.ps1`)

```powershell
.\audit.ps1             # Full audit, report to AUDIT.txt
.\audit.ps1 -CI         # Exit 1 on critical/high (CI gate)
.\audit.ps1 -Fix        # Attempt npm audit fix after scanning
.\audit.ps1 -Help       # Show usage
```

### Flags

| Flag | Bash | PowerShell | Behavior |
|------|------|------------|----------|
| CI gate | `--ci` | `-CI` | Exits with code 1 if any critical or high vulnerability is found. Use in CI pipelines to block releases. |
| Auto-fix | `--fix` | `-Fix` | After scanning, runs `npm audit fix` on frontend and mobile packages that have resolvable issues. Does not affect Rust crates (no equivalent auto-fix). |

---

## Severity Classification

| Severity | Console Color | CI Gate (`--ci`) | Recommended Action |
|----------|---------------|------------------|--------------------|
| **Critical** | Red | Exits 1 | Fix immediately. Block release. |
| **High** | Red | Exits 1 | Fix before release. Evaluate exploitability. |
| **Moderate** | Yellow | Passes (warning) | Review and fix when practical. |
| **Low** | Cyan | Passes (info) | Monitor. Fix when convenient. |

The `--ci` flag only fails on **critical** and **high**. Moderate and low findings are reported but do not block the pipeline.

---

## Output Format

### Console

```
Coheara Security Audit
Mode: Report  Fix: false

==> Auditing Rust dependencies (cargo audit)
[OK]    Rust: No known vulnerabilities found

==> Auditing FRONTEND npm dependencies
[INFO]  FRONTEND: 5 vulnerabilities (critical=0, high=0, moderate=2, low=3)

==> Auditing MOBILE npm dependencies
[WARN]  MOBILE: 4 vulnerabilities (critical=0, high=1, moderate=0, low=3)
[ERROR] FAIL: 0 critical + 1 high vulnerabilities found

==> Audit report written to: ./AUDIT.txt
```

### AUDIT.txt

The report file contains the full output from each tool, followed by an aggregated summary:

```
================================================================================
COHEARA SECURITY AUDIT REPORT
================================================================================
Date:     2026-02-18 04:01:10 UTC
Host:     SKY
Platform: Linux x86_64
================================================================================

────────────────────────────────────────────────────────────────────────────────
RUST DEPENDENCIES (cargo audit)
────────────────────────────────────────────────────────────────────────────────

    Fetching advisory database from `https://github.com/RustSec/advisory-db.git`
    ...
    [full cargo audit output]

RESULT: CLEAN — No known vulnerabilities

────────────────────────────────────────────────────────────────────────────────
FRONTEND NPM DEPENDENCIES (npm audit)
────────────────────────────────────────────────────────────────────────────────

    [full npm audit output]

RESULT: 5 vulnerabilities (critical=0, high=0, moderate=2, low=3)

────────────────────────────────────────────────────────────────────────────────
MOBILE NPM DEPENDENCIES (npm audit)
────────────────────────────────────────────────────────────────────────────────

    [full npm audit output]

RESULT: 4 vulnerabilities (critical=0, high=1, moderate=0, low=3)

────────────────────────────────────────────────────────────────────────────────
SUMMARY
────────────────────────────────────────────────────────────────────────────────

  Critical:  0
  High:      1
  Moderate:  2
  Low:       6
  ─────────────────
  Total:     9

VERDICT: FAIL — 0 critical + 1 high vulnerabilities require attention.
```

---

## CI Integration

### GitHub Actions

Add a security audit step to your workflow:

```yaml
- name: Security audit
  run: ./audit.sh --ci
```

The `--ci` flag ensures the workflow fails if critical or high vulnerabilities are found. Combine with scheduled runs to catch newly disclosed CVEs:

```yaml
on:
  schedule:
    - cron: '0 8 * * 1'  # Every Monday at 08:00 UTC
  push:
    branches: [main]

jobs:
  security-audit:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with: { node-version: 22 }
      - uses: dtolnay/rust-toolchain@stable
      - run: npm ci
      - run: cd mobile && npm ci
      - run: ./audit.sh --ci
```

### Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Clean, or only moderate/low findings |
| 1 | Critical or high vulnerability found (with `--ci` flag only) |

Without `--ci`, the script always exits 0 regardless of findings. This allows running audits for reporting without blocking local development.

---

## Fixing Vulnerabilities

### npm (frontend + mobile)

```bash
# Auto-fix resolvable issues
./audit.sh --fix

# Or manually in each directory
npm audit fix                    # Safe fixes only
npm audit fix --force            # Breaking changes allowed (review first)
```

### Rust crates

There is no `cargo audit fix`. Instead:

1. Check if a patched version exists: look at the `Solution:` line in the audit output
2. Update the dependency in `src-tauri/Cargo.toml`
3. Run `cargo update -p <crate_name>` to update just that crate
4. Re-run `./audit.sh` to verify

For transitive dependencies (crates you don't directly depend on), the fix usually requires waiting for an upstream update. Common categories:

| Category | Example | Action |
|----------|---------|--------|
| **Vulnerability with fix** | `transpose >= 0.2.3` | Update dependency |
| **Unmaintained crate** | GTK3 bindings (`RUSTSEC-2024-0413`) | Upstream (Tauri) must migrate; monitor |
| **Unsound API** | `glib VariantStrIter` (`RUSTSEC-2024-0429`) | Avoid the unsafe API; wait for upstream fix |

---

## Understanding the Report

### Rust advisories

Each advisory includes:

```
Crate:     transpose
Version:   0.1.0
Title:     Buffer overflow due to integer overflow
Date:      2023-12-18
ID:        RUSTSEC-2023-0080
URL:       https://rustsec.org/advisories/RUSTSEC-2023-0080
Solution:  Upgrade to >=0.2.3
Dependency tree:
  transpose 0.1.0
  └── rustfft 3.0.1
      └── img_hash 3.2.0
          └── coheara 0.2.0
```

The dependency tree shows the path from your project to the vulnerable crate. Direct dependencies are fixable in `Cargo.toml`. Transitive dependencies require upstream updates.

### npm advisories

```
cookie  <0.7.0
cookie accepts cookie name, path, and domain with out of bounds characters
fix available via `npm audit fix --force`
Will install @sveltejs/kit@0.0.30, which is a breaking change
```

When `npm audit fix --force` warns about breaking changes, test thoroughly after applying.

---

## Prerequisites

| Tool | Required By | Auto-installed? |
|------|-------------|-----------------|
| `cargo` | Rust audit | No — must be pre-installed |
| `cargo-audit` | Rust audit | Yes — installed automatically if missing |
| `npm` | npm audit | No — must be pre-installed |
| `python3` | JSON parsing (Linux only) | No — typically pre-installed on Linux |

### Cargo path

On WSL2, cargo may not be in the default PATH. Pass it explicitly:

```bash
CARGO=/root/.cargo/bin/cargo ./audit.sh
```

Or ensure `~/.cargo/bin` is in your PATH (see [BUILD.md](BUILD.md#1-nodejs-and-rust)).

---

## Files

| File | Committed | Purpose |
|------|-----------|---------|
| `audit.sh` | Yes | Linux/macOS audit script |
| `audit.ps1` | Yes | Windows audit script |
| `AUDIT.txt` | No (git-ignored) | Generated report — local artifact |
| `AUDIT.md` | Yes | This documentation |
