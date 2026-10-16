# Round-3 proof after the appearance-policy rebase

Source commit `759a0b8c96532dac8a992dce5b790dd41be0adae`.
Recorded merge base `afe406739b47d59b86ab50b4cd5fcf6f7f8ff331`.
Verbatim patch-id `3a901b58d6d904a8869c79077fea6ac9e2864da7`.

The runtime pathspecs are `crates/core/src/storage/store.rs`, `src-tauri/src/storage.rs`, `src-tauri/src/recovery.rs`, `src/components/storage-refusal.tsx`, and `src/storage-copy.ts`.

## Rebase and evidence

Main advanced to the appearance-policy documentation change while the full local gate ran. Rebase conflicts occurred only in the historical evidence annotation and the verification feature map. Both the no-system-settings policy and the recovery-proof correction were preserved. No runtime conflict or runtime edit occurred.

`pnpm check:all` passed all 11 stages on pre-rebase head `3cba8167e9798d1c1fe031efa62388f52abc34f9`. Record `.local-gate/3cba8167e9798d1c1fe031efa62388f52abc34f9.checks.json` remains local. The rebased final-head record is cited on PR #93 after its required rerun.

Every shipping source and app-configuration byte matches the pre-rebase head. `git diff --name-only 3cba8167e9798d1c1fe031efa62388f52abc34f9 759a0b8c96532dac8a992dce5b790dd41be0adae` lists only Markdown files. Each runtime file's SHA-256 below was computed before and after rebase and is equal.

| Runtime path | SHA-256 before and after rebase |
|---|---|
| `crates/core/src/storage/store.rs` | `086121a1166822227c79d260657e59ae121207b9a676f971efa93a25f8fc32e7` |
| `src-tauri/src/storage.rs` | `42b45a5ed7bf0c087cfbc39bc194bae5066246e289405b129d6d2d06f89ee000` |
| `src-tauri/src/recovery.rs` | `8fe0cb99cab7b23c1444a1ec5a38ead866ba33df065bcca98ea20aa2b0d55fc5` |
| `src/components/storage-refusal.tsx` | `602c6e9ab61bca80c412f57820585d9020b035ee22b57294cbdf442f8761955c` |
| `src/storage-copy.ts` | `56da10c1f75794d47f9c3a8c0fe722ec3a2bddab150a2625127e0b9ff5a8dc1f` |

The [first record](../shell-pending-refusal-fix3/README.md) holds nine native screenshots and the core/UI regression proof. The [supplemental record](../shell-pending-refusal-fix3-occupied/README.md) covers the destination that appears after startup, with three more native screenshots. This rebase record supersedes their source-path audits at the new base. Their captures and exact-byte assertions apply to the identical runtime files above. Historical source labels and records remain unchanged.

All native captures used the Mac's current light appearance. No system setting changed and no system prompt was answered. Original-folder and recovery-record byte comparisons passed. Real-data-folder listings matched before and after native proof and the pre-rebase cold-start runs. Current appearance and mock-only alternate themes follow the updated verification rules.

## Audit and limits

Run `python3 docs/evidence/shell-pending-refusal-fix3-rebase/audit-record.py.txt`. It verifies this source patch-id, every runtime pathspec and file hash against HEAD, without requiring pre-rebase commit objects in a fresh clone.

Native Finder operations and native flush-timeout receipt remain unproved. SimFs covers the pending flush and ENOSPC publication cases. The brief's Consider and Noted items remain deferred. Hosted performance budgets passed before rebase, with noisy typing measurements and informational local typing and page cold-start misses. No performance improvement is claimed.
