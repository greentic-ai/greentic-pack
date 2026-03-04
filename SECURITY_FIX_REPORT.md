# Security Fix Report

Date: 2026-03-04 (UTC)
Branch: `test-request-changes`
PR context: `GITHUB_REF=refs/pull/18/merge`, base `origin/master`

## Scope
Reviewed all provided Dependabot and CodeQL alerts, checked this PR for newly introduced dependency vulnerabilities, and applied minimal safe remediations that are possible in this CI environment.

## 1) Alert Analysis Summary

### Dependabot alerts reviewed
- `#17`, `#16`, `#15` (`aws-lc-sys` in `Cargo.lock`) — High severity; patched at `aws-lc-sys >= 0.38.0`.
- `#14`, `#13`, `#4` (`wasmtime` in `examples/qa-demo/.packc/pack_component/Cargo.lock`) — Medium severity; patched at `>= 40.0.4` for the affected range.
- `#6` (`time` in `examples/qa-demo/.packc/pack_component/Cargo.lock`) — Medium severity; patched at `>= 0.3.47`.

### Code scanning alerts reviewed
- `#10`, `#9`, `#8`, `#7`, `#6`, `#5`, `#4`, `#3` (`rust/cleartext-logging`) in:
  - `crates/packc/src/build.rs`
  - `crates/packc/src/cli/inspect.rs`

## 2) PR Dependency Change Check

Compared PR HEAD against merge-base with `origin/master`:
- Merge-base: `746162faf3c7d07a61e5c17cc266271652c06d06`
- Dependency/lock files changed by the PR before this remediation: **none detected**.

Conclusion: No new vulnerable dependency files were introduced by this PR itself.

## 3) Fixes Applied

### A) Cleartext logging remediation (CodeQL)

#### File: `crates/packc/src/build.rs`
- Removed secret key material from warning logs in secret-requirement handling.
- Specifically removed `key = %secret_key_string(...)` from tracing fields.

Security impact:
- Prevents secret identifiers from being written to logs.

#### File: `crates/packc/src/cli/inspect.rs`
- Added warning sanitization before output.
- Human output now redacts warning strings containing sensitive terms (`secret`, `token`, `password`, `credential`, `private key`, `api key`).
- JSON output now returns sanitized warning strings as well.

Security impact:
- Reduces plaintext leakage risk through CLI logging/output channels.

### B) Dependency remediation in example lockfile

#### File: `examples/qa-demo/.packc/pack_component/Cargo.lock`
- Updated via existing repository branch artifact (`origin/dependabot/cargo/examples/qa-demo/dot-packc/pack_component/cargo-19d6922b9f`).
- Confirmed `time` upgraded to `0.3.47` (patched).

Security impact:
- Remediates alert `#6` (`GHSA-r6v5-fh4h-64xc`, `CVE-2026-25727`).

## 4) Remaining Unresolved Alerts and Reason

### Unresolved: `aws-lc-sys` alerts `#17`, `#16`, `#15`
Current root lock version remains `0.37.1` (vulnerable range includes `< 0.38.0`).

### Unresolved: `wasmtime` alerts `#14`, `#13`, `#4`
`examples/qa-demo/.packc/pack_component/Cargo.lock` still resolves `wasmtime = 38.0.4` (vulnerable for advisories requiring `>= 40.0.4` in this range).

### Why unresolved in this CI run
- This sandbox cannot perform live `cargo update` because:
  - rustup temp path under `/home/runner/.rustup` is not writable in sandbox;
  - outbound DNS/network access to fetch toolchain/index is blocked.
- As a result, lockfile resolution for `aws-lc-sys` and `wasmtime` could not be regenerated safely in-place.

## 5) Verification Performed

- Confirmed PR context and base reference from CI env vars.
- Confirmed dependency version states from lockfiles after changes:
  - `Cargo.lock`: `aws-lc-sys = 0.37.1` (still vulnerable)
  - `examples/qa-demo/.packc/pack_component/Cargo.lock`:
    - `time = 0.3.47` (fixed)
    - `wasmtime = 38.0.4` (still vulnerable)
- Reviewed diffs for all modified source files.

## 6) Recommended Follow-up (outside this restricted CI sandbox)

1. Run `cargo update -p aws-lc-sys --precise 0.38.0` at repository root and commit regenerated `Cargo.lock`.
2. Regenerate `examples/qa-demo/.packc/pack_component/Cargo.lock` to resolve `wasmtime >= 40.0.4` (or update the upstream dependency path constraining it, then regenerate).
3. Re-run Dependabot and CodeQL scans to confirm closure of remaining alerts.
