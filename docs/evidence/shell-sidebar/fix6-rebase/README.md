# Sidebar fix round 6 after the recovery merge

Source `691746e69bea7182da78875eb3c77182c6829975`. Merge base `60a36a80c9fd5edba517e88245424fc0151e7451`. Patch ID `e3a7219fe02d301c9937776c61cb7c872a38421d`.

The proven pathspecs are `src/shell/page-focus.ts`, `src/stores/shell.ts`, `src/pages/page-frame.tsx`, `src/pages/page-view.tsx`, `src/router.tsx`, `src/styles.css`, `src/ipc/mock.ts`, `src/components/command-palette.tsx`, `tests/playwright/sidebar-round5.spec.ts`, and `tests/playwright/sidebar-round6.spec.ts`. This record is a separate commit after the source. The final head's local-gate record and hosted CI run are in the PR body.

PR #93 merged while the preceding full gate was reaching cold start. The branch was rebased as required. One CSS append conflict kept both the recovery and sidebar blocks. One mock tree-read conflict kept the recovery wait before the injected read failure. Type checking, lint, and all 12 targeted round-5 and round-6 WebKit cases passed after the rebase. The full final-head gate runs separately. PR #94 was still open before the final push, so its integration remains deferred.

The [original round-6 record](../fix6/README.md) preserves the data model, exact case names, red regressions, and native guard failure. Its historic source and base remain unchanged. The round-6 runtime fix is now `f2f356b8fdae5ec642bb790e2fc0663a9b609f51` after rebasing.

## Current mock proof

Run `~/.local/state/aelomi-verify/pr89-fix6-rebased-layout` passed the 82-row layout flow at 320 and 960 px with 200% text size and no non-local requests. The attached script waits for Page body before its initial screenshot. Both current light images were inspected, [posted on the PR](https://github.com/aelomi/aelomi/pull/89#issuecomment-6216446168), downloaded anonymously, and checked against their bytes and SHA-256 hashes. The final failed-row image is byte-identical to the preceding capture.

| Screenshot | Bytes | SHA-256 |
| --- | --- | --- |
| `before-status.png` | 42927 | `519685dbac50af73241dc759b46253d0dcdce738adc5252e38d443f3cf6002af` |
| `status-scrolling.png` | 50485 | `36add14b2b3d35826d7a62b513761e12609c5c805f06851197d9bef53bf15da9` |

The first screenshot shows Alpha and the initial rows. The second shows the last failed row fully clear of the header status. These are mock screenshots. The focus cases still assert the exact title and an unchanged body after held docLoad, and the held Return case still asserts the middle caret. The long-title palette case still checks Not created as its accessible description.

## Native gap

Native input proof remains INCONCLUSIVE. The supplied guard rejected lowercase `aelomi` after the permitted focus attempt, so the run stopped before keys, clicks, or screenshots. The lock was released and the owned app was cleaned up. No native input run was repeated after the rebase. No system prompt or setting was touched. The default data-folder listing was unchanged after that attempt and is checked again after the final gate.

The preceding two full-gate attempts were stopped for the connection-landmark correction and the required main rebase. They are not final-head passes. The earlier large-page typing checks passed the configured CI gate while missing the stricter informational local ceiling. No speedup is claimed and no budget changed. Final-head results belong to the PR's Verification section.
