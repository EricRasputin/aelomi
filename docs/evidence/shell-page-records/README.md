# Final record outcomes and native recovery

This record proves source commit `757c0a0608e455ce34ea3e4f6a1fe44e8ac4c7ae` against merge base `6d64d683d5bf02bea2f67e8038428caef1ab4d62`. Its patch-id is `18d1507b27366a1ad00330aa054deee875e7fd1a`.

The proved pathspecs are `crates/core/src/workspace/`, `crates/core/src/storage/`, `src/ipc/`, `src/editor/quit-drain*`, `src-tauri/src/commands.rs`, `src-tauri/src/commands/page_records_tests.rs`, `src-tauri/src/quit.rs`, `src-tauri/src/storage.rs`, `src/dev/`, `src/router.tsx`, `src/main.tsx`, `src/stores/page-tree*`, `src/stores/record-commands.ts`, `src/stores/index-status.ts`, `src/components/storage-notices.tsx`, `src/storage-copy.ts`, `tests/playwright/save-failure.spec.ts`, `scripts/check-all*`, `scripts/wait-for-quiet.ts`, `.github/workflows/ci.yml`, `perf/budgets*`, `perf/cold-start*`, `perf/fixtures/`, `perf/stats.test.ts`, `ARCHITECTURE.md`, and `.claude/skills/verify-aelomi/`.

This updates the single final-head record requested by the fix briefs. Git retains earlier versions. The hosted-series provenance below remains the source of the unchanged 712 ms and 2783 ms page ceilings.

## Data shape and decisions

One FIFO owns record jobs and their final-outcome promises. Each job carries `attempts`, first-failure `since`, typed `error`, the `accepted` reply, and `status`. One `sync()` projects retained jobs, terminal failures, pending-create parents and the head's drain entry. The IPC layer never reads control state from the observer store.

```ts
export type OperationId = string;
export type PageId = TreePage["id"];
export type TransportFailure = { Transport: string };
export type RecordCommandError =
  | PageError
  | StorageError
  | TransportFailure
  | "Abandoned";
export type RecordOp = "create" | "rename" | "trash";
export type RecordCommandResult<T> =
  | { status: "ok"; data: T }
  | { status: "error"; error: RecordCommandError };
export type RetainedRecordOp = {
  id: OperationId;
  op: RecordOp;
  pageId: PageId;
  error: RecordCommandError;
  attempts: number;
  since: number;
};
export type FailedRecordOp = {
  id: OperationId;
  op: RecordOp;
  pageId: PageId;
  error: RecordCommandError;
  at: number;
};
export type RecordCommandStore = {
  retained: Map<OperationId, RetainedRecordOp>;
  failed: Map<OperationId, FailedRecordOp>;
  pendingCreates: ReadonlySet<PageId | null>;
  dismissFailed: (id: OperationId) => void;
};
export function failing(state: RecordCommandStore): boolean;
export function pendingCreate(parentId: PageId | null): boolean;
```

The public signatures are:

```ts
pageCreate(parentId: PageId | null): Promise<RecordCommandResult<PageId>>;
pageRename(pageId: PageId, title: string): Promise<RecordCommandResult<null>>;
pageTrash(pageId: PageId): Promise<RecordCommandResult<string>>;
```

A promise stays pending through retry and, for a retained operation, durability confirmation. There is no `retrying` result. Create returns its ID only on `ok`; every requested create stays distinct. `pendingCreate(parentId)` covers queued, running and retained creates. The store's `pendingCreates` snapshot also supports reactive controls. Consecutive queued renames share one promise. After acceptance, a retained job retries only its durability confirmation.

A terminal failure after retention settles the caller with the error, moves to `failed`, and lets later jobs run. It remains in the banner until a later operation on that page succeeds or the user dismisses it. Terminal notices do not retry. Quit anyway closes admission before resolving all unsettled record promises with `Abandoned`, then invokes native exit. A callback from an abandoned caller cannot enqueue new work during exit.

All three round-6 Consider items were accepted. Typed transport failures replace unrestricted strings, and record op kinds name only wrappers that exist. The native EACCES test proves load-failure classification at IPC. Create replay checks projected active membership, so a child hidden under a trashed ancestor cannot replay as a successful create. The core and mock agree.

Model the Domain puts lifecycle facts on each job. Type System Discipline closes the error and op unions. Test Behavior, Not Implementation uses final caller results and exact stored trees. Make Operations Idempotent preserves client IDs and accepted replies across retry. Sequence Verifiable Units separates the implementation from this evidence commit. Prove It Works pairs visible native state with quit logs and relaunch values.

## Detecting tests and regressions

The four original round-6 regressions failed against round 5. The terminal test got an early `retrying` reply instead of `NotActive`; the create test saw that reply before acceptance; `pendingCreate` was absent; Quit anyway returned the earlier reply instead of `Abandoned`. The later exit-admission regression also failed before its fix, returning `ok` with a new page ID from an abandoned caller's callback. The inactive-ancestor replay regression failed before the active-set check by returning `Created(hidden-child)`.

`src/ipc/record-final-outcome.test.ts` contains these SHELL-4, SHELL-9 and SHELL-10 cases:

- `SHELL-10 an Io then terminal retry %j settles its caller and keeps the failure visible while later pages run`. The six errors are NotActive, OrderExhausted, InvalidUpdate, Unavailable, Internal and Malformed.
- `SHELL-10 a failing create exposes no page ID until acceptance and durable recovery`.
- `SHELL-4 two explicit creates behind a failing rename remain distinct and pending for the whole wait`.
- `SHELL-9 Quit anyway settles retained running and queued callers with Abandoned before native exit`.
- `SHELL-10 banner dismissal removes the terminal failure without replaying its edit`.
- `SHELL-10 resetting the observer snapshot cannot change retained create retry semantics`.
- `SHELL-9 Quit anyway refuses new work from abandoned callers`.

`src/ipc/record-recovery.test.ts` updates the round-5 contract:

- `SHELL-10 retained rename publishes every attempt and stays visible until durable recovery`.
- `SHELL-10 trash stays pending until the retained operation lands`.
- `a terminal head %j lets the next queued operation run`.
- `a %s refusal on retry reaches its caller before a later success clears its failed notice`.
- `Quitting on retry retains the operation until admission recovers`.

`src/ipc/record-commands.test.ts` keeps the FIFO, shared queued-rename promise, uncertain create and trash replay, and capped backoff cases. Its former `retrying` assertions now wait for final outcomes. `src/ipc/quit-invocations.test.ts` keeps failures during the initial yield, held deliveries, a superseded drain, a second normal quit, and Keep open with a queued edit. Those promises remain pending while the drain reports the retained workspace.

The boundary and UI cases are:

- DATA-2 and SHELL-10. `src-tauri/src/commands/page_records_tests.rs`, `page_rename_load_permission_denied_returns_retryable_io_and_recovers`. SimDisk returns real OS error EACCES from workspace loading; IPC returns exactly `{"Io":"Permission denied (os error 13)"}`, then a later rename succeeds after recovery.
- DATA-2. `crates/core/src/workspace/tests/edits.rs`, `create_replay_refuses_a_record_hidden_under_a_trashed_ancestor`.
- DATA-2. `src/stores/page-tree.test.ts`, `mock create replay refuses a record hidden under a trashed ancestor`.
- SHELL-10. `tests/playwright/save-failure.spec.ts`, `SHELL-10 a terminal retry failure stays visible until dismissed` and `SHELL-10 a pre-acceptance rename failure shows the banner with healthy storage and clears on recovery`.

The focused TypeScript set passed 62 cases on the source commit. All six WebKit save-failure cases passed, including both cases above. Both new native tests passed. The final committed-head `check:all` result and its `.local-gate/<sha>.checks.json` path are reported in the PR's Verification section.

The branch also retains its exact SHELL-2 opened-twice seed and body tests, the SHELL-9 controllable-clock edit tests, held native rename and mixed settings/rename quit tests, DATA-2 field-level merge tests, and TREE-5/9/12 cycle, random edit and restore tests.

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

The completed runs use the source commit above on macOS 27.0.1 build 26A434, arm64 Apple M1. Each passed doctor. Every launch went through verify-aelomi with synthetic `AELOMI_DATA_DIR`; every key, press, focus and screenshot held `/tmp/aelomi-ui.lock`. Locks were released between scenarios. The synthetic workspace folder's exact original mode, 755, was restored. All owned app processes were cleaned up. Read-only listings of the real data folder before the first and after the last launch are identical.

Run directories under `~/.local/state/aelomi-verify/pr84-r6/` are:

- `20261016-073821-dev-RJ0V`, fresh folder, typing and immediate rename quit.
- `20261016-073911-dev-NGDE`, immediate-rename relaunch and mixed settings/rename quit.
- `20261016-074010-dev-vYvO`, mixed-write relaunch.
- `20261016-074043-dev-q8Mg`, permission failure, two refused quits and durable recovery.
- `20261016-074210-dev-RmW9`, recovered-rename relaunch.

The fresh folder showed the three Getting started lines and accepted ` fix round six`. Immediate rename before native ⌘Q reopened as **Quit rename proof** with that body. The mixed quit reopened as **Settings and rename quit proof**. Disk settings exactly matched Dark, open page probe-page and block probe-block, recent pages probe-page and older-page, and expanded rows parent-page and probe-page.

The synthetic workspace folder was made inaccessible before relaunch. The seed deferred and the initial page-tree read failed. The dev Storage probe submitted the rename. The record banner showed `Permission denied (os error 13)` while the caller stayed pending; the probe still displayed its idle text. No native storage failure event was needed. Two normal quits asked with the workspace pending. Keep app open preserved the operation. Restoring mode 755 let the retry land, the caller finish with `ok`, and the banner disappear. Normal quit confirmed. The last relaunch showed **Quit rename proof** and the typed body.

The completed failure run's shell log contains:

```text
quit draining via=ExitRequested revision=1
quit drain failed pending=Pending { docs: [DocId(workspace)], settings: false }
quit draining via=ExitRequested revision=3
quit drain failed pending=Pending { docs: [DocId(workspace)], settings: false }
page rename accepted title=Quit rename proof
quit draining via=ExitRequested revision=6
quit flush via=CompleteQuit pending=None
quit confirmed via=drain
quit flush via=Exit pending=None
```

Earlier automation attempts are not counted. A standalone launch lost its processes when its tool session ended. Two combined quit helpers logged the edits but sent no quit to the app. The local driver now refocuses immediately before each shortcut; it generates that small change from the repository helper instead of committing a fork. One recovery attempt queried the header's first status region instead of the probe's second status region. The completed driver asserts the probe text in the accessibility tree. No application change was needed for these automation corrections.

`drive.sh.txt` owns each scenario's keyboard lock and permission-restoration trap. `run-native.sh.txt` launches and cleans up the scenarios on one fresh synthetic folder in one terminal session. `make-focus-guard.py.txt` generates the temporary helper with the late focus check. To reproduce from the repository root:

```bash
mkdir -p .audit/pr84-r6-builder
cp docs/evidence/shell-page-records/drive.sh.txt .audit/pr84-r6-builder/drive.sh
cp docs/evidence/shell-page-records/make-focus-guard.py.txt .audit/pr84-r6-builder/make-focus-guard.py
python3 .audit/pr84-r6-builder/make-focus-guard.py
ls -la "$HOME/Library/Application Support/Aelomi" > .audit/pr84-r6-builder/real-data-before.txt
bash docs/evidence/shell-page-records/run-native.sh.txt
```

Keep the terminal session alive until it finishes. Build and launch happen before the driver takes each scenario's lock.

## Screenshots

All eleven images were inspected. They show the seed, typed text, immediate and mixed renames after relaunch, the inaccessible baseline, the pending-caller banner, both quit questions, the cleared banner with the final accepted reply, and the recovered relaunch.

[PR screenshot comment](https://github.com/aelomi/aelomi/pull/84#issuecomment-6206512981). Every attachment was downloaded anonymously and matched its local PNG byte-for-byte by size and SHA-256.

| Run | Filename | Bytes | SHA-256 |
|---|---|---:|---|
| `20261016-073821-dev-RJ0V` | `seed-before.png` | 96516 | `f4d720f1faeef779b6b05cb9ffbcd0a450818425ac38743df5d883e55d19c6d0` |
| `20261016-073821-dev-RJ0V` | `typed-before-quit.png` | 103326 | `454d517ca6d89b21d24ae1514738f63e85d8e03d64bb38ee2605b65f7962545b` |
| `20261016-073911-dev-NGDE` | `immediate-rename-reopened.png` | 98779 | `b9625cc6a69a4b40b303dcd24dcc3f1d2b698317cd6afd299886135630df6464` |
| `20261016-073911-dev-NGDE` | `settings-before.png` | 101003 | `e622514823ebf480b8428a13e979709feddb002831556f6b15c54601099816e0` |
| `20261016-074010-dev-vYvO` | `settings-rename-reopened.png` | 108136 | `1d7db62f07e57b42cc195beab1d9e0e4e2a50ca15537192b5b1cac21486f8335` |
| `20261016-074043-dev-q8Mg` | `failure-before.png` | 114472 | `21fb5c9a07e2dce332c9ccf57af00e6169c242cc3861c5478083d68226623824` |
| `20261016-074043-dev-q8Mg` | `first-refused-quit.png` | 204338 | `41484349f615a341119e8d466e1a3c6637d5aa7f93bc0cad70c0913d78876dc9` |
| `20261016-074043-dev-q8Mg` | `recovered-banner-cleared.png` | 109981 | `f22d1ac344dac4feb5b1a31de2e3187556be8cf5f5e7a7873a80ce4d479c6d24` |
| `20261016-074043-dev-q8Mg` | `retained-rename-banner.png` | 124642 | `017af3917489382159f3d8d1e10addb54f94a27d882b65c5cb31ccaf58f84904` |
| `20261016-074043-dev-q8Mg` | `second-refused-quit.png` | 204010 | `39c53006f206a3642f5b0b86f3e251f110bbf8e6284ecc51b16462c34d5c0f22` |
| `20261016-074210-dev-RmW9` | `recovered-rename-reopened.png` | 103733 | `c48c129407d7880b55ce785b49a07d6ab8c04a8f5bb20162cc8be8f873344cf6` |

## Boundaries left for follow-ups

The existing whole-store durability confirmation can wait on unrelated documents and settings. Raw record banner reasons and decode-stack consolidation remain the round-6 verdict's deferred #87 work. Settings failures before native acceptance still need retained patches. Sidebar actions, move and restore commands, optional native create IDs, empty no-op appends, trash replay distinctions, the unused second-quit probe and listener-delay cleanup remain follow-ups. The seed body needs deterministic sync identity before v3. No performance budget changed and no new calibration or countersign is claimed. The orchestrator owns fresh review and independent verification.
