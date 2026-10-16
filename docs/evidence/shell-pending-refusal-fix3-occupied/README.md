# Newly occupied recovery destination

Source commit `a643f6a56afb982bfe22621b5fe530a60c2a3778`.
Recorded merge base `7a285d85bca3da5da66990f546471d703850d812`.
Verbatim patch-id `0e514350e574844f80f14798bced42f387b573a0`.

The runtime pathspec is `src-tauri/src/storage.rs`. This record supersedes the preceding round-3 record for that file. The [preceding record](../shell-pending-refusal-fix3/README.md) still covers the other four runtime paths. The combined audit below checks both source patches and their current pathspecs.

## Behavior and test

This is SHELL-11, Act on 5's next-step guarantee. A destination can become occupied after startup. The core classification says the folder exists, so the early creation refusal updates the existing `FinishMoving.destination_exists` flag before returning the named-item error. Its warning now points to a rename step that is on screen. The same enum remains the data shape. No new recovery state, numbered step, record operation, or workspace write is added.

`src-tauri/src/recovery_tests.rs::explicit_creation_refuses_unrecognized_items_and_keeps_the_record` now asserts `destination_exists: true` after items appear at an initially absent path. The assertion failed before the fix and all 27 shell recovery cases passed afterward. Logs stay local under `.local-gate/e3-fix3/occupied-refresh-red.log` and `occupied-refresh-green.log`.

## Native proof

Synthetic root `/private/tmp/aelomi-e3-live-occupied-lxrok4vg`. Successful runs under `~/.local/state/aelomi-verify/`:

- cloud: `20261016-161754-dev-KAuA`.
- live-occupied: `20261016-161813-dev-bqCD`.

The script ended with:

```text
PASS: destination appeared after startup; refusal shows the rename step; every destination and record byte matches
```

The proof uses only synthetic overrides and the exact required keyboard-lock helper. It sends no Finder action and changes no system setting. Native captures use the Mac's current light appearance. Every destination and record byte matches after refusal, and the original folder's bytes match after its shell move. The real-data listings match.

Copy `native-proof.py.txt` to `.local-gate/e3-fix3/occupied-refresh-proof.py` and run it from the repo root. It imports the shared helpers from the preceding record, creates a fresh synthetic root, records a cloud refusal, and launches with the destination absent. It then adds the original pages folder to that destination before confirming creation. The named-item refusal must keep its error dialog, and Keep moving my pages must show the rename step. Each owned instance is cleaned up.

## Screenshots

[Three supplemental screenshots](https://github.com/aelomi/aelomi/pull/93#issuecomment-6213866926) were inspected, downloaded without authentication, decoded as PNG, and matched exactly.

| Screenshot | Bytes | SHA-256 |
|---|---:|---|
| `live-before.png` | 313259 | `71799e11a0e067decd5fe8fd23718048bbd862204b3f59c99cc6fc05aa32e53e` |
| `live-refused.png` | 398167 | `9e0b7d1d76435e36334a22ab591413f1aa590058fcd2854cbc26b0c702338adf` |
| `live-rename.png` | 483846 | `ba392643171fffbffc622f5194535f8fdb1caf3bb839724891399fee3b139d94` |

## Limits and audit

The preceding nine screenshots cover the other round-3 paths. This supplemental run covers only the destination that appears after startup. Native Finder operations, native flush timeout, and the brief's Consider and Noted items remain outside the proof. Full committed-head CI parity is reported on the PR. No performance improvement is claimed.

Run `python3 docs/evidence/shell-pending-refusal-fix3-occupied/audit-record.py.txt`. The preceding record's audit by itself now fails for storage.rs because this newer record owns that file.
