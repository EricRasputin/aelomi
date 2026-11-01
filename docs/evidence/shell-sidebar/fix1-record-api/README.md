# Sidebar fix round 1: final record API integration

Source commit: `b95e5c3389f27a104dd1745001c341245d70a502`. Base branch: `origin/feat/shell-page-records`.
Recorded merge base and #84 head: `12a203541ecc461ee541f005006a06672a0046ab`.
Verbatim patch ID: `aa15f7f7a8bf802de58c878739760455458be740`.

This is the current evidence for the final API integration. The earlier [fix1 record](../fix1/README.md) remains an immutable account of the pre-round-five adapter and earlier native runs. D1's observer addition to #84's `src/ipc/record-commands.ts` has been removed. That file and its tests match the recorded #84 base byte for byte. There is no pending migration to the announced API.

Pathspecs proved, relative to the repository root:

```text
src/components/sidebar.tsx
src/pages/page-view.tsx
src/pages/page-route.tsx
src/stores/page-titles.ts
src/stores/page-tree.ts
src/stores/tree-expansion.ts
src/stores/shell.ts
src/shell/use-page-commands.ts
src/palette/page-commands.ts
src/router.tsx
src/styles.css
crates/core/tests/rename_latency.rs
```

## Shape and behavior

Visible rows are a flat projection of the source tree; expanded IDs form a set. Page-ID drafts contain title, version and a typed rename result, with null representing the first reply still pending. A single reconciler observes source snapshots and #84's retained-operation store. An ok result needs the matching source title; a retrying result additionally needs its operation ID to leave the retained map. A newer version cannot be acknowledged by an older reply or matching old snapshot. A retrying title has no per-title error; a final error keeps its text and shows a failure.

A retrying create navigates to its returned page ID and stores its pending creation ID and operation ID. The route shows a creation status until the source includes the record. No editor loads or writes that page's body beforehand. A source refresh after retention ends distinguishes delayed projection from a final create failure. A retrying trash waits for source recovery without a final-failure message. Final create and trash errors show failures and preserve the source.

## Native proof

Environment: Apple M1, eight logical CPUs, arm64, macOS 27.0.1 (26A434), native Tauri 2 WKWebView. All launches used app.sh with synthetic data. All interactive work and transient screenshots held the keyboard lock; failure paths released it. No lock was removed from another owner or held for 20 minutes. Read-only real-data folder listings before and after the final native launches match exactly.

The primary new run is `~/.local/state/aelomi-verify/20261016-063123-dev-Yckk`. It proves retrying creation and recovery with native title typing, title-only Undo and body preservation, failed title refresh followed by focused typing and Undo, queued title input with no per-title error, background acknowledgement, final create and trash failures, retrying trash and recovery to Getting started. Both appearances were captured. Temporary probe controls inject transport I/O and terminal Internal results; Rust storage writes remain real. They are absent from the shipped source.

Its initial shortcut-based composition quit attempt is excluded: no quit log appeared, the readback ran while the folder was locked, and the launch terminal was then closed too early. The rerun is `~/.local/state/aelomi-verify/20261016-064428-dev-SUY8`, reopening only that synthetic folder. It arms browser composition in the real WKWebView, immediately presses the cached native Quit menu item belonging to the exact PID, and waits for both the normal Exit log and process exit before opening storage. The exact readback is:

```text
active_titles=["Getting started", "日本語の題名"] trashed_titles=["Pending native title"]
```

The reader additionally asserts exactly three records, one trashed record, and exactly one body block with only `Text("body token")` on that trashed record. Normal quit emitted:

```text
quit draining via=ExitRequested revision=1
quit flush via=CompleteQuit pending=None
quit confirmed via=drain
quit allowed via=ExitRequested confirmed=true
quit flush via=Exit pending=None
```

Native runs before this API integration remain recorded in fix1: `20261016-055516-dev-3oMC`, `20261016-060640-dev-MNKc`, `20261016-060926-dev-Q49D`, `20261016-061204-dev-o8tb`, and `20261016-061751-dev-iAB8`. Sidebar, CSS, PageView, expansion and palette-command source bytes are unchanged from their tested snapshot (`git diff 41c24c2 b95e5c3389f27a104dd1745001c341245d70a502 -- src/components/sidebar.tsx src/styles.css src/pages/page-view.tsx src/stores/tree-expansion.ts src/palette/page-commands.ts` is empty). Those native frames prove current selection versus hover, one-line and wrapped-title overlay behavior, upper-scrollport caret visibility, empty-state/create/toggle, and collapse persistence. Updated title/store/route behavior was exercised in the new runs. The new app started on ca29bae; this recorded commit differs only in test/evidence history, with no product source difference.

The diagnostic footer prints none while the latest rename's first reply is still pending; it does not establish absence of a draft. The exact draft and operation-ID assertions below establish retention. Only the U.S. input source is enabled on this Mac. Hardware Japanese conversion Return remains unverified; event-level composition and native quit with exact Japanese persistence are proved.

## Reproduction and automated cases

Run `bash docs/evidence/shell-sidebar/fix1-record-api/run.sh.txt` from a clean checkout, sequentially with browser tests. It launches a fresh synthetic app.sh run, takes the UI lock, drives retrying create, rename and trash through the named controls, then uses the PID-bound composition/Quit helper and exact disk reader. It restores index.html and appearance, removes the probe, and cleans up its owned processes. The reader can also run independently on a closed synthetic folder: `bash read-records.sh.txt <folder> api`.

`src/stores/page-titles.test.ts` cases:

- `TREE-7 a retained rename stays a draft without an error until retry acknowledgement`
- `TREE-7 an unavailable title cannot submit a rename against an empty baseline`
- `TREE-7 a matching old snapshot cannot acknowledge a newer queued draft`
- `TREE-7 a final rename refusal keeps the draft and shows a failure`
- `TREE-7 a matching source title stays a draft until the retained operation is acknowledged`

All five pass. The last case holds the actual storage-flush reply after successful retry and matching source projection, asserts the draft and retained operation remain, then releases it and asserts both maps empty. No observer API is added to #84's file.

All sixteen WebKit sidebar cases include the original thirteen mapped in the [round-one case table](../fix1/README.md#proving-cases), plus these in `tests/playwright/shell-sidebar.spec.ts`:

- `TREE-1 a retrying create keeps its returned route until the record arrives and then focuses the title`
- `CMD-6 final create and trash errors show failures while a retrying trash waits for source recovery`
- `TREE-1 a retained create that becomes terminal returns to the existing page with a final failure`

They check the exact returned route, absence of premature docLoad, automatic title focus after recovery, exact source titles and record counts, and terminal feedback. The acceptance case `CMD-2 CMD-3 CMD-6 New page is reachable by query and shortcut from the empty state` waits for a visible title before typing and tests both actions from empty. The first full-gate attempt exposed that missing wait; the corrected five-case acceptance file passed. The final clean-head gate reruns all suites, including dispatcher finality, tree flattening, trash fallback, settings admission during Draining, source recovery, body history and composition.

The remaining round-one SPEC-to-case map and all six Consider decisions are unchanged in fix1, except its old observer test has been replaced by the store/result cases above. The final gate is recorded by exact committed head at `.local-gate/<sha>.checks.json` and cited in the PR body. The node shim in `../fix1/check-node.sh.txt` takes the lock for the repository's synthetic cold-start harness; other Node commands execute unchanged.

## Counted release measurement

Command: `CARGO_BUILD_JOBS=4 cargo test -p aelomi-core --features test-support --release --test rename_latency -- --ignored --nocapture --test-threads=1`. An initial invocation without test-support ran zero tests and is excluded. The enabled run executed one test, five rounds and twenty measured renames per round on 10,000 source records. All 100 measured samples succeeded, zero failed, with an exact source-title readback and Store flush per sample.

```text
rename pages=10000 round=0 successes=20 failures=0 accepted_p50_ms=76.237 accepted_p95_ms=94.574 durable_p50_ms=80.529 durable_p95_ms=102.222
rename pages=10000 round=1 successes=20 failures=0 accepted_p50_ms=74.009 accepted_p95_ms=181.500 durable_p50_ms=82.265 durable_p95_ms=185.797
rename pages=10000 round=2 successes=20 failures=0 accepted_p50_ms=73.290 accepted_p95_ms=274.238 durable_p50_ms=74.016 durable_p95_ms=277.258
rename pages=10000 round=3 successes=20 failures=0 accepted_p50_ms=62.004 accepted_p95_ms=90.135 durable_p50_ms=68.413 durable_p95_ms=91.055
rename pages=10000 round=4 successes=20 failures=0 accepted_p50_ms=62.210 accepted_p95_ms=93.193 durable_p50_ms=63.431 durable_p95_ms=98.431
```

Load before the run was 4.87/8.67/10.03 on eight logical CPUs. Native synthetic and WebKit work overlapped portions of this run, so scheduling noise is present. These timings cover acceptance and durability only, excluding FIFO wait, IPC and source projection. They establish no comparative performance claim or typing-budget pass. At 10,000 pages the 100 ms deadline is missed by the code's structure for keystrokes closer together than one edit; several measured rounds also exceed it. Existing O(pages) acceptance work remains the structural limiter, without a component-level profile. Per-input rename and FIFO coalescing remain. [#91](https://github.com/aelomi/aelomi/issues/91) tracks incremental edits and projection. Deep/narrow tree polish remains deferred to [#87](https://github.com/aelomi/aelomi/issues/87); the earlier worst-case stress captures and production-picker exclusion remain recorded in fix1.

## Published screenshots

All forty originals were opened and inspected. Every attachment was read back from its actual PR comment, downloaded without a token, opened as a PNG, and checked byte-for-byte against its original. True/false filename suffixes mean dark/light appearances. Result:

```text
PASS 40 published images downloaded without authentication, opened, and matched byte-for-byte
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
| `api-create-retrying-dark.png` | 153716 | `6bf0e8f99ef93ce1bc25322347f8399a774c3c324bfca01dabd8f6c00b59c934` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6205792356) |
| `api-create-retrying-light.png` | 157784 | `4bed3ab3e3ab42fda51f178f28c651dc2de2ba9888eb65c88103b85c5a5a81fe` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6205792356) |
| `api-title-retrying-true.png` | 162148 | `879a84aded84ef493ec8fbf678cfc3af74c2998c5bb167b1ba1c6fd537d1c40e` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6205792356) |
| `api-title-retrying-false.png` | 168119 | `afd2973cc57a8d4b31fa7d74ce1de19d5b684a3bcbd3f5a5922d92f4fa2e99ff` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6205792356) |
| `api-unknown-title-true.png` | 129045 | `06d625ee0ec9a2d61f891fd7b3f0e6f0616562d6ee1725866b75f8f6235c70da` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6205792356) |
| `api-unknown-title-false.png` | 134853 | `9da90808c2a664cb42d89e800992596e89670e3643203c3a59eb615fa7e53ba3` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6205792356) |
| `api-create-final-dark.png` | 145236 | `62b9f3d3d386acf1ea3a1922a720a19f29af42970b17659f261ca6866a5183f0` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6205793563) |
| `api-palette-debug.png` | 172160 | `3785743bc2622ef8e5dd8d82224e1f96897fbedeaa9a3ea3f93aba93f21e1db8` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6205793563) |
| `api-trash-final-light.png` | 151201 | `40987f9caf1f7131d513c7627387c14f1a11db9b34436fdc262ef9e0eea6231a` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6205793563) |
| `api-trash-retrying-true.png` | 160653 | `e4d36d4bcf403c212c988eba548ea2c4c00c416c4eaa067b59841a4535a6baad` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6205793563) |
| `api-trash-retrying-false.png` | 166155 | `f63f51ff6b0e5b5a67e9cd4c9783e53ee1f69325de20486cb84a07cd2ba45168` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6205793563) |
| `api-menu-composition-before-false.png` | 149741 | `3264d8f8c0afa81e72c8a3090bd4a7283fc56ce9a45979c5aee379d5230230a4` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6205835394) |
| `api-menu-composition-before-true.png` | 143922 | `161bc290d063b70a85127b32bcd9334b4d3ac46ee361d2bf4dc4c8604bd0571f` | [image](https://github.com/aelomi/aelomi/pull/89#issuecomment-6205835394) |
