# Final page-record fix proof

This record proves source commit `360fcf8e874f2c73ba0ac3e0212a5d579d492fe6` against merge base `a4db19d8ab2e1ac072dea8bc3a1cc3747af8fdae`. Its patch-id from `git diff --full-index --binary <base> <source> | git patch-id --verbatim` is `682007f30cdc743bc538f5d513253bebdc2c05d5`.

The proved pathspecs are `crates/core/src/workspace/edit.rs`, `crates/core/src/workspace/tests/edits.rs`, `src-tauri/src/quit.rs`, `src-tauri/src/storage.rs`, `src-tauri/src/commands/page_records_tests.rs`, `src/ipc/record-commands*`, `src/stores/page-tree*`, `src/dev/storage-probe.tsx`, `perf/budgets*`, `perf/cold-start*`, `perf/stats.test.ts`, `ARCHITECTURE.md`, and `.claude/skills/verify-aelomi/features/getting-started.md`.

## Data shape

The canonical future forest supplies complete `(order, id)` sibling lists, including foreign children that become active when a batch parent returns. Restore resolves inactive external parents before that projection and walks the batch once, parents first. Numeric allocation shifts only the suffix after each restored page.

The revisioned quit phase owns command admission under one lock. Completing and Store flush share a deadline. The frontend owns a FIFO record queue and coalesces consecutive queued renames of one page.

Model the Domain shaped these collections and state transitions. Test Behavior, Not Implementation led to exact projected orders, zero untouched-record writes and persisted titles. Prove It Works led to fresh native relaunches. Sequence Work into Verifiable Units kept each fix behind a detecting check and an ordered commit. One builder owns the diff and all keyboard-lock steps; no nested agent ran.

## Restore correction found during the final pass

The first round-2 implementation initialized sibling groups from the currently active forest. That omitted a foreign page added beneath a trashed parent. When the parent returned, that child became visible and could precede a batch sibling whose numeric order had shifted.

`crates/core/src/workspace/tests/edits.rs`, `restore_orders_newly_visible_foreign_children_with_the_batch_siblings`, failed with `p, a, z, c, untouched` instead of `p, a, c, z, untouched`. The fix projects the future active forest before it allocates orders. Both an ordinary root restore and an inactive-original-parent fallback now preserve the exact expected order. The unrelated root retains every typed field, while necessary collision shifts preserve the foreign child's update time.

All 15 edit tests passed, including this case, tied individual and subtree restores, repaired-cycle moves, pending independent foreign fields, restore cycle round trips and the random ordered-tree property test. The [earlier round-2 record](../review-fix-2/README.md) retains the initial red/green and calibration history. This record supersedes its source-path proof for the final head.

## Five Act-on proving cases

| Item and SPEC behavior | Test file and case |
|---|---|
| Restore ties, DATA-2 and TREE-12 | `crates/core/src/workspace/tests/edits.rs`, `restoring_tied_siblings_preserves_order_without_writing_the_untouched_sibling`, plus the newly-visible-foreign-child case above. The untouched sibling observer receives zero key writes. |
| Cycle move, TREE-5 and TREE-9 | The same file, `moving_out_of_a_repaired_cycle_keeps_both_pages_roots_and_trashes_only_the_moved_page`. Both projected parents remain null and trashing `b` leaves `a` active. |
| Admission and deadline, SHELL-9 and SHELL-10 | `src-tauri/src/quit.rs`, `a_refused_quit_then_a_second_request_admits_record_commands_during_its_drain`, `a_stale_timeout_resuming_after_keep_open_leaves_record_admission_open`, and `a_completing_record_wait_reaches_its_deadline_and_asks_without_losing_the_guard`. `src-tauri/src/commands/page_records_tests.rs`, `a_second_quit_drain_accepts_a_rename_and_reopens_with_its_title`, persists exactly `Second drain title`. |
| Queue order, SHELL-8 and SHELL-9 | `src/ipc/record-commands.test.ts`, `back-to-back renames retain submission order even when the native scheduler runs newest first`, persists `ab`. `queued consecutive renames coalesce but never cross another record command` proves the queue barriers. The frontend drain still waits for queued and running work. |
| Page budget selection | `perf/cold-start-budget.test.ts`, `page and index variants select their own hosted end-to-end ceiling`, and `perf/stats.test.ts`, `budgets.json holds every gate with a value and a why`. |

## Consider decisions and limits

- Accept the initial tree read after failed listener registration and subscription retries. `src/stores/page-tree.test.ts`, `a failed tree listener still reads the initial page and retries the subscription`, shows the seed and then a renamed title after recovery.
- Accept one planned-forest walk with per-parent lists. The final correction includes pages that become active during restore. There is no measured speedup claim.
- Defer no-op append suppression. Acknowledged sequences and shell/mock publication semantics need a separate API decision. Empty transaction appends remain a known cost.
- Accept separate refusal reporting. Typed edit refusals stay on command replies; storage, malformed-source, invalid-update and transport failures still reach the drain. The refusal and I/O cases assert exact results.

The queue remains self-contained for the later merge with PR #83's tracker. No settings code changed. Move and restore still have no shell commands or UI. Their supported source states are proved through the production core writer. The original and first-fix evidence retain the full SHELL-2 seed and controllable-clock SHELL-8/SHELL-9 coverage.

The final diff has no new narrative comments, suppressions or constraint comments. The manual no-comments pass deleted zero comments and added no encodings. No review gate, independent verifier or merge ran.

## Hosted calibration

The same three complete chronological series on `40955da` supply page navigation medians of 451, 164 and 439.5 ms, and page end-to-end medians of 1726.5, 423 and 1801 ms. The ceilings are 677 ms and 2702 ms respectively, each 1.5 times the worst median rounded up. The [calibration table, run links and scripts](../review-fix-2/README.md#hosted-calibration) preserve their provenance and validate all 60 successful timed launches. These are inherited calibration measurements, with no speedup claim. Index ceilings and the local 500 ms goal stay unchanged. The builder claims no countersign.

## Native self-proof on the final source

The app ran on an Apple M1 with macOS 27.0.1 build 26A434, arm64, on source commit `360fcf8e874f2c73ba0ac3e0212a5d579d492fe6`. All data was synthetic. The shared mkdir keyboard lock covered each press, focus, key and transient screenshot, with release between scenarios and on failure.

The four successful run directories under `~/.local/state/aelomi-verify/` are:

- `20261016-015157-dev-VBbT`, the fresh seed and typing.
- `20261016-015441-dev-uTCk`, typing after relaunch and a rename before quit.
- `20261016-015529-dev-Oxmt`, the first rename after relaunch, refused quit and rename during the second drain.
- `20261016-015625-dev-Ogh9`, the second rename and typed body after relaunch.

The latter three use the first run's data folder. Every doctor check passed. Every instance was cleaned up. `native.sh.txt` reproduces the sequence. Each source-tree log reports `tree warm pages=1`; the final read-only index query returns exactly `getting-started|Retried quit rename proof`.

The inspected screenshots show all three seed instructions, the typed `fix round two`, that text after relaunch, `Quit rename proof` after relaunch, the refused-quit dialog, and finally `Retried quit rename proof` with the same body. No Keep app open or Quit anyway was pressed between the two requests. The retry log reads:

```text
storage event health={"Failing":{"reason":"PermissionDenied","pending_docs":["probe"]}}
quit draining via=ExitRequested revision=1
quit flush via=CompleteQuit pending=[DocId(probe)]
quit draining via=ExitRequested revision=3
page rename accepted title=Retried quit rename proof
quit flush via=CompleteQuit pending=[]
quit confirmed via=drain
quit allowed via=ExitRequested confirmed=true
quit flush via=Exit pending=[]
```

Every successful quit reports `CompleteQuit pending=[]` and `Exit pending=[]`. [Six final-source screenshots](https://github.com/aelomi/aelomi/pull/84#issuecomment-6201601827) were inspected before upload. Their anonymous downloads were opened as PNGs and matched the originals byte for byte. GitHub CLI 2.102.0 matched its official archive checksum, as recorded in the earlier round-2 evidence. The [download script](../review-fix-2/screenshots.py.txt) reproduces the byte checks. Logs and accessibility trees stay local.

| Screenshot | Bytes | SHA-256 |
|---|---:|---|
| `before.png` | 94667 | `207bd7646d9dc30306602d08c999fca76b103ad9c18d04b70ca1c0a38c2c257b` |
| `after.png` | 101928 | `32881f52130c8fa6f43c35aa3acb6f1ad8096a6d0128d8d481dec2ad9e10e01f` |
| `relaunch.png` | 101730 | `c99032100b5e4cd77cc9a8b5fc3a1df2fce932b3e3d03e78eb7e3ec822df009d` |
| `renamed-relaunch.png` | 97142 | `5c822405c08b39d6cf6751a07b4b83169ba62601fe4c7762046c1fc430ce2409` |
| `refused-quit.png` | 208369 | `dc2f015d1bb72da67f895b333f2433d49746385de25636038c0e83d68f554aab` |
| `retried-rename-relaunch.png` | 98778 | `a566ae8de445d92df3994952545a29f51b58e74b929a03c30acc06f64262d667` |

## Committed-head gate

The final `pnpm check:all` result and `.local-gate/<head>.checks.json` path are cited in PR #84's Verification section. That gate runs after this evidence commit. No pass is claimed for the earlier run that was stopped to add the newly-visible-sibling regression. The runner-calibrated performance gates and their local informational verdicts remain distinct.
