# DATA-5 review follow-ups: native and timing record

This record proves the fixes for issues #70 to #78 on source commit `d37c4ec4e17139cbbdfe8929d1fe3f0fcb319878`. Later commits on the branch only publish this record unless a later record says otherwise.

## Environment

- Apple M1 MacBook Pro, 8 cores, macOS 27.0.1 (26A434). Aelomi 0.1.0, the release `.app` that `pnpm tauri build --bundles app` built from the source commit. Each run used a new synthetic data folder written by `seed_index`.
- The Mac ran on battery from 09:18 IST, before every run here. The repository's own numbers from earlier records were taken on mains power, so compare absolute timings only within this record. Each comparison below alternates the two builds under the same conditions.
- Other apps kept 2 to 3 cores busy. Each run's 1-minute load average is in its log.
- Native proofs used [verify-aelomi](../../../.claude/skills/verify-aelomi/SKILL.md) `app.sh launch app`, `doctor`, `focus`, `tree` and `screenshot`. Keyboard input went to the owned app PID through [keys-pid.js.txt](keys-pid.js.txt). Edit-menu probes used [menu-pid.js.txt](menu-pid.js.txt).
- Personal path prefixes are redacted to `~` and the worktree path to `<worktree>`. Terminal color escapes are removed. The data folders, `run.env` files and PID files stay local.

## Results by issue

**#77, an unreadable pages root.** The `foreign-pages-root` seed writes the valid fixture and then sequence content under `pages`. On launch, the shell logged `index cache=reused` and then `index status=Failed { reason: "workspace: workspace doc: the pages root is not a Y.Map, found a sequence" }`. It logged no erasure replacement. The index screen showed the new alert: "Aelomi can't read this workspace's page list, so no page can open. Nothing on disk was changed." Its second line gave the reason ([d77-index-alert.ax.txt](d77-index-alert.ax.txt)). The palette showed the Failed reason ([d77-palette-failed.ax.txt](d77-palette-failed.ax.txt)). After ⌘Q (`quit flush via=Exit pending=[]`), every source file hashed the same as before launch ([before](d77-source-before.sha256), [after](d77-source-after.sha256)).

**#74, a body root that is not XML.** The `not-a-fragment` seed adds "Plain text body", a page whose `prosemirror` root is a Y.Text containing `café 😀`. With the index Ready, typing `kestrel` in ⌘K showed "1 page is only partly searchable: Plain text body." ([d74-palette-issue.ax.txt](d74-palette-issue.ax.txt)). Opening the page showed the read-only notice ([d74-readonly-page.ax.txt](d74-readonly-page.ax.txt)). Reading the multi-byte text did not hang the index worker. Source hashes were unchanged ([before](d74-source-before.sha256), [after](d74-source-after.sha256)).

**#78, faults that recur after Ready.** With ⌘K open on `cafe`, [d78-tamper.log](d78-tamper.log) corrupted the cached title of `reading` eight times with `sqlite3`, each time after the index logged Ready, and a query hit the corrupt row. The first repair ran at once. Each later fault logged `index status=Failed { reason: "the index was repaired repeatedly: corrupt (field)" }` and repaired after 108, 211, 408, 811, 1,611, 3,210 and 6,411 ms ([d78-shell.log](d78-shell.log)). Those intervals match the 100 ms doubling schedule. [d78-failed-waiting.ax.txt](d78-failed-waiting.ax.txt) shows the alert during the last wait, and [d78-recovered.ax.txt](d78-recovered.ax.txt) shows results after tampering stopped. Source hashes were unchanged. macOS turned two typed spaces in the query into `cafe.`; the search words stayed `cafe`.

**#71, page links, the palette and the quit handshake.** On the `valid` seed's Daily page, Shift+⌘→ selected the first line, and ⌘C ran five times. The clipboard held 1,484 bytes of HTML with `data-target-page-id="aelomi"` and the label `Aelomi` ([d71-clipboard.html.txt](d71-clipboard.html.txt)). → then Return then ⌘V pasted a second "see Aelomi today" line with a working link. That paste made two `doc_append` calls in [d71-shell.log](d71-shell.log), traced with `AELOMI_TRACE_IPC=1`. Next, ⌘K chose Daily, the page already open, and `zq` landed at the caret ([d71-palette-open-page.ax.txt](d71-palette-open-page.ax.txt)). After ⌘Q and a relaunch, the page still showed the pasted link and `zq` ([d71-reopened.ax.txt](d71-reopened.ax.txt)). Each launch logged `quit frontend ready`. The browser suite proves the subscription count: [page-links.spec.ts](../../../tests/playwright/page-links.spec.ts) asserts one label subscription per shown link through copies and drag starts. On the previous code, the same spec counted 48 subscriptions for 3 shown links.

**#70, compacting an attributed page.** `seed_index search-10k --big-page` printed no compaction failure, and `docs/big-page/` held only `snapshot-0000000001.ysnap2` ([d70-seed.log](d70-seed.log)). A launch on that folder built the index and logged no compaction error ([d70-shell.log](d70-shell.log)).

**#73, populated startup.** The unchanged harness ran with `--data-dir <10,001-page seed> --expected-variant page` and timed ten runs after a discarded warm-up. The median end-to-end time to the first editable page frame was **527.5 ms** (range 507 to 548 ms, load 3.14 at start, 4.05 at end). The **500 ms local gate failed** ([cold-10k.log](cold-10k.log)). The empty-workspace run measured 458 ms and passed ([cold-empty.log](cold-empty.log)). Alternating series against main `a217b48` on the same folder measured 604.5 and 609.5 ms for main and 527.5 and 531 ms for this commit. Navigation-to-paint fell from 251 to about 161 ms ([startup-ab-d37c4ec.log](startup-ab-d37c4ec.log), driven by [run-ab.sh.txt](run-ab.sh.txt)). Pre-squash diagnostics record how the change got there:

- Warming the tree alone gave no gain ([startup-ab-warm-only.log](startup-ab-warm-only.log)), because the frontend's call repeated the in-flight projection.
- Single-flight plus the position-based projection gave the gain above ([startup-ab-single-flight.log](startup-ab-single-flight.log)).
- An editor-chunk prefetch was rejected after it improved navigation-to-paint by only 4 ms ([startup-ab-prefetch-rejected.log](startup-ab-prefetch-rejected.log)).

**#72, timing coverage.** The keystroke spec now reports WebKit Event Timing next to the app-owned split. On the mock-only layout-delay control, the app-owned p95 was 8 ms while the full key-to-paint p95 was 31 ms. Event Timing reported 285 of 405 keys at 16 ms or more ([keystroke-d37c4ec.log](keystroke-d37c4ec.log)). So the engine clock sees layout that runs before the rAF stamp, which the app-owned split leaves out.

The 5,000-block editor measured 25 or 26 ms full p95 on this commit and 25 ms on main, in alternating runs at load 1.7 to 3.3 ([keystroke-ab-d37c4ec.log](keystroke-ab-d37c4ec.log)). That is not a regression from this commit. Main also exceeded the 19 ms local ceiling here, on battery, against 18 to 19 ms in the earlier mains-powered record. The ceiling is unchanged.

All ten scroll runs passed (work p95 9 to 14 ms, longest gap 18 to 23 ms; [scroll-repeats-d37c4ec.log](scroll-repeats-d37c4ec.log)). Two kinds of long gap appeared:

- **Content frames.** Most long gaps were the previous frame's own rendering work at fixed scroll positions, where code blocks, tall headings, checklists or images enter the view.
- **Load spikes.** One pre-squash run had a 102 ms gap between frames while the machine's load spiked to 19.7 ([pre-squash-scroll-ab.log](pre-squash-scroll-ab.log)).

Main and this commit matched in those alternating runs.

## Known limits

- **Startup.** The 500 ms startup goal still fails on a populated workspace. The record keeps that failure.
- **Native presentation.** Two `xctrace` Animation Hitches recordings were attempted on the native app. The first export was incomplete. In the second, the typed keys never reached the app because it lost the front, and xctrace reported a malformed transfer ([d72-xctrace-record.log](d72-xctrace-record.log)). Each recording also left a 6 to 7 GB kernel trace in the temporary folder, which filled this Mac's disk. Native compositor presentation therefore remains unmeasured. Event Timing covers event to paint inside WebKit. A later recording on the same product code measured it; see the [568168d record](../data-5-review-followups-568168d/README.md).
- **⌘A.** ⌘A and Edit > Select All did not select editor text under automation, on main and on this commit alike ([base71-shell.log](base71-shell.log) is the main run). Shift+Arrow selection, ⌘C and ⌘V worked. The gap predates these issues.

## Screenshots

The PNGs are attached to the pull request. Each one is the 960 by 640 window at 2x.

| File | Bytes | SHA-256 | Shows |
|---|---|---|---|
| d77-index-alert.png | 112275 | 5998f7938711cc2433fe9f221510ed33464b31a71c58cb961eb05c98a95ee941 | The index screen alert with the workspace reason |
| d77-palette-failed.png | 165806 | ef7b85bb1afbedd902240b20c80ec4a7a12edfad361d659d77676ed9351428bc | The palette's Failed alert over the index screen |
| d74-page-open.png | 82417 | 230b1704f4e206ae56a99e080f518fe58221262b5b3b6b326a9b18642f2ffaaa | Projects open on the `not-a-fragment` seed |
| d74-palette-issue.png | 113000 | 7078e3630f2fe639c2ddbc8fb701cab71518eef742f428f6e85d6fd5ed69c07a | "1 page is only partly searchable: Plain text body." |
| d74-readonly-page.png | 90988 | 65dc951d75a4c41482a8dd6f2ddb83de3f9f9a55a20651eabb9eea01d20f4735 | Plain text body open read-only |
| d78-before.png | 101823 | 9cce0b7bd6425092a7b1c79a96aece8adceb43b970a8fe839d9ad6b4839d573a | `cafe` results before tampering |
| d78-failed-waiting.png | 112688 | 335bb781e16c680a293fc9064ee8b8a89ae902ac6a5e14eb4e35960b78856d9c | "the index was repaired repeatedly: corrupt (field)" during a wait |
| d78-recovered.png | 102037 | e3cda899c26939131f49f9a74b7664c877156ebba4a2405ff2a5255eea46d50b | Results again after tampering stopped |
| d71-before.png | 122312 | 5b22a9464afda836526c220fd83f5ce95429cd06fab7ef52a52d3bff58583f49 | Daily before the copy |
| d71-pasted.png | 129953 | e9bf6835e18dbf406e93d12c9ca0f533e8a37159cee007cd439e8f9dc6de7d33 | The pasted second line with its Aelomi link |
| d71-palette-open-page.png | 136065 | a02e3d276216880718c298c07bfea7307b6e6f0e614a270a1a58d81813f35598 | `zq` typed after choosing the open page, with macOS's correction bubble |
| d71-reopened.png | 130798 | a24e913dc79b377d5ca9ee1d8af82921bf8843061d0135e13b8049fc3474f76c | The pasted link and `zq` after a relaunch |
| d72-big-page.png | 254356 | 2f741dcfd255fd4a41da9f626fbff0d87916869bcdbb3e22fe5e2f975b3e67ff | Kestrel field notes, the 5,000-block page, before the xctrace attempt |

[files.sha256.json](files.sha256.json) records the bytes and SHA-256 of every text file here except this README and the manifest. [decisions.tsv](decisions.tsv) is the run's decision trail; commits made before the squash are named by issue, because their SHAs never reached GitHub.
