# Sidebar round 3: preserve input and page intent

Source commit: `8937ead589ea1970c157ec0784f6d4916cd95433`. Base branch: `origin/feat/shell-page-records`. Recorded base head and merge base: `3fd585586dd6593cb9876367a0a786223e0414eb`. Verbatim patch ID: `3bd5fc0a5dd24f9f4cb68c93e10176d3f0133557`.

This record supersedes the prior sidebar records for the paths below. Earlier records remain historical evidence. The record-command implementation and store match #84's base; D1 adds no queue API or compatibility result.

Pathspecs proved, relative to the repository root:

```text
src/components/sidebar.tsx
src/components/pending-page-title.tsx
src/components/command-palette.tsx
src/pages/page-view.tsx
src/pages/page-route.tsx
src/editor/field-history.ts
src/editor/history-commands.ts
src/stores/page-tree.ts
src/stores/page-titles.ts
src/stores/tree-expansion.ts
src/stores/shell.ts
src/stores/palette.ts
src/stores/quit.ts
src/shell/page-opening.ts
src/shell/use-page-commands.ts
src/palette/page-commands.ts
src/router.tsx
src/styles.css
src/command-dispatcher.ts
src/shortcut-events.ts
src/shortcuts.ts
src-tauri/menu.json
src/use-shortcut-commands.ts
src/ipc/mock.ts
```

## Shape and behavior

Create intent contains the origin page, IDs present at dispatch, editor-focus revision, buffered title, activation source and pending/opening/failed stage. The temporary New page title input captures immediate keyboard input while the new record is pending. Button activation retains focus on the visible busy control and transfers typing into that buffer. Buffering lets the user continue typing or leave a delayed create without editing the old body. On final ok, navigation and title focus require the original page and unchanged editor-focus revision; otherwise the sidebar reveals the created row and shows a quiet notice. A failed create retains its text for another attempt.

Title histories and drafts are session maps keyed by page ID. Navigation preserves title Undo and Redo, and initial buffered text becomes an undoable title step. The mounted title receives the buffer with its insertion point at the end. Pending trash holds the target's affected IDs, making its title and descendants read-only. The ready source-tree shape retains its last tree plus an error flag, so a failed refresh preserves rows and title, displays Try again, and retries with the shared storage backoff.

Sidebar, palette and route opening reject pending create IDs, including IDs observed before the first attempt has a retained entry. Existing rows remain openable during an outage. Navigation stops as soon as Quit leaves Open. Titles mounted during an existing drain start frozen. Palette commands match only prefixes and word starts; initial selection skips Move to trash, including when content results arrive.

## Environment and native proof

Apple M1, MacBookPro17,1, eight logical CPUs; arm64 macOS 27.0.1 (26A434); real Tauri 2 WKWebView. All runs use verify-aelomi app.sh with fresh synthetic data or an explicitly named relaunch of the same synthetic fixture. Every native key, focus, AXPress and transient screenshot holds `/tmp/aelomi-ui.lock`. No Finder action was used. Both changed folder permissions and the original appearance were restored. Long compilation and fixture seeding ran outside interaction locks; short app.sh port relinks were serialized under a separate launch lock to keep window activation from interrupting another proof.

The main run verifies Getting started, explicit trash to the empty state, empty-state New page with immediate title typing, title/sidebar agreement, Return into the body, and the sidebar toggle in both appearances. The button outage changes only the synthetic root from 755 to 500 and docs from 755 to 000. Creating page keeps visible keyboard focus and the save banner appears. Typed text reaches the recovered title without another focus action; both modes return exactly to 755.

In light and dark, native Command N followed immediately by typing fills the pending new title while Alpha's body remains unchanged in the capture. Repeated Command N does not add a duplicate. After recovery, text appends at the end; Undo twice reaches an empty title and Redo restores the buffered text. A → B → A preserves the next Undo/Redo step. Return after `to` never trashes a page; a safe page result may open. Move to trash requires explicit Down and Return.

Both drain attempts hold a body delivery, start native Quit, attempt another row plus Command K/Command N during Draining, and keep Alpha open with the exact title, a frozen title field and no palette. The captured probe suffix is:

```text
TITLE=D1 native Alpha;FROZEN=true;QUIT=draining;PALETTE=false
PASS light safe trash, immediate title, navigation history, drain page freeze
PASS dark safe trash, immediate title, navigation history, drain page freeze
PASS native main
```

Separate relaunches in both appearances verify the exact saved title, selected-row contrast against another hovered row, retained tree/title through failed reads, palette layering over one-line/wrapped titles, and native typing near the upper scroll edge while the title is scrolled away. Synthetic composition followed immediately by native Quit exits normally in both appearances.

The drain probe holds only the body transport promise before it reaches Rust. A real native Quit starts the drain. A subscribed test probe then attempts the actual sidebar row click and synthetic Command K/Command N events inside that short drain window. It records route, exact title, read-only state and palette visibility. No store mutation forces the expected navigation result. The preserved-page captures are taken after Keep app open: their panel shows the snapshot recorded during Draining, while editing has resumed. The real body transport is then released. The WebKit regression separately drives sidebar, palette and keyboard interactions while a delivery is held.

Additional native probes hold source-tree reads, position the selected body block at the start of its scrollport, and synthesize a composition lifecycle. Native typing then proves upper-edge visibility without a fixed screen-point click. All record and body writes still use real Rust storage. The composition probe begins an unfinished title, then native Quit runs in the same JXA operation. Its blur completion writes the exact final title. These are synthetic timing/composition probes, not a hardware IME claim. Probe modules and temporary HTML imports are removed before the final gate and never ship.

Rust opens storage only after the native instance has exited and cleanup has stopped its dev processes. The main read and both post-composition reads assert exactly the same records and body:

```text
PASS records=5 active_titles=["Button recoveredx", "D1 native Alpha", "Immediate darkx", "Immediate lightx"] trashed_titles=["Getting started"] body=[Text("body token dark")]
quit flush via=Exit pending=None
```

The nested fixture expands and collapses Projects, then normal Quit and relaunch keep its children hidden. Both appearances are captured, and disk checks after each normal Quit report:

```text
PASS expandedRows=[] after normal quit
PASS native proof and exact real-data listings
```

Before/after read-only listings of the maintainer's real data folder match byte-for-byte. No app instance used that folder.

## Reproduction and verification

From a clean checkout, run `bash docs/evidence/shell-sidebar/fix3/run.sh.txt`. The wrapper copies archived helpers to ignored scratch, installs temporary probes, serializes native interaction, restores appearance/permissions/HTML, reads storage only after the app exits, and cleans up its own processes. Run sequentially with browser builds. Move its helper scratch outside the checkout before `pnpm check:all`, because Biome scans ignored helpers.

For the mock sweep, copy `stress.mjs.txt` to a temporary `.mjs` file and pass it to `.claude/skills/verify-aelomi/scripts/mock-drive.mjs --flow <file> --rebuild` after native cleanup. `gate-node.sh.txt` is a transparent Node launcher: it forwards all arguments to `AELOMI_GATE_NODE` or the next Node executable on PATH, acquiring the keyboard lock only around `perf/cold-start.ts` so the final gate's native benchmark cannot interrupt another proof. Build stages stay outside that lock; no test or threshold is changed.

The original eight requested WebKit regressions all failed on `666bc6310b24db4bfe2ab0c426afab8129e9ab06` before the fixes. Further red cases caught a buffered title receiving appended text at the start, a first-attempt create opening before the final result, and accessible button activation losing focus/typing. After the fixes the focused 35-case WebKit run passed. The final clean committed-head `pnpm check:all` record is cited by exact SHA and path in the PR body. Its hosted performance thresholds do not establish a general SPEC performance pass.

The final mock stress sweep exercises Demo, Worst case, empty, one and 10,000 rows; widths 320/960/2560; light/dark and 200% CSS zoom. It also holds a new-page creation and fills its pending title with a long URL, CJK, emoji and Arabic. Exact recovery retains the full title and normal widths have no document overflow. All 20 local captures were inspected. The network guard found no non-local requests, and the preview port was free after cleanup. The 320 px viewport is below the native minimum; existing deep/narrow limits remain #87.

This is builder self-proof. No independent review gate or verifier was run.

## Round-3 SPEC regressions

| File | Exact case name |
| --- | --- |
| `tests/playwright/sidebar-round3.spec.ts` | `EDIT-11 title Undo and Redo survive A to B to A` |
| `tests/playwright/sidebar-round3.spec.ts` | `TREE-1 typing immediately after Command N reaches only the new title` |
| `tests/playwright/sidebar-round3.spec.ts` | `TREE-1 accessible New page activation keeps focus and buffers typing` |
| `tests/playwright/sidebar-round3.spec.ts` | `TREE-1 buffered title keeps its insertion point and Undo after creation` |
| `tests/playwright/sidebar-round3.spec.ts` | `CMD-3 Move to trash requires explicit selection after to and content replies` |
| `tests/playwright/sidebar-round3.spec.ts` | `TREE-1 late create reveals its row without navigating or renaming after editor focus` |
| `tests/playwright/sidebar-round3.spec.ts` | `TREE-7 refresh error keeps rows and title with a retry notice then recovers` |
| `tests/playwright/sidebar-round3.spec.ts` | `TREE-9 pending trash freezes the target title and its descendants` |
| `tests/playwright/sidebar-round3.spec.ts` | `TREE-1 pending create cannot open from a row, palette, or route before durable ok` |
| `tests/playwright/sidebar-round3.spec.ts` | `SHELL-9 draining blocks sidebar and palette navigation and preserves the exact title` |
| `src/shell/page-opening.test.ts` | `TREE-1 opening a first-attempt create waits for its final result` |
| `src/palette/page-commands.test.ts` | `CMD-3 command matching stops at prefix and word starts` |
| `src/stores/page-tree.test.ts` | `a failed refresh retains the last tree and retries with storage backoff` |

## SPEC and test cases

### e2e/shell-sidebar.e2e.ts

- `SHELL-1 SHELL-6 Toggle sidebar`
- `SHELL-4 TREE-1 Empty state creates an untitled page with its title focused`
- `TREE-1 TREE-7 EDIT-1 Rename updates the sidebar and Return moves to the body`
- `TREE-9 Trash the open page chooses its sibling then the empty state`
- `CMD-2 CMD-3 CMD-6 New page is reachable by query and shortcut from the empty state`

### e2e/indexing.e2e.ts

- `DATA-2 a page list another writer stored as a sequence says why no page opens, and nothing is written`

The acceptance check scopes the workspace diagnostic to the main pane and also asserts the sidebar's new read-error notice. Its final `docAppend` call list remains exactly empty.

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

No new motion or dependency was added. The manual no-comments pass kept the required quit-admission explanation and test environment directives; no new phase narration was introduced. The earlier six Consider decisions remain adopted: pointer press feedback, descriptive disclosures with truncation-only tooltips, whitespace display without changing saved text, one body-focus owner, composing Return retaining title focus, and focus transfer when hiding a focused sidebar.

At 10,000 pages the 100 ms deadline is missed by the code's structure for keystrokes closer together than one record edit. Per-input renames and FIFO coalescing remain; [#91](https://github.com/aelomi/aelomi/issues/91) tracks incremental edits and projection. This round makes no new latency claim or typing-budget pass. Narrow/deep tree polish remains [#87](https://github.com/aelomi/aelomi/issues/87). Hardware Japanese conversion Return remains unverified on this U.S.-input-source Mac. Native dev logs reported `ResizeObserver loop completed with undelivered notifications.` during wrapped-title setup. Both appearances continued through typing, normal Quit, and exact readback; that notification remains uninvestigated. Terminally rejected title drafts at Quit remain the follow-up identified by round-2 review; this round addresses its eight Act items.

Earlier partial runs used during harness diagnosis are excluded. Cached accessibility references across a drain produced an invalid-index error; an empty title has no AX value string; and an incorrect macOS key code inserted a literal k. Their frames do not support this record. A caret probe first reported NO_CARET_BLOCK after AX activation; its run is excluded and was repeated using the synthetic text marker. 

## Runs and published captures

- Main: `~/.local/state/aelomi-verify/20261016-112640-dev-2FaJ`.
- Light relaunch/composition: `~/.local/state/aelomi-verify/20261016-114852-dev-ahvo`.
- Dark relaunch/composition: `~/.local/state/aelomi-verify/20261016-115338-dev-uLbV`.
- Nested expand/collapse: `~/.local/state/aelomi-verify/20261016-115830-dev-Blpv`.
- Nested relaunch: `~/.local/state/aelomi-verify/20261016-115957-dev-MAQq`.
- Mock stress: `~/.local/state/aelomi-verify/20261016-063100-mock-mDXjn5`.
- Nested synthetic data: `~/.local/state/aelomi-verify/20261016-115829-d1-fix3-nested-T7il/data`.

All 42 native captures were inspected. Every attachment in [the PR comment](https://github.com/aelomi/aelomi/pull/89#issuecomment-6209592572) was downloaded without authentication, decoded as PNG, and matched byte-for-byte with its original. The 20 mock stress captures remain local in the named run.

| Filename | Bytes | SHA-256 |
| --- | ---: | --- |
| `main-button-create-focused-dark.png` | 144247 | `ea04d9d9ef0a386fc8100b99bdbc544addeaa0ba0e75ae6a660a0455c5eea756` |
| `main-button-create-focused-light.png` | 148008 | `635bdb6b68b95c4af8d15ec8ff04e5d13883d05e90eaf6689dfa6ec6221a63ba` |
| `main-button-create-pending-dark.png` | 150714 | `c40538638579678727f3abe216f1145ad50f0ad600d7c42bcefc150cb89ddbd5` |
| `main-button-create-pending-light.png` | 155427 | `7056b3223ea86c9d2904ec82e711b12aa34c03d43f0c0dad9b0a49004d1b7efa` |
| `main-drain-blocked-dark.png` | 212269 | `f0eb643ffd5a1950c3fdac211bdcebea7b3bcbd12fbd6850b2eb69525bb794d9` |
| `main-drain-blocked-light.png` | 240529 | `d66417c2e28692a83e43eb9d5b703216c88f774588616dd73c29835f390f28d5` |
| `main-drain-page-preserved-dark.png` | 151383 | `9b7758e6be1e130577c75445e110280597a62d8106c384aa1d3161e9989b5f9e` |
| `main-drain-page-preserved-light.png` | 153970 | `4f3a0ac64f6a56e86df3601926ef19e093bf10cda1ecf6745873087ad6892c5c` |
| `main-empty-dark.png` | 111634 | `64d5e3f43e5784e5b8c3efa8b9c48a24ce051742af2da55c685c5a8f04ed2c53` |
| `main-empty-light.png` | 114795 | `582e33f45ab240f0dc8d8700042b07087a58dfc7fea70c1e2c87afbc8fc3e739` |
| `main-immediate-created-dark.png` | 136088 | `0ffdfe0d65a665c18fac8516762a15d688e3c870397f60037c9bef342b68edad` |
| `main-immediate-created-light.png` | 133990 | `f2a7415c0a7a35abb33a0bcfbab8e2d6372556f0aecf2b7a4f48d4b4db03831e` |
| `main-immediate-pending-dark.png` | 166319 | `cb529e6fb491a99aae8fc313eca44996a8dfcec4a9bcff2f4718e3ec91e69fcc` |
| `main-immediate-pending-light.png` | 162301 | `31cfd4013de05c0844291f8db6c583a8b38ad139f7b2e4219b3b5dd1b9bf4f0a` |
| `main-navigation-redo-dark.png` | 137524 | `7b47e718562fae1fc9eff136c1e3008175b4e6ce446d128c801f5a8cfa142836` |
| `main-navigation-redo-light.png` | 135320 | `ecdc0481f638391446204bc7767d1accf6fd35b88e5402b525117c54eb5d352b` |
| `main-navigation-undo-dark.png` | 136088 | `0ffdfe0d65a665c18fac8516762a15d688e3c870397f60037c9bef342b68edad` |
| `main-navigation-undo-light.png` | 133990 | `f2a7415c0a7a35abb33a0bcfbab8e2d6372556f0aecf2b7a4f48d4b4db03831e` |
| `main-safe-to-return-dark.png` | 137971 | `139aa73616abefaa2e47c954789e6ea7de8dd3068660fcfef261b0a769bece35` |
| `main-safe-to-return-light.png` | 136109 | `b560e25ff485ca3acb31cf6448162415bced5aafe84ad9095e7d90533652771b` |
| `main-seed-dark.png` | 130762 | `7edf96499991badbb02a06780f5488048890e166f1820d02c1c1129a220482d3` |
| `main-seed-light.png` | 136684 | `a970502db0a1a65f6aeb41c1d959e75b002dcde82c04ef417b126efeb7368cc3` |
| `main-sidebar-hidden-dark.png` | 96195 | `5e18b0e716b5582ff2c766c5352e8e9e0a95925afbb3ba8911d62fb4f870edda` |
| `main-sidebar-hidden-light.png` | 100566 | `48fc0028bef5e1c75ed1bdd122c1416cd51a84b8192961e1edc00df664cfb666` |
| `extra-light-active-versus-hover-light.png` | 134740 | `62691184f3876f7bd0902af205e13d112d560402623ac91ea0fc583fbf1e9329` |
| `extra-light-exact-title-relaunch-light.png` | 134940 | `d6755220586db16178afb6d65784d0bbb8739b68c5252604857e4057814b09da` |
| `extra-light-palette-one-line-light.png` | 161129 | `a1649995c8af3acae303d066c7009d5a462497bd0ab438e3f6416810df73b10f` |
| `extra-light-palette-wrapped-light.png` | 215147 | `bab4e904d2aadac1a766ca52818fb2aa7603fdfec20bead418059b85004150d4` |
| `extra-light-retained-tree-and-title-light.png` | 161500 | `65e41c4537914f06d9e2c203c837d177f3b767254ceb8d55b727b93f83eef2d9` |
| `extra-light-title-scroll-and-upper-caret-light.png` | 264310 | `00ec64eaed15cc684a15d947ef075af33ec98fbc32e70df96667ec0b9cfadcea` |
| `extra-dark-active-versus-hover-dark.png` | 130919 | `18fca8fd062aaac9a3443f358ae5b3b87a940066f6af136c01c845f8330e66fb` |
| `extra-dark-exact-title-relaunch-dark.png` | 130917 | `d0ada2ca9c024d02452dfb710500e709997e3fa472e408ca082b054b63c17f90` |
| `extra-dark-palette-one-line-dark.png` | 153398 | `4b5e6e622ed6e479e147e25aba44bb421380838964feab47bb3e8bf4161aa8c9` |
| `extra-dark-palette-wrapped-dark.png` | 213366 | `3dd9858ef2b2f20a31edc9d6abd177fa400802d781aa19e43028d49d7a2bc652` |
| `extra-dark-retained-tree-and-title-dark.png` | 156863 | `ce00a2e9bdcda3ec2c6174b26ec0c5d87f90a2faee9db4c3ffae04751b81e5c8` |
| `extra-dark-title-scroll-and-upper-caret-dark.png` | 256355 | `3da9e0c5702198f2ac531a7e5e3e3e9399c4d4afeb8bb12ecd4cd6a82f1aac57` |
| `nested-expanded-collapsed-false.png` | 112274 | `ccb49d355a128e5a8db905474bc3e8902ffba8a79487e93e36283d4cf40f5308` |
| `nested-expanded-collapsed-true.png` | 109025 | `f1c27dce25cb4ca8791bcb4b6685e792a39c998dd02f902e3dbb6f0a801addf1` |
| `nested-expanded-expand-false.png` | 118998 | `27dd997041e7ac0d94929e1c12716f6e3663259c8c44fc1c934d059d43f3de41` |
| `nested-expanded-expand-true.png` | 115884 | `919e5b4001af17f189cdc6d916e81ad554894cad6a932a23967014105d473daf` |
| `nested-reopened-reopen-false.png` | 112274 | `ccb49d355a128e5a8db905474bc3e8902ffba8a79487e93e36283d4cf40f5308` |
| `nested-reopened-reopen-true.png` | 109046 | `68dc08dbebf130c5068fe677d94c3b186ce84feee98e681c3d6f9b60bd3b3e10` |
