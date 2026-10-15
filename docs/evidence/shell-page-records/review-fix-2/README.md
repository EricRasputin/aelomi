# Page-record review fix round 2

This record proves source commit `16b2f00e143cf6985fb8bf3ce3e3c4432738fd84` against merge base `a4db19d8ab2e1ac072dea8bc3a1cc3747af8fdae`. Its patch-id from `git diff --full-index --binary <base> <source> | git patch-id --verbatim` is `12eec5d6c6581742eebe471c7c8ce2a815789ae3`.

The proved pathspecs are `crates/core/src/workspace/edit.rs`, `crates/core/src/workspace/tests/edits.rs`, `src-tauri/src/quit.rs`, `src-tauri/src/storage.rs`, `src-tauri/src/commands/page_records_tests.rs`, `src/ipc/record-commands*`, `src/stores/page-tree*`, `src/dev/storage-probe.tsx`, `perf/budgets*`, `perf/cold-start*`, `perf/stats.test.ts`, `ARCHITECTURE.md`, and `.claude/skills/verify-aelomi/features/getting-started.md`.

## Data shape and decisions

Restore uses the canonical forest and per-parent sibling sequences keyed by original `(order, id)`. One parent-first walk plans the batch. Numeric allocation preserves that sequence without rewriting preceding siblings. Moving a cycle member writes the cycle's cut edge in the same append.

Record admission comes from the phase and revision held by the quit mutex. There is no separate closed flag. Completing and the Store flush share one deadline. Record guards remain alive when a wait times out.

The frontend boundary owns a FIFO queue. One command runs at a time. Consecutive queued renames of one page share the latest title and result. The quit drain tracks queued and running work. This queue stays separate from PR #83's settings tracker.

Model the Domain determined the sibling collections, phase-derived admission and queue. Test Behavior, Not Implementation determined the exact persisted-title and untouched-record assertions. Sequence Work into Verifiable Units determined the focused red/green checks and ordered commits. Prove It Works determined the native relaunch proof below.

## Five Act-on items

| Item | Regression and result |
|---|---|
| Restore ties, DATA-2 and TREE-12 | `crates/core/src/workspace/tests/edits.rs`, `restoring_tied_siblings_preserves_order_without_writing_the_untouched_sibling`. The baseline returned `b, a`. The fix returns `a, b`, or `p, a, b, c, untouched` for the subtree. A Y.Map observer receives zero key writes on the untouched sibling. The projected order is exact. Batch members' numeric orders can change to allocate distinct integers; unrelated records remain exact. |
| Move out of a repaired cycle, TREE-5 and TREE-9 | The same file, `moving_out_of_a_repaired_cycle_keeps_both_pages_roots_and_trashes_only_the_moved_page`. The baseline returned `b` with child `a`. The fix returns both roots; trashing `b` leaves only active `a`. |
| Quit admission, SHELL-9 and SHELL-10 | `src-tauri/src/quit.rs`, `a_refused_quit_then_a_second_request_admits_record_commands_during_its_drain` and `a_stale_timeout_resuming_after_keep_open_leaves_record_admission_open` both failed on the baseline. `a_completing_record_wait_reaches_its_deadline_and_asks_without_losing_the_guard` proves the deadline. `src-tauri/src/commands/page_records_tests.rs`, `a_second_quit_drain_accepts_a_rename_and_reopens_with_its_title` persists exactly `Second drain title`. The existing blocked-record-lock rename/reopen test also passes. |
| Submission order, SHELL-8 and SHELL-9 | `src/ipc/record-commands.test.ts`, `back-to-back renames retain submission order even when the native scheduler runs newest first` stored `a` on the baseline and now stores `ab`. `queued consecutive renames coalesce but never cross another record command` proves coalescing and FIFO barriers. The quit drain still waits for all results. |
| Page end-to-end calibration | `perf/cold-start-budget.test.ts`, `page and index variants select their own hosted end-to-end ceiling` failed before selection existed. The harness now selects 2702 ms for the page and retains the index entry. |

The focused core suite passed all 14 edit cases, including `random_create_move_trash_restore_keeps_an_ordered_tree`. The focused shell suite passed 20 cases. The combined queue, page-tree, frontend drain and budget suite passed 27 cases. TypeScript checking passed. `regressions.sh.txt` reproduces these checks.

## Consider decisions

- Accept subscription recovery. Initial tree reads proceed after listener attempts settle, even on rejection. Failed subscriptions retry with bounded backoff, refresh on recovery and stop on disposal. `src/stores/page-tree.test.ts`, `a failed tree listener still reads the initial page and retries the subscription`, first rejected startup on the baseline and now shows Getting started followed by the renamed title.
- Accept one planned-forest walk in restore. Per-parent sibling lists replace per-member projections and the quadratic pending dependency scan. This is a structural change, with no measured speedup claim.
- Defer no-op append suppression. Changing the acknowledged sequence and whether shell and mock emit a change event is a separate API decision. Empty transaction appends remain a known cost; they do not undo edits.
- Accept separate refusal reporting. `Quitting`, invalid targets and other typed edit refusals remain on the command reply. Storage, malformed-source, invalid-update and transport failures still report the workspace to the drain. Four refusal cases and the existing I/O failure case assert those results exactly.

The manual no-comments pass covered this round's diff. It found no new narrative comments, suppressions, constraint comments or workaround comments. It deleted zero comments and added no encodings. No subagent was spawned, as required by the implementation brief.

## Hosted calibration

The same first three complete chronological series on `40955da6caf7d54fd86f94f49f03a3fbaacb46a4` supply both page ceilings. These are inherited calibration measurements, not new measurements of the fixed head and not a speedup claim.

| Series | Page navigation median, ms | Page end-to-end median, ms |
|---|---:|---:|
| [Initial](https://github.com/aelomi/aelomi/actions/runs/38784875274/attempts/1) | 451 | 1726.5 |
| [First rerun](https://github.com/aelomi/aelomi/actions/runs/38784875274/attempts/2) | 164 | 423 |
| [Second rerun](https://github.com/aelomi/aelomi/actions/runs/38784875274/attempts/3) | 439.5 | 1801 |

The page end-to-end ceiling is `ceil(1.5 * 1801) = 2702 ms`. Navigation retains `ceil(1.5 * 451) = 677 ms`. The extra fourth series remains excluded by the recorded first-three-complete-series rule. The index ceilings and local 500 ms goal stay unchanged. The builder claims no countersign. Orchestrator review and the independent verifier remain outstanding.

`hosted-summary.py.txt` reads the existing artifacts at `~/.local/state/aelomi-verify/pr84-fix1-hosted/40955da/{initial,series-1,series-2}`. It recomputes the medians from ten successful timed launches per variant in each series and derives both ceilings. All 60 timed launches report `exit 0` and the expected variant. The original calibration record has the host versions, load, navigation ranges and warm-up limits. This round changes budget selection only.

The first full gate caught the missing new entry in the budget inventory test. `perf/stats.test.ts`, `budgets.json holds every gate with a value and a why`, now includes the page end-to-end entry. Its focused five-case suite passes.

## Native self-proof

The app ran on an Apple M1 with macOS 27.0.1 build 26A434, arm64, on source commit `16b2f00e143cf6985fb8bf3ce3e3c4432738fd84`. All data was synthetic. The shared mkdir keyboard lock covered every press, focus, key and transient screenshot. It was released between scenarios and on failure.

The four successful run directories under `~/.local/state/aelomi-verify/` are:

- `20261016-012644-dev-L7fN`, the fresh isolated folder and typing.
- `20261016-012803-dev-NUqs`, the typed body after relaunch and rename before quit.
- `20261016-013502-dev-5sS8`, the first rename after relaunch, refused quit and rename during the second drain.
- `20261016-013711-dev-iF9y`, the second rename and typed body after relaunch.

The latter three use the first run's data folder. Every doctor check passed. Every instance was cleaned up. `native.sh.txt` records the action sequence and locking. The final read-only index query returned exactly `getting-started|Retried quit rename proof`; each source-tree log reports `tree warm pages=1`.

The inspected screenshots show the fresh three-paragraph body, the typed `fix round two`, that text after relaunch, `Quit rename proof` after another relaunch, the refused-quit dialog, and finally `Retried quit rename proof` with the same typed body. No Keep app open or Quit anyway was pressed between the two quit requests. The successful retry log reads:

```text
quit draining via=ExitRequested revision=1
quit flush via=CompleteQuit pending=[DocId(probe)]
quit draining via=ExitRequested revision=3
page rename accepted title=Retried quit rename proof
quit flush via=CompleteQuit pending=[]
quit confirmed via=drain
quit allowed via=ExitRequested confirmed=true
quit flush via=Exit pending=[]
```

The failing write reported exactly `storage event health={"Failing":{"reason":"PermissionDenied","pending_docs":["probe"]}}`. All other normal quits report `CompleteQuit pending=[]` and `Exit pending=[]`.

An earlier attempt stopped before fault injection because `app.sh wait-status` selected the global IPC status instead of the probe status. That instance was cleaned up. The successful script waits for the probe text through the accessibility tree. This is a helper limitation, with no app-code change.

[Six screenshots](https://github.com/aelomi/aelomi/pull/84#issuecomment-6201264445) were inspected, uploaded with GitHub CLI 2.102.0, read back, downloaded without authentication, opened as PNGs and matched byte for byte. The CLI archive matched its official SHA-256 `da922c20d1792e5b2cbf375593d7a658acf034c12c84e007e71c76ef959c337e`. `screenshots.py.txt` repeats the anonymous download and byte checks. Logs and accessibility trees stay local.

| Screenshot | Bytes | SHA-256 |
|---|---:|---|
| `before.png` | 99531 | `17e8ad4a76bc063e21d77d7e70f2d1a31b7d7145fca162c207bbedc228f0dc74` |
| `after.png` | 101945 | `1d4c0844d7e08ecf39909160916cf5585aebc48124e767dad60e1efcfb853436` |
| `relaunch.png` | 96730 | `805109632c1655b7972f65569704e46f015239133587c8489e0b37ee7d4cd284` |
| `renamed-relaunch.png` | 97142 | `5c822405c08b39d6cf6751a07b4b83169ba62601fe4c7762046c1fc430ce2409` |
| `refused-quit.png` | 208193 | `45413ceae86a06de7c4b343cdd36677beadbdb77125b111a352c3d184ed2056f` |
| `retried-rename-relaunch.png` | 98778 | `a566ae8de445d92df3994952545a29f51b58e74b929a03c30acc06f64262d667` |

## Final gate and limits

The final committed-head `pnpm check:all` record and result are cited in PR #84's Verification section. The gate runs after this evidence commit; its logs remain local.

The original and first-fix records retain the full seed and field-delta evidence for SHELL-2, DATA-2 and the controllable-clock SHELL-8/SHELL-9 cases. This round extends placement, submission and quit regressions and reruns the native seed proof.

No review gate, independent verifier or merge ran in this implementation round. No settings code, other branch or other PR was changed. No-op append suppression remains deferred. The hosted budget countersign belongs to the orchestrator.
