# Observable record retries and native recovery

This record proves source commit `12a203541ecc461ee541f005006a06672a0046ab` against merge base `6d64d683d5bf02bea2f67e8038428caef1ab4d62`. Its patch-id is `d00219ecea4fe34607cf30b61d31ab0b6fe700b2`.

The proved pathspecs are `crates/core/src/workspace/`, `crates/core/src/storage/`, `src/ipc/`, `src/editor/quit-drain*`, `src-tauri/src/commands.rs`, `src-tauri/src/commands/page_records_tests.rs`, `src-tauri/src/quit.rs`, `src-tauri/src/storage.rs`, `src/dev/`, `src/router.tsx`, `src/main.tsx`, `src/stores/page-tree*`, `src/stores/record-commands.ts`, `src/stores/index-status.ts`, `src/components/storage-notices.tsx`, `src/storage-copy.ts`, `tests/playwright/save-failure.spec.ts`, `scripts/check-all*`, `scripts/wait-for-quiet.ts`, `.github/workflows/ci.yml`, `perf/budgets*`, `perf/cold-start*`, `perf/fixtures/`, `perf/stats.test.ts`, `ARCHITECTURE.md`, and `.claude/skills/verify-aelomi/`.

This updates the single final-head record requested by the fix briefs. Git retains earlier versions. The hosted-series provenance below remains the source of the unchanged 712 ms and 2783 ms page ceilings.

## Data shape and decisions

`useRecordCommandStore` holds `retained: Map<OperationId, RetainedRecordOp>` and a `failing` selector derived from its size. Each retained value has exactly this shape:

```ts
type RetainedRecordOp = {
  id: OperationId;
  op: "create" | "rename" | "move" | "trash" | "restore";
  pageId: PageId;
  error: PageError | string;
  attempts: number;
  since: number;
};
```

One FIFO still owns record retry. The store publishes a new map on retain, attempt and forget. The banner reads it alongside native health and document deliveries. A retried operation stays retained after acceptance until `storage_flush` confirms durability. Later confirmation attempts do not replay an accepted edit. This closes the healthy-storage, pre-acceptance failure gap without moving command retry native.

The public wrappers keep their argument lists and now resolve with `RecordCommandResult<T>`:

```ts
type RecordCommandResult<T> =
  | { status: "ok"; data: T }
  | { status: "error"; error: PageError | string }
  | { status: "retrying"; pageId: PageId; operationId: OperationId };
```

Create chooses a page ID once. A second user create for the same parent while that create is retained returns the same operation and ID, so recovery creates exactly one page. Rename and trash also tell callers `retrying` rather than reporting a final failure for a live operation. Move and restore remain core-only.

Only typed record `Io` and uncertain invokes retry. `Internal` now identifies a blocking-pool JoinError or panic. Every terminal record error dequeues, including refusals on retry; only retry-time `Quitting` stays retained. Terminal storage-flush refusals dequeue too. The next queued operation can then run. The generic command tracker stores `{ retry, docs }`, sorts and deduplicates failure docs, and `drainCommands()` returns void so quit reads current retained failures after all in-flight work settles.

All three Consider items were accepted. Create replay requires the original parent, active trash state and title. A new record cannot reattach an ID with existing body state; mismatches return terminal `AlreadyExists`. Dead `MissingBatch` was removed from core, IPC and generated bindings. Restoring an unknown batch is a successful no-op. Record validation, order allocation, create, rename and trash now live in `mock-records.ts`; `mock.ts` is 964 lines.

Model the Domain kept FIFO ownership and added operation identity. Make Operations Idempotent kept create IDs stable and required matching replay content. Test Behavior, Not Implementation chose exact recovered titles and page counts. Prove It Works required the native permission and relaunch proof. Sequence Work into Verifiable Units put detecting tests before the implementation commit. A direct no-comments pass added no comments or suppressions. The builder spawned no agents and ran no review gate or independent verifier.

## Detecting tests

Tests were committed before implementation in `0f45212`, `eb36d3c` and `cfba23a`. The initial result/store tests failed against the previous replies. The create-conflict and orphan-body tests failed against the previous core writer. Isolated regression checks also failed with the old banner, terminal-error retry policy and Io panic mapping restored. The corrected native panic injector targets a new body document, so the index thread cannot consume its fault.

`src/ipc/record-recovery.test.ts` covers these cases:

- `SHELL-10 a second user create reuses the retained page and lands exactly once`.
- `SHELL-10 retained rename publishes every attempt and stays visible until durable recovery`.
- `SHELL-10 trash reports retrying before the retained operation lands`.
- `a terminal head %j lets the next queued operation run`, for Unavailable, InvalidUpdate, Malformed and Internal.
- `a %s refusal on retry dequeues the operation`, for NotActive and OrderExhausted.
- `Quitting on retry retains the operation until admission recovers`.

Additional cases are:

- `src/ipc/quit-invocations.test.ts`, `SHELL-10 Keep open preserves a failed head and queued edit until both recover in order`.
- `tests/playwright/save-failure.spec.ts`, `SHELL-10 a pre-acceptance rename failure shows the banner with healthy storage and clears on recovery`.
- `crates/core/src/workspace/tests/edits.rs`, `create_replay_requires_matching_parent_title_and_active_record` and `create_cannot_reattach_an_orphaned_body_document`.
- `src-tauri/src/commands/page_records_tests.rs`, `a_record_planner_panic_returns_terminal_internal_and_releases_admission`.
- `src/stores/page-tree.test.ts`, `mock create replays require the original active placement and empty title and reject orphaned bodies`.

Existing FIFO, coalescing, lost-reply create/trash, backoff, late-failure and repeated-quit cases now assert the new reply contract. Focused runs passed 76 TypeScript cases, 17 core edit cases, seven native IPC cases and five WebKit warning cases. `pnpm bindings`, `pnpm typecheck` and lint passed. The PR Verification section names the final committed-head `check:all` artifact, produced after this evidence commit.

The complete branch retains these SPEC tests:

- SHELL-2 and DATA-1/2. `crates/core/src/workspace/tests/seed.rs`, `a_new_folder_opened_twice_has_one_getting_started_page_with_the_same_id_and_three_lines` and `all_trashed_or_all_erased_workspaces_are_never_reseeded`.
- DATA-2, SHELL-4 and SHELL-8. `src-tauri/src/commands/page_records_tests.rs`, `page_commands_accept_pending_edits_and_publish_source_changes_before_durability`. `crates/core/src/workspace/tests/edits.rs` also covers `pending_independent_record_fields_win_after_rename_in_both_client_orders` and `index_search_finds_a_title_renamed_through_the_production_writer`.
- SHELL-9. `crates/core/src/workspace/tests/ack.rs`, `typing_paste_drag_rename_move_trash_and_restore_are_flushed_within_100_ms`. Native held-rename and mixed rename/settings tests remain in `src-tauri/src/commands/page_records_tests.rs`.
- SHELL-10. The four detecting frontend cases above, the exact recovery and backoff cases, and the merged pending-doc timeout case.
- TREE-5 and TREE-9. `crates/core/src/workspace/tests/edits.rs`, `moving_out_of_a_repaired_cycle_keeps_both_pages_roots_and_trashes_only_the_moved_page` and `random_create_move_trash_restore_keeps_an_ordered_tree`.
- TREE-12. The same file's `trash_restore_preserves_tree_sibling_order_and_favorites_field_by_field` and `trash_restore_round_trips_self_parent_and_two_page_cycles_field_by_field`.


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

## Native self-proof

The completed runs use source `12a2035` on macOS 27.0.1 build 26A434, arm64 Apple M1. Each passed doctor. Every launch went through verify-aelomi with synthetic `AELOMI_DATA_DIR`; every key, press, focus and screenshot held `/tmp/aelomi-ui.lock`. Locks were released between scenarios. The synthetic workspace folder's exact original mode, 755, was restored. All owned app processes were cleaned up. Read-only listings of the real data folder before the first and after the last launch are identical.

Run directories under `~/.local/state/aelomi-verify/` are:

- `20261016-062313-dev-aAuI`, the fresh folder, typing and immediate rename quit.
- `20261016-062400-dev-hVrF`, immediate-rename relaunch and mixed settings/rename quit.
- `20261016-062452-dev-CJ40`, mixed-write relaunch.
- `20261016-062743-dev-iB9M`, a baseline reset to a different title before repeating the failure scenario.
- `20261016-062832-dev-ZmXU`, the completed permission failure and recovery scenario.
- `20261016-062952-dev-DvCy`, recovered-rename relaunch.

The fresh folder showed the three Getting started lines and accepted ` fix round five`. Immediate rename before native ⌘Q reopened as **Quit rename proof** with that body. The mixed quit reopened as **Settings and rename quit proof**. Disk settings exactly matched Dark, open page probe-page and block probe-block, recent pages probe-page and older-page, and expanded rows parent-page and probe-page.

For pre-acceptance failure, the synthetic workspace folder was made inaccessible before relaunch. The seed deferred and page-tree read failed without changing source history. The dev Storage probe then submitted the rename. The banner showed `Permission denied (os error 13)` and the probe reported `page rename retrying title=Quit rename proof`, with no native storage failure event. Two normal quits asked with workspace pending. Keep app open preserved the operation. Restoring mode 755 let the retry land and the banner disappear. Normal quit confirmed, and the last relaunch showed **Quit rename proof** with the original typed body.

The completed failure run's shell log contains:

```text
page rename retrying title=Quit rename proof
quit draining via=ExitRequested revision=1
quit drain failed pending=Pending { docs: [DocId(workspace)], settings: false }
quit draining via=ExitRequested revision=3
quit drain failed pending=Pending { docs: [DocId(workspace)], settings: false }
quit draining via=ExitRequested revision=6
quit flush via=CompleteQuit pending=None
quit confirmed via=drain
quit flush via=Exit pending=None
```

One earlier proof attempt stopped because the Apple SQLite CLI could not open the derived cache for its read-only recovery query. Relaunch showed that the retry had reached the workspace log. The completed proof uses the banner and native relaunch instead; it does not wait for a derived-index query. A first standalone launch also lost its process before doctor when its tool session ended. The completed scenarios keep launch, drive and cleanup in one session. Neither failed attempt is counted as proof.

`drive.sh.txt` owns the per-scenario keyboard lock and permission-restoration trap. `run-native.sh.txt` launches and cleans up each scenario on one synthetic folder. To reproduce from the repository root, copy the driver to a temporary `.sh` file and run `bash docs/evidence/shell-page-records/run-native.sh.txt /absolute/path/to/driver.sh /absolute/path/to/local-output`. Keep the terminal session alive until it finishes. Build and launch happen before each scenario takes the lock.

## Screenshots

All eleven images were inspected. They show the seeded lines, typed text, durable immediate and mixed renames, the inaccessible baseline, the retained-operation banner, both quit questions, the cleared banner and the recovered relaunch. The probe keeps displaying its original `retrying` reply after recovery; the cleared banner and final relaunch show the current state.

[PR screenshot comment](https://github.com/aelomi/aelomi/pull/84#issuecomment-6205607095). Every attachment was downloaded anonymously and matched its local PNG byte-for-byte by size and SHA-256.

| Run | Filename | Bytes | SHA-256 |
|---|---|---:|---|
| `20261016-062313-dev-aAuI` | `seed-before.png` | 101298 | `af099bfb411268075f74c8eb35d795d113bca9e3204e4d83a6d990bbc34a40ca` |
| `20261016-062313-dev-aAuI` | `typed-before-quit.png` | 103230 | `8cd95f75d41ad5238f3c45115a7735b9160228ab88e39c24d922dfdccf2cd6e6` |
| `20261016-062400-dev-hVrF` | `immediate-rename-reopened.png` | 103455 | `f9da4f255d8d6f4c0b8af5d935bfa7302800ec77dc2755644910b9654eef0a75` |
| `20261016-062400-dev-hVrF` | `settings-before.png` | 106109 | `42d5cd75f205c0f074a05adf389b461277632e5cf075188b47d2020e41121710` |
| `20261016-062452-dev-CJ40` | `settings-rename-reopened.png` | 108000 | `1775261f7fafd0e57a2e2c7b010d40f39eab996d32a7a4cb7c005a0fc1c8d1a0` |
| `20261016-062832-dev-ZmXU` | `failure-before.png` | 114472 | `21fb5c9a07e2dce332c9ccf57af00e6169c242cc3861c5478083d68226623824` |
| `20261016-062832-dev-ZmXU` | `retained-rename-banner.png` | 128970 | `012498dc646ed15519d52519cb0c1c777418cd801e0495c7b73acd12c56cfd1e` |
| `20261016-062832-dev-ZmXU` | `first-refused-quit.png` | 208382 | `1dfcabf2d3fd95eb4330d2e4ace537bd7dff6994cdffdba3722bba620acb0609` |
| `20261016-062832-dev-ZmXU` | `second-refused-quit.png` | 208054 | `1003de72c6e43004b98411b20ea5198432f9f8f85197b90c9d5f53578b0cac53` |
| `20261016-062832-dev-ZmXU` | `recovered-banner-cleared.png` | 109832 | `17ebdb9f19591812d9090cd41c1337969c5dd55f4ed3944fd9ffbf1dce21119f` |
| `20261016-062952-dev-DvCy` | `recovered-rename-reopened.png` | 103626 | `1a3866a66797ae175872b3f036e836d6c740e92b9b23b11eba7dd67403e3df5c` |

## Boundaries left for follow-ups

The settings snapshot writer still owns native write retry. Settings failures before native acceptance need their own retained patch model; this round only makes the generic tracker reusable for it. Sidebar actions and move/restore commands remain follow-ups. Optional native create IDs, empty no-op appends, trash replay distinctions, the unused second-quit probe and shared listener-delay cleanup remain the previously deferred #87 work. The fixed seed body needs its sync-specific deterministic identity before v3. No performance budget changed and no new calibration or countersign is claimed. The orchestrator owns fresh review and independent verification.
