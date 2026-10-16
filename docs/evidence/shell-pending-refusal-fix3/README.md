# Pending cloud-refusal guard, fix round 3

The lock path in the historical scripts is an old orchestrator-private path. Use `.claude/skills/verify-aelomi/scripts/ui-lock.sh` instead.

Source commit `1926783281ebce819db36ab7a9e7eeb0bc2645a3`.
Recorded merge base `7a285d85bca3da5da66990f546471d703850d812`.
Verbatim patch-id `f767ef88d303583fd908f4da94bf610f23adf405`.

This record supersedes fix round 2 for `crates/core/src/storage/store.rs`, `src-tauri/src/storage.rs`, `src-tauri/src/recovery.rs`, `src/components/storage-refusal.tsx`, and `src/storage-copy.ts`. Earlier records remain historical evidence.

## Data shape and scope

`DestinationFolder` remains the core classification for absent, empty, occupied, arrived, and unrecognized destinations. Recoverable marker leftovers and torn first root frames belong to its empty skeleton. No new recovery state or numbered step is added.

This round implements SHELL-11's five Act on items. A failed create reclassifies the destination. A landed confirmed root frame opens normally. Otherwise the record stays and the UI reads the refreshed status before returning to the move instructions. Unrecognized startup destinations report UnknownFormat and keep the record. The rename step checks its own exact path, opens its parent through a separate chip, and distinguishes that folder from Show in Finder's selected folder. The occupied-location warning says that the named items may be pages and points to the rename step. Refreshed instructions keep the error dialog open because an unchanged title does not take focus again.

## Environment and recipe

macOS 27.0.1, build 26A434, arm64. All launches use this worktree's `app.sh launch dev`, explicit synthetic `AELOMI_DATA_DIR`, `AELOMI_RECOVERY_DIR`, and `AELOMI_CLOUD_ROOTS`, and dev port 55393. Native captures use only the Mac's current light appearance. No Finder action, system setting, or system prompt is used. The exact required `ui-lock.sh acquire|release` helper owns every press, focus, and screenshot. Every owned instance is cleaned up. An accessibility read after path-chip focus failed, so the final rename capture uses a 960 by 850 owned app window without path-chip focus. The failed-create screen and a subsequent guarded relaunch cover that scenario.

Create `.local-gate/e3-fix3`. Copy `native-proof.py.txt` there as `native-proof.py`. Create a fresh `/tmp/aelomi-e3-fix3-*` directory with `mktemp -d` and write its canonical absolute path to `.local-gate/e3-fix3/proof-root`. List the real data folder read-only before the first launch and after the last, then run `python3 .local-gate/e3-fix3/native-proof.py` from the repository root. The script prints each run directory and saves its exact-byte assertions under the synthetic root. Use a fresh root for a complete rerun. `occupied-proof.py.txt` records the continuation used after the failed doctor read.

The publication case denies `delete_child` only on its synthetic destination. This permits marker creation and writing but refuses the marker rename, leaving a complete `format.json.tmp`. Removing that ACL permits retry. The initialization case makes only its synthetic `docs/` read-only after startup has classified an absent destination. Shell `mv` operations handle the moved-folder case. The original cloud fixture comes from `fixtures/data-folders/format-1`.

Copy `stress.mjs.txt` outside the checkout and pass it to `.claude/skills/verify-aelomi/scripts/mock-drive.mjs --flow <file> --rebuild`. Its injected Demo data / Worst case toggle never ships.

## Regression tests

All 33 focused Rust cases and all five focused WebKit cases passed. Red and green logs are local under `.local-gate/e3-fix3/`.

- `crates/core/src/storage/tests/destination.rs` adds `failed_marker_publication_leaves_a_retryable_empty_destination`, `torn_first_root_frame_is_retryable_without_changing_it_during_classification`, `blocking_names_exclude_metadata_and_publication_skeleton`, and `an_empty_frame_can_land_after_initialization_flush_reports_pending`.
- `src-tauri/src/recovery_tests.rs` strengthens `failed_empty_initialization_keeps_the_record_guards_relaunch_and_allows_retry`, restores `failed_workspace_opens_retain_the_matching_record` and `docs_without_marker_reports_unknown_format_without_changing_destination_or_record`, and adds `a_landed_empty_frame_after_creation_error_returns_ready`.
- `src/storage-copy.test.ts` checks the exact rename step for both destination-exists values.
- `tests/playwright/pending-refusal.spec.ts` adds `SHELL-11: failed creation refreshes the destination before returning to the move steps` and updates `SHELL-11: a non-empty destination keeps moving controls and names the blocking items`.

The core pending-flush test uses a real SimFs write with delayed full sync. The shell test injects the uncertain error after a real stored empty frame, then checks that startup reclassification returns Ready and clears the record. Native proof uses publication and initialization permission failures. It does not force a real-app flush timeout.

## Native results

All requested native paths passed. The synthetic root is `/private/tmp/aelomi-e3-fix3-qr2klxpy`. Exact assertions passed for the original fixture bytes, retained record bytes, broken-marker destination bytes, and occupied destination bytes. The before/after real-data listings match.

- PASS: marker publication rename failed with Permission denied, left format.json.tmp, retained record, retry Ready, original source bytes equal
- PASS: creation failed, record retained, refreshed rename step and both chips visible, source bytes equal
- PASS: correctly moved folder with broken marker reports UnknownFormat, exact record and destination bytes equal
- PASS: warning names only Aelomi Notes, says these may be pages, points to rename step, exact record and destination bytes equal

The native receipts include:

```text
recovery new workspace status=Ready
storage status=FinishMoving { destination_exists: false, ... }
storage status=UnknownFormat
AXStaticText "Permission denied (os error 13)"
AXStaticText "could not finish creating the empty workspace"
```

The skeleton error was captured in its original launch. The rename screenshot was captured on a guarded relaunch after a path-focus accessibility read failed. One occupied-cloud launch failed the doctor status-region read and was cleaned up. Its stored refusal then guarded the successful occupied launch. The published captures all have successful doctor checks and complete accessibility sidecars.

Successful native run directories under `~/.local/state/aelomi-verify/`:

- full-cloud: `20261016-155120-dev-HiWo`.
- full-retry: `20261016-155132-dev-MYKi`.
- skeleton-cloud-final: `20261016-155524-dev-Elt2`.
- skeleton-failure-final: `20261016-155538-dev-vblk`.
- skeleton-rename-final: `20261016-155902-dev-TCOm`.
- broken-marker: `20261016-160019-dev-nRG7`.
- occupied-refusal: `20261016-160231-dev-dhs7`.

The final browser stress run is `~/.local/state/aelomi-verify/20261016-103428-mock-LJ5uAW`. It rebuilt the mock bundle from the recorded source.

## Screenshots

[Nine native screenshots](https://github.com/aelomi/aelomi/pull/93#issuecomment-6213602026) were read back, downloaded without authentication, decoded as PNG, and matched byte size and SHA-256 with the inspected originals. No screenshot PNG or raw log is committed.

| Screenshot | Bytes | SHA-256 |
|---|---:|---|
| `temp-before.png` | 307878 | `545f4dcd47bdab4f5ce46e59ebb2eee10afb2d273c76ba14e7cde7eb48712ef2` |
| `temp-create-failed.png` | 351174 | `e191bacf25463825c190777ab07e0f2a5be8ce89cf907d1012d8bc4fe8e8a14a` |
| `temp-retry-ready.png` | 85122 | `aa0f9a387daaf0fa675848d50644f96707b36c004c180a313aef441b5009a043` |
| `skeleton-before.png` | 313300 | `ac55e7097740aa9a075e13f02a0cd24433edb00e55749c0c96366990da99d7cd` |
| `skeleton-create-failed.png` | 380794 | `e9427e3a08d2c1210eed6ee9f6d67c264bf7c8a4637726fbb656eda05b9b9c29` |
| `rename-paths.png` | 426740 | `48f07bca77233c6999095f2ccdef0a8c9c7a9878e9005c364535fee1c9eca855` |
| `broken-marker-unknown.png` | 90357 | `ae849865c167fb45b6345a04731638b460f0f90264b40977d10216ad4c7fe128` |
| `occupied-before.png` | 306179 | `e725ec86b0034144b12369efae3a446bce2250f773aa950901cace0fb85bca35` |
| `occupied-message.png` | 370594 | `a7e8d9dfc64e4c71d6399e591ce5af1e8ce81ebeb594593319e5d6120ddb120a` |

## Limits

Finder movement and native reveal receipt remain unproved because Finder is reserved for the maintainer. The review gate and independent verifier remain the orchestrator's work. Consider and Noted items stay deferred, including partial transfers, the unreadable-record move sequence, large-text dialog scrolling, raw engine strings, the unchanged reassurance after a failed create, and second-rename collisions.

The browser stress run passed both themes, widths 320, 960, and 1440, normal and doubled text, and long multilingual paths with emoji. Chips did not clip or overflow horizontally. All eight saved screenshots were inspected. At 320 px with doubled text, dialog paths need body scrolling, as already deferred to #87. Native proof covers the current appearance only. No performance improvement is claimed.

The no-comments pass ran directly because the brief forbids nested agents. It found no added code comments, suppressions, or constraint comments. No deletions, restorations, or encoding offers were needed.

Run `python3 docs/evidence/shell-pending-refusal-fix3/audit-record.py.txt` to audit the source patch-id and runtime pathspecs against HEAD.
