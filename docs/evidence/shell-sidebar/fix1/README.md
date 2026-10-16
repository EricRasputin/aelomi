# Sidebar fix round 1 evidence

Source commit: `41c24c2c82bef17287774ef80ed0a2c12f7af9de`. Base branch: `origin/feat/shell-page-records`.
Recorded merge base and #84 head: `e4251b62e2e0f3250b3ff3d77ebf376c148b7e14`.
Verbatim patch ID: `b8304298076f39d8440ffd6dd66e379df540e8e0`.

Pathspecs proved by this record, relative to the repository root:

```text
src/components/sidebar.tsx
src/pages/page-view.tsx
src/stores/page-titles.ts
src/stores/page-tree.ts
src/stores/tree-expansion.ts
src/stores/shell.ts
src/shell/use-page-commands.ts
src/ipc/record-commands.ts
src/palette/page-commands.ts
src/router.tsx
src/styles.css
crates/core/tests/rename_latency.rs
```

Environment: Apple M1, arm64, 8 logical CPUs, macOS 27.0.1 (26A434), native Tauri 2 WKWebView. Every self-proof app launch used `verify-aelomi/scripts/app.sh` with synthetic data. The keyboard lock was acquired before focus, keys, presses, appearance changes and transient screenshots, released on failure and between scenarios, and never held for 20 minutes. The read-only real-data directory listings before and after all native runs match exactly.

## Data shape and ownership

The source tree projects to a flat array of visible rows; expanded page IDs form a set. Title drafts form a page-ID map containing title, version and the queue's existing rename promise. A single reconciler requires both operation acknowledgement and the matching source title, so an old snapshot cannot discard a new draft.

D1 adds `renameState` and `observeRename` to #84's `src/ipc/record-commands.ts`. This additive observation of existing jobs changes no existing export signature, caller, command result, FIFO ordering, coalescing or retry decision. Its own Vitest case follows one promise through failure and background success. It is a temporary adapter for the published #84 head named above. #84's announced `useRecordCommandStore` and `status: "retrying"` replies were not published at this rebase; migration of titles, New page and Move to trash to that contract remains dependent on #84's round 5. Retained titles already stay drafts without a per-title error. Final refusals retain the text and show a failure.

## Native runs and exact results

All paths below start at `~/.local/state/aelomi-verify/`.

| Run | Result |
| --- | --- |
| `20261016-055516-dev-3oMC` | Fresh Getting started sidebar; trash the only page; empty state; New page title focus; native typing updates sidebar; title-only Undo preserves body; hide/show sidebar; palette above one-line and wrapped titles; visible upper-scrollport caret; selected row stronger than another hovered row; failed title refresh ignores typing and Undo; recovery preserves history; synthetic composition followed immediately by native Quit. Light and dark captures. |
| `20261016-060640-dev-MNKc` | Independent light appearance repeat: trash, empty state, create, title/body focus, title-only Undo, sidebar toggle, synthetic composition and immediate native Quit. |
| `20261016-060926-dev-Q49D` | Fresh nested fixture; expand Projects in dark and light, then collapse and normal quit. Persisted `expanded_rows` equals `[]`. |
| `20261016-061204-dev-o8tb` | Relaunch the preceding synthetic folder. Projects remains collapsed, child buttons are absent, persisted `expanded_rows` remains `[]`; dark and light captures, normal quit. |
| `20261016-061751-dev-iAB8` | Fresh queued-title proof on this source commit. Diagnostic transport I/O faults keep the source at Getting started while native typing displays Pending native title in field and sidebar without a per-title final error. Restore transport and native Quit retries and drains the title. Dark and light captures. |

The first four runs used the pre-rebase source snapshot `8ca46b5`. Comparing its `src/` and rename benchmark paths with this source commit shows only the retained-title feedback change, which the fifth native run proves. The final clean-head gate reruns the automated suite on the recorded implementation plus this evidence commit.

Each doctor check passed, including process ownership, window, accessibility, web content, dev port ownership, and `ping seq=1`. Every normal quit completed with these lines:

```text
quit draining via=ExitRequested revision=1
quit flush via=CompleteQuit pending=None
quit confirmed via=drain
quit allowed via=ExitRequested confirmed=true
quit flush via=Exit pending=None
```

The core reader opens each closed synthetic folder and checks record counts, titles, trash state and exact body segments. First run:

```text
active_titles=["D1 fix1 notes", "日本語の題名"] trashed_titles=["Getting started"]
```

It also asserts exactly three records and one body block containing only `Text("body token")` for D1 fix1 notes. The independent light run asserts exactly two records and the same single body segment:

```text
active_titles=["日本語の題名"] trashed_titles=["Getting started"]
```

Queued-title run asserts exactly one active record, zero trashed records:

```text
active_titles=["Pending native title"] trashed_titles=[]
```

The composition probe dispatches browser composition events in the real WKWebView, then a PID-scoped native Quit immediately. The exact Japanese title is read from Rust storage after exit. Only the U.S. input source is enabled on this Mac, so hardware Japanese conversion Return was not verified. WebKit event tests cover composing Return, `isComposing`, and conversion key code 229; this evidence does not claim a hardware IME test.

## Reproduction

From a clean checkout, run `bash docs/evidence/shell-sidebar/fix1/run.sh.txt` for the native retained-title failure, both appearances, recovery, normal quit and exact disk readback. It temporarily installs the diagnostic probe, launches only through app.sh on a fresh synthetic folder, uses the lock, and restores index.html, appearance and process state on exit. Run it outside `check:all`, since the probe is intentionally absent from production and from the committed tree. `read-records.sh.txt <closed synthetic folder> <fresh|light|pending>` reruns the exact readback assertions.

For the remaining native scenarios, launch a fresh app.sh dev run, take the lock with `../keyboard-lock.py.txt`, and drive the named Page title, Page body, New page, and Projects children controls. Use the original `../run.sh.txt` as the basic title/body/toggle recipe, adjusting the title to D1 fix1 notes. Query Move to trash in ⌘K on the only page, then New page from empty state. `native-probe.mjs.txt` supplies failed/recovered title reads and composition finalization. `compose-quit.js.txt` arms the probe and sends native Quit in the same PID-scoped JXA process. `body.js.txt` and `hover.js.txt` reproduce the upper-scrollport caret and inactive hover screenshots. Seed the nested fixture with `cargo run -p aelomi-core --example seed_index -- <new synthetic folder> missing`; relaunch only that synthetic folder with app.sh to check collapse persistence.

## Proving cases

Unless another file is named, WebKit cases live in `tests/playwright/shell-sidebar.spec.ts`.

| Item / SPEC | File and case |
| --- | --- |
| Act 1, TREE-7, SHELL-8 | WebKit: `TREE-7 failed refresh preserves the title and Undo baseline without accepting edits`; `src/stores/page-titles.test.ts`: `TREE-7 an unavailable title cannot submit a rename against an empty baseline`. Native unknown-title and recovered-title captures. |
| Act 2, SHELL-1 | WebKit: `SHELL-1 palette stays above one-line title and the title scrolls with the body`, and its wrapped-title variant. Native palette and upper-caret captures in both appearances. |
| Act 3, CMD-2, CMD-3, CMD-6 | `src/palette/page-commands.test.ts`: `CMD-2 CMD-3 commands require a non-empty query`, `CMD-6 only Move to trash requires an active page`; `e2e/shell-sidebar.e2e.ts`: `CMD-2 CMD-3 CMD-6 New page is reachable by query and shortcut from the empty state`. |
| Act 4, TREE-6 | WebKit: `TREE-6 settings failure keeps the source tree collapsed and disclosures disabled`. The test recovers settings and checks children return. |
| Act 5, TREE-1 | WebKit: `TREE-1 current page reveals its ancestors and scrolls into the virtualized sidebar`; native selected-hover captures in both appearances. |
| Act 6, TREE-7 | `src/stores/page-titles.test.ts`: `TREE-7 a retained rename stays a draft without an error until retry acknowledgement`, `TREE-7 a matching old snapshot cannot acknowledge a newer queued draft`, `TREE-7 a final rename refusal keeps the draft and shows a failure`; `src/ipc/record-commands.test.ts`: `rename observers follow the same promise through a retained failure and background acknowledgement`. Native queued-title captures and exact disk readback. |
| Act 7, SHELL-9 | WebKit: `SHELL-9 title composition commits its final input before quit freezes the field`; native synthetic composition plus immediate native Quit with exact Japanese title and body readback. |
| Act 8, SHELL-9 | `crates/core/tests/rename_latency.rs`: `measure_edit_page_rename_on_10000_pages`; actual counters and exact title readback for every successful sample. Tradeoff below. |
| SHELL-1, SHELL-6 | Acceptance: `SHELL-1 SHELL-6 Toggle sidebar`; WebKit: `SHELL-1 SHELL-6 hidden sidebar gives its width to the editor`. Native hide/show captures. |
| SHELL-4, TREE-1 | Acceptance: `SHELL-4 TREE-1 Empty state creates an untitled page with its title focused`; WebKit: `SHELL-4 TREE-1 CMD-6 trash the only page then create from the empty state`. Native fresh/empty/create captures. |
| TREE-1, TREE-7, SHELL-8, EDIT-1 | Acceptance: `TREE-1 TREE-7 EDIT-1 Rename updates the sidebar and Return moves to the body`; WebKit: `TREE-1 TREE-7 SHELL-8 EDIT-1 title input persists immediately and Return starts the body`. |
| TREE-7, TREE-15 | WebKit: `TREE-7 TREE-15 title history never consumes body history after repeated Undo`; `src/editor/field-history.test.ts`: `TREE-7 title undo and redo stop at that title's initial value`. Native exact body-token readback. |
| TREE-2, TREE-6 | WebKit: `TREE-2 TREE-6 disclosure persists settings and a row click focuses the editor`; `src/stores/tree-expansion.test.ts`: `TREE-6 expansion loads persisted IDs and writes the next set without touching other settings`, `TREE-6 a disclosure change during Draining is refused with feedback`. Native collapse/relaunch. |
| TREE-6 | `src/shell/page-navigation.test.ts`: `TREE-6 visible rows retain source order and omit collapsed descendants`, `TREE-6 flattening uses an explicit stack for deep source trees`. |
| TREE-9 | Acceptance: `TREE-9 Trash the open page chooses its sibling then the empty state`; `src/shell/page-navigation.test.ts`: `TREE-9 trash opens the next active sibling before ancestor or recent page`, `TREE-9 trash opens the nearest active ancestor when there is no next sibling`, `TREE-9 trash opens the most recently opened active page after siblings and ancestors`, `TREE-9 trash shows the empty state when no active fallback was opened`. |
| SHELL-7 | `src/command-dispatcher.test.ts`: `SHELL-7 a modal-declined editor Undo cannot reach a downstream keymap`, `SHELL-7 declined webview keys are cancelled and native keys remain native`; `src/shortcut-events.test.ts`: `SHELL-7 menu commands recover when the composing input is removed`, `SHELL-7 keyboard composition comes from the event, independent of menu state`, `SHELL-7 user commands stop at Draining, before IPC admission closes`. |

Red runs reproduced the unknown-title overwrite, composition loss, hidden tree on settings error, palette occlusion/scroll failure, offscreen active page, repeated content-search jump, whitespace-only unnamed rows, stale retry drafts and per-title retained-error behavior before their corresponding fixes. Logs stay local at `/tmp/aelomi-d1-red-*.log`. A later full WebKit run exposed a row-relative locator timeout; changing only that locator made the padding-click case pass. No product behavior was weakened to satisfy it.

## Consider decisions

All six suggestions were adopted. Pointer presses get a 160 ms transform release with the existing strong easing; keyboard activation and reduced motion get none. Disclosures use the draft display title plus children, with `aria-expanded` carrying state; a title tooltip exists only on measured truncation. Whitespace-only records display Untitled without rewriting saved text; page buttons cover the highlighted row padding. PageView consumes body focus once, separately from content-search jumps. Composing Return and WebKit key code 229 retain title focus. Hiding a focused sidebar transfers focus to the page.

WebKit cases `TREE-2 sidebar focus requests do not repeat a content-search jump, and hiding transfers focus` and `TREE-7 whitespace titles stay named and IME confirmation Return keeps title focus` cover the focus and naming details. The disclosure case clicks row padding. Truncation is measured after every render and on resize. Deep-nesting display at narrow widths remains the review's explicitly deferred #87 issue.

## Stress and measurement limits

`20261016-001940-mock-aq3zc0` passed the mock stress sweep: demo, worst case, empty, one page, and 10,000 rows; 320/960/2560 widths; light/dark and 200% CSS zoom. All twelve captures were inspected. The 320 px case is below the native window's 480 px minimum. Long and unbreakable titles truncate; non-Latin and emoji titles stay readable. Deep indentation at narrow widths remains #87. The development picker is absent from production; the production bundle check is part of the local gate. `stress.mjs.txt` is the rerunnable sweep.

Counted release measurement: five rounds, twenty sequential renames per round, 10,000 records, system clock and OsFs, each successful rename read back exactly. All 100 succeeded, zero failed. Command: `CARGO_BUILD_JOBS=4 cargo test -p aelomi-core --release --test rename_latency -- --ignored --nocapture --test-threads=1`. Load was 3.16/5.87/11.06 on eight cores before the run.

```text
round=0 successes=20 failures=0 accepted_p95_ms=52.722 durable_p95_ms=55.434
round=1 successes=20 failures=0 accepted_p95_ms=92.142 durable_p95_ms=96.372
round=2 successes=20 failures=0 accepted_p95_ms=54.433 durable_p95_ms=56.846
round=3 successes=20 failures=0 accepted_p95_ms=53.656 durable_p95_ms=56.396
round=4 successes=20 failures=0 accepted_p95_ms=70.386 durable_p95_ms=74.997
```

These are a lower bound on title latency, excluding queue wait, IPC and source projection. There is no speedup claim or proof of the 100 ms typing budget. At 10,000 pages the deadline is missed by the code's structure for keystrokes closer together than one edit. Existing O(pages) acceptance work is the structural limiter; its individual decode, planning and append components were not profiled. Per-input rename and FIFO coalescing remain. [#91](https://github.com/aelomi/aelomi/issues/91) tracks incremental edits and projection.

The final `pnpm check:all` run is recorded separately by exact committed head at `.local-gate/<sha>.checks.json` and cited in the PR body. `check-node.sh.txt` serializes its synthetic cold-start harness under the same keyboard lock; all other Node commands execute unchanged. Native self-proof instances were closed before the gate.

## Published screenshots

All 27 originals were opened and inspected. `verify-uploads.py.txt` read the actual PR comments, downloaded each URL without a token, checked PNG bytes and dimensions, and compared byte sizes and SHA-256 against the originals. Result:

```text
PASS 27 published images downloaded without authentication, opened, and matched byte-for-byte
```

| Filename | Bytes | SHA-256 | PR comment |
| --- | ---: | --- | --- |
| `getting-started-light.png` | 135084 | `cf3829d398bdebb7bd473579af816a5b887ea2456ccaeaf2d7cd932475f6f890` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6205308744) |
| `empty-state-dark.png` | 110059 | `7cad8ca690e270be8de387958205d5b30b8d362758d83ca834aef427abc08598` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6205308744) |
| `create-title-focused-dark.png` | 117236 | `204b0e9c80d65ca5515ee4da5986cd66753ed45a01c368e418e438f324e76d9d` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6205308744) |
| `title-undo-dark.png` | 111852 | `122f8cdf1968551087803dbff62c99f8eb1fc5e5349f30a846ef937859cd744b` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6205308744) |
| `sidebar-hidden-dark.png` | 93792 | `e4aca4786094080ade343d4aa9c30ac4ca17706d1ec5a548a48ca11983dd123d` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6205308744) |
| `palette-one-line-light.png` | 147522 | `37d56b9610ea6491caa41edef767c966c56b894bea66b5bab279a751ba39997e` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6205310266) |
| `palette-one-line-dark.png` | 139690 | `c5e1ab04d455808b711c40b42d9539b7d976e526353694de62d05ab7b1533fab` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6205310266) |
| `palette-wrapped-light.png` | 164059 | `14c4f6714830f0d49bcb0a1ae59053dda9360a12821f5aa1f59a7d1327e722c4` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6205310266) |
| `palette-wrapped-dark.png` | 155970 | `c4a1f6ac896c581251b0e3be218fbc2098a9eecf7808a532c86f72631d0b5793` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6205310266) |
| `caret-near-top-light.png` | 198379 | `783b6d288fdfd2865ca1d1e17b180199b1bfad143196eda34b2a47799fbd4d17` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6205310266) |
| `caret-near-top-dark.png` | 193043 | `4ac6a7142eb6078dd063e325b523eecfba935ee2653f0370038e3a5d07be4b89` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6205310266) |
| `selected-hover-light.png` | 126686 | `9493abed6efb5fabc279c9aa77d6a72ea5f443785beb37c0fe816f12030d90a0` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6205311400) |
| `selected-hover-dark.png` | 125926 | `00344b252bc64792288cdbc4d9a2d193731cb697cf123b216e839121bc15cbf8` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6205311400) |
| `unknown-title-light.png` | 125496 | `334cb9fdc1b0aa7c556fe27435f5b3c89a8b66bd35a57806829c1a05221c82bc` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6205311400) |
| `unknown-title-dark.png` | 119428 | `8ef1c734a96308921d8053ecbb6e205b965eab881ac080b98b7eb16395929905` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6205311400) |
| `recovered-title-dark.png` | 125926 | `00344b252bc64792288cdbc4d9a2d193731cb697cf123b216e839121bc15cbf8` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6205311400) |
| `empty-state-light.png` | 113059 | `d958bb9f4bd5e0948e4af09e64a0ec04704ec1767052aca624117c3b17191383` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6205339569) |
| `title-undo-light.png` | 115838 | `30d84014abc85d5561df899523b513809d902ee34cf837a85e324ae8787585db` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6205339569) |
| `sidebar-hidden-light.png` | 98208 | `d7d4437e22a2879805230923eada84daaeaa6dd1cc5badb902a2568a49f5da1a` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6205339569) |
| `composition-before-light.png` | 123723 | `805764fdf75ca5654e0ea6f770aaa01167f3564456865c35455586ba138ed9ee` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6205339569) |
| `parent-collapsed-light.png` | 112218 | `36c957971a7e3f4c708bf4c51d4e95d52fa18f9af7241e5b6c1321daff31dc9c` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6205454188) |
| `parent-expanded-dark.png` | 116774 | `9c23b77c2d17d232005fc7de71ae754b6354d7e0b9d9e40415d6bc7a4977f93a` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6205454188) |
| `parent-expanded-light.png` | 119346 | `5571e5453e7eea04cae1e5cbab8db39a32ca211d08945459405fb4824ba63b03` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6205454188) |
| `parent-relaunch-dark.png` | 108922 | `5b5ac92465c5fb50ab1529c4ef510ea38513a5b7ac0356a5816a4f74d508d930` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6205454188) |
| `parent-relaunch-light.png` | 112274 | `ccb49d355a128e5a8db905474bc3e8902ffba8a79487e93e36283d4cf40f5308` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6205454188) |
| `pending-title-dark.png` | 139294 | `b50c820e5084027124a11bd52e5c2ac1c68cfbc50a5fd1286ffa02695ace8b8e` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6205475625) |
| `pending-title-light.png` | 145150 | `467ca9937afea1182c4ade56db52a8c0ceee662674042b6215794003fa217d3e` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6205475625) |
