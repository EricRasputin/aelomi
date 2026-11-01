# Pending refusal guard, fix round 1

Historical evidence only. These scripts change the Mac's appearance and **must not be rerun as is**, per [#95](https://github.com/aelomi/aelomi/issues/95). Follow the current verify-aelomi rules, keep system settings unchanged, and use app theme controls or mock-browser color-scheme emulation for appearance variants.

Source commit `1be241f0106e075c74a0a64ce28d854cad515a27`.
Recorded merge base `9323b3c401c3c7d1e3b7de9fe0d3aa8bef7767ec` against `origin/main`.
Verbatim patch-id `27dfdb1a4dbb69ee91de6dc29bf6c0a8cfe24eda`.

The proven runtime pathspecs are `crates/core/src/storage/mod.rs`, `src-tauri/src/storage.rs`, `src-tauri/src/recovery.rs`, `src-tauri/src/macos.rs`, `src-tauri/src/commands.rs`, `src-tauri/src/lib.rs`, `src/components/storage-refusal.tsx`, `src/storage-copy.ts`, `src/styles.css`, `src/ipc/bindings.ts`, and `src/ipc/mock.ts`.

Native verification started at `868cdc853a20c72c4b29524f2ee7394268bea61c`. The recorded source adds only mock readiness and a test-fixture correction. Its native runtime files are identical, checked with `git diff --exit-code 868cdc8 1be241f -- src-tauri/src/storage.rs src-tauri/src/recovery.rs src-tauri/src/macos.rs src-tauri/src/commands.rs src-tauri/src/lib.rs crates/core/src src/components/storage-refusal.tsx src/storage-copy.ts src/styles.css`. The final mock stress run used the recorded source.

## Environment and isolation

macOS 27.0.1 build 26A434, Apple Silicon. Every native launch used the verification helper with a synthetic `AELOMI_DATA_DIR`, `AELOMI_CLOUD_ROOTS`, and `AELOMI_RECOVERY_DIR`. The synthetic base is `/private/tmp/ae92-e3-fix1-cINdht`. Recovery files live in its Preferences directory, outside both cloud storage and the workspaces.

The driver held `/tmp/aelomi-ui.lock` for every key, accessibility press, appearance change, and capture. It released the lock between scenarios and restored the original appearance. Shell commands removed only synthetic links and moved only synthetic folders. Finder received no command and Show in Finder was not pressed. The real-data folder was listed read-only before the first launch and after final cleanup. Both listings are byte-identical. Every owned app was cleaned up.

## Native results

All five paths passed. The source page body is exactly `Original pages survive E3 recovery.`

- A misplaced drop leaves the configured path absent and its parent listing identical. The app shows Finish moving your pages.
- A destination containing only the format marker remains on Finish moving your pages. Its complete file set and bytes, and the record bytes, match their prelaunch values. No document directory is created.
- A correct shell move opens the original page and clears only its pending record. The original workspace file names and bytes matched before reopening.
- Cancel leaves the destination absent and keeps its record. Confirm opens an empty workspace and clears its record. The dialog visibly labels the new destination and last-seen pages location. Every original cloud-folder file name and byte matches afterward.
- A corrupt per-path record leaves the destination absent and the corrupt file unchanged. The app says it lost track of where the pages were. Explicit confirmation still opens an empty workspace and clears the corrupt record. The original pages remain byte-identical.

The driver and saved native trees produced these exact lines:

```text
PASS: cloud refusal recorded exact paths, variant, and timestamp
PASS: misplaced drop creates nothing at configured path; parent listing is identical
PASS: marker-only destination and pending record are byte-identical; no docs directory was created
PASS: native Page body equals exactly Original pages survive E3 recovery.
PASS: correct shell move opens exact original page text and clears pending record
PASS: cancel keeps pending refusal and creates no workspace
recovery new workspace status=Ready
PASS: confirmed creation opens empty workspace; original cloud folder is untouched
PASS: corrupt record keeps guard, destination absent, and corrupt bytes unchanged
PASS: explicit creation succeeds after corrupt record and original pages remain byte-identical
```

Native directories under `~/.local/state/aelomi-verify/` are:

- `20261016-120201-dev-VIpt` proves leaf.
- `20261016-120340-dev-KGOo` proves misplaced.
- `20261016-120542-dev-zNXz` proves marker-only.
- `20261016-120740-dev-FlDx` proves recovered.
- `20261016-120825-dev-n7Ux` proves new-leaf.
- `20261016-120859-dev-9cTP` proves new-workspace.
- `20261016-121055-dev-fZRc` proves corrupt-leaf.
- `20261016-121113-dev-NuVV` proves corrupt-record.

## Automated proof and review items

The TDD regression runs failed before the fixes. Five Rust cases failed for marker-only arrival, docs without a marker, corrupt records, unrelated destination files, and independent configured paths. Six exact-copy mappings failed. The reveal adapter check failed on `open_path`. Both create-flow browser cases reproduced the dialog waiting on the tree or a rejected snapshot. Passing targeted runs are retained in `.local-gate/e3-fix1/`.

SHELL-11 now has the approved guard sentence in SPEC.md. Its proving tests are:

- Item 1 uses `src/storage-copy.test.ts`, `maps safe recovery steps for $name` and `maps a pending leaf refusal to only the remaining move steps`. Exact mappings include the remedy's parent chip and the destination-first clipboard sequence.
- Item 2 retains `src-tauri/src/commands.rs`, `recovery_opener_receives_only_the_native_last_seen_path`. `recovery_command_reveals_instead_of_opening` checks the production adapter's reveal API wiring. Native Finder receipt remains unproved under the no-Finder rule.
- Item 3 uses `src-tauri/src/recovery_tests.rs`, `marker_only_destination_keeps_guard_and_every_byte_unchanged` and `docs_without_marker_reports_unknown_format_without_changing_destination_or_record`.
- Item 4 uses `clearing_x_after_read_does_not_remove_a_new_refusal_for_y` and `normalized_configured_paths_have_one_stable_record_filename` in that Rust file. Y is saved between X's read and clear; Y survives.
- Item 5 uses `unreadable_records_keep_the_guard_without_overwriting_them` and `unknown_record_versions_and_unreadable_files_require_explicit_creation`. The version fixture has the correct configured path, so the unknown version is the failing field. `src/storage-copy.test.ts` also checks the lost-location message and the confirmation state.
- Item 6 and SHELL-12 readiness use `tests/playwright/pending-refusal.spec.ts`, both `creation becomes ready before tree refresh and preserves the Ready event` cases, with reject and stale snapshots. The tree reply is held while the user sees Ready. Search then shows No page content matches. No second index snapshot is requested.
- Item 7 uses `record_failure_refuses_recovery_instructions_and_does_not_touch_pages`, whose validated directory is mode `0o500`, and `records_cannot_be_written_inside_cloud_or_the_data_folder`, whose local-data case has no cloud link.
- The additional create refusal uses `explicit_creation_refuses_unrecognized_items_and_keeps_the_record` in the Rust file. The browser cases also cover the inline reveal error and both dialog labels. `a lost recovery record still requires explicit empty-workspace confirmation` covers cancel and confirm without a remembered location.

The prior `docs/evidence/shell-refusal-polish-fix1/README.md` already has a historical scope annotation. Its failure-remedy claim is superseded there. The old pending-refusal record is historical too; its runtime pathspec audit does not prove this fix. This record replaces it for the current behavior.

## UI stress proof

The final mock run is `~/.local/state/aelomi-verify/20261016-064246-mock-oFKdkY`. Both themes passed at 320, 960, and 1440 px, with 16 and 32 px root text. Paths and names include deep nesting, long unbreakable names, CJK, Vietnamese diacritics, and emoji. All four screen chips fit their own text without horizontal overflow. The dialog labels the two locations. Its body scrolls at the narrow doubled-text size while the title and actions stay reachable. The fixture toggle exists only in the proof script.

The dynamic fields are the configured path, last-seen path, parent paths, and folder names. IPC does not bound these strings. Titles, buttons, and instruction counts are fixed. Page rows and page-title extremes do not apply to this screen. No new motion was added.

## Reproduce and audit

Copy `prepare-proof.sh.txt` and `native-proof.sh.txt` to `.local-gate/e3-fix1/`. Run the preparation from the repository root. Save the read-only real-data folder listing to `.local-gate/e3-fix1/real-data-before.txt`, then run the native driver. It creates fresh synthetic workspaces and uses only shell moves. It never touches Finder.

Copy `stress.mjs.txt` to a temporary `.mjs` file outside the checkout. Pass it to `.claude/skills/verify-aelomi/scripts/mock-drive.mjs --flow <file> --rebuild`. Run `python3 docs/evidence/shell-pending-refusal-fix1/audit-record.py.txt` to check the recorded patch-id and runtime pathspecs against HEAD.

## Published screenshots

[The native self-proof comment](https://github.com/aelomi/aelomi/pull/93#issuecomment-6209751023) contains the inspected PNGs below. The public downloads are checked without authentication against these sizes and SHA-256 values. Raw logs and trees stay local.

- `leaf-refusal-dark.png`, 388661 bytes, SHA-256 `1a35d400e9c7f2dcce33d3e9b10974507a36229994aa478abc1362ede2cc23d1`.
- `leaf-refusal-light.png`, 416102 bytes, SHA-256 `b5dffac35aa9090f044baec5204d4e77804cbcc3aa97cf8694211b18f9494d7b`.
- `finish-moving-dark.png`, 395513 bytes, SHA-256 `229a9316e94e0b9bf1413e42b053d7c179f137daf188a319dd64040525cf8627`.
- `finish-moving-light.png`, 425618 bytes, SHA-256 `cb03254671bfe3ba840cd4cb9a9f6d841cbd54670f2792267a048a98172ce456`.
- `marker-only-dark.png`, 400019 bytes, SHA-256 `812e7a432364fcdd0ecbe3f875afb45a576a4702e0afea027903ce0c0b2b4aca`.
- `marker-only-light.png`, 431823 bytes, SHA-256 `7db53012ea020dc8a34324e40a2afd924ea9d61bf9c769863d46dfee19a59c8d`.
- `recovered-page-dark.png`, 106273 bytes, SHA-256 `37461428e1fb15834547278c74ed88b33dc6808c145f25dfa31a38dd97a8bc3a`.
- `recovered-page-light.png`, 103434 bytes, SHA-256 `28ab838e8e57b408f56dd9d0662819a6e74a5d3be2b2cb3d95f03bb35e861412`.
- `new-confirmation-dark.png`, 415219 bytes, SHA-256 `d1de7b1bc2cec9f4c86902df5a424aadb8129a3b2ad6cb08bd7821a76f9113c4`.
- `new-confirmation-light.png`, 445559 bytes, SHA-256 `8aa6277241048a0426100202a7a9e744654f1821b456f9d181c16181c4c3d203`.
- `new-empty-dark.png`, 103871 bytes, SHA-256 `5fe8685ee3ea8794343c2498849a9ac64d315bd9d4fb345dd7c354a6644ba88d`.
- `new-empty-light.png`, 104771 bytes, SHA-256 `44bb1c519560d72b38e721eddb63628b3ec892df3148915f97132ca39801198b`.
- `corrupt-empty-dark.png`, 103871 bytes, SHA-256 `5fe8685ee3ea8794343c2498849a9ac64d315bd9d4fb345dd7c354a6644ba88d`.
- `corrupt-empty-light.png`, 104771 bytes, SHA-256 `44bb1c519560d72b38e721eddb63628b3ec892df3148915f97132ca39801198b`.
- `corrupt-record-dark.png`, 125307 bytes, SHA-256 `adeeca18b6a175a0d15b76e997a2811654ecfa4a400f4b5c17415d4ec1053274`.
- `corrupt-record-light.png`, 127611 bytes, SHA-256 `02a63cb363b5e1000925e662bfd8a5558dbda9d59edcdbc664f3d44bf58c17fb`.
- `dark-320-32-dialog.png`, 61817 bytes, SHA-256 `b704f4a031b898de37de39ac6b2e48fe3998a824d3f3902f2f76876f928d56b3`.
- `light-320-32-dialog.png`, 68043 bytes, SHA-256 `b3b6717695de2c700051ca42fe454e99225451e19db33e02e9e335df31836755`.
- `dark-960-16-dialog.png`, 186065 bytes, SHA-256 `27ac68f5c0d8641805de61665e5a6cce7182cac72982f45d6b9d0a79d4644b1b`.
- `light-960-16-dialog.png`, 195328 bytes, SHA-256 `1c6bb7d3352a63c0966adf09867069b5af54c7afca5d75192ed4662847439192`.
