# SECURITY_FIX_REPORT

## Scope
- Reviewed provided Dependabot alerts: #14, #13, #6, #4.
- Reviewed provided CodeQL alerts: #10, #9, #8, #7, #6, #5, #4, #3.
- Checked PR-style dependency deltas against `origin/master...HEAD`.

## Alert Analysis

### Dependabot
- Alerts #14 and #13 (`wasmtime`) require patched versions at/above `41.0.4` for the affected major line.
- Alert #6 (`time`) is fixed in `0.3.47`.
- Alert #4 (`wasmtime` AVX issue) is fixed in `>=40.0.3` / `>=41.0.1`; this repo already exceeds that specific floor where `41.x` is used.

### Code Scanning (cleartext logging)
- Existing branch already redacts warnings in `inspect` output.
- Additional sensitive-field reduction was still needed in `build` logging around secret requirements.

## PR Dependency-Change Check
- Compared `origin/master...HEAD`.
- Dependency-file delta in this PR context was primarily the generated `.packc` lockfile path under `examples/qa-demo/.packc/pack_component/`.
- No newly introduced vulnerable dependency version was found in active tracked lockfiles from this branch diff.

## Fixes Applied
1. Pinned Wasmtime to patched line in manifests:
- Updated [Cargo.toml](/home/runner/work/greentic-pack/greentic-pack/Cargo.toml) to:
  - `wasmtime = "41.0.4"`
  - `wasmtime-wasi = "41.0.4"`
- Updated [crates/vendor/greentic-flow/Cargo.toml](/home/runner/work/greentic-pack/greentic-pack/crates/vendor/greentic-flow/Cargo.toml) to:
  - `[dependencies.wasmtime] version = "41.0.4"`

2. Reduced secret-derived logging fields:
- Updated [crates/packc/src/build.rs](/home/runner/work/greentic-pack/greentic-pack/crates/packc/src/build.rs) to remove secret-key material from `tracing::warn!` fields:
  - removed `key = %secret_key_string(&req)`
  - removed `key = %secret_key_string(base)`

## Validation Performed
- Confirmed manifest pins are present via diff.
- Confirmed secret-key log fields are removed in `build.rs`.
- Confirmed `.packc` lockfiles are gitignored (`.gitignore` contains `**/.packc/`).

## Constraints / Follow-up
- This CI environment cannot access `index.crates.io` (DNS/network blocked), so lockfile regeneration/update commands could not be executed.
- Required follow-up in a network-enabled runner:
  1. regenerate/update relevant `Cargo.lock` files,
  2. ensure resolved `wasmtime >= 41.0.4` (or newer patched line),
  3. rerun Dependabot/CodeQL scans to verify alert closure.
