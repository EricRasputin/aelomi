# Sidebar fix round 5 proof

Source commit `ad0adee1e233e684827cabaeb9e1f7ddfa885ea7`. Merge base `afe406739b47d59b86ab50b4cd5fcf6f7f8ff331`. Patch ID `d525888b4d14ceaad775229a3b8376f8ab18583d`.

The native create and trash runs used the bundle built at `53a1c3495ad2c1525377076c4499a42075159732`. Rebasing onto main's documentation-only #95 preserved the patch ID and every runtime and build input. A fresh bundle at the rebased source proved relaunch. This record is a separate commit after the source it proves. The PR's Verification section names the final head's local-gate record and hosted CI state.

Proven pathspecs are `src/shell/**`, `src/stores/shell.ts`, `src/pages/page-frame.tsx`, `src/router.tsx`, `src/components/sidebar.tsx`, `src/components/command-palette.tsx`, `src/ipc/record-commands.ts`, `src/ipc/record-commands.test.ts`, `src/styles.css`, `tests/playwright/sidebar-round4.spec.ts`, and `tests/playwright/sidebar-round5.spec.ts`.

## Domain and rule ownership

`NewPage` is an insertion-ordered map keyed by activation ID. Creating and failed entries preserve acceptance; created entries imply success. `activateNewPage` and `forgetEmptyFailuresExcept` in `src/shell/new-pages.ts` use the displayed title and acceptance, so an untitled created page is never reused and an accepted failure is never forgotten.

`bodyReady` in `src/shell/page-presence.ts` admits only listed records and successful creates. `requestFocus` in `src/stores/shell.ts` redirects gated body requests to the title immediately. `useOpenPage` is the single user-navigation gate. The palette's precheck only controls whether it closes. Trash fallback waits for the palette to close.

Model the Domain put acceptance in each entry. Test Behavior, Not Implementation added exact call, text, geometry, route and persistence assertions. Sequence Work into Verifiable Units required red regressions before fixes. Explain the Number required alternating native runs and an untimed trace. Prove It Works required native actions and exact Rust readback. The builder brief prohibited subagents, so the no-comments pass was manual. It found no new comments or suppression directives to remove.

## Native proof

Apple M1, eight cores, macOS 27.0.1, Node v24.13.1. Native captures show the Mac's current light appearance. No system setting changed and no system prompt was answered. Dark captures below are the mock browser with `emulateMedia`.

All launches used `app.sh` with synthetic `AELOMI_DATA_DIR`. The launch and its actions stayed in one terminal session because this command host terminates descendants after the parent session ends. The supplied lock helper acquired and released the same `/tmp/aelomi-ui.lock` protocol. Scripts accept that helper through `AELOMI_UI_LOCK`; no private coordinator path is stored here.

- Create and held-title run `~/.local/state/aelomi-verify/20261016-164353-app-wHZm`. Focus Page body, send Command N and text in one JXA call, then hold creation with `chmod 000` on the synthetic docs folder. Focus a sidebar row, press Command Backslash, return to the title, type, and recover permissions. The validated create prefix ends at `recovered-current.png`. Its later exploratory trash attempt is superseded by the separate queued-trash run.
- Queued-trash run `~/.local/state/aelomi-verify/20261016-164758-app-2hx2`. Create a fallback page, hold another create, reopen Getting started, and queue its trash behind that retained create. Reopen the palette before restoring permissions. The held and recovered screenshots retain Getting started in the editor; the recovered screenshot has no Getting started row and no Move to trash command. Escape opens Trash fallback. This run exited through normal Quit.
- Relaunch run `~/.local/state/aelomi-verify/20261016-165351-app-cai1` reopened the first run's synthetic folder. Instant round 5 opened immediately. Reopening Held round 5 exact recovered retained its full title. Round5 quit title was present after an immediate create, type and Quit in the intermediate `20261016-164635-app-QgMy` run on that same synthetic folder.

Exact observed AX values were:

```text
instant: {"title":"Instant round 5","titleFocused":true,"body":"\n","fields":[]}
held: {"title":"Held round 5 exact","titleFocused":true,"body":null,"fields":[]}
recovered: {"title":"Held round 5 exact recovered","titleFocused":true,"body":"\n","fields":[]}
fallback: {"title":"Trash fallback","titleFocused":false,"body":"\n","fields":[]}
```

After the app exited, `read-records.sh.txt` opened each synthetic folder through the real Store and asserted the exact records and body segments:

```text
PASS records=4 active=["Held round 5 exact recovered", "Instant round 5", "Round5 quit title"] trashed=["Getting started"] original_body_unchanged=true new_bodies_empty=true
PASS records=3 active=["Trash fallback", "Trash queue hold"] trashed=["Getting started"] original_body_unchanged=true new_bodies_empty=true
quit flush via=CompleteQuit pending=None
quit confirmed via=drain
quit flush via=Exit pending=None
```

The real data folder was only listed. Before-first-launch and after-last-launch listings had the same entries, sizes and modification times. Synthetic folder permission modes were restored on every exit path, and all owned apps were cleaned up.

## Cold start

Every number below comes from the same `pnpm perf:cold --ci --expected-variant page --app <bundle> --out <folder>` command on this Mac, with release bundles, one discarded warm-up and ten timed launches. The supplied-folder relaunch rows additionally pass `--data-dir`. The harness checks page paint markers and successful process exits. There were no harness failures or wrong variants. It measures the first editable content frame using rAF and MessageChannel, not compositor presentation or a cold filesystem cache.

Final same-command comparison:

| Revision | End-to-end median and range, ms | Navigation-to-paint median and range, ms |
| --- | --- | --- |
| Main runtime `7a285d8`, identical at docs-only `afe4067` | 505.5, 491–529 | 162.5, 151–174 |
| Round 3 `40c6eb2` | 531.5, 510–610 | 168, 164–192 |
| Final runtime `53a1c34`, identical after rebase at `ad0adee` | 534, 500–609 | 167.5, 164–235 |
| Main on the populated relaunch folder | 519, 492–596 | 156.5, 151–184 |
| Final on that same relaunch folder | 534, 522–1804 | 176, 160–242 |

All final comparisons passed the hosted page ceilings of 2,783 ms end-to-end and 712 ms navigation-to-paint. The final 534 ms end-to-end median still misses the 500 ms local ceiling. `perf/budgets.json` is unchanged. The review's 740.5 ms hosted failure was not reproduced as an absolute value on this M1; the local `pnpm perf:cold` reproduction failed its 500 ms ceiling at 1,017.5 ms.

Five alternating main/round-3/candidate rounds gave 50 timed launches per build. Their pooled navigation medians were 180, 192.5 and 176.5 ms. Five further alternating round-4/candidate pairs gave 50 launches per side. Round 4 was 486.5 ms, range 401–532; the candidate was 228 ms, range 131–292. Their pre-navigation medians were both 432.5 ms, while module-evaluation-to-content-commit fell from 378 to 119 ms. The cold-start logic was unchanged by the later palette-label fix; the final table above retests that final source separately.

The untimed WebKit trace found the limiter. Round 4 mounted the title and fallback at 286 ms, finished the body chunk by 307 ms, then mounted the editor at 638 ms while animation frames continued. The fixed route loaded the chunk by 130 ms and mounted its editor at 246 ms, 44 ms after its frame. The installed React 19.3.0 production source applies a 300 ms retry-lane fallback throttle in `react-dom-client.production.js`. The router's resolved lazy component removes that retry; the empty index path still does not import the editor.

The remaining final startup cost is mostly before navigation, median 362 ms, followed by 79 ms to module evaluation and 85 ms to content commit. Rust's tree warm-up was below 1 ms in the native logs. This bounds what a body-load fix can remove. Main and final ranges overlap, so this record claims recovery of the round-4 suspension cost, not a measurable speedup over main.

The Mac was shared and frequently flagged noisy. An exploratory TypeScript suite overlapped the early paired rounds and was stopped after an unchanged editor oracle timed out; that oracle passed alone afterward. No suite ran during the final five-row comparison. Treat absolute local timings and small differences as noise. The large round-4 gap is supported by non-overlapping ranges, equal pre-navigation medians, the trace, and the installed scheduler code.

Raw reports, traces and harness logs remain local at `~/.local/state/aelomi-verify/pr89-fix5-performance`. Build logs are local only.

## Regression and layout proof

All six original browser regressions failed on round 4. The preload regression failed with a title already mounted while its chunk was held. The palette-label regression failed with its suffix outside the result's right edge. The two sidebar/palette drain tests both failed when `useOpenPage`'s sole gate was temporarily removed. The gate was restored afterward. The targeted rounds 4 and 5 passed 29 cases before the extra palette-label case, and the final round-5 file passed all eight cases.

New cases are:

| File | Exact case name |
| --- | --- |
| `tests/playwright/sidebar-round5.spec.ts` | `SHELL-1 cold page navigation preloads the body before the first frame mounts` |
| `tests/playwright/sidebar-round5.spec.ts` | `SHELL-6 hiding the sidebar during a held create cannot defer body focus` |
| `tests/playwright/sidebar-round5.spec.ts` | `TREE-1 a forgotten failed ID under stale reads never loads an orphan body` |
| `tests/playwright/sidebar-round5.spec.ts` | `TREE-1 an untitled created page with body text is never reused under stale reads` |
| `tests/playwright/sidebar-round5.spec.ts` | `TREE-1 leaving an accepted untitled failed create keeps its body gate` |
| `tests/playwright/sidebar-round5.spec.ts` | `TREE-9 open-page trash fallback waits for the palette to close` |
| `tests/playwright/sidebar-round5.spec.ts` | `SHELL-1 status messages never move sidebar rows and failed rows stay distinct` |
| `tests/playwright/sidebar-round5.spec.ts` | `TREE-7 a long failed palette title leaves its failure label fully visible` |
| `src/shell/new-pages.test.ts` | `TREE-1 only an uncreated empty activation can be reused` |
| `src/shell/new-pages.test.ts` | `TREE-1 acceptance survives a later retry failure and prevents forgetting under stale reads` |
| `src/ipc/record-commands.test.ts` | `TREE-1 a terminal durability failure reports that the create was accepted` |

The pinned `sidebar-round4.spec.ts` cases are `SHELL-10 held create asks immediately and retains Q during recovery in Asking`, `SHELL-9 each sidebar and direct-route activation stays on the frozen page during Draining`, and `SHELL-9 palette activation stays blocked during Draining independently of the sidebar`. The first now asserts the held page's URL after navigation attempts; the latter two pin the single gate. `page-presence.test.ts` expands `TREE-1 only a successful create unlocks its body` to both missing states. Existing SHELL-4 empty-state and TREE-6 expansion acceptance cases remain in the [D1 matrix](../fix4/README.md).

The mock stress run `~/.local/state/aelomi-verify/pr89-fix5-stress` exercised Demo, Worst case, Empty, One and 10,000 rows, deep nesting, unbreakable and multilingual titles, 320/960/2560 px and 200% zoom. The separate final failed-row run `~/.local/state/aelomi-verify/pr89-fix5-failed-row-final` proved that the failure suffix stays inside the row and palette result in both emulated themes. Every selected screenshot was visually inspected. The dev toggle remains excluded from production.

## Screenshot manifest

The following inspected images are the PR comment attachments. Native filenames contain `current`; their appearance is the Mac's unchanged light appearance. Files prefixed `failed-row` are mock captures with emulated appearance. Upload verification downloads each public URL without a token and compares its byte size and SHA-256 with this manifest.

| Filename | Bytes | SHA-256 |
| --- | ---: | --- |
| `create-before-current.png` | 118762 | `0066ee0ac8c9b3e7203616d689e8b1b673157fd5bb3d6d60e44c1be86be5b40e` |
| `create-held-current.png` | 104552 | `c882e1880b0a3989e0d96070967eff82479700cec5f38a04e50dff826fcac518` |
| `create-instant-current.png` | 111462 | `7fc1786395e61ea59918eb3ddf049d22fad3c263fa8941dfb1e7a5a2d7f37dfa` |
| `create-recovered-current.png` | 93030 | `d6a1864ee95c957f93bc8ad8a90aca428e07df99ea86e7b4bedd20c7bb59c939` |
| `failed-row-dark-960.png` | 60707 | `9e46386d2a7c4a4d634062963f2573ccb14693acdb03d8adde046517b49dee4f` |
| `failed-row-light-320.png` | 54153 | `76c660d0322124c7c3546e187d0b7de19057fa318ee17823b98318950249b29c` |
| `failed-row-palette-dark.png` | 78890 | `67962e5b316804155eb8bbea7229c6125431e02f5da85127f13fa20f0592b0d8` |
| `fallback-current.png` | 109519 | `94ddb67248b53e3b87c29bc2dfcf95f93a3bd7b803ed2b5339a23474c470e5b8` |
| `held-relaunch-current.png` | 121026 | `ce0c6aee014aca541cdc8051da60be43dfa6268eab352a6bccd2516716dc7e21` |
| `relaunch-current.png` | 117345 | `c172723d92688b949dc7736cf38c711397ec9161ab5df3efbf8cb38e3400eff8` |
| `trash-palette-held-current.png` | 184190 | `665ee51a625c8a851b340a1c64abc374277c912007e8a5636f7a2a7a92000d61` |
| `trash-palette-recovered-current.png` | 116145 | `11e05dc2bc676aba80188d601268ed6ae6e63ff395541b761a779233f40beecd` |

## Remaining scope

PR #94 was still open at the source's final integration check. The shared settings job, admission-open subscription, theme sanitization and Acceptance 1 sidebar assertion are therefore deferred as the brief requires. The round-4 Consider and Noted findings remain, including title-draft retirement against delayed snapshots, the gated Return experience, lifecycle subscription ownership, palette title caching and #87/#91 polish and storage deadlines. Native VoiceOver and hardware IME remain unverified; neither was enabled. No review gate or independent verifier was run by this builder.
