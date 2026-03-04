# Security Fix Report

Date: 2026-03-04 (UTC)
Reviewer: CI Security Reviewer

## 1) Alert Analysis

Reviewed the provided security alerts:
- Dependabot (open): `aws-lc-sys`, `wasmtime`, `time`
- Code scanning (open): `rust/cleartext-logging` in `crates/packc/src/build.rs` and `crates/packc/src/cli/inspect.rs`

Key risk themes:
- Cryptographic verification / side-channel issues in `aws-lc-sys` (high severity)
- DoS/resource exhaustion issues in older `wasmtime` (medium severity)
- Stack exhaustion DoS in `time` RFC2822 parser (medium severity)
- Potential secret data exposure in logs/console output (CodeQL high security severity)

## 2) PR Dependency Delta Check

Checked dependency-file changes against `origin/master`:
- Command: `git diff --name-only origin/master...HEAD | rg '(Cargo\\.(toml|lock)|...)'`
- Result: no dependency file changes introduced by this branch before remediation.

## 3) Applied Fixes

### A. Code scanning fixes (cleartext logging)

1. `crates/packc/src/build.rs`
- Removed secret key material from warning logs:
  - dropped structured `key = %secret_key_string(...)` field from warning events in secret-requirement aggregation/merge paths.

2. `crates/packc/src/cli/inspect.rs`
- Redacted sensitive output paths:
  - JSON output now uses `redacted_manifest_json(...)` to remove `secret_requirements` from emitted manifest JSON.
  - Warnings are passed through `redact_warnings(...)` before printing/serialization.
  - Human output warning lines now print redacted warning text when warning content appears secret-related.

### B. Dependabot lockfile fixes (example lockfile)

Updated `examples/qa-demo/.packc/pack_component/Cargo.lock`:
- `time` from `0.3.44` -> `0.3.47` (patched for GHSA-r6v5-fh4h-64xc)
- `wasmtime` from `38.0.4` -> `41.0.3` (outside vulnerable ranges for GHSA-vc8c-j3xm-xj73, GHSA-852m-cvvp-9p4w, GHSA-243v-98vx-264h)

## 4) Remaining/Open Item

`Cargo.lock` still contains:
- `aws-lc-sys = 0.37.1` (alerts #15/#16/#17 remain until upgraded to >= `0.38.0`)

Reason not auto-remediated in this CI run:
- Network/DNS is unavailable in this environment, and `cargo update` could not reach crates.io index.
- Without registry access, I could not perform a safe, resolver-validated lockfile upgrade for `aws-lc-sys`.

## 5) Validation Performed

- Confirmed modified files:
  - `crates/packc/src/build.rs`
  - `crates/packc/src/cli/inspect.rs`
  - `examples/qa-demo/.packc/pack_component/Cargo.lock`
- Confirmed current key versions:
  - `examples/.../Cargo.lock`: `time=0.3.47`, `wasmtime=41.0.3`
  - `Cargo.lock`: `aws-lc-sys=0.37.1` (pending)

## 6) Recommended Follow-up (when network access is available)

Run:
- `/home/runner/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/cargo update -p aws-lc-sys --precise 0.38.0`
- Re-run security scans and CI tests.
