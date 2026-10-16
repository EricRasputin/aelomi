# Sidebar: final record-command contract

Source: `907e3bc18058b29f43defa2dd531da6373b72c1c`. Base branch: `origin/feat/shell-page-records`; recorded base head and merge base: `1f94c822078dcbb340f904773366320589f84fd8`. Verbatim patch ID: `f623148dbf0ed09f73b8c73015a59893d7304181`.

This record supersedes the earlier sidebar records for the current contract. Those records remain unchanged. #84's record-command implementation, store and tests match the recorded base. D1 adds no queue API or compatibility result.

Pathspecs proved, relative to the repository root:

```text
src/components/sidebar.tsx
src/pages/page-view.tsx
src/stores/page-titles.ts
src/stores/page-tree.ts
src/stores/tree-expansion.ts
src/stores/shell.ts
src/shell/use-page-commands.ts
src/palette/page-commands.ts
src/router.tsx
src/styles.css
```

## Shape and outcomes

Title drafts are keyed by page ID and version, with the latest caller's awaited rename result. A pending result has no per-title error. An ok result clears only when the source title matches; an error preserves the exact draft with feedback. Coalesced calls share a promise. The version check prevents an older caller from acknowledging a newer draft, including renames queued behind a different page's retained operation. The reconciler observes source changes and final caller results; it does not infer acknowledgement from retained-map projection.

New page uses #84's pending-create set: the button subscribes to `pendingCreates.has(null)` for disabled and `aria-busy`, while dispatch checks `pendingCreate(null)`. Button, empty-state button, shortcut and menu share the command. Navigation and title focus use only the ok create result, after refreshing source. Retained and failed page IDs never become navigation targets.

A local trash request contains the target page and pending/error state. Source disappearance cannot trigger fallback while the request is pending, even when a user opens a child of the retained parent. Final ok releases the fallback; error preserves the target page and shows feedback. Pure fallback order remains next active sibling, nearest active ancestor, most recently opened active page, then empty state.

## Native steps and results

Environment: Apple M1, eight logical CPUs, arm64, macOS 27.0.1 (26A434), native Tauri 2 WKWebView. The launched runtime was 31d3f42; source 907e3bc adds only a test, with identical product files. Every launch used the verify-aelomi app.sh workflow and fresh synthetic data or an explicitly named relaunch of that same fixture. Every key, focus, AXPress and transient screenshot held the keyboard lock. Locks were released between runs and on failures; no other owner's lock was removed. Appearance and every changed permission were restored.

The main run proves fresh Getting started, trash-only-page to empty, empty-state New page and immediate title focus, title-to-body Return, title/sidebar agreement, toggle, and selected row versus another hovered row. Both appearances were inspected. The create outage sets only the synthetic data root from 755 to 500 and docs from 755 to 000, presses New page, and observes the real permission-denied banner with a busy disabled control. Repeated ⌘N does not create duplicates. Both modes return to 755 before recovery, after which the new page opens and receives title typing without another focus operation.

Temporary native probe buttons inject transport I/O or terminal Internal results for rename/trash and a failed title refresh; Rust data writes remain real. Pending rename has no per-title error, terminal rename retains text with an error, and another edit recovers. Failed refresh followed by typing and Undo preserves the saved title. Palette overlays work over one-line and wrapped titles. Probe controls and the temporary HTML import are removed before the final gate and never ship.

Synthetic browser composition is armed in the real WKWebView, followed immediately in the same JXA operation by the exact PID's native Quit menu item. The app exits normally before the reader opens storage. Rust readback before and after relaunch asserts exactly four records, these exact active/trash titles, and exactly one block containing only the body token on D1 fix2 notes:

```text
records=4 active_titles=["D1 fix2 notes", "日本語の題名"] trashed_titles=["Getting started", "Recovered native title"] body=[Text("body token")]
quit flush via=Exit pending=None
```

The reopened run verifies title-only Undo and the Japanese title, then types a long body paragraph, moves its caret upward and types again. Its caret captures prove that the title scrolls away with the body. The main run's first body helper did not produce sufficient scroll distance; its two caret captures are excluded. The reopened body-end captures were taken while native typing was still arriving and are excluded as well. The first nested keyboard-only caret captures stopped around the middle of the scrollport, and the nested relaunch's paired upper-caret captures did not both show the requested upper-edge state; those four frames are excluded. A final relaunch of the main synthetic fixture applies each appearance first, focuses the body, uses native Page Down and a window-relative native click near the upper edge, then types a short marker before each capture. Those upper-caret frames prove typing near the top in both appearances.

The nested fixture is seeded only inside a new synthetic folder. Its parent expands, collapses, and remains collapsed after normal Quit and relaunch. `expandedRows=[]` is asserted on disk after collapse and again on the relaunched settings before explicit expansion. A fresh retained-subtree repeat keeps one frontend initialization throughout. It retains parent trash, opens the child, accepts the trash while holding durability, and checks `RETAINED=1` plus the open child before both light and dark captures. Releasing durability selects the empty fallback. Its final optional light capture failed during the AX read with `The window has no web content yet`; that frame is excluded, the run was cleaned up, and no normal Quit is claimed for that run. Normal Quit and exact persistence are proved by the main, reopen, collapse, relaunch and final scrolling runs. The earlier transient subtree captures were invalidated by dev frontend reloads and are excluded. The before/after read-only listings of the maintainer's real data folder match exactly.

## Reproduction

From a clean checkout, run `bash docs/evidence/shell-sidebar/fix2-final-contract/run.sh.txt`. It copies archived helpers to ignored scratch, installs a temporary probe, launches synthetic app.sh runs, serializes interaction, restores permissions/appearance/HTML, reads exact persisted values after normal Quit, and cleans up its owned processes. Run sequentially with browser suites. Helpers remain saved with .txt suffixes; the wrapper makes executable copies only for its own scratch workflow.

The initial red run exposed the missing accessible busy state and premature trash fallback. The subtree regression changed to `/` before durability before its fix. Focused Vitest, title tests and WebKit sidebar cases passed after the fixes. The final clean committed-head `pnpm check:all` record is cited by exact SHA in the PR body. This is builder self-proof; the orchestrator owns independent review and verification.

## SPEC and test cases

### e2e/shell-sidebar.e2e.ts

- `SHELL-1 SHELL-6 Toggle sidebar`
- `SHELL-4 TREE-1 Empty state creates an untitled page with its title focused`
- `TREE-1 TREE-7 EDIT-1 Rename updates the sidebar and Return moves to the body`
- `TREE-9 Trash the open page chooses its sibling then the empty state`
- `CMD-2 CMD-3 CMD-6 New page is reachable by query and shortcut from the empty state`

### tests/playwright/shell-sidebar.spec.ts

- `SHELL-1 SHELL-6 hidden sidebar gives its width to the editor`
- `SHELL-4 TREE-1 CMD-6 trash the only page then create from the empty state`
- `TREE-1 TREE-7 SHELL-8 EDIT-1 title input persists immediately and Return starts the body`
- `TREE-7 TREE-15 title history never consumes body history after repeated Undo`
- `TREE-2 TREE-6 disclosure persists settings and a row click focuses the editor`
- `TREE-7 failed refresh preserves the title and Undo baseline without accepting edits`
- `SHELL-9 title composition commits its final input before quit freezes the field`
- `TREE-6 settings failure keeps the source tree collapsed and disclosures disabled`
- `SHELL-1 palette stays above one-line title and the title scrolls with the body`
- `SHELL-1 palette stays above wrapped title and the title scrolls with the body`
- `TREE-1 current page reveals its ancestors and scrolls into the virtualized sidebar`
- `TREE-2 sidebar focus requests do not repeat a content-search jump, and hiding transfers focus`
- `TREE-7 whitespace titles stay named and IME confirmation Return keeps title focus`
- `TREE-1 a create pending through a long outage navigates only after durable ok`
- `TREE-1 repeated New page buttons, shortcut and menu while pending create exactly one page`
- `CMD-6 final create and trash errors show failures while a pending trash waits for durable ok`
- `TREE-1 a pending create ending in terminal error keeps the existing page and failure banner`
- `TREE-9 opening a child during retained parent trash waits for durable ok before fallback`

### src/stores/page-titles.test.ts

- `TREE-7 a rename pending through an outage keeps its draft without an error then clears on ok`
- `TREE-7 an unavailable title cannot submit a rename against an empty baseline`
- `TREE-7 a matching old snapshot cannot acknowledge a newer queued draft`
- `TREE-7 a final rename refusal keeps the draft and shows a failure`
- `TREE-7 a matching source title stays a draft until the retained operation is acknowledged`
- `TREE-7 a pending rename ending in a terminal error keeps the exact text and failure`
- `TREE-7 the latest coalesced rename stays pending behind another page's retained operation`

### src/shell/page-navigation.test.ts

- `TREE-6 visible rows retain source order and omit collapsed descendants`
- `TREE-6 flattening uses an explicit stack for deep source trees`
- `TREE-9 trash opens the next active sibling before ancestor or recent page`
- `TREE-9 trash opens the nearest active ancestor when there is no next sibling`
- `TREE-9 trash opens the most recently opened active page after siblings and ancestors`
- `TREE-9 trash shows the empty state when no active fallback was opened`

### src/stores/tree-expansion.test.ts

- `TREE-6 expansion loads persisted IDs and writes the next set without touching other settings`
- `TREE-6 a disclosure change during Draining is refused with feedback`

### src/palette/page-commands.test.ts

- `CMD-2 CMD-3 commands require a non-empty query`
- `CMD-6 only Move to trash requires an active page`

### src/command-dispatcher.test.ts

- `SHELL-7 composition blocks both keyboard and menu commands`
- `SHELL-7 one enabled command runs once per key and once per menu activation`
- `SHELL-7 declined webview keys are cancelled and native keys remain native`
- `SHELL-7 disposing an old registration preserves its replacement`
- `SHELL-7 editor Undo and Redo share the keyboard and menu handler`
- `SHELL-7 field keyboard history runs and cancels without a menu listener`
- `SHELL-7 palette chrome, inert editors, and modal targets cannot undo the page`
- `SHELL-7 menu history targets the plain field and a read-only editor is disabled`
- `SHELL-7 a modal-declined editor Undo cannot reach a downstream keymap`

### src/shortcut-events.test.ts

- `SHELL-7 menu commands recover when the composing input is removed`
- `SHELL-7 keyboard composition comes from the event, independent of menu state`
- `SHELL-7 user commands stop at Draining, before IPC admission closes`

### src/editor/field-history.test.ts

- `TREE-7 title undo and redo stop at that title's initial value`

## Decisions and limits

The earlier six Consider decisions remain adopted: pointer-only press feedback; descriptive disclosures and truncation-only tooltips; Untitled for whitespace without changing saved text and buttons spanning row padding; a single body-focus owner; composing Return retaining title focus; and focus transfer when hiding a focused sidebar. No new motion or dependency is introduced in this round. The no-comments pass kept only the required quit-admission explanation and test environment directives.

At 10,000 pages the 100 ms deadline is missed by the code's structure for keystrokes closer together than one edit. Per-input rename and FIFO coalescing remain; [#91](https://github.com/aelomi/aelomi/issues/91) tracks incremental edits and projection. The earlier counted release results remain historical measurements in [fix1-record-api](../fix1-record-api/README.md#counted-release-measurement); this round makes no new latency claim or typing-budget pass. Deep/narrow tree polish remains [#87](https://github.com/aelomi/aelomi/issues/87). Only the U.S. input source is enabled, so hardware Japanese conversion Return remains unverified. No independent review gate or verifier was run by this builder.

## Runs and published captures

- Main: `~/.local/state/aelomi-verify/20261016-085303-dev-ZLEv`.
- Reopened: `~/.local/state/aelomi-verify/20261016-090618-dev-TIvz`.
- Nested expansion/collapse: `~/.local/state/aelomi-verify/20261016-091027-dev-pZKt`.
- Nested relaunch/subtree: `~/.local/state/aelomi-verify/20261016-091522-dev-WdpB`.
- Upper-edge typing: `~/.local/state/aelomi-verify/20261016-092054-dev-WG20`.
- Retained subtree repeat: `~/.local/state/aelomi-verify/20261016-092217-dev-EvjR`.
- Mock stress: `~/.local/state/aelomi-verify/20261016-035940-mock-k0bulW`.

The mock stress sweep inspected all twelve frames: Demo, Worst case, empty, one and 10,000 rows; widths 320/960/2560; light/dark and 200% CSS zoom. Unbreakable and long titles truncate, non-Latin/emoji titles remain readable, and the 10,000-row view stays virtualized. The 320 px case is below the native minimum. Existing narrow/deep limits remain #87. The mock picker is absent from production.

All 42 published originals were opened and inspected. Every public attachment was read back from [the PR comment](https://github.com/aelomi/aelomi/pull/89#issuecomment-6207766419), downloaded without authentication, decoded as PNG and compared byte-for-byte with its original.

| Filename | Bytes | SHA-256 | PR comment |
| --- | ---: | --- | --- |
| `native-create-outage-dark.png` | 152969 | `837e5fb1c93003ff01e2a7484ccbefbb90f6606f38d51ef66a13e202fa10f53d` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6207766419) |
| `native-create-outage-light.png` | 159783 | `8eaf5f26ccaf572699f9467f68ed585998291b1ec1d981f8607170be459ba1fd` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6207766419) |
| `native-created-dark.png` | 121528 | `4710a6a571ad009fd480b4a303f5cf3fd09099358dfeac5ecd3b7e0589e58e78` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6207766419) |
| `native-created-light.png` | 126470 | `fa237ddfc107c3556bbc70ddbbe1eff0f911a0191f92f09e27ec267f48fe03af` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6207766419) |
| `native-empty-dark.png` | 119509 | `b0b500eabf391c717b7cc31913e163201bc512d393206399cf5ba3b41129030f` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6207766419) |
| `native-empty-light.png` | 123270 | `b837475420faac8de7bddcfb87fa220a138269ba14098553c6cccaf13729e32d` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6207766419) |
| `native-palette-one-line-dark.png` | 153593 | `c38a86a262653188ee0a6f1c298974bac9836393a60d620ca9811e58e797d462` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6207766419) |
| `native-palette-one-line-light.png` | 162884 | `6b257cd3bbfe6e58dc2fa2d2023d37934b2da1c19849609fbacf4fc100f5cdc1` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6207766419) |
| `native-palette-wrapped-dark.png` | 195265 | `5881d8df618210c84f52c0fdcd91186a044fd72c77b8d3e37bf49bbed61f950c` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6207766419) |
| `native-palette-wrapped-light.png` | 200444 | `a52361dd63949cc5a167dc351939cab54489978431f37829cfd0b2e6a3a060d8` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6207766419) |
| `native-recovered-selection-dark.png` | 138894 | `13f99c1438d0aa61c43722c22d3ba6fb1cdec044d0f24bea1c2e40565299f9e4` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6207766419) |
| `native-recovered-selection-light.png` | 145465 | `ac14fcef741da4c8a1fefb45b86df1292599157d64f0bc7d832ae72619d3d8d9` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6207766419) |
| `native-rename-pending-dark.png` | 163655 | `772015de036969681ada94690f4013049b0831e8c7b47e034c8557474e37f93d` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6207766419) |
| `native-rename-pending-light.png` | 170957 | `12657808f7a4124b94f971f1bd41d3df0b22313fff0e22ae6d887b16ca02b44d` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6207766419) |
| `native-rename-terminal-dark.png` | 178487 | `815a61282fe69a22c3c24dc3831a49e1c3ac3539ef0302ae49d7eb21e33d3368` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6207766419) |
| `native-rename-terminal-light.png` | 185780 | `df55ea5a8f3610cd647f781ffdb51b6728fdb854f0cfe043505c0b238b64550a` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6207766419) |
| `native-seed-dark.png` | 133985 | `ea9103f1cb9c88138ca5fbc9806cd1b929fca092bb14fc458ea9443155c198ef` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6207766419) |
| `native-seed-light.png` | 138332 | `b84c6b105abb78b16abe7d2273522bad7d30835d797dfc12ce341c2127e72a0a` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6207766419) |
| `native-sidebar-hidden.png` | 103733 | `64766024175bd73c025c9f6c1ee0fb1ef89873aabf08703bf49dbc83e6102537` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6207766419) |
| `native-title-read-failure-dark.png` | 126066 | `4eceb76b9c0921cfc54e4485de0453e151985a73906494c2854142d3059703d7` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6207766419) |
| `native-title-read-failure-light.png` | 131490 | `d1914b23fa05f656f4181314e6d4bf0da751b4f4d8926a57cdce886495b5f7ff` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6207766419) |
| `native-trash-pending-dark.png` | 303651 | `1df39dc14f942df4966f9161050549f42c9b01018a6e9d629fe77f80be7e63cc` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6207766419) |
| `native-trash-pending-light.png` | 312529 | `99a1933472dfeba9c744f0982b0cf0c491a987b9be2e9aacd3ad8994bb86e6cd` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6207766419) |
| `reopened-caret-scroll-false.png` | 448161 | `90acad479339c6453435dbd435c05aacfafde1988af751c149503aae58075069` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6207766419) |
| `reopened-caret-scroll-true.png` | 436677 | `9b5000a227337bce0afd4a7948ca2d931888d72d92ccb61a1de6cc01908ba4bc` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6207766419) |
| `reopened-composition-reopened-false.png` | 131990 | `20da4a2fa035355125911aa12606c41255d10973fa11613f58a065e26a3eabae` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6207766419) |
| `reopened-composition-reopened-true.png` | 132838 | `054fa5310b341c5b2880b5171dac811c40b383fdfd497b1a1fc5dbfb2ac48555` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6207766419) |
| `reopened-title-undo-false.png` | 131380 | `5934b2070e31e80de46b58c91e037f3862ede54f3ff1b1985d6538c936a7bfa3` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6207766419) |
| `reopened-title-undo-true.png` | 127627 | `df8b058cc0e4cd5e56303627965840dd4d83e724faec0a419df438a9a1ed2937` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6207766419) |
| `nested-collapsed-false.png` | 129319 | `854ce374f9a567ebb614f795ab8d43003f8bae291c061d41d1bcc3f469cc33a4` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6207766419) |
| `nested-collapsed-true.png` | 126130 | `1677fa19578c9eb0abd3e9f2e3f8a6c8002b33bd0708f0907b7ce8e617de51e3` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6207766419) |
| `nested-expanded-false.png` | 135784 | `88cfc32d75a12fe47ae20c57f11072278e425423a4f8d1115948df99debeb3b1` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6207766419) |
| `nested-expanded-true.png` | 133318 | `52ae0b8f3fade3406bb73470ba89e9a8573ef0a88b68c1b65f74edd89ca6676d` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6207766419) |
| `nested-sidebar-hidden-false.png` | 443202 | `53af733191b6672c780e5323361d07c4b8d617d5eb88712e6a71ec6a6248ab35` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6207766419) |
| `nested-sidebar-hidden-true.png` | 429525 | `9696e1a8aa586c8f25c26ddef7e3922307e6c273d1613f7813d932d931a5df6b` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6207766419) |
| `nested-reopened-collapsed-false.png` | 380166 | `02c526aff4f746664374d930f40ca254b86ba927bb87e3de898ce92ecc2a6829` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6207766419) |
| `nested-reopened-collapsed-true.png` | 369845 | `093dd8501d65bdd881f1aa0e691a8cfc2a3a01cd1a61d083c34c0fa198939e8a` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6207766419) |
| `scroll-upper-caret-false.png` | 450687 | `f0a93f6dd80f05dd25412e8cab494749d2c87aa71129a51c3ab065d73e0f734f` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6207766419) |
| `scroll-upper-caret-true.png` | 435877 | `fb12acd1541920443d40ebc942e08182a569c45a3cb4e42d60f0ed7088ca8ce4` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6207766419) |
| `subtree-subtree-pending-false.png` | 195201 | `12b279d662f79a4a17f58e878ca31542e09e9895a600fa47faa068d16e572d5a` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6207766419) |
| `subtree-subtree-pending-true.png` | 189851 | `632c53ba1674711a99f68338b22e90ca3252590973cdfa376ac6ce6028d4b5b7` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6207766419) |
| `subtree-subtree-recovered-true.png` | 126807 | `fe84a58e8e23d19e9913e5d2837f7594ed61afb595c9bd25bc05191ce774a8ea` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6207766419) |
