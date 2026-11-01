# Pending cloud refusal recovery

Historical evidence only. These scripts change the Mac's appearance and **must not be rerun as is**, per [#95](https://github.com/aelomi/aelomi/issues/95). Follow the current verify-aelomi rules, keep system settings unchanged, and use app theme controls or mock-browser color-scheme emulation for appearance variants.

Source commit `b37c17c96bc52a710a1dbd208f01b01c42b34708`.
Recorded merge base `9323b3c401c3c7d1e3b7de9fe0d3aa8bef7767ec` against `origin/main`.
Verbatim patch-id `b82845065410bb8d0b5ac4360bfa8990852e1277`.

The proven runtime pathspecs are `src-tauri/src/storage.rs`, `src-tauri/src/recovery.rs`, `src-tauri/src/macos.rs`, `src-tauri/src/commands.rs`, `src-tauri/src/lib.rs`, `src/components/storage-refusal.tsx`, `src/storage-copy.ts`, `src/styles.css`, `src/ipc/bindings.ts`, and `src/ipc/mock.ts`.

## Environment and isolation

macOS 27.0.1 build 26A434 on Apple Silicon. Every native launch used `app.sh launch dev` with a synthetic `AELOMI_DATA_DIR`, `AELOMI_CLOUD_ROOTS`, and a separate `AELOMI_RECOVERY_FILE`. The synthetic base was `/private/tmp/ae92-e3-1qSHaA`. The cloud and configured folder names differ. The misplaced folder was renamed to Aelomi inside OtherApp under the configured parent.

The script acquired `/tmp/aelomi-ui.lock` before every input and capture, and released it between launches. Appearance was restored before each release. Finder received no command or opener action. Shell `rm` removed only the two synthetic symlinks. Shell `mv` performed the misplaced and correct moves. No claim about Finder drag ergonomics, cloud-provider prompts, or native opener receipt is made. The opener button has WebKit acceptance coverage and a Rust test that checks its exact path.

The real-data folder was listed read-only before the first launch and after final cleanup. The two listings were identical.

## Native results

PASS for all three required paths. Native run directories under `~/.local/state/aelomi-verify/` are:

- `20261016-104358-dev-3ZQc` for the initial refusal.
- `20261016-104743-dev-Lcvh` for the misplaced drop.
- `20261016-104933-dev-bdMr` for the correct move and original page.
- `20261016-105416-dev-Z9Hh` for the second refusal.
- `20261016-105438-dev-otjY` for canceled and confirmed empty creation.

The first refusal wrote the configured path, resolved cloud path, LinkAtDataFolder payload, and timestamp outside the workspace. The script compared each field with its literal expected value.

For the misplaced drop, the configured path remained absent and the parent listing was byte-identical before and after relaunch. The full-window heading appeared in the accessibility tree. Both themes show the last-seen folder and remaining move steps.

The correct shell move put the original files at the configured path. Every original file name and SHA-256 matched before reopen. The real app then displayed the exact original Page body and cleared the record.

The empty-workspace dialog showed the warning and focused Keep moving my pages. Cancel created nothing and kept the record. Confirm opened the empty workspace, installed quit draining, and cleared the record. The original cloud-folder file names and bytes were identical afterward. The new ready tree contained Ping again and no Page body. Normal quit drained and completed.

These lines are from the native run and its saved accessibility trees:

```text
PASS: cloud refusal recorded exact paths, variant, and timestamp
PASS: misplaced drop creates nothing at configured path; parent listing is identical
AXTextArea "Page body" = "Original pages survive E3 recovery."
PASS: cancel keeps pending refusal and creates no workspace
recovery new workspace status=Ready
PASS: original folder listing and every file byte are unchanged
quit confirmed via=drain
PASS: confirmed creation opens empty workspace; original cloud folder is untouched
```

The separate exact-value audit of the saved native trees and real-data listings printed:

```text
PASS: native Page body equals exactly Original pages survive E3 recovery.
PASS: the ready empty workspace has no Page body
PASS: real-data before/after listings are byte-identical
```

## Automated coverage

All new behavior belongs to SHELL-11 and issue #92. SPEC.md has no separate behavior ID for pending recovery. SHELL-2 remains the normal first-launch rule; this change gates it after a cloud refusal.

`src-tauri/src/recovery_tests.rs` contains these cases:

- `every_cloud_variant_records_paths_variant_and_timestamp_outside_the_workspace`.
- `misplaced_drop_blocks_creation_for_missing_and_markerless_destinations`.
- `a_correct_move_opens_the_original_document_and_clears_the_record`.
- `still_cloud_keeps_the_refusal_and_record`.
- `a_different_configured_path_ignores_and_retains_the_record`.
- `unreadable_records_are_ignored_without_overwriting_them`.
- `explicit_empty_creation_leaves_original_bytes_and_clears_only_after_open`.
- `explicit_creation_rechecks_cloud_and_never_overwrites_unrecognized_files`.
- `record_failure_refuses_recovery_instructions_and_does_not_touch_pages`.
- `records_cannot_be_written_inside_cloud_or_the_data_folder`.
- `failed_workspace_opens_retain_the_matching_record`.
- `atomic_record_replacement_keeps_one_complete_latest_record`.

`src-tauri/src/commands.rs` adds `recovery_opener_receives_only_the_native_last_seen_path`.

`e2e/pending-refusal.e2e.ts` adds `SHELL-11 issue 92: finish moving keeps pages gated until empty-workspace confirmation`. It checks the last-seen path, opener payload, warning, initial cancel focus, canceled creation, confirmed creation, and the ready empty-workspace UI. `e2e/cloud-folder.e2e.ts` updates acceptance 7's exact configured-path check and Return instructions.

`src/storage-copy.test.ts` adds `maps a pending leaf refusal to only the remaining move steps` and both cases of `maps pending parent and override refusals without link removal: %s`. Its five `maps safe recovery steps for $name` cases pin the revised cloud copy. The whole shell test run passed 79 tests with one measurement ignored.

## UI stress proof

The WebKit run is `~/.local/state/aelomi-verify/20261016-051058-mock-Vp25H6`. The flow's Demo data / Worst case toggle exists only in the proof script. Inputs enter through MockSetup. Both themes, widths 320, 960, and 1440, and root text sizes 16 and 32 px passed without horizontal overflow or clipped path text. The long folder names, deep paths, CJK, Vietnamese diacritics, and emoji remain readable and selectable. The dialog keeps its heading and actions visible while its body and long path scroll.

The field catalog consists of the configured and resolved paths and the two folder names, which IPC does not bound. Titles, button labels, and the two-to-four instruction count are fixed by the application. User page titles, page rows, and nesting trees are not rendered on this screen. Empty or thousands-row fixtures do not apply to the fixed instruction list.

The first stress pass found that focus on the cancel button could scroll the warning out of view behind a tall path. The final layout uses a scrolling dialog body and fixed heading and actions. The final screenshots were inspected in both themes at normal width and at 320 px with doubled text. At the narrow doubled size, the warning and path can be scrolled; both actions stay visible.

## Reproduce and audit

From the repository root, copy `prepare-proof.sh.txt` and `native-proof.sh.txt` into `.local-gate/e3/`. Run `bash .local-gate/e3/prepare-proof.sh.txt`, then save `ls -la "$HOME/Library/Application Support/Aelomi"` to `.local-gate/e3/real-data-before.txt`. Run `bash .local-gate/e3/native-proof.sh.txt`. The preparation seeds new synthetic folders through the real Store and removes its temporary Rust example when done. The native script records each owned run in `.local-gate/e3/runs.tsv` and keeps raw output local.

Copy `stress.mjs.txt` to a temporary `.mjs` path outside the checkout and run `node .claude/skills/verify-aelomi/scripts/mock-drive.mjs --flow <that path>`. It checks the shared guarded WebKit surface and saves the fixture toggle, screenshots, and assertions outside the checkout.

Run `python3 docs/evidence/shell-pending-refusal/audit-record.py.txt` to check the recorded source patch-id and runtime pathspecs against HEAD. After a rebase, use the final review gate's recorded merge base as required by `docs/native-verification-evidence.md`.

## Published screenshots

The PR native self-proof comment publishes these inspected PNGs. Their names identify the local run and state above. Public downloads are checked without credentials against these byte counts and SHA-256 values.

| Screenshot | Bytes | SHA-256 |
| --- | ---: | --- |
| `leaf-refusal-dark.png` | 387725 | `e80b83d12cfcddbe25156448a8d6cf96b0ff3672794476bccb92c9391d27e493` |
| `leaf-refusal-light.png` | 409358 | `186c45f0361f0ec84545d12070658d1e0c41c555cbd1444b03344197c8bb5139` |
| `finish-moving-dark.png` | 387369 | `17728e116537efaee9f3f008bf33554fd6c4e6523d371a4cce77cdfd82b1748c` |
| `finish-moving-light.png` | 411236 | `0cbc53833e9bb0324de7fa4a611932d95720fae858a90b77410360a0dc4133c4` |
| `recovered-page-dark.png` | 106826 | `301f8f5f19bdc7da18859b944aaa97162de44d2267a4afdec70d30c6fcef4fc1` |
| `recovered-page-light.png` | 110850 | `ac787eb3476ef09835fc657ab07c88ecc5e45b87f81c18150cf2759c3d5a3201` |
| `new-confirmation-dark.png` | 421577 | `bbda552292d5c044d7fccd899b1f24424bf936db4cd1ce8e7b8d7df9d3f6a08a` |
| `new-confirmation-light.png` | 443402 | `03adee9973be83f8d81fa689488bde2dea126c7e5761295a1349b60a78fe654b` |
| `new-empty-dark.png` | 103871 | `5fe8685ee3ea8794343c2498849a9ac64d315bd9d4fb345dd7c354a6644ba88d` |
| `new-empty-light.png` | 104771 | `44bb1c519560d72b38e721eddb63628b3ec892df3148915f97132ca39801198b` |
| `dark-320-32-dialog.png` | 61817 | `b704f4a031b898de37de39ac6b2e48fe3998a824d3f3902f2f76876f928d56b3` |
| `light-320-32-dialog.png` | 68043 | `b3b6717695de2c700051ca42fe454e99225451e19db33e02e9e335df31836755` |
| `dark-960-16-screen.png` | 170353 | `0b051e05be3ef7f7e99e96ef4fae9bc738233ba488c661998fc68ce0d959a553` |
| `light-960-16-screen.png` | 177269 | `d3edff852963dbbdf6067efedb92e5d9c9c07bc0c66d8d00817e4cb3e186fbe2` |
