# Record edits preserve pending operations

This record proves source commit `c99e3e8edd64cc6bc6aaaf1c41bce5d5bc8b4325` against merge base `a4db19d8ab2e1ac072dea8bc3a1cc3747af8fdae`. Its patch-id from `git diff --full-index --binary <base> <source> | git patch-id --verbatim` is `30d4aad79076af07f0f6a6e5d67ac92a6eec3a74`.

The proved paths are `crates/core/src/workspace/edit.rs`, `crates/core/src/storage/tests/record_edits.rs`, `crates/core/src/storage/tests/mod.rs`, and `ARCHITECTURE.md`. This is the newest record for the writer. The [earlier record](../README.md) covers the unchanged body, seed, IPC and frontend paths.

## Regression and correction

The final source pass found a full-state encoder where the writer needed only the current transaction's delta. yrs 0.28's full-state encoder includes pending operations. The storage architecture already requires preserving their original encoded operations.

`storage::tests::record_edits::a_record_edit_never_reencodes_pending_foreign_operations_and_they_survive_reopen` withholds a text dependency, appends the later operation, and renames a page through the production writer. Before the fix its accepted rename update also carried foreign client 9001's pending operation. The test failed with `left: 11`, `right: 0` at the assertion that the rename carries no pending foreign operations.

After replacing the full-state encoding with `TransactionMut::encode_update_v1`, the targeted run reports `38 tests run: 38 passed`. The regression also crashes SimFs, reopens the store, supplies the withheld dependency, and checks the exact title `After` and exact text `dependency arrives`. No pending operation is discarded or re-authored by the record edit.

## Native proof

The native app ran on macOS 27.0.1 build 26A434, arm64, through `pnpm dev:app`, with synthetic data only. Successful run directories are `~/.local/state/aelomi-verify/20261015-220314-dev-BHV2` and `~/.local/state/aelomi-verify/20261015-220508-dev-bqQf`. The latter reused the first run's `data` folder. Both passed every doctor check.

1. A fresh folder opens Getting started with all three exact instructions in an editable `AXTextArea "Page body"`.
2. Typing ` fixed proof` changes the first paragraph to ` fixed proof⌘K to search and run commands`.
3. A normal ⌘Q and relaunch keep that text editable. Both source-tree logs say `tree warm pages=1`. A read-only cache query returns only `getting-started`, titled `Getting started`.
4. Both normal quits log `quit flush via=CompleteQuit pending=[]` and `quit flush via=Exit pending=[]`. Both instances are cleaned up.

The [new screenshots](https://github.com/aelomi/aelomi/pull/84#issuecomment-6197166857) prove the corrected writer's seed and relaunch path. Each was viewed locally, uploaded with GitHub CLI 2.102.0, and downloaded without authentication to verify a byte-for-byte match. Logs and accessibility trees stay local.

| File | Bytes | SHA-256 |
|---|---:|---|
| `before.png` | 95764 | `92977b338a65d66229b10d97b4bd15f5ee53344e3ab8e1ea64ebc25ecc40ecc4` |
| `after.png` | 97295 | `c372df4979f066423b590d3a73f50989e43d67956430ba88c28129146cbfc10f` |
| `relaunch.png` | 97311 | `152c187c0813f87b073cd5d1e259ef28e5868c3e1706d09a8fc92880ba176e60` |

An intermediate relaunch exited before doctor could validate it. It was discarded and cleaned up. The successful relaunch above supplies the proof. The earlier full-gate attempt met another worktree's occupied browser port; its retry was cancelled when the encoder issue was found. The PR's Verification section cites the full gate for the final committed head.
