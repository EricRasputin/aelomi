# Page records and retained quit failures

This record proves source commit `a61adebb12dd1f215d0edd5042757d658a2c059b` against merge base `96cdc94ccb48be742d5bdc907de1f609f4a747fe`. Its patch-id is `fa1db9f69addf85850c26c9406f8d72153dace44`.

The proved pathspecs are `crates/core/src/workspace/`, `crates/core/src/storage/`, `src/ipc/`, `src/editor/quit-drain*`, `src-tauri/src/commands.rs`, `src-tauri/src/commands/page_records_tests.rs`, `src-tauri/src/quit.rs`, `src-tauri/src/storage.rs`, `src/dev/`, `src/router.tsx`, `src/main.tsx`, `src/stores/page-tree*`, `src/stores/index-status.ts`, `scripts/check-all*`, `scripts/wait-for-quiet.ts`, `.github/workflows/ci.yml`, `perf/budgets*`, `perf/cold-start*`, `perf/fixtures/`, `perf/stats.test.ts`, `ARCHITECTURE.md`, and `.claude/skills/verify-aelomi/scripts/`.

This single record replaces the earlier shell-page-records records at the round-4 brief's request. Their historical contents remain in Git. The calibration provenance below stays because the accepted performance ceilings depend on it.

## Data shape and guarantees

Each FIFO job owns its invocation and first reply promise, with queued, running, or failed state. Failure retains that exact job independently of the promise, blocks newer record jobs, and registers a retry with the shared command tracker. Every drain retries retained jobs and reads current failures when admission closes. The next record command and a timer also retry them. Timer delays use the shared `storageRetry` policy from main. They start at 100 ms, double, and cap at 5 s. The FIFO still owns its timer because record operations must serialize, while document byte updates commute. A successful retry removes only its own failure. Quit anyway explicitly permits exit with outstanding operations.

A document-level failure set alone cannot recover independent edits, and clearing it after any successful edit could hide an older failed operation. The FIFO keeps the operations and their order. Model the Domain chose FIFO ownership. Make Operations Idempotent required client-chosen create IDs. Test Behavior, Not Implementation chose exact invocation payloads and recovered titles. Prove It Works requires native relaunches. Sequence Work into Verifiable Units kept detecting tests before the fix. Laziness Protocol removed the obsolete promise-failure callback channel.

The exported TypeScript wrappers remain `pageCreate(parentId)`, `pageRename(pageId, title)`, and `pageTrash(pageId)`. Only the native create transport and generated binding add an optional `pageId`, supplied once by the boundary. Native callers may still omit it.

Create replays return the existing client-chosen ID without replacing its record. Rename replays preserve an unchanged title and timestamp. Move replays see the same parent and position and leave placement unchanged. Trash replays return the existing batch. Restore replays with no remaining batch members change nothing. Core tests advance the clock between attempts and compare every record field. Unknown fields remain preserved by the existing field-delta writer. Empty core transactions still append an empty delta, the previously deferred no-op-history cleanup.

## Detecting tests

All four required frontend tests failed before the fix by reaching Confirmed with the rename unsaved. They now pass in `src/ipc/quit-invocations.test.ts`:

- `SHELL-10 a record failure during the drain's initial yield is reported`.
- `SHELL-10 a record failure after command collection while deliveries are held is reported`.
- `SHELL-10 a second normal quit retains a refused rename until recovery saves it`.
- `SHELL-10 a drain superseded by the five second timer retains a failure that settles afterward`.

That file also adds `SHELL-9 a new drain after Asking admits both rename and settings before completion`.

`src/ipc/record-commands.test.ts` adds FIFO recovery, uncertain create and trash replies, and the exact capped timer schedule. `crates/core/src/workspace/tests/ack.rs` adds `rename_move_trash_and_restore_replays_preserve_records_with_a_later_clock`. `src-tauri/src/commands/page_records_tests.rs` adds `client_chosen_create_and_trash_replays_return_the_same_ids_and_records`. Both core timestamp and native client-ID tests failed before their fixes. `src-tauri/src/storage.rs` adds `record_wait_timeout_keeps_all_failing_documents_sorted_and_deduplicated`, which failed with only workspace in its Asking list before the timeout fix.

The complete branch retains these SPEC tests:

- SHELL-2 and DATA-1/2. `crates/core/src/workspace/tests/seed.rs`, `a_new_folder_opened_twice_has_one_getting_started_page_with_the_same_id_and_three_lines` and `all_trashed_or_all_erased_workspaces_are_never_reseeded`.
- DATA-2, SHELL-4 and SHELL-8. `src-tauri/src/commands/page_records_tests.rs`, `page_commands_accept_pending_edits_and_publish_source_changes_before_durability`. `crates/core/src/workspace/tests/edits.rs` also covers `pending_independent_record_fields_win_after_rename_in_both_client_orders` and `index_search_finds_a_title_renamed_through_the_production_writer`.
- SHELL-9. `crates/core/src/workspace/tests/ack.rs`, `typing_paste_drag_rename_move_trash_and_restore_are_flushed_within_100_ms`. Native held-rename and mixed rename/settings tests remain in `src-tauri/src/commands/page_records_tests.rs`.
- SHELL-10. The four detecting frontend cases above, the exact recovery and backoff cases, and the merged pending-doc timeout case.
- TREE-5 and TREE-9. `crates/core/src/workspace/tests/edits.rs`, `moving_out_of_a_repaired_cycle_keeps_both_pages_roots_and_trashes_only_the_moved_page` and `random_create_move_trash_restore_keeps_an_ordered_tree`.
- TREE-12. The same file's `trash_restore_preserves_tree_sibling_order_and_favorites_field_by_field` and `trash_restore_round_trips_self_parent_and_two_page_cycles_field_by_field`.

## Round-4 scope decisions

- Settings failure retention is deferred. Settings already retain snapshots after a native Write failure, but Io before native acceptance, Internal, and rejected invokes need their own retained patch model and a frontend pending-settings failure channel. Sending a fake settings doc ID through the record channel would mislabel the quit warning. This inherited gap remains open.
- The record-wait timeout now merges all failing docs with workspace, sorts, and deduplicates. Native admission is named explicitly in ARCHITECTURE. The new drain test covers frontend reopen without Keep app open.
- All performance budget values are unchanged, including 712 and 2783 ms. Index ceiling descriptions now state that the stored-empty fixture inherits the earlier calibration.
- The dev Settings probe link has the Storage probe spacing. Shared verify helpers now own rename-quit and both-quit. The forked combined.js.txt was removed.
- No nested agents, independent verifier, review gate, or merge ran. A direct no-comments pass found no newly added comments, suppressions, or constraint comments to remove. The implementation brief forbids nested agents.

## Source provenance and calibration

The three hosted calibration series ran on runtime source `53b8763c1ae1b4a46d193113fe67767bbde72578`. Those accepted ceilings remain unchanged in this fix round. The final committed-head gate measures the current runtime against them. This calibration remains provenance for the ceilings, rather than a measurement of this round’s runtime.

Each hosted series is one discarded warm-up and ten timed launches per variant on macos-15, Node v24.20.0, three Apple M1 virtual CPUs. The populated measurement is the editable Getting started page on the seeded folder after warm-up. The empty-workspace fixture has stored empty workspace history, so the seed cannot change that variant. The summary script verifies all 60 timed launches exit 0, report the expected variant, and reproduce the recorded medians. This is calibration, with no speedup or bottleneck attribution claim.

The first run and two job reruns use [workflow run 38816440437](https://github.com/aelomi/aelomi/actions/runs/38816440437) on that source. Each series was downloaded before requesting the next rerun. `hosted-reruns.py.txt` records the bounded job-only sequence.

| Series and job | Index start load1 | Index navigation / end-to-end medians, ms | Page start load1 | Page navigation / end-to-end medians, ms |
|---|---:|---:|---:|---:|
| [1, attempt 1, job 116703847882](https://github.com/aelomi/aelomi/actions/runs/38816440437/job/116703847882) | 4.97 | 228 / 1319 | 2.43 | 474.5 / 1855 |
| [2, attempt 2, job 116707102319](https://github.com/aelomi/aelomi/actions/runs/38816440437/job/116707102319) | 5.27 | 283 / 1622 | 2.22 | 443.5 / 1717.5 |
| [3, attempt 3, job 116709218715](https://github.com/aelomi/aelomi/actions/runs/38816440437/job/116709218715) | 8.45 | 262.5 / 1530.5 | 2.50 | 424 / 1687.5 |

Both CI and check:all wait for load1 below 2.5 for up to 120 seconds immediately before each variant. The page waits ended at 60, 101, and 120 seconds. Series 3 printed a rounded load1 of 2.50 with no page-timeout message. All three index waits reached the 120-second limit and logged that they were measuring anyway. These limits are retained rather than relabeled as quiet starts.

The method yields `ceil(1.5 × max(474.5, 443.5, 424)) = 712 ms` for `hostedPageNavToPaintMs`, and `ceil(1.5 × max(1855, 1717.5, 1687.5)) = 2783 ms` for `hostedPageCiMs`. Both page entries remain separate. The index ceilings and local 500 ms goal are unchanged. Both page `why` texts record the method, all three medians, and the corresponding page-start loads. The builder claims no countersign.

Run the summary from the repository root with `python3 <copy of hosted-summary.py.txt> <series-1> <series-2> <series-3>`. The downloaded series directories remain in the prior builder’s local `.audit/fix3/`.

## Rebase onto storage refusal and delivery retry

PR #86 landed while this round was in progress. The branch was rebased onto main at `96cdc94ccb48be742d5bdc907de1f609f4a747fe`. The sole conflict was `src/main.tsx`. The resolution keeps `connectStorageStartup` and its disposal, plus `connectPageTreeEvents` and its subscribe-before-snapshot refresh. It removes the older direct refresh to avoid a second startup owner. Records now share `storageRetry.firstMs` and `storageRetry.maxMs` with #86’s refused-byte delivery. Pre-rebase native runs are superseded. The native runs below use this merged source.

## Source checks

`pnpm bindings` regenerated no differences after the rebase, and `pnpm typecheck` passed. Focused quit, queue, page-sync, storage-event, startup and page-tree cases pass after integration. The final clean committed-head `pnpm check:all` record is cited in the PR Verification section after the evidence commit.

## Native self-proof

All five post-rebase launches passed doctor on macOS 27.0.1 build 26A434, arm64 Apple M1. They used one fresh synthetic data folder and explicit AELOMI_DATA_DIR on every reuse. All key, focus, press, combined quit, and screenshot steps held the keyboard lock. It was released between scenarios and on failure. No default-folder instance was launched.

The final local run directories under `~/.local/state/aelomi-verify/` are:

- `20261016-052301-dev-1m9x`.
- `20261016-052437-dev-4FAS`.
- `20261016-052544-dev-77HC`.
- `20261016-052624-dev-rahF`.
- `20261016-052728-dev-Qjpd`.

The first screenshot shows the original three editable instructions. The second adds synthetic text. Relaunch shows `Quit rename proof` and that text. The combined run writes settings and a distinct title just before normal quit, then relaunch shows `Settings and rename quit proof`. The persisted settings compare exactly to:

```json
{"expandedRows":["parent-page","probe-page"],"openPage":{"pageId":"probe-page","topVisibleBlockId":"probe-block"},"recentPages":["probe-page","older-page"],"theme":"Dark"}
```

The failure run revokes workspace-directory access before relaunch, because a warm source doc could otherwise accept an edit from memory. The startup screenshot shows the intended Permission denied page-list error. The rename is rejected before Store acceptance. Both normal quits remain refused, with separate Asking revisions visible in the screenshots. The folder’s original 755 mode is restored. A read-only index query must see the timer-retried `Quit rename proof` before the script sends the final normal quit. Relaunch preserves that title, the original page ID and typed body. No Keep app open or Quit anyway button is pressed.

The failure run logs:

```text
storage probe failed: {"Io":"Permission denied (os error 13)"}
quit draining via=ExitRequested revision=1
quit drain failed pending=Pending { docs: [DocId(workspace)], settings: false }
quit draining via=ExitRequested revision=3
quit drain failed pending=Pending { docs: [DocId(workspace)], settings: false }
quit draining via=ExitRequested revision=5
quit flush via=CompleteQuit pending=None
quit confirmed via=drain
quit flush via=Exit pending=None
```

The exact mode and the read-only real-data-folder listing match their originals after the last launch. All owned processes are cleaned up. The final source app files are unchanged after these runs. `run-native.sh.txt` copies the recorded drivers and reruns all scenarios. The shared verify skill owns the combined actions, so no forked accessibility helper is required.

All nine screenshots were inspected. [The PR comment](https://github.com/aelomi/aelomi/pull/84#issuecomment-6204803918) publishes them. Each anonymous attachment download is byte-identical to its local PNG, with the same size and SHA-256. The CLI 2.102.0 archive matched its published SHA-256 `da922c20d1792e5b2cbf375593d7a658acf034c12c84e007e71c76ef959c337e` before extraction. Logs and accessibility trees remain local.

| Screenshot | Bytes | SHA-256 |
|---|---:|---|
| `seed-before.png` | 101129 | `59a183cdae8071718e77f7a53d5113526b3b7367c6e2409ab652bc48ff75b6fd` |
| `typed-before-quit.png` | 102711 | `94ab034fa4f274572574c41286bdf34a3866025c90e2d0def7ba3cf92fc125c6` |
| `immediate-rename-reopened.png` | 103141 | `eb6453efc95cc7679fd7a0b978fd3b2225db1b462805b20fb14f8a5fd24037f6` |
| `settings-before.png` | 106109 | `42d5cd75f205c0f074a05adf389b461277632e5cf075188b47d2020e41121710` |
| `settings-rename-reopened.png` | 107525 | `6d79776ca6d454195c27d9587e7c5aa52a797e59a8efa2ad7904cf3b209a0a43` |
| `failure-before.png` | 114472 | `21fb5c9a07e2dce332c9ccf57af00e6169c242cc3861c5478083d68226623824` |
| `first-refused-quit.png` | 195537 | `6e7e79e71ac7db62bbaba54f4e6351a341b4d1246d60c7462362f8bda2f97909` |
| `second-refused-quit.png` | 195089 | `380eb3cea2d52f6f19b8d257ea632c2eea57a980d13a2b8657758f43062e3099` |
| `recovered-rename-reopened.png` | 103165 | `38e7d5d456ddbdf7ffb763c798954f6d14beb63e6cb51af12615a1785a4e5719` |
