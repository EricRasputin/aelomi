# Sidebar fix round 4 proof

Runtime source commit `7881314eaab29c56e821ee020810ab47887d3f80`. Base branch `origin/main`, head and merge base `7a285d85bca3da5da66990f546471d703850d812`. Verbatim patch ID `5bb21dc887bac97e2958a047f9a0b6990cd6ab14`. The evidence commit sits above that source. Its final local-gate record is cited in the PR body.

This record supersedes fix3 for the create flow and the paths below. The historical records retain their original provenance. The current implementation head was driven natively and through clean mock builds.

```text
src/components/sidebar.tsx
src/components/command-palette.tsx
src/pages/page-frame.tsx
src/pages/page-view.tsx
src/shell/new-pages.ts
src/shell/page-presence.ts
src/shell/use-page-commands.ts
src/stores/page-titles.ts
src/stores/page-tree.ts
src/stores/palette.ts
src/stores/shell.ts
src/ipc/record-commands.ts
src/router.tsx
src/styles.css
```

## Data shape

A `ReadonlyMap<PageId, "creating" | "created" | "failed">` owns each activated page and preserves activation order.
The ordinary title draft queues renames behind that page's create in the existing FIFO.
Page presence checks session trash and new-page state before the tree. The body waits for successful creation.

## Native runs and outcomes

Environment: arm64 macOS 27.0.1, real Tauri 2 WKWebView, synthetic data only. Every launch used verify-aelomi app.sh, which sets AELOMI_DATA_DIR. Every native key, AXPress and transient screenshot acquired and released the orchestrator's ui-lock.sh. Build and launch work stayed outside the keyboard lock. All owned instances were cleaned up.

| Local run under `~/.local/state/aelomi-verify/` | Source | Outcome |
| --- | --- | --- |
| `20261016-145051-dev-yqOF` | `87d9a178a86d699a9c7cf250cb429e22dd979da1` | Getting started, Command N plus typing in one JXA call, matching title and row, rename, and normal Quit. |
| `20261016-145440-dev-EcJw` | `87d9a178a86d699a9c7cf250cb429e22dd979da1` | Rename kept after relaunch. Trash chose Getting started, then empty state. The empty-state button accepted immediate typing. Sidebar toggle worked. Another Command N, title and Quit ran in one JXA call. |
| `20261016-145716-dev-iVEF` | `87d9a178a86d699a9c7cf250cb429e22dd979da1` | Real chmod failure retained Held native with an editable title and no body. Quit asked for workspace changes. Recovery kept the title and Body recovered. A queued trash exposed its linked read-only reason in the AX tree. |
| `20261016-150918-dev-12is` | `1805ec44079569619e867e6c1fa51ec52595d46e` | Fresh-folder repeat after the failed-entry fix. Final held stayed gated during chmod failure. Recovery kept Final body. Command N, Final head quit and native Quit ran without a wait between them. |
| `20261016-151640-dev-TI5E` | `7881314eaab29c56e821ee020810ab47887d3f80` | Relaunch showed Final head quit. A new Head certified title stayed editable and gated during another chmod failure in both themes. Title Undo and Redo survived navigation. Final held and Final body remained exact after recovery and a second normal Quit. |

The earlier captures exercise navigation and trash code unchanged by the later commits. The later commits keep failed entries gated after an accepted record appears in a tree, describe a frozen title during Draining, and name the connection status separately from the permanent shell status. The clean WebKit runs cover those changes.

The native read-only panel reads the source tree, title, page stage, body presence and status-node identity. Its two theme buttons change CSS media rules only inside that synthetic window. It changes no system setting. AXPress focuses the panel button, so its later focused:false value records that action. Immediate native typing proves the title focus. The panel and HTML import were removed before the final local gate.

VoiceOver remained off, as the brief requires. The initial AX tree has the empty shell group before its message. The pending-trash capture shows Moving to trash and a read-only title, and the panel reports the same status node and a resolving description. WebKit additionally asserts the empty status node survives the message update and that aria-describedby resolves to the visible reason, including a title first mounted during Draining. Speech output was not tested.

Exact Rust readback opens only closed synthetic stores. These assertions passed:

```text
PASS records=4 active=["From empty", "Quit exact"] trashed=["Getting started", "Round4 renamed"] original_body_unchanged=true
PASS recovered body=[Text("Body recovered")]
PASS records=4 active=["", "", "Held native"] trashed=["Getting started"] original_body_unchanged=true
PASS recovered body=[Text("Final body")]
PASS records=4 active=["Final head quit", "Final held", "Getting started", "Head certified"] trashed=[] original_body_unchanged=true
quit drain failed pending=Pending { docs: [DocId(workspace)], settings: false }
quit flush via=CompleteQuit pending=None
quit flush via=Exit pending=None
```

All changed folder modes returned to 755. The before/after read-only top-level listings of `~/Library/Application Support/Aelomi` match byte-for-byte. Every shell storage line names an isolated folder. The two untitled records in the supplemental outage fixture are deliberate retained creates used to queue trash behind a failing create.

## Reproduce

Run `bash docs/evidence/shell-sidebar/fix4/run.sh.txt` from this worktree. It archives the original HTML, installs a temporary read-only panel, drives fresh native folders and explicit relaunches, checks exact stored records and bodies, and restores HTML and permissions. The native scripts acquire their own keyboard locks. No script changes VoiceOver or system settings. The helper archive was checked with bash -n.

After native cleanup, copy `terminal.mjs.txt` or `stress.mjs.txt` to a temporary .mjs file and pass it to `.claude/skills/verify-aelomi/scripts/mock-drive.mjs --flow <file> --rebuild`. Browser runs and native dev runs must stay sequential.

Local evidence remains in the run folders above. The final native run's `validation/` folder holds local check logs. Raw logs and AX trees are not committed. The final committed-head `pnpm check:all` record belongs in the PR body, where its SHA is available without a self-reference in this record.

## Design corrections and limits

- Keep failed entries until a successful retry, even when a fresh tree lists their accepted record. The design's stage != creating pruning rule would unlock the body after a terminal durability error. A red unit test and the WebKit accepted-record/terminal-failure case prove the stricter rule.
- The caret example needs four Left presses after Bufered before inserting f to get Buffered at selectionStart 4. Three presses produce Bufefred.
- A body-focus request made while the body is gated is consumed. Carrying it until recovery moved focus out of the title. Only a later explicit focus request can focus that body.
- The first native recovery helper waited for a Healthy event that a record-load error does not emit. The corrected recipe checks the actual body. A direct AXPress could not address the palette option, so the supplemental trash check used Down and Return after its AX query settled. Both failed helper attempts restored permissions and released their locks.
- #94 was still open at the final base check, and main remained at the recorded base. The sidebar expansion writer and the theme acceptance assertion remain separate until that PR lands.
- The Noted design issues remain with #87. The 10,000-page 100 ms miss remains with #91. Native hardware IME and VoiceOver speech are not claimed.

Model the Domain produced the per-page map. Type System Discipline made page presence a closed union. Test Behavior, Not Implementation produced exact stored titles and bodies plus zero body calls before creation. Prove It Works required real native actions and Rust readback. Sequence Work into Verifiable Units kept the FIFO tests, lifecycle tests, browser flows and native proof separate. The no-comments pass ran directly because the brief forbids subagents. It added no comments or suppressions and found no new constraint comment.

## Browser and logic tests

The focused clean WebKit run passed 42 cases. `pr89-fix4-terminal` passed its two-theme failure and retry flow. `pr89-fix4-mock-stress` passed 22 inspected frames with no non-local requests and a free preview port after cleanup. The stress fixture covers unbounded titles, long URLs, empty and whitespace titles, CJK, diacritics, Arabic, emoji, line breaks, deep nesting and 10,000 rows at 320, 960 and 2560 pixels and 200% CSS zoom. Existing deep horizontal-scroll and narrow-layout limits stay deferred to #87.

Each case below maps its leading SPEC IDs to the tested behavior. TREE-2 covers the row-click rule in this PR. CMD-2, CMD-3 and CMD-6 cover this slice's page-command group. Section 2 actions remain outside this PR.

| Test file | Exact case name |
| --- | --- |
| `e2e/shell-sidebar.e2e.ts` | `SHELL-1 SHELL-6 Toggle sidebar` |
| `e2e/shell-sidebar.e2e.ts` | `SHELL-4 TREE-1 Empty state creates an untitled page with its title focused` |
| `e2e/shell-sidebar.e2e.ts` | `TREE-1 TREE-7 EDIT-1 Rename updates the sidebar and Return moves to the body` |
| `e2e/shell-sidebar.e2e.ts` | `TREE-9 Trash the open page chooses its sibling then the empty state` |
| `e2e/shell-sidebar.e2e.ts` | `CMD-2 CMD-3 CMD-6 New page is reachable by query and shortcut from the empty state` |
| `tests/playwright/sidebar-round4.spec.ts` | `TREE-1 Command N focuses the real title synchronously and leaves the old body unchanged` |
| `tests/playwright/sidebar-round4.spec.ts` | `SHELL-4 first Command N accepts typing before the body chunk loads` |
| `tests/playwright/sidebar-round4.spec.ts` | `TREE-1 held page reopens by row palette and route with no body read or write` |
| `tests/playwright/sidebar-round4.spec.ts` | `TREE-1 create acknowledgement preserves the caret and every title undo step` |
| `tests/playwright/sidebar-round4.spec.ts` | `TREE-1 two titled activations own distinct IDs through an outage` |
| `tests/playwright/sidebar-round4.spec.ts` | `TREE-1 repeated empty activations reuse one ID with stable New page controls` |
| `tests/playwright/sidebar-round4.spec.ts` | `TREE-1 stale reads keep the new title editable until one exact record is listed` |
| `tests/playwright/sidebar-round4.spec.ts` | `SHELL-10 held create asks immediately and retains Q during recovery in Asking` |
| `tests/playwright/sidebar-round4.spec.ts` | `SHELL-10 terminal create keeps its title read-only and retries the same ID by keyboard` |
| `tests/playwright/sidebar-round4.spec.ts` | `TREE-1 leaving an empty failed page removes it and the next Command N owns focus` |
| `tests/playwright/sidebar-round4.spec.ts` | `CMD-6 held new page cannot be trashed behind the palette` |
| `tests/playwright/sidebar-round4.spec.ts` | `TREE-9 trash completion cannot move focus off a new title` |
| `tests/playwright/sidebar-round4.spec.ts` | `TREE-1 a body mounted after a gated create never steals row focus` |
| `tests/playwright/sidebar-round4.spec.ts` | `TREE-9 successful trash removes stale rows and disables stale palette results` |
| `tests/playwright/sidebar-round4.spec.ts` | `SHELL-9 draining disables New page visibly with no press transform and keeps the status node` |
| `tests/playwright/sidebar-round4.spec.ts` | `TREE-7 full truncated title is on the hovered row button` |
| `tests/playwright/sidebar-round4.spec.ts` | `TREE-1 a long outage on A keeps Held gated through acceptance and durability` |
| `tests/playwright/sidebar-round4.spec.ts` | `CMD-6 final create and trash failures keep their own pages and retry targets` |
| `tests/playwright/sidebar-round4.spec.ts` | `SHELL-9 each sidebar and direct-route activation stays on the frozen page during Draining` |
| `tests/playwright/sidebar-round4.spec.ts` | `SHELL-9 palette activation stays blocked during Draining independently of the sidebar` |
| `tests/playwright/sidebar-round4.spec.ts` | `SHELL-9 a title mounted during an existing drain starts frozen` |
| `tests/playwright/sidebar-round4.spec.ts` | `TREE-1 an accepted record cannot unlock a terminally failed create through a fresh tree` |
| `tests/playwright/sidebar-round3.spec.ts` | `EDIT-11 title Undo and Redo survive A to B to A` |
| `tests/playwright/sidebar-round3.spec.ts` | `CMD-3 Move to trash requires explicit selection after to and content replies` |
| `tests/playwright/sidebar-round3.spec.ts` | `TREE-7 refresh error keeps rows and title with a retry notice then recovers` |
| `tests/playwright/sidebar-round3.spec.ts` | `TREE-9 pending trash freezes the target title and its descendants` |
| `tests/playwright/shell-sidebar.spec.ts` | `SHELL-1 SHELL-6 hidden sidebar gives its width to the editor` |
| `tests/playwright/shell-sidebar.spec.ts` | `SHELL-4 TREE-1 CMD-6 trash the only page then create from the empty state` |
| `tests/playwright/shell-sidebar.spec.ts` | `TREE-1 TREE-7 SHELL-8 EDIT-1 title input persists immediately and Return starts the body` |
| `tests/playwright/shell-sidebar.spec.ts` | `TREE-7 TREE-15 title history never consumes body history after repeated Undo` |
| `tests/playwright/shell-sidebar.spec.ts` | `TREE-2 TREE-6 disclosure persists settings and a row click focuses the editor` |
| `tests/playwright/shell-sidebar.spec.ts` | `TREE-7 failed refresh preserves the title and Undo baseline without accepting edits` |
| `tests/playwright/shell-sidebar.spec.ts` | `SHELL-9 title composition commits its final input before quit freezes the field` |
| `tests/playwright/shell-sidebar.spec.ts` | `TREE-6 settings failure keeps the source tree collapsed and disclosures disabled` |
| `tests/playwright/shell-sidebar.spec.ts` | `TREE-1 current page reveals its ancestors and scrolls into the virtualized sidebar` |
| `tests/playwright/shell-sidebar.spec.ts` | `TREE-2 sidebar focus requests do not repeat a content-search jump, and hiding transfers focus` |
| `tests/playwright/shell-sidebar.spec.ts` | `TREE-7 whitespace titles stay named and IME confirmation Return keeps title focus` |
| `tests/playwright/shell-sidebar.spec.ts` | `TREE-9 opening a child during retained parent trash waits for durable ok before fallback` |
| `src/shell/page-navigation.test.ts` | `TREE-6 visible rows retain source order and omit collapsed descendants` |
| `src/shell/page-navigation.test.ts` | `TREE-6 flattening uses an explicit stack for deep source trees` |
| `src/shell/page-navigation.test.ts` | `TREE-9 trash opens the next active sibling before ancestor or recent page` |
| `src/shell/page-navigation.test.ts` | `TREE-9 trash opens the nearest active ancestor when there is no next sibling` |
| `src/shell/page-navigation.test.ts` | `TREE-9 trash opens the most recently opened active page after siblings and ancestors` |
| `src/shell/page-navigation.test.ts` | `TREE-9 trash shows the empty state when no active fallback was opened` |
| `src/shell/page-presence.test.ts` | `TREE-1 presence checks trash and new pages before the listed tree` |
| `src/shell/page-presence.test.ts` | `TREE-1 only a successful create unlocks its body` |
| `src/shell/page-presence.test.ts` | `TREE-9 visible pages omit session trash and append new IDs in activation order` |
| `src/shell/page-presence.test.ts` | `SHELL-9 opening admission closes during Draining and reopens with Open` |
| `src/shell/new-pages.test.ts` | `SHELL-10 a failed page with text retains a quit retry using the same ID` |
| `src/shell/new-pages.test.ts` | `SHELL-10 an empty failed page needs no retained title retry` |
| `src/shell/new-pages.test.ts` | `TREE-1 a fresh tree forgets listed settled entries but keeps creating and failed entries` |
| `src/stores/page-titles.test.ts` | `TREE-7 a rename pending through an outage keeps its draft without an error then clears on ok` |
| `src/stores/page-titles.test.ts` | `TREE-7 an unavailable title cannot submit a rename against an empty baseline` |
| `src/stores/page-titles.test.ts` | `TREE-7 a matching old snapshot cannot acknowledge a newer queued draft` |
| `src/stores/page-titles.test.ts` | `TREE-7 a final rename refusal keeps the draft and shows a failure` |
| `src/stores/page-titles.test.ts` | `TREE-7 a matching source title stays a draft until the retained operation is acknowledged` |
| `src/stores/page-titles.test.ts` | `TREE-7 a pending rename ending in a terminal error keeps the exact text and failure` |
| `src/stores/page-titles.test.ts` | `TREE-7 the latest coalesced rename stays pending behind another page's retained operation` |
| `src/stores/page-titles.test.ts` | `TREE-7 title admission shares one rule for listed, new, closing and trashed pages` |
| `src/stores/page-titles.test.ts` | `TREE-7 a refused rename sends no command while an admitted rename writes a draft` |
| `src/ipc/record-commands.test.ts` | `TREE-1 a caller-owned create ID orders its title behind its create` |
| `src/ipc/record-commands.test.ts` | `TREE-1 a retained create coalesces its queued title to the latest draft` |
| `src/ipc/quit-invocations.test.ts` | `SHELL-10 a retained new page asks immediately and saves its exact title after Keep open` |
| `src/ipc/quit-invocations.test.ts` | `SHELL-10 a terminal new-page failure retains its exact title for the quit drain` |

`new-pages.test.ts` also expands `TREE-1 create settlement %j retry %s becomes %s` over ok, Internal, Abandoned, initial AlreadyExists and retried AlreadyExists. The wrapped-title loop in `shell-sidebar.spec.ts` runs both one-line and wrapped title cases.

## Published screenshots

All 14 attachments in [the PR comment](https://github.com/aelomi/aelomi/pull/89#issuecomment-6212909065) were downloaded without credentials. Each returned a PNG matching the original byte size and SHA-256. Every original capture was visually inspected.

| Image | Bytes | SHA-256 |
| --- | ---: | --- |
| [head-held-light](https://github.com/user-attachments/assets/1ed17dca-b11b-45ba-a58f-00b2f1409b55) | 148161 | `289605b6056f82f758bfcf035975f0578571e4cdc50722359d73f49532447b36` |
| [head-held-dark](https://github.com/user-attachments/assets/7d67aa25-452f-464b-941f-f9720831688f) | 144380 | `8561cac16257cad4da0e786acfecc3e20f32f81d938579042da8ac1cd1715d83` |
| [final-body-relaunch](https://github.com/user-attachments/assets/ab2f3fec-422b-4a2f-b55f-c6be390aa063) | 144795 | `89423ce94a5a1ed6eab4710ef47bfce71c38b96c80c1072a41779a4047180145` |
| [final-quit-relaunch](https://github.com/user-attachments/assets/31a40ba4-8fe1-4e90-a5e2-c3b9e008d054) | 126852 | `a673f2a8071ca75fec9e3c681ef83bb5b8eaacdae91fb270094443a8e4f1d70a` |
| [sidebar-light](https://github.com/user-attachments/assets/6c4186b7-faab-427c-9de2-232ce9eca752) | 155350 | `98d63fec3107c21ffc5ac102a33e1515de26765dfb5797353d02e404ced8fc00` |
| [sidebar-dark](https://github.com/user-attachments/assets/2d2f11ad-b163-4230-9a9d-dfacbc13f54a) | 124301 | `52f83201b246cbfb7efd5aedee5a369f580602be86aaee56b9449260b62aff9f` |
| [empty-state](https://github.com/user-attachments/assets/f8aa9898-ac16-47ab-ae47-e74c2789b4e6) | 111254 | `f369591271bc51707b0acd218dd519a1b5f01da98496a3d62186d2eed7d7bca0` |
| [from-empty](https://github.com/user-attachments/assets/7f8ddc65-f146-421d-bd21-ba1ab736a722) | 119306 | `5dc5745ee7210d285f544120db0add7b03f6a07bc03a38e282ed8957548ce851` |
| [trash-fallback](https://github.com/user-attachments/assets/400396f4-1f2b-49a9-bd2e-0f0533ff541c) | 133241 | `44ca32f29a6f8c21961efa88b8a06d2d448c85a98f0b7250605a6bb1b3a5d169` |
| [rename-relaunch](https://github.com/user-attachments/assets/891bf426-a1f6-48b4-86ca-a0424b733c41) | 127285 | `fbaa46972f3bc6d815689e1dc10cb4f8c6e16442cef5098c2191fa368c4b88b8` |
| [pending-trash-description](https://github.com/user-attachments/assets/74c0ccb8-7152-4878-8d2e-c5fa208bf79c) | 197206 | `6a5deda350ed94745c7765cf14650cebe452f9054489b007396aae381524196b` |
| [quit-retained-create](https://github.com/user-attachments/assets/ea7d76a0-ad8a-4edc-b477-d3a77c5a44ba) | 215604 | `383b81ba902a33ac4960f3b3e8e216bd4d691137a20571b27c165bbadea98d69` |
| [terminal-create-light](https://github.com/user-attachments/assets/0fc818b9-692e-45b9-9d37-ed8f21e5d930) | 46497 | `e0d487db2c5cf3cecf5d8f6080f28e0096039257ec1fd9794bb6e07f6d76e63e` |
| [terminal-create-dark](https://github.com/user-attachments/assets/c934d47a-d934-4d54-8b79-558222f80927) | 46394 | `e8a746fc88bc22207186c85dea8b1818479cdc4df45ad6b9152d1305f7382d7c` |
