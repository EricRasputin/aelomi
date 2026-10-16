# Pending cloud-refusal guard, fix round 2

Source commit `27630a848f6b59e8e8970ba36851f79c5366ee30`.
Recorded merge base `7a285d85bca3da5da66990f546471d703850d812`.
Verbatim patch-id `7d7b3ebce3d7bba19c2d56d80f09b554994de3ac`.

The runtime pathspecs are `crates/core/src/storage/mod.rs`, `crates/core/src/storage/store.rs`, `crates/core/src/workspace/seed.rs`, `src-tauri/src/quit.rs`, `src-tauri/src/storage.rs`, `src-tauri/src/recovery.rs`, `src-tauri/src/macos.rs`, `src-tauri/src/commands.rs`, `src-tauri/src/lib.rs`, `src/components/storage-refusal.tsx`, `src/storage-copy.ts`, `src/styles.css`, `src/ipc/bindings.ts`, `src/ipc/mock.ts`, and `src/ipc/mock-recovery.ts`.

This record supersedes the earlier pending-refusal records for those paths. Their recorded source commits remain historical evidence. The refusal-polish failure-remedy annotation remains historical too.

## Domain and scope

`DestinationFolder` is a core enum beside `check_format`. It distinguishes absent, empty, non-empty with item names, arrived workspace, and unrecognized destinations. `Layout` and `Fs` own the reads, so SimFs exercises the classification without a real disk. The shared root-document pristine check requires stored history before a workspace counts as arrived. Finder metadata and Aelomi's empty skeleton permit explicit creation.

This implements SHELL-11. Existing SHELL-2 seeding and SHELL-12 Ready-before-refresh behavior remain covered by their suites. A pending unrecognized destination now retains Finish moving, as round 2 requires. Without a pending record, the unknown-format refusal remains. The app never moves or removes the original pages.

## Environment and recipe

macOS 27.0.1, build 26A434, Apple M1, arm64. The native helper launches a debug build through `app.sh` with explicit synthetic `AELOMI_DATA_DIR`, `AELOMI_RECOVERY_DIR`, and `AELOMI_CLOUD_ROOTS`. No Finder command is sent. Every press, key, native window resize, and screenshot runs under `/tmp/aelomi-ui.lock`. Each owned app exits and is cleaned up between scenarios. The appearance helper assigns only the owned app's [NSApplication appearance](https://developer.apple.com/documentation/appkit/nsapplication/appearance). Its constructor ignores every other executable. The runner sets DYLD immediately before Cargo launches Aelomi, after the system nohup boundary that strips it. The helper contains no storage or IPC logic and does not set system preferences. The app binary comes from the recorded source commit. No helper is shipped.

Owned instances stop through the helper's cleanup after interactive steps, which releases the UI lock first. These runs prove storage transitions and startup, not keyboard quit. An abandoned earlier attempt called the inherited global-appearance restore with the already-active value during its cloud-only run. It produced no screenshots and did not change the selected appearance. It was replaced before the published proof. Launch arguments did not change WKWebView's scheme, and a debugger attach did not complete. Neither is used by the published run.

The source folder is a copy of `fixtures/data-folders/format-1`. Its file names and SHA-256 values are compared exactly after the run. The real-data folder is listed read-only before and after the proof.

Copy `native-proof.sh.txt` to `.local-gate/e3-fix2/native-proof.sh`. Create that output directory first. Before running it, run `ls -la "$HOME/Library/Application Support/Aelomi" > /tmp/e3-fix2-real-data-before.txt`. Copy `prepare-appearance-helper.sh.txt` to `.local-gate/e3-fix2/prepare-appearance-helper.sh` and execute it. Copy `appearance-runner.sh.txt` to `.local-gate/e3-fix2/appearance-runner` and `pnpm-shim.sh.txt` to `.local-gate/e3-fix2/bin/pnpm`. Make both executable. Run the proof once per `AELOMI_PROOF_THEME=Dark` and `Light`, with `AELOMI_PROOF_REPO="$PWD"` and that bin directory prepended to PATH. The shim forwards only dev launches to the existing Tauri launcher with the appearance runner. Execute from the repository root. Keep the generated run directories until PR #93 merges.

Copy `stress.mjs.txt` outside the checkout and pass it to `.claude/skills/verify-aelomi/scripts/mock-drive.mjs --flow <file> --rebuild`. The Demo data / Worst case toggle exists only in the injected test page.

## Tests and limits

SHELL-11 shell cases in `src-tauri/src/recovery_tests.rs` include:

- `finder_metadata_only_destination_accepts_explicit_creation`.
- `explicit_creation_refuses_unrecognized_items_and_keeps_the_record`.
- `failed_empty_initialization_keeps_the_record_guards_relaunch_and_allows_retry`.
- `marker_and_empty_docs_keep_the_guard_and_all_bytes`.
- `older_format_fixture_opens_through_the_guard_with_original_bytes`.
- `records_cannot_be_written_inside_cloud_or_the_data_folder`.
- `recovery_validation_uses_the_normalized_configured_path`.
- `arrived_workspace_opens_even_when_recovery_directory_is_invalid`.

Core SimFs cases in `crates/core/src/storage/tests/destination.rs` include `destination_classification_is_read_only_and_uses_workspace_content` and `real_items_and_unrecognized_formats_stay_distinct`. All 378 core tests passed. All 109 non-ignored shell tests passed.

`src/storage-copy.test.ts` has 17 passing cases, including `destination exists %s drives the exact rename step`. It asserts both flag values and the exact instruction. `tests/playwright/pending-refusal.spec.ts` has four passing cases, including the named-item refusal, retained move controls, the unreadable-record destination, rejected snapshots, and an intervening Ready event. Those same flows exercise the extracted recovery mock.

The baseline failed the metadata-only create, empty-docs guard, forced-initialization failure, and rename-copy regressions. The local logs are `.local-gate/e3-fix2/rust-red.log` and `copy-red.log`. The forced failure uses a read-only synthetic docs directory and checks the exact initialization error, retained record, guarded relaunch, and successful retry.

The stress run is `~/.local/state/aelomi-verify/20261016-082834-mock-jJ9HKx`. It passed both themes at 320, 960, and 1440 px with 16 and 32 px root text and long multilingual paths. Each chip wrapped without horizontal overflow. Screenshots were inspected. At 320 px with doubled text the dialog paths require scrolling, as already deferred to #87. The chip's accessible-name mismatch and general shared-link/override move-copy redesign remain deferred. No new motion was added.

Native reveal receipt and Finder move ergonomics are not proved, because Finder is reserved for the maintainer. The concurrent-create state and recovery override-layer considerations remain outside this fix. The no-comments pass ran directly, because the implementation brief forbids nested agents. No new explanatory comments or suppressions were added.

Run `python3 docs/evidence/shell-pending-refusal-fix2/audit-record.py.txt` to check the source patch-id and runtime pathspecs against HEAD.


The production build check initially caught eager recovery-mock initialization in the shipped bundle. Lazy initialization fixes that boundary. `pnpm build` followed by `pnpm check:no-mock` passed and reported six production files with no mock markers. The focused four-case WebKit suite passed again after this fix. An earlier full-gate attempt exceeded the existing search timing ceiling. The unchanged standalone retry passed. No performance improvement is claimed.


## Native results

All four changed paths passed in both themes. Metadata-only creation opened empty, preserved `.DS_Store`, and cleared the pending record. An occupied destination showed the rename step, refused `notes.txt` by name, and retained the moving controls. The unreadable-record screen showed the expected path, check, and restart. A marker plus empty docs stayed guarded, without a destination or record change.

The inline shell receipts include:

```text
recovery new workspace status=Ready
proof app appearance=Dark
proof app appearance=Light
PASS: original files, partial destination, and pending record are byte-identical
```

`postflight.py.txt` also passed exact assertions for the saved empty workspace, preserved metadata, occupied file names and bytes, partial destination, hashed record filename and bytes, original fixture files, successful doctors, and the real-data listings. The successful native run directories are below. The light unreadable-record launch first failed the offscreen doctor check, was cleaned up, then relaunched on the same synthetic path. The last successful run is listed.

- dark, cloud: `~/.local/state/aelomi-verify/20261016-143140-dev-URDF`.
- dark, metadata: `~/.local/state/aelomi-verify/20261016-143152-dev-uNV7`.
- dark, occupied: `~/.local/state/aelomi-verify/20261016-143250-dev-shJ9`.
- dark, unreadable: `~/.local/state/aelomi-verify/20261016-143410-dev-mJFJ`.
- dark, partial: `~/.local/state/aelomi-verify/20261016-143442-dev-yPnk`.
- light, cloud: `~/.local/state/aelomi-verify/20261016-143534-dev-IzfK`.
- light, metadata: `~/.local/state/aelomi-verify/20261016-143547-dev-G4sE`.
- light, occupied: `~/.local/state/aelomi-verify/20261016-143645-dev-xACa`.
- light, unreadable: `~/.local/state/aelomi-verify/20261016-143933-dev-enIK`.
- light, partial: `~/.local/state/aelomi-verify/20261016-144003-dev-a5wc`.

All owned native instances were cleaned up. The UI lock was released after each interactive scenario. The original real-data listing and the post-native listing match exactly. The final-gate comparison is kept with the local gate output.

## Published screenshots

[Fourteen native screenshots](https://github.com/aelomi/aelomi/pull/93#issuecomment-6212139340) were read back from the posted comment, downloaded without authentication, and compared by PNG signature, size, dimensions, and SHA-256 with the inspected originals. Every comparison passed. No raw logs or screenshot PNGs are committed.

| Screenshot | Bytes | SHA-256 |
|---|---:|---|
| `metadata-screen-dark.png` | 403455 | `044ed5381a664be3abfd98bfe5a7c58b40f61c3320d1992828bdbbb3aacca41a` |
| `metadata-confirm-dark.png` | 464462 | `e52fd9edfc4e76daa87e8627aee46521a08599a81ceb48ebb5abf698f01a36f8` |
| `metadata-ready-dark.png` | 102601 | `17224fc0a3a414d6a51c62cb13d87d202ad2ffea6fea2f13370fe7c6a916ee11` |
| `occupied-rename-dark.png` | 403455 | `044ed5381a664be3abfd98bfe5a7c58b40f61c3320d1992828bdbbb3aacca41a` |
| `occupied-error-dark.png` | 475026 | `26236f7c84ce0938a70c472e3241431f55ca87e23b217d812e1add96e45f1c1b` |
| `unreadable-path-dark.png` | 217991 | `0a5620746de0f40a750adfe996aceccab8aafdb84893328c88c12c8039cd85c8` |
| `partial-guard-dark.png` | 403455 | `044ed5381a664be3abfd98bfe5a7c58b40f61c3320d1992828bdbbb3aacca41a` |
| `metadata-screen-light.png` | 437918 | `ded3538e30f4ff0b1eb445848d535b8487db62a604b8ed01b51bd59224da4297` |
| `metadata-confirm-light.png` | 497866 | `8198a5d1fe2a0bb552ffb3317ebbe9d09c80d5f07f0558fbdf00b93474a47769` |
| `metadata-ready-light.png` | 105780 | `25a93a3caf88af8f36f532ebe142136f86d9da56a77c4e36c13fa91149d1793e` |
| `occupied-rename-light.png` | 437918 | `ded3538e30f4ff0b1eb445848d535b8487db62a604b8ed01b51bd59224da4297` |
| `occupied-error-light.png` | 511622 | `758172cf66366d57a836472cab02e284c15a069be5864d58acd3987c722ee974` |
| `unreadable-path-light.png` | 232044 | `8f5cb8c5fd2064a4277cb99d45b10499272373ea53f6d86add9e6fa03742f55c` |
| `partial-guard-light.png` | 437918 | `ded3538e30f4ff0b1eb445848d535b8487db62a604b8ed01b51bd59224da4297` |


Within each theme, the metadata screen, occupied rename screen, and partial guard screen have identical image bytes because the same configured path and destination-exists flag produce the same moving screen. Each is a fresh capture from a separate owned process. The filesystem checks distinguish those scenarios. The ready and error captures provide visible positive controls.
