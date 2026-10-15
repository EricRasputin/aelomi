# Sidebar, empty state, and title evidence

This record proves source commit `a2e07e4e9e3964dc4025674b7620372f066bbfe5` on base `53b8763c1ae1b4a46d193113fe67767bbde72578` from `origin/feat/shell-page-records`. The patch-id is `515be66502854deea935e76a9399d3d7832bf931`.

The proven pathspecs are `src/components/sidebar.tsx`, `src/components/command-palette.tsx`, `src/pages/page-view.tsx`, `src/router.tsx`, `src/shell/`, `src/stores/page-titles.ts`, `src/stores/tree-expansion.ts`, `src/editor/field-history.ts`, `src/command-dispatcher.ts`, `src/shortcut-events.ts`, `src/stores/quit.ts`, and `crates/core/tests/rename_latency.rs`.

## Environment and runs

The machine was an Apple M1 with eight logical CPUs, arm64, macOS 27.0.1, build 26A434. All app launches used verify-aelomi's `app.sh launch app` and synthetic AELOMI_DATA_DIR folders. Every native key, click, focus, and transient screenshot held `/tmp/aelomi-ui.lock`. The lock was released between scenarios. The original dark system appearance was restored.

Native runs stay local under `~/.local/state/aelomi-verify/`.

- `20261016-040650-app-uPh2` holds the fresh-folder, trash, empty-state, create, title, body, toggle, and light/dark proof.
- `d1-nested-20261016-0410/data` holds the synthetic nested fixture from `seed_index ... missing`.
- `20261016-041009-app-zBQP` holds the expanded-parent screenshot and the collapse before quit.
- `20261016-041108-app-VS3a` holds the collapsed-parent relaunch proof.

Doctor passed on each exercised instance. Two earlier launch attempts ended when their terminal session closed. They were not driven and their scratch folders were removed. The successful launches kept their terminal session open through cleanup.

Read-only `ls -la ~/Library/Application\ Support/Aelomi` snapshots before launch and after the last native scenario were identical. No app instance used that folder.

## Native results

1. A fresh folder showed Getting started in the sidebar, its editable title, and the three instruction paragraphs.
2. The palette's Move to trash command on that only page showed No page open and New page. The sidebar had no page rows.
3. With the sidebar hidden, pressing the empty state's New page and typing D1 notes, without a focus action, changed Page title. Showing the sidebar revealed the same title.
4. Return in the title and typing body token changed Page body. Appending x in the title and pressing ⌘Z restored D1 notes while preserving body token.
5. ⌘\ hid and showed the sidebar. The hidden capture shows the editor occupying the released width.
6. Light and dark captures use the existing theme tokens. No save-status text appears.
7. Projects was expanded and then collapsed. A normal quit and relaunch on the same synthetic folder retained the collapsed state. Its row showed Expand Projects; child page buttons were absent. The persisted settings contained `"expandedRows": []`.

The native script's exact assertions reported these lines.

```text
PASS new page title focus and matching sidebar title
PASS title Return and title-only Undo preserve body token
PASS parent remains collapsed after relaunch
```

After the first normal quit, a separate core reader opened that synthetic folder and asserted exactly two records, one active page titled D1 notes, one trashed page titled Getting started, and one body block with exactly `Text("body token")`. It reported this line.

```text
records=2 active=1 trashed=1 active_title=D1 notes trashed_title=Getting started body=Block { id: BlockId("initialBlockId"), segments: [Text("body token")] }
```

Each exercised normal quit completed with these shell lines.

```text
quit draining via=ExitRequested revision=1
quit flush via=CompleteQuit pending=None
quit confirmed via=drain
quit allowed via=ExitRequested confirmed=true
quit flush via=Exit pending=None
```

## Rename latency and remaining limit

`cargo test -p aelomi-core --features test-support --release --test rename_latency -- --ignored --nocapture` measured the production `Workspace::edit_page` Rename path. Each round discarded three warmups and consumed twenty successful edits, flushed each edit, and read back the exact title. The fixture holds 10,000 records, including 9,700 active pages. Index rebuild completed before timing. The acceptance clock includes record-lock acquisition, loading, decoding, planning, encoding, and append acceptance. The durable clock also includes Store flush. It excludes IPC, frontend rendering, and input scheduling.

The five rounds reported these acceptance and durable p95 values in milliseconds.

| Round | Acceptance p95 | Durable p95 |
|---|---:|---:|
| 0 | 68.957 | 73.030 |
| 1 | 65.951 | 69.706 |
| 2 | 75.869 | 78.692 |
| 3 | 73.736 | 76.488 |
| 4 | 143.692 | 158.880 |

All 100 measured renames succeeded and matched the literal readback title. The median of the five durable p95 values was 76.488 ms, with a 69.706 to 158.880 ms range. Other agents compiled on this shared machine; load was 10.93 on eight CPUs before the run and rose during later work. This is a loaded-machine observation, not proof of a stable 100 ms deadline. The whole-record decode and plan are visible costs in A's existing implementation; no profiler separated their shares, so their individual costs remain unmeasured.

The UI retains per-input renames and uses A's existing FIFO boundary unchanged. It permits one running record command and coalesces consecutive queued renames for the same page. The immediate IPC mock check passed under 100 ms. Improving native rename cost remains an upstream question for PR A rather than a change to its implementation here.

## Sidebar stress and scrolling

The mock-only `/dev/sidebar-stress` picker exposes Demo, Worst case, Empty, One, and 10,000 rows through the mock data boundary. The visual pass covered long and unbreakable titles, empty and whitespace titles, one-letter names, Vietnamese, CJK, Arabic, emoji, literal markup, line breaks, forty nesting levels, light and dark, 320 px, 960 px, 2560 px, and 200% zoom.

The pass exposed a clipped editor title and newline text crossing a fixed-height row. The title now wraps in a textarea with bounded header height, and rows use one-line truncation with a full-value title. Deep indentation has horizontal scrolling. Whitespace-only titles remain literal whitespace; this PR does not normalize record text.

The five-second WebKit sidebar check used 10,000 active records and rendered 36 rows. Its first run measured 3 ms rendering p95 and a 60 ms maximum raw interval at load 28.24, failing the 50 ms interval budget. The repeat passed with these consumed results.

```text
sidebar source_pages=10000 rendered_rows=36 frames=300 moved=300 rendering_p95_ms=4 raw_max_ms=29 idle_cadence_ms=17 load=21.3359375,64.4873046875,46.06005859375
```

The frame clock uses the existing scroll recorder. Rendering work is a lower bound from the rAF timestamp to its MessageChannel task. The raw maximum includes scheduling between frames; neither clock measures compositor presentation.

## Reproduce

Build the app after a disk-space check. Keep the launch terminal session open. Use a fresh synthetic folder and acquire the keyboard lock before interaction. The verifier's feature recipe is `.claude/skills/verify-aelomi/features/shell-sidebar.md`.

Copy `keyboard-lock.py.txt` and `key.js.txt` to `/tmp/aelomi-d1-lock.py` and `/tmp/aelomi-d1-key.js`. Launch through app.sh, write its run directory into `/tmp/aelomi-d1-native-run`, and acquire the lock. The fresh-folder getting-started and palette-trash steps precede `run.sh.txt`, which drives creation from the empty state through quit. Its failure path releases the lock. Restore the original system appearance on any interrupted run.

For the disk assertions, copy `inspect.rs.txt` into a temporary `crates/core/examples/d1_inspect.rs`, run it only after the synthetic instance quits, then remove the temporary example. The ignored rename test is committed separately and reruns the measurement with exact readbacks.

## Published screenshots

[The shell captures](https://github.com/aelomi/aelomi/pull/89#issuecomment-6203752167) and [the disclosure captures](https://github.com/aelomi/aelomi/pull/89#issuecomment-6203783808) are attached to PR comments. Every attachment was downloaded without a token through `curl --disable --fail --location` and compared byte-for-byte with the inspected local PNG. GitHub CLI 2.102.0 was downloaded from its official release and its archive SHA-256 matched the published checksum file.

| Filename | Bytes | SHA-256 |
|---|---:|---|
| getting-started.png | 107835 | 309b99a48cbc4b690efe495bfa35d05126f7835aa7a5fd0dfc42967ae1bc9d82 |
| empty-state.png | 94723 | f89498107de74ab03a3535e5c39417381dccc87a77daca31fcfc911d278cf179 |
| sidebar-dark.png | 93596 | 524678f6d3d89de167e575719b59b8fe021a69a496221accf836907c6e92b64f |
| sidebar-hidden.png | 77408 | 8b9c4fbd61b41cb92217580ed81417ccbb58cdf0542378561ddbc56045a2f270 |
| sidebar-light.png | 95797 | 0933641141f75f3fdc672c7ca9b1e16c6bfce1d9b6dbabcb5301d1495c9d5134 |
| parent-expanded.png | 103753 | d615996a23d735844f670fdb74b71d4973c5f6ef714b6ea5c25be8d3ddab25cc |
| parent-collapsed-relaunch.png | 96985 | a6c5445a3c78938a384dc5864da6f6e0bc161cccd49fddf7681e86e22d8ffa43 |
