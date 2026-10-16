# Sidebar fix round 6 proof

Source commit `fc479a621ed003a980bf0198fd3a8b6df4a7cd0f`. Merge base `afe406739b47d59b86ab50b4cd5fcf6f7f8ff331`. Patch ID `0e728619b6e466d6ce349b2c15fb311629d064a9`.

The proven pathspecs are `src/shell/page-focus.ts`, `src/shell/page-focus.test.ts`, `src/stores/shell.ts`, `src/pages/page-frame.tsx`, `src/pages/page-view.tsx`, `src/components/command-palette.tsx`, `src/router.tsx`, `src/styles.css`, `src/ipc/mock.ts`, `tests/playwright/sidebar-round5.spec.ts`, and `tests/playwright/sidebar-round6.spec.ts`. This record is a separate commit after the source. The PR body names the final committed head's local-gate record and hosted CI state.

## Focus policy

`FocusRequest` holds the page ID and a title, body, or body-start target. `FocusContext` is a discriminated union for request admission, title focus, and body mount. `resolvePageFocus` in `src/shell/page-focus.ts` owns the focus-yields-to-user rule. Gated Return is dropped. Title focus cancels a pending non-title request. A body request also yields to a focused text field in its frame when the body mounts. A ready Return still enters the first body block.

Model the Domain kept that policy in one typed resolver. Test Behavior, Not Implementation required exact title, body, selection, accessibility, and rectangle assertions. Sequence Work into Verifiable Units required reproductions before fixes. Prove It Works required reporting the blocked native run separately from successful mock proof. The brief forbade subagents, so the no-comments pass was manual. It found no added comments or suppression directives to remove.

## Browser proof

Apple M1, macOS 27.0.1, Node v24.13.1, Playwright WebKit. All 12 targeted round-5 and round-6 cases passed. Both pure focus-policy tests passed. The full final-head gate is recorded in the PR body.

The first held-load run reproduced both title-input failures. After releasing the load, the title remained `Beta draft`, rather than `Beta draft complete`. Return in a held title produced `Plan review Q3`, rather than `Plan Q3 review`. After the fix, the exact final titles, unchanged body text, persisted title, and middle selection pass. The corrected scrolling fixture fails when the old bottom overlay is restored, with `overlaps: true`. Removing the palette description fails with an empty accessible description. Both mutations were restored before committing.

The exact cases are:

| File | Case |
| --- | --- |
| `tests/playwright/sidebar-round6.spec.ts` | `TREE-7 row click yields pending body focus to title typing` |
| `tests/playwright/sidebar-round6.spec.ts` | `TREE-7 trash fallback yields pending body focus to title typing` |
| `tests/playwright/sidebar-round6.spec.ts` | `TREE-1 Return in a held title preserves its middle caret through recovery` |
| `tests/playwright/sidebar-round6.spec.ts` | `SHELL-1 the status strip leaves the last failed row clear in a scrolling tree` |
| `tests/playwright/sidebar-round5.spec.ts` | `TREE-7 a long failed palette title leaves its failure label fully visible`, extended with the exact accessible description |
| `src/shell/page-focus.test.ts` | `TREE-7 body focus yields to a focused text field, while Return can enter the body` |
| `src/shell/page-focus.test.ts` | `TREE-1 gated Return is dropped and a title focus cancels pending body targets` |

The final status test also preserves the connection landmark before the empty notice, which the native helper reads first. The layout flow used 82 rows and proved that the failed final row stays clear of the header status. The same rectangle and horizontal-overflow checks passed at 320 and 960 px with 200% text size. This round changes no animation and adds no component. Existing stress datasets and deeper design nits remain unchanged.

Final mock run `~/.local/state/aelomi-verify/pr89-fix6-layout-final` contains screenshots, ARIA snapshots, and `summary.json`. The flow reported PASS and no non-local requests. Both inspected light screenshots were [attached to the PR](https://github.com/aelomi/aelomi/pull/89#issuecomment-6216127548). Anonymous downloads matched their bytes and SHA-256 hashes, and the downloaded images were inspected again.

| Screenshot | Bytes | SHA-256 |
| --- | --- | --- |
| `before-status.png` | 42979 | `99f89ff787c29877e950394f41d1a6d392778469110a08f6ac41f4ffa423f8b7` |
| `status-scrolling.png` | 50485 | `36add14b2b3d35826d7a62b513761e12609c5c805f06851197d9bef53bf15da9` |

The first image shows Alpha and the initial rows. The second shows the last failed row fully visible at the bottom and its status in the header. Browser emulation matches the Mac's current light appearance. No system appearance setting changed.

## Native proof is inconclusive

Run `~/.local/state/aelomi-verify/20261016-182457-app-SoYs` launched the release bundle through `app.sh` with the synthetic folder `~/.local/state/aelomi-verify/pr89-fix6-native-data`. The seed has 82 records and Beta has 500 original blocks. That blocked attempt used the bundle from `b73edb9f99d32c29e20d114df839f2d4266b130b`, before the final connection-landmark ordering correction. No native input behavior was proven on either source. Every doctor check passed. After waiting for other runs, the supplied helper acquired the keyboard lock.

Before the first screenshot or input scenario, the required frontmost check returned lowercase `aelomi`. The one permitted `app.sh focus` attempt focused Page title, but the second check still returned `aelomi`. The supplied guard requires exactly `Aelomi`, so the run stopped. No keys or clicks were sent. The system prompt was not touched. The helper released the lock, the owned process was cleaned up, and the default data-folder listings were identical.

The relevant output was:

```text
ok    built app is newer than the sources
ok    shell log has 'ping seq=1'
acquired by pr89-fix6-native
focused "Page title": true
INCONCLUSIVE frontmost=aelomi
released by pr89-fix6-native
app.sh: stopped every process this run started
```

There are no native screenshots for this round. Native title typing, scrolling-status clearance, and the Rust readback remain unproven. The scripts preserve the exact guard and require a supplied lock helper through `AELOMI_UI_LOCK`. No system setting changed, system prompt was answered, or debugger ran.

## Rerun

From the repository root, copy these inert scripts to `.local-gate/fix6/`, removing the final `.txt` suffix. Move those scratch copies outside the checkout before `pnpm check:all`, because Biome scans that folder. Run the guarded fixtures on a new synthetic folder under `~/.local/state/aelomi-verify/`. The native script uses the real app and requires the frontmost guard's exact process name. Resolve that prerequisite before rerunning native input.

```sh
pnpm build:mock
AELOMI_TEST_PORT=4396 pnpm exec playwright test tests/playwright/sidebar-round6.spec.ts tests/playwright/sidebar-round5.spec.ts --project=webkit --workers=1
pnpm exec vitest run src/shell/page-focus.test.ts
node .claude/skills/verify-aelomi/scripts/mock-drive.mjs --flow .local-gate/fix6/layout.mjs
```

`data.sh.txt` creates and removes a temporary core example only in the current worktree. Its `seed` mode writes a fresh synthetic folder. Its `read` mode asserts the exact native title, every original body, and the trashed final record after the native scenario succeeds. That read mode was not run after this blocked attempt.

## Scope left unchanged

PR #94 was still open when this source was completed. Its settings integration remains conditional at the final push. The earlier cold-start measurements remain in [round 5's record](../fix5/README.md). This round makes no new speedup claim and changes no budget. The review's Consider and deferred design findings remain outside this fix.
