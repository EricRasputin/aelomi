# Pending refusal guard, fix round 1 after workspace seeding

Historical evidence only. These scripts change the Mac's appearance and **must not be rerun as is**, per [#95](https://github.com/aelomi/aelomi/issues/95). Follow the current verify-aelomi rules, keep system settings unchanged, and use app theme controls or mock-browser color-scheme emulation for appearance variants.

Source commit `decccf106294a73af247409e291ab773cd368fad`.
Recorded merge base `7a285d85bca3da5da66990f546471d703850d812` against `origin/main`.
Verbatim patch-id `b5fa953fc14a2a09baff5892dfce31dd6dd0a84c`.

The proven runtime pathspecs are `crates/core/src/storage/mod.rs`, `crates/core/src/workspace/seed.rs`, `src-tauri/src/quit.rs`, `src-tauri/src/storage.rs`, `src-tauri/src/recovery.rs`, `src-tauri/src/macos.rs`, `src-tauri/src/commands.rs`, `src-tauri/src/lib.rs`, `src/components/storage-refusal.tsx`, `src/storage-copy.ts`, `src/styles.css`, `src/ipc/bindings.ts`, and `src/ipc/mock.ts`.

This record proves the rebased runtime, including normal first-launch seeding and explicit empty-workspace persistence. It supersedes both earlier pending-refusal records for the paths above. Their source hashes and audits describe historical runs. They do not prove the current head.

## Environment and isolation

macOS 27.0.1 build 26A434, Apple Silicon. Every native launch used the verification helper with a synthetic `AELOMI_DATA_DIR`, `AELOMI_CLOUD_ROOTS`, and `AELOMI_RECOVERY_DIR`. The synthetic base is `/private/tmp/ae92-e3-fix1-rebased-KEouUP`. Recovery files live in its Preferences directory, outside both cloud storage and the workspaces.

The driver held `/tmp/aelomi-ui.lock` for every key, accessibility press, appearance change, and capture. It released the lock between scenarios and restored the original appearance. Shell commands removed only synthetic links and moved only synthetic folders. Finder received no command and Show in Finder was not pressed. The real-data folder was listed read-only before the first launch and after final cleanup. Both listings are byte-identical. Every owned app was cleaned up.

## Native results

The five recovery paths and the empty-workspace relaunch passed. The source page body is exactly `Original pages survive E3 recovery.`

- A misplaced drop leaves the configured path absent and its parent listing identical. The app shows Finish moving your pages.
- A destination containing only the format marker remains on Finish moving your pages. Its complete file set and bytes, and the record bytes, match their prelaunch values. No document directory is created.
- A correct shell move opens the original page and clears only its pending record. The original workspace file names and bytes matched before reopening.
- Cancel leaves the destination absent and keeps its record. Confirm opens an empty workspace and clears its record. The dialog visibly labels the new destination and last-seen pages location. Every original cloud-folder file name and byte matches afterward.
- A confirmed empty workspace remains empty after a normal quit and relaunch. No Getting started page is added.
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
PASS: explicitly created workspace stays empty on normal relaunch
PASS: corrupt record keeps guard, destination absent, and corrupt bytes unchanged
PASS: explicit creation succeeds after corrupt record and original pages remain byte-identical
```

Native directories under `~/.local/state/aelomi-verify/` are:

- `20261016-124625-dev-9IxK` proves leaf.
- `20261016-124818-dev-6NJ5` proves misplaced.
- `20261016-124940-dev-4Mc3` proves marker-only.
- `20261016-125108-dev-3NSr` proves recovered.
- `20261016-125151-dev-ZNyf` proves new-leaf.
- `20261016-125212-dev-jxp5` proves new-workspace.
- `20261016-125353-dev-ykij` proves empty-reopened.
- `20261016-125432-dev-k12H` proves corrupt-leaf.
- `20261016-125500-dev-vGN0` proves corrupt-record.

## Automated proof and review items

The TDD regression runs failed before the fixes. Five Rust cases failed for marker-only arrival, docs without a marker, corrupt records, unrelated destination files, and independent configured paths. Six exact-copy mappings failed. The reveal adapter check failed on `open_path`. Both create-flow browser cases reproduced the dialog waiting on the tree or a rejected snapshot. Passing targeted runs are retained in `.local-gate/e3-fix1/`. After the rebase, two more regression tests failed before integration: an empty workspace gained Getting started on relaunch, and its Store-only quit path admitted record edits. The corrected shell suite passed 103 tests with one ignored; the three recovery browser cases passed. The red and green runs are `rebase-red.log`, `rebase-shell.log`, and `rebase-create-green.log` in that directory.

SHELL-11 now has the approved guard sentence in SPEC.md. Its proving tests are:

- Item 1 uses `src/storage-copy.test.ts`, `maps safe recovery steps for $name` and `maps a pending leaf refusal to only the remaining move steps`. Exact mappings include the remedy's parent chip and the destination-first clipboard sequence.
- Item 2 retains `src-tauri/src/commands.rs`, `recovery_opener_receives_only_the_native_last_seen_path`. `recovery_command_reveals_instead_of_opening` checks the production adapter's reveal API wiring. Native Finder receipt remains unproved under the no-Finder rule.
- Item 3 uses `src-tauri/src/recovery_tests.rs`, `marker_only_destination_keeps_guard_and_every_byte_unchanged` and `docs_without_marker_reports_unknown_format_without_changing_destination_or_record`.
- Item 4 uses `clearing_x_after_read_does_not_remove_a_new_refusal_for_y` and `normalized_configured_paths_have_one_stable_record_filename` in that Rust file. Y is saved between X's read and clear; Y survives.
- Item 5 uses `unreadable_records_keep_the_guard_without_overwriting_them` and `unknown_record_versions_and_unreadable_files_require_explicit_creation`. The version fixture has the correct configured path, so the unknown version is the failing field. `src/storage-copy.test.ts` also checks the lost-location message and the confirmation state.
- Item 6 and SHELL-12 readiness use `tests/playwright/pending-refusal.spec.ts`, both `creation becomes ready before tree refresh and preserves the Ready event` cases, with reject and stale snapshots. The tree reply is held while the user sees Ready. Search then shows No page content matches. No second index snapshot is requested.
- Item 7 uses `record_failure_refuses_recovery_instructions_and_does_not_touch_pages`, whose validated directory is mode `0o500`, and `records_cannot_be_written_inside_cloud_or_the_data_folder`, whose local-data case has no cloud link.
- The additional create refusal uses `explicit_creation_refuses_unrecognized_items_and_keeps_the_record` in the Rust file. The browser cases also cover the inline reveal error and both dialog labels. `a lost recovery record still requires explicit empty-workspace confirmation` covers cancel and confirm without a remembered location.

SHELL-2 explicit-empty persistence uses `explicit_empty_workspace_stays_empty_after_a_normal_relaunch`. SHELL-9 quit admission uses `explicit_creation_enables_the_store_only_record_edit_quit_barrier`. Both are in `src-tauri/src/recovery_tests.rs`. The normal-first-launch Storage test still expects Getting started, matching main.

The full SHELL-11 acceptance case is `SHELL-11 issue 92: finish moving keeps pages gated until empty-workspace confirmation` in `e2e/pending-refusal.e2e.ts`. The full gate runs all 20 cases in `src-tauri/src/recovery_tests.rs`. Besides the cases mapped above, SHELL-11 is covered by:

- `every_cloud_variant_records_paths_variant_and_timestamp_outside_the_workspace`.
- `misplaced_drop_blocks_creation_for_missing_and_markerless_destinations`.
- `a_correct_move_opens_the_original_document_and_clears_the_record`.
- `still_cloud_keeps_the_refusal_and_record`.
- `a_different_configured_path_ignores_and_retains_the_record`.
- `explicit_empty_creation_leaves_original_bytes_and_clears_only_after_open`.
- `explicit_creation_rechecks_cloud_and_never_overwrites_unrecognized_files`.
- `failed_workspace_opens_retain_the_matching_record`.
- `atomic_record_replacement_keeps_one_complete_latest_record`.

`src/storage-copy.test.ts` also covers `maps pending parent and override refusals without link removal: %s`. The prior refusal-polish failure-remedy annotation remains historical.

## UI stress proof

The final mock run is `~/.local/state/aelomi-verify/20261016-071531-mock-HZMljd`. Both themes passed at 320, 960, and 1440 px, with 16 and 32 px root text. Paths and names include deep nesting, long unbreakable names, CJK, Vietnamese diacritics, and emoji. All four screen chips fit their own text without horizontal overflow. The dialog labels the two locations. Its body scrolls at the narrow doubled-text size while the title and actions stay reachable. The fixture toggle exists only in the proof script.

The dynamic fields are the configured path, last-seen path, parent paths, and folder names. IPC does not bound these strings. Titles, buttons, and instruction counts are fixed. Page rows and page-title extremes do not apply to this screen. No new motion was added.

## Reproduce and audit

Copy `prepare-proof.sh.txt` and `native-proof.sh.txt` to `.local-gate/e3-fix1-rebased/`. Run the preparation from the repository root. Save the read-only real-data folder listing to `.local-gate/e3-fix1-rebased/real-data-before.txt`, then run the native driver. It creates fresh synthetic workspaces and uses only shell moves. It never touches Finder.

Copy `stress.mjs.txt` to a temporary `.mjs` file outside the checkout. Pass it to `.claude/skills/verify-aelomi/scripts/mock-drive.mjs --flow <file> --rebuild`. Run `python3 docs/evidence/shell-pending-refusal-fix1-rebased/audit-record.py.txt` to check the recorded patch-id and runtime pathspecs against HEAD.

## Published screenshots

[The rebased native self-proof comment](https://github.com/aelomi/aelomi/pull/93#issuecomment-6210381100) contains the inspected PNGs below. Every public download was checked without authentication against these byte sizes and SHA-256 values, and decoded as PNG. Raw logs and trees stay local.

- `leaf-refusal-dark.png`, 394777 bytes, SHA-256 `94509eec059650d000e9d83d2d8b3a0235a51b95ba29ccb6ccc316b3e6f3c42e`.
- `leaf-refusal-light.png`, 423131 bytes, SHA-256 `97ee3b8e084e6d3aeb732a70aa306b44a01c0651482fbf6dc2349f02e33cca9e`.
- `finish-moving-dark.png`, 405184 bytes, SHA-256 `5d8eb8be97d35ad7023feac8696245c6de3bf80272ce974072ac62e60242410a`.
- `finish-moving-light.png`, 437489 bytes, SHA-256 `2ca9625d3b2b81fe60261d28a789e4eef23db9ef93338781a348b4af58aef1ce`.
- `marker-only-dark.png`, 400699 bytes, SHA-256 `0d9c1baa5e0635c42fe41648bdf2686194f2665bf636b6f03e3b8c12791328cf`.
- `marker-only-light.png`, 431415 bytes, SHA-256 `656a07a3193f8e203647b2c48d5fce0404710ebb8eb9ddaa02fa9bb349fb9f8a`.
- `recovered-page-dark.png`, 111642 bytes, SHA-256 `38ed754a08760efb06bd91b141d8133b8a457b85d02bde1f1d98dff9f0a04343`.
- `recovered-page-light.png`, 116728 bytes, SHA-256 `1defcba7b80a7b4ed22da8cf8586c6259103a6a3043041f4e6d1b409fe18a7b3`.
- `new-confirmation-dark.png`, 455955 bytes, SHA-256 `9ce8f9d6143a31bace13895932e98044cf34b3b4dc3352774e280a2b5676a9be`.
- `new-confirmation-light.png`, 482569 bytes, SHA-256 `8164c8ed18e1bf9e6bf5c3a38f48367643104e3b5df5032313f924244cfd44b1`.
- `new-empty-dark.png`, 102601 bytes, SHA-256 `17224fc0a3a414d6a51c62cb13d87d202ad2ffea6fea2f13370fe7c6a916ee11`.
- `new-empty-light.png`, 106030 bytes, SHA-256 `76979a99b574eed92f63f1729fce4c401c07d0b0c7e31456140d57e3ce860503`.
- `empty-reopened-dark.png`, 102601 bytes, SHA-256 `17224fc0a3a414d6a51c62cb13d87d202ad2ffea6fea2f13370fe7c6a916ee11`.
- `empty-reopened-light.png`, 106030 bytes, SHA-256 `76979a99b574eed92f63f1729fce4c401c07d0b0c7e31456140d57e3ce860503`.
- `corrupt-empty-dark.png`, 102601 bytes, SHA-256 `17224fc0a3a414d6a51c62cb13d87d202ad2ffea6fea2f13370fe7c6a916ee11`.
- `corrupt-empty-light.png`, 106030 bytes, SHA-256 `76979a99b574eed92f63f1729fce4c401c07d0b0c7e31456140d57e3ce860503`.
- `corrupt-record-dark.png`, 128881 bytes, SHA-256 `d75668875634ce23048b65b51b420ca2b6ade9b0efe27b0d75e4c15cacea15f1`.
- `corrupt-record-light.png`, 133862 bytes, SHA-256 `5caaf98a52e4b0d04f4023616c4c0e0741a69808848a1a1d62b2eda16a2eca84`.
- `dark-320-32-dialog.png`, 61817 bytes, SHA-256 `b704f4a031b898de37de39ac6b2e48fe3998a824d3f3902f2f76876f928d56b3`.
- `light-320-32-dialog.png`, 68043 bytes, SHA-256 `b3b6717695de2c700051ca42fe454e99225451e19db33e02e9e335df31836755`.
- `dark-960-16-dialog.png`, 186065 bytes, SHA-256 `27ac68f5c0d8641805de61665e5a6cce7182cac72982f45d6b9d0a79d4644b1b`.
- `light-960-16-dialog.png`, 195328 bytes, SHA-256 `1c6bb7d3352a63c0966adf09867069b5af54c7afca5d75192ed4662847439192`.
