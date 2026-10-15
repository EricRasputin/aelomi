# Page-record rebase and shared quit proof

This record proves source commit `1b0cbeed0ba08c9ec4f0dfee5817564f4e740f29` against merge base `a39625b427c8675d4ca2a5ffa62bc8c1de2aa82e`. Its patch-id is `2aa0c4298f5fa452da66ecf0b017694488bba091`.

The proved pathspecs are `crates/core/src/workspace/`, `crates/core/src/storage/`, `src/ipc/pending-commands.ts`, `src/ipc/record-commands*`, `src/ipc/quit-invocations.test.ts`, `src/ipc/index.ts`, `src/ipc/mock*`, `src/ipc/bindings.ts`, `src/editor/quit-drain*`, `src-tauri/src/commands.rs`, `src-tauri/src/commands/page_records_tests.rs`, `src-tauri/src/quit.rs`, `src-tauri/src/storage.rs`, `src/dev/`, `src/router.tsx`, `src/main.tsx`, `src/stores/page-tree*`, `src/stores/index-status.ts`, `scripts/check-all*`, `scripts/wait-for-quiet.ts`, `.github/workflows/ci.yml`, `perf/budgets*`, `perf/cold-start*`, `perf/fixtures/`, `perf/stats.test.ts`, `ARCHITECTURE.md`, and `.claude/skills/verify-aelomi/features/`.

## Shared quit model

One `Set<Promise<string[]>>` owns settings invocations and both queued and running record promises. A record completion returns its unsaved document IDs to that registry. Settings keep their existing command reply and native health behavior. One frontend admission state closes immediately before completion and reopens on Open or a newer Draining revision.

The record queue retains FIFO transport calls and coalesces only consecutive queued renames of the same page. Native record admission remains derived from the quit phase and revision under one mutex. The record wait and Store flush share the existing deadline. Settings keep their native in-flight admission and flush behavior.

Model the Domain chose one promise registry instead of two drain models. Test Behavior, Not Implementation chose held native invokes and exact reopened values. Prove It Works requires the relaunches below. Sequence Work into Verifiable Units kept the rebase/tracker merge and the quiet-wait change as separate commits. One implementation delegate owns all edits and proof. The manual no-comments pass found only public API contract comments added by this branch, deleted zero comments, and added no suppressions or constraints. No subagent, review gate, independent verifier, or merge ran.

## Rebase and detecting checks

All 29 branch commits were rebased onto main with settings and shortcut behavior retained. Bindings were regenerated with `pnpm bindings`. Tests were adapted to the shared `Pending { docs, settings }` result and health flag, and the settings test clock gained the record clock method. No settings persistence algorithm changed.

`src/ipc/quit-invocations.test.ts`, `SHELL-9 one command drain waits for both a rename and settings before both land at quit`, failed before the registry merge because it completed while the rename remained pending. It passes with the shared tracker and asserts both exact values and the command order. Existing held-settings, document-delivery, admission, FIFO, coalescing, and record-refusal tests pass.

`src-tauri/src/commands/page_records_tests.rs`, `complete_quit_waits_for_a_rename_and_settings_and_reopens_with_both`, holds the record mutex and the settings disk write independently. Quit waits after each hold. A simulated crash and reopen produce exactly the new title and Dark settings.

`check-all.test.ts` checks that both CI and local command sequences place the quiet wait immediately before each cold-start variant. Its existing command, environment, and job parity checks pass.

The source workspace suite passed all 439 Rust tests. The committed-head gate passed all 210 Vitest cases. The existing SPEC coverage remains:

- SHELL-2, DATA-1 and DATA-2. `workspace/tests/seed.rs`, `a_new_folder_opened_twice_has_one_getting_started_page_with_the_same_id_and_three_lines`, and `all_trashed_or_all_erased_workspaces_are_never_reseeded`.
- SHELL-8 and SHELL-9. `workspace/tests/ack.rs`, `typing_paste_drag_rename_move_trash_and_restore_are_flushed_within_100_ms`, plus the native command and mixed-write cases above.
- DATA-2 and TREE-12. `workspace/tests/edits.rs`, `trash_restore_preserves_tree_sibling_order_and_favorites_field_by_field`, the tied-restore and newly-visible-child cases, and both cycle restore cases.
- TREE-5 and TREE-9. `workspace/tests/edits.rs`, `moving_out_of_a_repaired_cycle_keeps_both_pages_roots_and_trashes_only_the_moved_page`, and `random_create_move_trash_restore_keeps_an_ordered_tree`.
- SHELL-10. Existing failed/slow/unreadable/corrupt seed tests, stale timer and refused-then-retried quit tests remain passing.

## Scope decisions

The round-3 Consider items stay with the orchestrator's follow-up work. Restore may still shift a close successor unnecessarily. Splitting the restore planner and measuring 10k-page edits remain deferred. The noted Store-only phase race, reply accessor types, refusal-list exhaustiveness, no-op appends, and storage-level record-wait timeout test remain unchanged. Move and restore still have no shell commands or UI. The earlier evidence records remain historical and are not edited after the rebase.

The dev header exposes the existing Settings probe from a seeded page. Its extra rename button allows two real IPC edits immediately before native quit. It ships only in dev builds.

## Source provenance and calibration

The native runs and all three hosted series ran on the final runtime source `53b8763c1ae1b4a46d193113fe67767bbde72578`. The source commit above adds only `perf/budgets.json` and its selector test. `git diff 53b8763c1ae1b4a46d193113fe67767bbde72578 <source> -- <native pathspecs>` is empty. The native app does not read performance budgets. No app code changed after the proof. The 13 calibration, budget-selector and parity cases pass with the new values.

Each hosted series is one discarded warm-up and ten timed launches per variant on macos-15, Node v24.20.0, three Apple M1 virtual CPUs. The populated measurement is the editable Getting started page on the seeded folder after warm-up. The empty-workspace fixture has stored empty workspace history, so the seed cannot change that variant. The summary script verifies all 60 timed launches exit 0, report the expected variant, and reproduce the recorded medians. This is calibration, with no speedup or bottleneck attribution claim.

The first run and two job reruns use [workflow run 38816440437](https://github.com/aelomi/aelomi/actions/runs/38816440437) on that source. Each series was downloaded before requesting the next rerun. `hosted-reruns.py.txt` records the bounded job-only sequence.

| Series and job | Index start load1 | Index navigation / end-to-end medians, ms | Page start load1 | Page navigation / end-to-end medians, ms |
|---|---:|---:|---:|---:|
| [1, attempt 1, job 116703847882](https://github.com/aelomi/aelomi/actions/runs/38816440437/job/116703847882) | 4.97 | 228 / 1319 | 2.43 | 474.5 / 1855 |
| [2, attempt 2, job 116707102319](https://github.com/aelomi/aelomi/actions/runs/38816440437/job/116707102319) | 5.27 | 283 / 1622 | 2.22 | 443.5 / 1717.5 |
| [3, attempt 3, job 116709218715](https://github.com/aelomi/aelomi/actions/runs/38816440437/job/116709218715) | 8.45 | 262.5 / 1530.5 | 2.50 | 424 / 1687.5 |

Both CI and check:all wait for load1 below 2.5 for up to 120 seconds immediately before each variant. The page waits ended at 60, 101, and 120 seconds. Series 3 printed a rounded load1 of 2.50 with no page-timeout message. All three index waits reached the 120-second limit and logged that they were measuring anyway. These limits are retained rather than relabeled as quiet starts.

The method yields `ceil(1.5 × max(474.5, 443.5, 424)) = 712 ms` for `hostedPageNavToPaintMs`, and `ceil(1.5 × max(1855, 1717.5, 1687.5)) = 2783 ms` for `hostedPageCiMs`. Both page entries remain separate. The index ceilings and local 500 ms goal are unchanged. Both page `why` texts record the method, all three medians, and the corresponding page-start loads. The builder claims no countersign.

Run the summary from the repository root with `python3 <copy of hosted-summary.py.txt> <series-1> <series-2> <series-3>`. The downloaded series directories remain local under this worktree's `.audit/fix3/`.

## Native self-proof

The native run used macOS 27.0.1 build 26A434 on an arm64 Apple M1. All five launches passed doctor. The first launch explicitly removed any inherited data override so app.sh created a fresh synthetic folder. Every later launch explicitly reused that folder, and every startup log names it. The keyboard lock covered all keys, focus, press actions, and transient screenshots. The script released the lock between scenarios and on exit. All owned processes are cleaned up.

The local run directories under `~/.local/state/aelomi-verify/` are:

- `20261016-041423-dev-hTwF`, fresh seed and typing.
- `20261016-041523-dev-A0iF`, typed text after relaunch and immediate rename before quit.
- `20261016-041555-dev-McQV`, immediate rename after relaunch and refused-then-retried quit.
- `20261016-041648-dev-yjve`, retry rename after relaunch and settings plus rename before quit.
- `20261016-041729-dev-kekf`, both values after relaunch and exact settings-file equality.

Every startup reports `tree warm pages=1`. The final read-only index query returns exactly `getting-started|Settings and rename quit proof|1`. The typed `fix round three` text remains with all three original instruction paragraphs. The retry run reports:

```text
storage event health={"Failing":{"reason":"PermissionDenied","pending_docs":["probe"],"pending_settings":false}}
quit draining via=ExitRequested revision=1
quit flush via=CompleteQuit pending=Some(Pending { docs: [DocId(probe)], settings: false })
quit draining via=ExitRequested revision=3
page rename accepted title=Retried quit rename proof
quit flush via=CompleteQuit pending=None
```

The combined run reports:

```text
aelomi-dev-settings-probe updated {"theme":"Dark","open_page":{"page_id":"probe-page","top_visible_block_id":"probe-block"},"recent_pages":["probe-page","older-page"],"expanded_rows":["parent-page","probe-page"]}
page rename accepted title=Settings and rename quit proof
quit draining via=ExitRequested revision=1
quit flush via=CompleteQuit pending=None
quit confirmed via=drain
quit flush via=Exit pending=None
```

The persisted settings compare exactly to:

```json
{"expandedRows":["parent-page","probe-page"],"openPage":{"pageId":"probe-page","topVisibleBlockId":"probe-block"},"recentPages":["probe-page","older-page"],"theme":"Dark"}
```

The read-only `ls -la ~/Library/Application\ Support/Aelomi` listings before the first launch and after the last are identical. No default-directory instance was launched. The lock is released.

All nine screenshots were inspected before upload. [The PR comment](https://github.com/aelomi/aelomi/pull/84#issuecomment-6203886880) shows the seed, typing, three title relaunches, refused-quit dialog, and settings before and after. Each anonymous attachment download is a decoded PNG with the same byte count and SHA-256 as its local original. GitHub CLI 2.102.0's cached archive matched its published checksum `da922c20d1792e5b2cbf375593d7a658acf034c12c84e007e71c76ef959c337e` before use. Logs, trees, doctor output, raw series, and downloaded PNGs stay local.

| Screenshot | Bytes | SHA-256 |
|---|---:|---|
| `after.png` | 105120 | `1203bb1505f1b00aba764d6559de7189f80c5948692edb775fd91522cd86a909` |
| `before.png` | 103201 | `05d188e357dfeab2e1e1645d6422a424fee7966ddcaee71cfa33a2f2b5908fa1` |
| `relaunch.png` | 105053 | `7e97964f003d60e18d4e57de3b05751a5362fc4b3caf431e33db0d78a5d0a2d9` |
| `refused-quit.png` | 216111 | `0dc5a087fc9b9c90d8b54dc954f31b410f15b91f94ffb14297727397819e4ea1` |
| `renamed-relaunch.png` | 105468 | `da3505e24789ddf275c1f0239e3736889c1623684fe5548840e527aa03820dcf` |
| `retried-rename-relaunch.png` | 106959 | `801adafcfe43e56fb7256c93c558353d745735a40f8ffa8e3cadbbf3a547e2af` |
| `settings-before.png` | 108179 | `1cb7a74cab14640a887a06c85785765948bab5aa8ea74d98d3e95a80e0b584b0` |
| `settings-and-rename-relaunch.png` | 110118 | `eedbdac1b6f2e9cdcc4dc98899aa0902126c3e9ccfd3c2ba01819453434fc636` |
| `settings-reopened.png` | 130732 | `2c8bdb7a331b8b62b0ba462a23ed389885e22f42adfd875da6934ea3c010494d` |

To replay the native proof, copy `native.sh.txt` and `combined.js.txt` to `.audit/fix3/native.sh` and `.audit/fix3/combined.js`, then run the shell script from the repository root. The combined action helper resolves both buttons before pressing either and sends native ⌘Q without waiting for command replies. `download-screenshots.py.txt` verifies the published bytes against the table above.

## Committed-head gate

`pnpm check:all` passed on runtime source `53b8763c1ae1b4a46d193113fe67767bbde72578`, including all 439 Rust tests, 210 Vitest tests, acceptance and WebKit suites, and the runner-calibrated gates. Its local informational results were 547.5 ms for seeded-page cold start and 21 ms full key-to-paint p95 for the large editor, above their 500 ms and 19 ms goals. These are noisy local misses, not local-budget passes.

The complete gate runs again after this evidence commit with the recalibrated budgets. PR #84's Verification section cites that final head and its `.local-gate/<sha>.checks.json`. Only the JSON pass record is copied to the orchestrator's allowed local-gate directory. The orchestrator still owns the review gate and independent verifier. This builder did not merge or countersign.
