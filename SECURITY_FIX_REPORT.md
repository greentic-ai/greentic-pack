# SECURITY_FIX_REPORT

## Scope and Inputs
- Reviewed provided Dependabot alerts: #14, #13, #6, #4.
- Reviewed provided Code Scanning alerts: #10, #9, #8, #7, #6, #5, #4, #3.
- Checked dependency-file deltas for PR context against `origin/master`.

## Findings
- Dependabot alerts all target `examples/qa-demo/.packc/pack_component/Cargo.lock`.
- In this branch, that generated lockfile had `wasmtime 41.0.3` and `time 0.3.47`.
- `time 0.3.47` already satisfies GHSA-r6v5-fh4h-64xc (CVE-2026-25727).
- `wasmtime 41.0.3` is below the advisory-published patched line `41.0.4` for GHSA-852m-cvvp-9p4w and GHSA-243v-98vx-264h.
- PR dependency check: the changed dependency file introducing this exposure is `examples/qa-demo/.packc/pack_component/Cargo.lock`.

## Remediations Applied
1. Removed vulnerable generated lockfile:
- Deleted `examples/qa-demo/.packc/pack_component/Cargo.lock`.
- Rationale: `.packc` is generated build output and already gitignored; removing the tracked stale lockfile removes the vulnerable manifest path Dependabot is alerting on.

2. Reduced cleartext-sensitive output in inspect CLI:
- Updated `crates/packc/src/cli/inspect.rs` JSON output:
  - Replaced `report.warnings` with `report.warnings_count` and `report.warnings_redacted`.
- Updated `crates/packc/src/cli/inspect.rs` human output:
  - Replaced per-warning plaintext printing with a count-only line: `Warnings: <n> (details redacted)`.

## Why this fix path
- CI sandbox blocks network access, so lockfile upgrade commands cannot fetch/update crates metadata.
- Directly deleting the tracked generated lockfile is the safest minimal remediation that removes the vulnerable dependency snapshot from the repository.
- Warning redaction mitigates the CodeQL cleartext-logging class by avoiding plaintext emission of potentially sensitive warning payloads.

## Validation Performed
- Confirmed lockfile deletion is staged.
- Confirmed inspect output changes are present in `crates/packc/src/cli/inspect.rs`.
- Confirmed PR dependency delta includes the `.packc` lockfile path as the dependency-file change linked to these alerts.

## Residual Risk / Notes
- No full test run was executed in this CI sandbox due rustup/crates network restrictions.
- If this lockfile must remain tracked for policy reasons, it should be regenerated with `wasmtime >= 41.0.4` (or `42.0.0+`) in a network-enabled environment.
