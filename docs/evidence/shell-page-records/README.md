# Page records and Getting started

Source commit `1a6a0a008aabad7534d4f90053628cdea9ba8a4a` was tested against merge base `a4db19d8ab2e1ac072dea8bc3a1cc3747af8fdae`. Its patch-id from `git diff --full-index --binary <base> <source> | git patch-id --verbatim` is `9336c9ed825bf528dd45f57837a46796f1203164`.

The proved paths are `crates/core/src/storage/clock.rs`, `crates/core/src/storage/fs.rs`, `crates/core/src/storage/store.rs`, `crates/core/src/workspace/`, `src-tauri/src/commands.rs`, `src-tauri/src/commands/`, `src-tauri/src/storage.rs`, `src/ipc/`, `src/stores/page-tree.ts`, `src/stores/page-tree.test.ts`, `src/stores/index-status.ts`, and `perf/cold-start.ts`. Every path matches the change from that merge base. This record covers DATA-2 writers, SHELL-2, record and body edits in SHELL-8 and SHELL-9, and the core parts of TREE-5, TREE-9 and TREE-12.

## Environment and runs

The native app ran on macOS 27.0.1 build 26A434, arm64, through `pnpm dev:app`. It used synthetic data in its isolated `AELOMI_DATA_DIR`. Both runs passed every `app.sh doctor` check. No daily-use folder was opened.

Local run directories are `~/.local/state/aelomi-verify/20261015-214238-dev-bB1a` and `~/.local/state/aelomi-verify/20261015-214451-dev-pWVl`. The second run reused the first run's `data` folder. Logs, accessibility trees and doctor output stay there. Both instances were cleaned up after normal ⌘Q exits.

## Steps and results

1. Launch on a fresh folder. The window shows Getting started with `⌘K to search and run commands`, `/ to insert a block`, and `[[ to link a page`. The accessibility tree exposes `AXTextArea "Page body"`, so the frontend body guard accepted the Rust writer's canonical body.
2. Focus Page body and type ` seed proof`. The first paragraph becomes ` seed proof⌘K to search and run commands`. The screenshot shows the actual edit.
3. Quit with ⌘Q and relaunch on the same folder. The same text is visible in the editable body. Both source-tree logs say `tree warm pages=1`. A read-only cache query after each launch returns exactly `[("getting-started", "Getting started")]`.
4. Quit normally again. Each run logs `quit flush via=CompleteQuit pending=[]` and `quit flush via=Exit pending=[]`.

The core tests independently read the source records and `body::read_body`. `a_new_folder_opened_twice_has_one_getting_started_page_with_the_same_id_and_three_lines` asserts one page ID across opens and every exact paragraph. `all_trashed_or_all_erased_workspaces_are_never_reseeded`, `a_crash_after_the_seed_body_but_before_the_record_reuses_the_body_and_page_id`, and `even_an_empty_stored_workspace_update_prevents_seeding` cover seed history and retry behavior.

The writer tests cover concurrent edits, random create/move/trash/restore sequences against raw parent and order fields, exact restoration of records except `updatedAt`, unknown fields and renamed-title search. The manual-clock test names typing, paste, drag, rename, move, trash and restore, checks every durability acknowledgement against 100 ms, crashes SimFs, and asserts the exact body and records after reopen. Settings belongs to PR B.

The IPC tests use Tauri MockRuntime with the production core and a failing SimDisk. They prove accepted pending edits, immediate source events, subtree batches, and typed refusals. The frontend tests prove source refresh during index rebuild and write failure, and discard a stale tree reply. The cold-start harness now expects the seeded page by default and retains explicit `--expected-variant index` for deliberately empty folders.

Before native proof, cargo-nextest passed 370 tests with 8 intentional ignores, Vitest passed 169 tests, and typecheck and lint passed. The full committed-head gate record is cited in the PR's Verification section. No reviewer or verifier agent ran.

## Screenshots

The [before and after comment](https://github.com/aelomi/aelomi/pull/84#issuecomment-6196743052) and [relaunch comment](https://github.com/aelomi/aelomi/pull/84#issuecomment-6196762649) hold the screenshots. Every public attachment was downloaded without authentication and matched its local file byte for byte.

| File | Bytes | SHA-256 |
|---|---:|---|
| `before.png` | 90411 | `b34ff4f043b00786a025a9b2e029b45ed413b8ffcf58077f7e7586b0fd14429b` |
| `after.png` | 92211 | `eb93b2ce4257d77342e54ef39df3efd84afe319596033d406e3758cfe0f54aac` |
| `relaunch.png` | 97583 | `45fcd9bcf1917392367278e064e8ca7969656aed36c975b681c6e6a712b085d8` |

GitHub CLI 2.102.0 uploaded the images with `--attach`. Its official macOS arm64 archive matched the release's SHA-256 checksum before extraction. Screenshots contain only the synthetic seed and typed marker.

## Limits

The native proof exercises the seed and editing through a real user path. Create, rename and trash commands have no new UI in this PR; their IPC behavior is proved through MockRuntime and frontend logic tests. Move and restore stay core-only. Sidebar, empty state, editable title, settings and expanded state belong to later stack layers. The random property test covers the four operations owned by this brief; favorite, unfavorite and permanent erasure remain for section 2.
