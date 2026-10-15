# Page-record review fixes

This record proves source commit `339e5fc58a4c1bfb1c3c3bb039f0eed79577b6ef` against merge base `a4db19d8ab2e1ac072dea8bc3a1cc3747af8fdae`. Its patch-id from `git diff --full-index --binary <base> <source> | git patch-id --verbatim` is `4e15f654bb4dd8e00f0d72a13bd16ce298ddca09`.

The proved pathspecs are `crates/core/examples/seed_index.rs`, `crates/core/src/storage/store.rs`, `crates/core/src/storage/tests/`, `crates/core/src/workspace/`, `src-tauri/src/commands.rs`, `src-tauri/src/commands/`, `src-tauri/src/quit.rs`, `src-tauri/src/storage.rs`, `src/ipc/`, `src/editor/quit-drain*`, `src/stores/page-tree*`, `src/stores/index-status.ts`, `src/main.tsx`, `src/router.tsx`, `src/dev/storage-probe.tsx`, `perf/`, `.github/workflows/ci.yml`, `scripts/check-all-stages.ts`, `scripts/check-all.test.ts`, `ARCHITECTURE.md`, and `.claude/skills/verify-aelomi/features/`.

## Data shape

Typed `PageRecord` values describe the known fields of each nested Y.Map. Existing records write only changed typed fields, and coupled Any objects retain unknown fields.

The canonical forest supplies complete ordered sibling sequences and cycle-cut restore dependencies. Tied orders and missing-parent roots remain in those sequences.

`PreparedWorkspace` owns initialization before index readers start. One random record client belongs to each launch. Record-command guards and frontend promises cover quit, while one tree request and a dirty flag cover refresh bursts.

## Review findings and regressions

| Finding | Change and proving case |
|---|---|
| 1. Independent fields | `workspace/tests/edits.rs`, `pending_independent_record_fields_win_after_rename_in_both_client_orders`, stores pending favorite, placement and trash writes, renames locally, flushes and reopens, then supplies each dependency. Both client-ID orderings retain the foreign field and title `After`. Before the fix, favorite read null instead of order 7. |
| 2. Projected siblings | `moving_before_tied_siblings_keeps_every_sibling_in_projected_order` asserts exactly `z, a, b` at orders 10 and 0. `end_append_and_collision_shifts_include_projected_orphan_roots` asserts exact append and move orders. Before the fix these produced `a, z, b` and `root, new, orphan`. The mock has the same orphan regression. |
| 3. Restore cycles | `trash_restore_round_trips_self_parent_and_two_page_cycles_field_by_field` compares restored records and projected trees for a self-parent and a two-page cycle. The cut root restores at the top level. Only edit timestamps are normalized for the round-trip comparison. Before the fix restore returned `Cycle`. |
| 4. Seed lifecycle | `workspace/tests/seed.rs` proves two opens, stable ID and exact body lines, no reseed after trash or erase, no page from `Workspace::open`, and no index before the explicit seed. `storage/tests/seed.rs` covers failed and slow flushes, unreadable logs and retries, corrupt and quarantined history, a one-record payload bit flip, large first records, and torn first writes. The failing-flush and unreadable-log tests first returned errors; corrupt history first gained a seed. |
| 5. Quit | `commands/page_records_tests.rs`, `complete_quit_waits_for_a_rename_held_on_the_record_lock_and_reopens_with_its_title`, first observed confirmation before the blocked rename appended. It now waits and reopens with `Just before quit`. `editor/quit-drain.test.ts` holds the frontend promise, and `ipc/record-commands.test.ts` covers create, rename and trash. QuitGate regressions preserve a non-confirming timeout and explicit confirmation; both failed before that correction. |
| 6. Record clients | `record_edits_add_one_client_per_launch_and_resume_its_clock` first counted 21 clients for a create and 20 renames. It now counts exactly one, then exactly two after edits in a reopened workspace. |
| 7. Cold start | The harness measures an empty workspace containing a stored empty update and the seeded page separately. `storage/tests/seed.rs`, `the_cold_start_empty_fixture_has_one_stored_empty_update_and_never_seeds`, decodes the exact fixture record. `perf/cold-start-budget.test.ts` first selected the index calibration for a page, then passed with separate entries. |

The test paths above are relative to `crates/core/src/`, `src-tauri/src/`, or `src/` as named by their subsystem. `regressions.sh.txt` runs the focused suites.

## Consider decisions

- Accept one tree request with a dirty flag. `a burst keeps one tree request in flight and installs its reply before the next finishes` first observed 21 concurrent calls. The corrected connection also subscribes before the snapshot and refreshes after a successful command if its native event is lost.
- Accept gap insertion. `inserting_into_an_order_gap_and_noop_moves_leave_other_records_unchanged` compares the unrelated records exactly. Collision shifts retain their update times.
- Split planning from load, encoding and append, and remove the seed-only create-ID argument. Keep the shared `PageEdit` and `PageChange` API. Separate per-command reply types would add public APIs without changing the behavior this round fixes; the existing variant invariant remains covered by the command tests.

The touched noted items are also covered. Mock edits capture transaction updates, the random test accepts only `Cycle` move errors, large pristine probes have tests, the fixture writer now adapts the full-write case, and tree listeners precede the initial snapshot.

## SPEC coverage

| Behavior | Test file and case |
|---|---|
| DATA-2 | `workspace/tests/edits.rs`, `edits_preserve_unknown_keys_and_coupled_object_fields`, the pending-field regression, and `concurrent_record_edits_use_the_latest_base_and_survive_reopen` |
| SHELL-2 | `workspace/tests/seed.rs`, `a_new_folder_opened_twice_has_one_getting_started_page_with_the_same_id_and_three_lines`, `all_trashed_or_all_erased_workspaces_are_never_reseeded`, and the interrupted-body and stored-empty-update cases |
| SHELL-8 and SHELL-9, excluding settings | `workspace/tests/ack.rs`, `typing_paste_drag_rename_move_trash_and_restore_are_flushed_within_100_ms`, plus the blocked-rename and frontend drain cases above |
| SHELL-10 seed failure path | `storage/tests/seed.rs`, `a_failed_first_body_flush_keeps_the_workspace_open_unseeded_and_retries_on_relaunch`, with the exact DiskFull health value and pending body doc |
| TREE-5 core | The tied-order, projected-orphan and gap cases, plus `moves_refuse_cycles_and_restore_uses_top_level_when_original_parent_is_inactive` |
| TREE-9 and TREE-12 core | `trash_restore_preserves_tree_sibling_order_and_favorites_field_by_field`, the cycle round trips, and `random_create_move_trash_restore_keeps_an_ordered_tree` |

Sidebar actions, the empty-state UI, editable title, settings, favorite commands and permanent erasure remain outside PR A.

## Native self-proof

The app ran on macOS 27.0.1 build 26A434, arm64, with synthetic data only. The native code was `40955da6caf7d54fd86f94f49f03a3fbaacb46a4`. Later changes for this record set the measured hosted budget, strengthen a regression assertion, and update documentation. The native runtime and timing code are unchanged.

Successful run directories are `~/.local/state/aelomi-verify/20261015-235054-dev-iuKa`, `20261015-235148-dev-G3mk`, and `20261015-235245-dev-Ydo7`. The latter two reuse the first run's data folder. All doctor checks passed. `native.sh.txt` contains the sequence actually driven.

1. The fresh window shows Getting started and all three instructions in an editable Page body.
2. Typing ` fix round one` changes the first paragraph. A normal quit and relaunch retain it.
3. The dev Storage probe issues a rename, followed by ⌘Q. The third launch shows Quit rename proof and the same body. Its read-only cache query returns exactly `getting-started|Quit rename proof`.
4. Each launch reports `tree warm pages=1`. Every normal quit reports `quit flush via=CompleteQuit pending=[]` and `quit flush via=Exit pending=[]`. Every instance is cleaned up.

The native proof follows the user action sequence. The deterministic record-lock test proves the overlapping command/quit interleaving.

[Four screenshots](https://github.com/aelomi/aelomi/pull/84#issuecomment-6199261541) were viewed, uploaded with GitHub CLI 2.102.0, downloaded without authentication, opened as PNGs, and checked byte for byte. Its CLI archive matched the official release checksum. Logs and accessibility trees remain local.

| Screenshot | Bytes | SHA-256 |
|---|---:|---|
| `before.png` | 99531 | `17e8ad4a76bc063e21d77d7e70f2d1a31b7d7145fca162c207bbedc228f0dc74` |
| `after.png` | 101367 | `95c62c5d126f1d045197618a4d1fde4d84fb193097a060627b0c579994942d7d` |
| `relaunch.png` | 96126 | `86e0f57d4637a4e7456e9afa9e09f97a4356ef40eba0272b1ae06dd9e3f32898` |
| `renamed-relaunch.png` | 101678 | `91758e570a461eb0c3a5d69c678e699d928ce677eaaf35af82242b4e021714fc` |

## Hosted cold-start calibration

The first three chronological complete series on runtime commit `40955da6caf7d54fd86f94f49f03a3fbaacb46a4` give page navigation medians of 451, 164 and 439.5 ms. The budget is `ceil(1.5 × max(451, 164, 439.5)) = 677 ms`.

| Series | Empty navigation median (ms) | Page navigation median (ms) | Page end-to-end median (ms) | Page navigation range (ms) |
|---|---:|---:|---:|---|
| [Initial](https://github.com/aelomi/aelomi/actions/runs/38784875274/attempts/1) | 301.5 | 451 | 1726.5 | 383–490 |
| [First cold-job rerun](https://github.com/aelomi/aelomi/actions/runs/38784875274/attempts/2) | 165.5 | 164 | 423 | 143–236 |
| [Second cold-job rerun](https://github.com/aelomi/aelomi/actions/runs/38784875274/attempts/3) | 120 | 439.5 | 1801 | 389–530 |

The old 400 ms page check failed the first and third series; all 60 timed app runs exited successfully with the expected variant. Each job used Node 24.20.0 and a three-core Apple M1 Virtual host. Page-start one-minute loads were 9.44, 5.89 and 10.33. The benchmark includes this hosted variability instead of selecting the fastest series.

An extra [third rerun](https://github.com/aelomi/aelomi/actions/runs/38784875274/attempts/4) measured 460.5 ms for the page and 279.5 ms for the index. It also exited successfully and fits the proposed 677 ms page budget; its job still used the old 400 ms page threshold and therefore reports failure. It was scheduled before the first rerun completed and does not replace a calibration series.

Raw harness artifacts remain outside the checkout at `~/.local/state/aelomi-verify/pr84-fix1-hosted/40955da/{initial,series-1,series-2,series-3}`. `hosted-reruns.py.txt` records the actual cold-job rerun procedure. The first three directories are the inputs to `hosted-summary.py.txt`. Final-head confirmation series are cited in the PR body without changing this evidence commit.

The calibration uses release builds, one discarded warm-up and ten timed launches per variant per series. The harness validates the variant and exit status of every run. `hosted-summary.py.txt` recomputes medians, reports ranges and host load, and derives the budget. The empty-workspace calibration stays at 400 ms. The shared hosted end-to-end ceiling stays at 2500 ms, and the local 500 ms goal stays unchanged.

The page path includes route selection, body loading and the first editable BlockNote frame. These measurements calibrate that variant and make no speedup claim. The hosted budget requires the orchestrator's countersign; the builder does not countersign it.

## Gate and limits

The final committed-head `pnpm check:all` result and its `.local-gate` record path are cited in PR #84's Verification section. Full logs remain local.

No settings code, other branch or other PR was changed. No review gate, independent verifier or merge ran in this implementation round. The final PR body cites the committed-head gate and any final-head hosted confirmation runs.
