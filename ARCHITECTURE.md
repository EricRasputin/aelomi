# Aelomi architecture

This file maps the code. `SPEC.md` has the behaviors and the budgets, and `README.md` has the commands to run, build, and test.

## Find a file

| Path | Contents |
|---|---|
| `Cargo.toml` | Cargo workspace with members `crates/core` and `src-tauri`, edition 2024 |
| `crates/core/` | `aelomi-core`, the Rust core |
| `crates/core/src/settings.rs` | DATA-4 settings model, typed patch and additive JSON codec |
| `crates/core/src/storage/settings.rs` | Settings file loading and atomic persistence through Layout and Fs |
| `crates/core/src/storage/settings_tests.rs` | Settings crash steps, session round-trip, compatibility, recovery and SHELL-9 clock tests |
| `crates/core/src/storage/` | The append-only Y.Doc store. `mod.rs` lists the public API |
| `crates/core/src/storage/store.rs` | `Store`: open the data folder, `load`, `append`, `flush`, `health`, `close` |
| `crates/core/src/storage/writer.rs` | The writer thread and its group commit |
| `crates/core/src/storage/compact.rs` | Compaction into a verified snapshot (DATA-6) |
| `crates/core/src/storage/recover.rs` | Loading a doc from its files, noticing an interrupted compaction, and merging every update a doc holds |
| `crates/core/src/storage/log/` | Log files. `mod.rs` reads each one with the parser its name selects: `v1.rs` for `.ylog`, `v2.rs` for `.ylog2` |
| `crates/core/src/storage/fs.rs` | The `Fs` seam and `OsFs`, with the `F_FULLFSYNC` sources |
| `crates/core/src/storage/sim_fs.rs` | `SimFs`, the crash-simulating filesystem that checks the append-only rules |
| `crates/core/src/storage/data_path.rs` | `check_data_path`, the cloud folder check (SHELL-11) |
| `crates/core/src/storage/test_support.rs` | `SimDisk`, a crash-simulating disk for the shell's tests, behind the `test-support` feature |
| `crates/core/src/storage/tests/` | Store tests: crash points, compaction, failures, acknowledgement time, refusals, the fixture. `measure.rs` holds ignored measurements: `cargo nextest run -p aelomi-core --cargo-profile release --run-ignored only measure --no-capture` |
| `crates/core/src/workspace/mod.rs` | `Workspace`: the store plus its index. Every append goes through it, so the index hears of each change the store accepts |
| `crates/core/src/workspace/records.rs` | `PageId` and the workspace doc's page records (DATA-2), read with typed failures |
| `crates/core/src/workspace/body.rs` | The page body reader for BlockNote's Yjs layout: blocks, text, and page links |
| `crates/core/src/workspace/tree.rs` | The page tree, favorites, and trash list projected from the records |
| `crates/core/src/workspace/fixture.rs` | Writers for records and bodies, the index fixture, and `break_index`, behind the `test-support` feature |
| `crates/core/src/index/` | The derived SQLite index: `mod.rs` owns status and marks, `worker.rs` the single writer, `db.rs` schema and cache replacement, `query.rs` ordered search and backlinks, `tokenizer.rs` the SQLite FFI, and `highlight.rs` safe text spans |
| `crates/core/src/index/tests/` | Index tests: rebuild equality, races, failures, repair backoff, unreadable bodies, latency, the backlink property test, and the 10,000-page search fixture |
| `crates/core/examples/seed_index.rs` | Seeds a new folder through the real Store with rebuild cases, a body root that is not XML, an unreadable pages root, or `search-10k` with optional copies and the 5,000-block page |
| `src-tauri/` | `aelomi`, the Tauri 2 shell (library name `aelomi_lib`) |
| `src-tauri/src/lib.rs` | `run()` builds the Tauri app, registers the commands and events, opens the workspace, logs and emits index events, and flushes the store before quitting |
| `src-tauri/src/commands.rs` | Tauri commands, the tauri-specta `builder()`, and the `bindings_are_current` test |
| `src-tauri/src/settings.rs` | Settings IPC types and conversions, with owner, retry, quit and command tests in `settings/tests.rs` |
| `src-tauri/src/storage.rs` | `Storage`, the open workspace or why it did not open, the data folder overrides, the flush before quitting, and the storage IPC types |
| `src-tauri/src/quit.rs` | `QuitGate`: revisioned frontend drain, Store flush, confirmation, and the quit IPC types |
| `src-tauri/src/index.rs` | The index commands (`index_status`, `page_tree`, `search`, `backlinks`), the `IndexStatusChanged` and `IndexChanged` events, and their IPC types |
| `src-tauri/capabilities/default.json` | Lets the window listen to the events the core emits |
| `src-tauri/src/macos.rs` | All macOS-specific code |
| `src-tauri/src/perf.rs` | The cold-start probe, armed only by `AELOMI_PERF_REPORT` |
| `src-tauri/tauri.conf.json` | Window, content security policy, and the ad-hoc signed `.app` bundle |
| `src/main.tsx` | UI entry point, loaded by `index.html` |
| `src/router.tsx` | Routes and the screens they render |
| `src/stores/` | Zustand stores |
| `src/ipc/` | Generated bindings, the IPC mock, the switch between them, and the base64 helpers for doc bytes |
| `src/dev/` | Routes that only dev builds register, such as the storage probe |
| `src/editor/` | Raw Y.Doc delivery, pre-bind body guard, BlockNote core binding, link picker and quit drain |
| `src/pages/` | Lazy page route and page view |
| `src/palette/` | Fuzzy title ranking and content-result projection |
| `src/perf/` | Actual page-content cold-start marks, real editor perf routes and synthetic diagnostic routes |
| `src/styles.css` | Tailwind v4 import and the light and dark color variables |
| `e2e/` | TesterArmy acceptance tests and their guarded fixture |
| `tests/playwright/` | Playwright WebKit tests and their guarded fixture |
| `perf/` | Native cold-start and RSS harnesses, frame timing, statistics and performance budgets |
| `scripts/check-search.ts` | Serial release gate for complete public index queries, shared by CI and check:all |
| `patches/` | Version-pinned dependency patches installed through pnpm |
| `fixtures/` | The seeded fixture generator and its output in `fixtures/generated/` |
| `fixtures/data-folders/` | A data folder per storage format, for migration tests |
| `fixtures/index/` | A page body the real BlockNote binding wrote, its generator, and its provenance |
| `tests/is-local-url.ts` | The loopback allow list both browser suites share |
| `.github/workflows/ci.yml` | Every CI job |
| `scripts/check-all.ts` | `pnpm check:all`, which runs every CI job locally on a clean committed head and records a pass in `.local-gate/` |
| `scripts/check-all-stages.ts` | The stage list `check:all` runs, one per CI job, and the environment every stage gets; `check-all.test.ts` fails when its jobs, commands or effective environment drift from `ci.yml`, or a pinned action lacks its version comment |
| `scripts/check-licenses.mjs` | Fails on an npm license outside the allow list |
| `scripts/check-no-mock.mjs` | Fails if the IPC mock or a dev route is in the production bundle in `dist/` |
| `scripts/protect-main.sh` | Makes each CI job a required check on `main` |
| `scripts/tauri-dev.ts` | `pnpm dev:app`, which runs `tauri dev` with the UI on `AELOMI_DEV_PORT` or a free port |
| `scripts/vendor-skills.sh` | Vendors the pinned pstack and Emil skills into `.claude/`, and fetches the gitignored pen.dev skill. It unhides the pstack skills `how` and `why` for agents and records that change in `.claude/vendor/pstack/PATCHES` |
| `deny.toml` | cargo-deny policy: Rust licenses, advisories, and sources |
| `vite.config.ts` | Vite config. Mock builds go to `dist-mock/`, production builds to `dist/` |
| `tsconfig.json` | Strict TypeScript settings for `src/`, `e2e/`, and `tests/` |
| `biome.json` | Lint and format rules, including the test import restrictions |
| `.claude/skills/verify-aelomi/` | The skill agents use to launch, drive, and screenshot the app to prove a change works, with its feature map |

## Rust workspace

The workspace has two crates. Both use the version, edition, and license from `[workspace.package]` in `Cargo.toml`.

`crates/core` is the package `aelomi-core`. It owns storage, indexing, search, markdown export, the MCP server, and later the sync client. Today it holds the storage engine, `aelomi_core::storage`, the source model and its owner, `aelomi_core::workspace`, the SQLite index, `aelomi_core::index`, and the constant `VERSION`. It depends on no Tauri crate, so its tests need no window.

`src-tauri` is the package `aelomi`, the desktop shell. `src-tauri/src/main.rs` calls `aelomi_lib::run()`. `run()` in `src-tauri/src/lib.rs` passes the command list from `commands::builder()` to Tauri's `invoke_handler`. Tauri's `setup` hook mounts the events, calls `macos::setup`, and opens the workspace (see "Storage in the shell").

## macOS-specific code lives in one module

`src-tauri/src/macos.rs` holds all code that works only on macOS. `src-tauri/src/lib.rs` declares it with `#[cfg(target_os = "macos")] mod macos;` and calls `macos::setup(app)` under the same `cfg`. Today `setup` installs the default native menu with its own Quit item and adds `applicationShouldTerminate:` to tao's app delegate, `data_location` supplies the data folder and the cloud folders it must not resolve into, and `process_start_us` reads the kernel's process start time for the cold-start probe. A port to another platform replaces this one module. The `bundle.macOS` block in `src-tauri/tauri.conf.json` sets the macOS 14 minimum and ad-hoc signing.

## A command call goes from a store to the shell and back

The `ping` command is the working example.

1. `IndexScreen` in `src/router.tsx` calls `ping` from `usePingStore` in `src/stores/ping.ts`.
2. The store calls `commands.ping({ seq })`, imported from `src/ipc/index.ts`.
3. `src/ipc/index.ts` exports the generated `commands` from `src/ipc/bindings.ts`, or the mock in a mock build.
4. The generated `ping` calls `invoke("ping", { request })` from `@tauri-apps/api/core`.
5. Tauri runs `ping` in `src-tauri/src/commands.rs`, which reads `aelomi_core::VERSION` and returns a `PingReply`.
6. The store sets `status` to `ready`. Only components that select `status` re-render.

Outside `src/ipc/`, import `commands` and the IPC types from `src/ipc/index.ts`. A direct import from `bindings.ts` would skip the mock swap, so Biome rejects any import of it outside `src/ipc/`.

## Bindings are generated and checked in CI

`commands::builder()` lists every command in `tauri_specta::collect_commands![...]`. Tauri registers commands from the same builder, so the registered commands and the TypeScript types come from one list.

`src/ipc/bindings.ts` is generated and committed. Biome skips it. Never edit it by hand.

The `bindings_are_current` test in `src-tauri/src/commands.rs` generates the bindings to a temporary file and compares them with the committed file. If they differ, `cargo nextest run --workspace` fails, and so does the `nextest` CI job. `pnpm bindings` runs the same test with `AELOMI_WRITE_BINDINGS=1`, which writes the new bindings to `src/ipc/bindings.ts`. After that, `pnpm typecheck` fails on every caller and mock entry that no longer matches. The mock declares `satisfies Commands`, so a command missing from the mock also fails the typecheck.

To add a command:

1. Add a function with `#[tauri::command]` and `#[specta::specta]` to `src-tauri/src/commands.rs`. Derive `specta::Type` on its argument and return types.
2. Add the function to `collect_commands![...]` in `builder()`.
3. Put logic that needs no window in `aelomi-core`, with a core test.
4. Run `pnpm bindings`.
5. Add the command to `commands` in `src/ipc/mock.ts`.
6. Run `pnpm typecheck`.

## Frontend layout

`src/main.tsx` connects storage events before rendering the code-based TanStack router inside StrictMode. The connection registers listeners before snapshots. Health events supersede stale health snapshots, and quit revisions order events, snapshots and command replies. Editing becomes available only after the quit listener and snapshot are installed and `frontend_ready` succeeds. A refused quit status read or `frontend_ready` call is retried with backoff from 100 ms to 5 s until it succeeds or the connection is disposed. Disposal ignores late results and unregisters listeners.

The root shell retains the startup ping and an accessible status landmark on every route. `/` opens the first active page in source-tree order, while an empty workspace retains IndexScreen. The page route lazily loads BlockNote core. The page body uses the raw page Y.Doc, and its header remains fixed above the scrolling page area. `src/editor/` owns schema checks, source delivery and the link picker. `src/pages/` owns page loading and presentation.

`src/components/storage-notices.tsx` renders the save-failure banner and confirmation dialog. `src/stores/quit.ts` handles open, draining, asking and quitting decisions. Draining freezes editors and awaits delivery before acknowledging the matching native revision. A current Keep app open reply unfreezes them. Health recovery clears its banner independently of an unresolved quit decision.

`src/components/command-palette.tsx` opens with ⌘K. One layout effect owns what opening and closing do: it makes the page inert and focuses the input, and its cleanup, which every close commits synchronously, releases the page and returns focus to where it was, including after choosing the open page. TypeScript fuzzy title matching follows CMD-4 and caps the complete title group at 200. Content requests exclude those IDs, including virtualized rows. `src/stores/palette.ts` orders replies by request sequence, reruns on Ready and index changes, and keeps title and content groups disjoint. Indexing replies show Indexing… even when the last status event was Ready. Failed shows an error. Per-page issues show a quiet completeness notice. Selecting content scrolls to and briefly highlights its matching block. The `[[` picker uses the source tree and remains closed in code blocks and inline code.

Zustand components use selectors. `index-status.ts` subscribes before its status snapshot. `page-tree.ts` loads the source-backed tree independently of index availability. `recent-pages.ts` records visits for title ties. The subset does not yet provide the palette command group, recent-page list, sidebar or link creation.

`src/ipc/` owns generated bindings, the mock switch and base64 helpers. The mock boot configuration seeds DATA-2 workspace records and actual prosemirror bodies before commands run. `mock-workspace.ts` projects its own tree, including missing parents, cycles, favorites and ancestor trash. `mock-body.ts` builds canonical Yjs bodies, and `mock-search.ts` implements the typed scopes, fixed ordering, exclusions and partial diagnostics. Status and quit controls emit the same generated event types. Raw-body fixtures prove read-only preservation and source delivery through actual editor tests.

Mock builds alone register real editor routes `editor-empty` and `editor-page-5000-blocks`, with the original synthetic routes retained for diagnostics. Dev builds retain `/dev/storage`. `pnpm build:mock` writes dist-mock for the browser suites. Production builds omit the mock and dev routes; `check:no-mock` verifies that boundary.

## Tests by layer

| Layer | Tool | Location | CI job |
|---|---|---|---|
| Pure TypeScript logic | Vitest | `*.test.ts` in `src/`, `tests/`, `perf/`, `fixtures/`, and `scripts/`, configured in `vitest.config.ts` | `vitest` |
| Rust core and shell | cargo-nextest | `#[cfg(test)]` modules in each crate | `nextest` |
| Acceptance | TesterArmy `e2e` | `e2e/*.e2e.ts`, configured in `e2e.config.ts` | `e2e` |
| WebKit flows and the network check | Playwright, `webkit` project | `tests/playwright/*.spec.ts`, configured in `playwright.config.ts` | `playwright` |
| Keystroke-to-paint | Playwright, `perf` project | `tests/playwright/perf/` | `keystroke` |
| Cold start | Node harness on the built `.app` | `perf/cold-start.ts` | `cold-start` |

`.config/nextest.toml` terminates ordinary tests after 60 s, gives ignored storage measurements 10 minutes, and gives the serial release search gate 5 minutes.

Both the local and hosted Vitest stages set a 2 GB old-space heap limit. The binding oracle's editor-state and source-update comparisons exercise the full 5,000-block fixture.

Page-link labels the editor shows share one page tree subscription, and each node view releases its label on destroy. BlockNote serializes outside the editor for copy, drag and HTML export with a `getPos` that returns undefined, and those labels are painted once and held by nothing. `tests/playwright/page-links.spec.ts` counts the shown labels in the mock build through copies, drag starts, a rename and a page change.

Acceptance tests import `test` from `e2e/fixtures.ts`, and Playwright tests import `test` and `expect` from `tests/playwright/fixtures.ts`. Both fixtures fail a test that makes a non-local request (SHELL-13). Biome rejects a test file that imports `test` from the library directly. There are no React component unit tests. `e2e` covers components.

## CI jobs

`.github/workflows/ci.yml` runs on every pull request and on pushes to `main`. Every `ci.yml` job runs on `macos-15`.

| Job | Runs |
|---|---|
| `biome` | `pnpm lint` |
| `tsc` | `pnpm typecheck` |
| `vitest` | `pnpm test` |
| `nextest` | The locked workspace suite and bindings check, serial release LINK-8 tests, and `node scripts/check-search.ts --ci` |
| `cargo-deny` | `cargo deny check` |
| `pnpm-audit` | `pnpm audit --audit-level moderate` and `pnpm check-licenses` |
| `build-app` | `pnpm tauri build --bundles app` and `pnpm check:no-mock`, then uploads the zipped `.app` as the `Aelomi-app` artifact |
| `e2e` | `pnpm build:mock` and `pnpm test:e2e` |
| `playwright` | `pnpm build:mock` and `pnpm test:playwright` |
| `cold-start` | Downloads `Aelomi-app` and runs `perf/cold-start.ts --ci` |
| `keystroke` | `pnpm build:mock` and the `perf` Playwright project in CI mode |

To add a job, add its ID to `CHECKS` in `scripts/protect-main.sh` and rerun the script, so `main` requires it. Add a matching stage to `scripts/check-all-stages.ts` so `pnpm check:all` runs it too.

## Performance gates

`SPEC.md` ("Performance budgets") sets the budgets and defines how each is measured. Every number the gates read lives in `perf/budgets.json`, each with a `why`. Hosted macOS runners cannot reach every spec number, so each gate has two modes. Local mode enforces the local budget on Apple Silicon. CI mode (`--ci` or `AELOMI_PERF_CI=1`) enforces its separate budget and prints the local verdict as information. The maintainer allowed a small typing-budget miss on 2026-10-15. The chosen ceiling is 19 ms, with the original 16 ms goal still recorded. The immediate-source editor measured 18 ms in five alternating untraced rounds and 19 ms in a quiet full-suite run. This tolerance changes neither the 100 ms storage deadline nor the 250 ms backlink deadline.

- **Cold start.** The native harness launches the built app, discards one warm-up and takes the median of ten runs. Its default data folder is temporary. `--data-dir` uses a supplied synthetic folder without deleting it, and `--expected-variant page` requires the first editable page content frame. Empty workspaces report index. The opt-in perf report exits through the same quit drain as other routes. CI gates end-to-end and navigation-to-paint against its measured hosted budgets. On 2026-10-15 the maintainer accepted the populated-workspace result of 618 ms median. The shell now projects the page tree on a setup thread while the webview starts, so the frontend's first `page_tree` call finds it in the memo or waits for it. The 500 ms goal remains, and the populated result after this change is recorded next to the original failed run in `docs/evidence/`; the hosted gates remain unchanged.
- **Keystroke-to-paint.** The perf project gates both actual Yjs-bound BlockNote surfaces, empty and 5,000 blocks. Synthetic surfaces remain information. The recorder splits app work from frame wait. Local mode gates full key-to-paint p95 at the chosen 19 ms ceiling. CI uses a provisional 45 ms app-owned ceiling from the measured hosted large-page series, whose three load p95 values were 31, 30 and 41 ms. The second hosted attempt failed fixture setup before collecting that surface. Both modes run without tracing, because Playwright snapshots before keyboard actions change the measured workload. The app-owned clock omits initial layout before WebKit freezes the rAF timestamp, scheduler and IPC delay, frame wait, and presentation. The recorder also reports WebKit's Event Timing entries, the engine's own clock from the key event to the next paint, which WebKit rounds to 8 ms and reports only at 16 ms or more. A mock-only `layout-delay` control lays out a large block before the rAF stamp on every key, and the spec asserts that Event Timing reports that work while the app-owned split leaves it out. Neither clock covers compositor presentation. Scrolling gates rendering work from that timestamp to the following task, with a separate raw-gap maximum, and reports whether its longest interval came from the previous frame's work or from the time between frames. The untraced slow-work controls distinguish that lower bound from raw frame cadence. Palette and scrolling specs exercise the 10k and large-page fixtures too. Palette retains 16 ms locally and uses a provisional 40 ms hosted ceiling from two measured p95 values of 29 and 36 ms. The [hosted calibration record](docs/evidence/data-5-ci-calibration-0ad6f34/README.md) summarizes the failed jobs and the limits of this baseline, and links the full job logs at the commit before the evidence trim.
- **Search.** `node scripts/check-search.ts` runs the complete public-query release gate serially. Its CI mode reads the hosted search entry from budgets.json. The local gate remains 5 ms. The provisional hosted gate is 35 ms, from two unchanged-head runs whose worst fresh-reader p95 values were 27.137709 and 30.837625 ms. The hosted calibration record summarizes both complete-query matrices and their noisy three-core host context, and links the full data at the commit before the evidence trim.
- **Memory.** `node perf/native-process.ts --pid <owned app PID> --out <record.json>` records RSS samples with load and platform context. It reports observed main-process RSS after attachment, excluding WebKit helpers. At rebuild completion, the shell logs `main_peak_rss_bytes` from macOS `getrusage(RUSAGE_SELF)`, the kernel's resident-memory peak since this process launched. This also excludes WebKit helpers and does not depend on sampler attachment or frequency. Apple's [kernel implementation](https://github.com/apple-oss-distributions/xnu/blob/f6217f891ac0bb64f3d375211650a4c1ff8ca1ea/bsd/kern/kern_resource.c) reads the maximum resident size, whose [units are bytes](https://github.com/apple-oss-distributions/xnu/blob/f6217f891ac0bb64f3d375211650a4c1ff8ca1ea/osfmk/mach/task_info.h).
- **Fixtures.** `fixtures/generator.ts` (`pnpm fixtures`) writes `fixtures/generated/`: 10,000 pages for search, their expected query results, and a 5,000-block BlockNote page. `fixtures/generator.test.ts` regenerates them and compares hashes, so a hand edit fails Vitest.
- **CI.** The `cold-start` job downloads the `build-app` artifact. The `keystroke` job runs the `perf` project. Both are in `CHECKS` in `scripts/protect-main.sh`.

## Settings and session state

`crates/core/src/settings.rs` owns the DATA-4 model and file encoding. `Settings` holds a theme, an optional open page and block anchor, recent page IDs in newest-first order, and a sorted set of expanded page IDs. Recent pages keep their first occurrence and at most 50 IDs. This bounds file size while retaining navigation history. Root and open-page extension maps preserve future JSON keys. Unparsable known fields remain raw JSON until a patch replaces that field. Invalid list entries are dropped independently. Open-page extensions merge only when the page ID stays the same. An unknown theme retains its original string, while patches accept only System, Light, or Dark.

`settings.json` sits beside `format.json`. A missing file loads defaults. Malformed JSON logs a failure and loads defaults. A read error other than NotFound refuses reads and updates until a later read succeeds, so a transient failure cannot publish defaults over an unread file. `storage/settings.rs` owns file access through `Layout`. Each write creates a temporary file, fully syncs it, replaces the target, then syncs and fully syncs the root directory. `Fs::replace` accepts only the root-file names in `Layout`, establishes source durability on `OsFs`, and checks it on `SimFs`. Format-marker validation and document append-only rules remain unchanged.

`Workspace` owns one lazy, mutex-guarded settings snapshot. Setup warms it on Tauri's blocking pool beside the page-tree projection. Reads clone the cache without disk access after a successful load. An update validates its typed patch, applies it under the workspace mutex, and queues the full snapshot on the existing storage writer. It returns only after durability, or after a typed write failure. Holding the snapshot lock until that result serializes concurrent patches. The writer retains failed snapshots, retries with the document backoff, and includes settings in health notifications. A new patch combines with the retained snapshot and triggers an immediate retry. Every normal quit route flushes both documents and settings by the shared deadline, and a failure names settings.json in quit status. The existing confirmation tells the user that settings changes have not been saved. Neither owner nor file access exists when storage refused the folder.

The IPC types use snake_case like the other commands. The on-disk JSON uses camelCase. The patch has optional `theme`, `open_page`, `recent_pages`, and `expanded_rows` fields. `open_page` is `{ Set: { page_id, top_visible_block_id } }` or `Clear`, so omission and clearing differ. Patches accept the closed `ThemeChoice` enum. Unknown theme strings appear only in snapshots as `{ Unknown: value }`. The mock shares normalization between patches and seeded `MockSetup.settings`. No frontend store is needed until a UI consumes these commands. `/dev/settings` offers a synthetic session write and displays the loaded snapshot for native proof. Production builds exclude it, checked by `check:no-mock`.

## Data model and storage

Yjs is the source of truth (DATA-1, DATA-2). Each page body is one Y.Doc, and one workspace Y.Doc, `DocId::workspace()`, holds every page's record. Each database in v2 will be one Y.Doc with a Y.Map per row. `aelomi_core::storage::Store` keeps them.

SQLite is a derived index for search, backlinks, and page metadata, always rebuildable from the Y.Docs (DATA-5). See "The index". The tree the sidebar shows is read from the workspace doc itself, so it never waits for the index. Page content skips the index path, because BlockNote edits Yjs directly.

**Data folder.** `~/Library/Application Support/Aelomi`, or `Aelomi Dev` in debug builds, so `pnpm dev:app` never touches daily-use data. `AELOMI_DATA_DIR` replaces the path in every build. `check_data_path` checks it before opening: it resolves every symlink and refuses a path inside `~/Library/Mobile Documents` or `~/Library/CloudStorage` (SHELL-11). Debug builds also read `AELOMI_CLOUD_ROOTS`, a `:`-separated list that replaces those two roots, so a test can prove the refusal against a fake folder. The store opens the resolved path.

```text
Aelomi/
  format.json                       {"format": 1} until the writer's first .ylog2, then 2
  .lock                             exclusive lock while a Store has the folder open
  docs/<doc-id>/
    snapshot-<gen>.ysnap            the doc's full state when generation <gen> began
    snapshot-<gen>.ysnap2           the same state held twice
    log-<gen>-<seg>.ylog            updates a format-1 build appended during generation <gen>
    log-<gen>-<seg>.ylog2           updates appended during generation <gen>, in format 2
  quarantine/<doc-id>/              files that failed validation, never deleted
  index.sqlite                      the derived index, with its -wal and -shm sidecars; disposable
```

`<gen>` and `<seg>` are 10-digit zero-padded numbers. A `.ylog`, which only format-1 builds wrote, is the 8-byte magic `AELOG\0\0\x02`, a random 8-byte salt, then records of a `u32` length, a `u32` CRC-32 of the salt and the payload, and the payload, one Yjs v1 update. The salt stops a record framed inside a payload, in note text say, from passing for a real record, unless its author read the salt from the file. A `.ylog2` has no file header. Each update is one frame: a 20-byte head, the update packed 7 bytes at a time into bytes below 0x80, and a 20-byte tail. Every head and tail byte is 0x80 or above, and each of the two names the update's length and CRC-64/NVME under its own CRC-32. So no update's bytes can hold a frame boundary. Reading checks each run of bytes below 0x80 against the head before it and the tail after it, and takes an update only when its CRC-64 matches. Damage confined to a frame's head, or to its tail, loses nothing. Reading unpacks and hashes at most twice the file's length and never decodes. A frame planted in note text can load if a fault sets bit 7 on all 20 bytes of either anchor while keeping their other bits. Segment numbers are unique across both log formats, and the writer writes only `.ylog2`. A snapshot is a magic header, length, CRC-32 of the payload, and the doc's state as one update. A `.ysnap2` holds that state twice, between a 32-byte head at offset 0 and a 32-byte tail at the end of the file, each naming the length and a CRC-64/NVME of the payload. Copy A follows the head. Copy B starts one aligned 4 KiB block past the block where copy A ends, so no 4 KiB block holds bytes of both. Reading uses the head's length, ranges, and CRC-64 whenever the head passes its own check, and consults the tail only if the head fails. It takes the first copy, A then B, that passes that CRC-64 and never searches for an anchor inside the file. A `.ysnap2` that loads with an anchor, a copy, or its padding damaged counts as corrupt, as a damaged log does. Generation numbers are unique across both snapshot formats. The compactor writes a `.ysnap2` once the format gate has confirmed `format.json` says 2 in this store lifetime, which the writer's first new segment does, and creates its `.tmp` only through that `Format2Ready`. Before that, such as for a compaction a load queued, it writes a `.ysnap`, and it never touches `format.json` itself.

**Refusals.** `Store::open` returns `Locked` if another process holds `.lock`, `NewerFormat` if the `format` in `format.json` is newer than this build, whatever other keys it has (checked before anything is written), and `UnknownFormat` if there is data but no readable `format.json`. This build reads folders at format 1 and 2. Opening and loading never change `format.json`, so a format-1 build can still open a folder until this build's writer starts its first `.ylog2` there. From then on a format-1 build refuses the folder as `NewerFormat`, before it writes anything. Opening does no per-doc work.

**Write path.** `append` checks that the bytes decode, applies them to the doc in memory, queues them, and returns a `Seq` without waiting. One writer thread takes the queued updates of every doc whose file lock is free, appends each doc's records to its current segment with one write per file, plain-`fsync`s each touched file and each directory with a new entry, then calls `F_FULLFSYNC` once for the whole batch. Only then are the updates acknowledged. Every new segment is a `.ylog2`, and only a `Format2Ready` value from the format gate can create one. Before the first new segment of each store lifetime, the gate makes `format.json` durably say 2. If it says 1, the gate writes `format.json.tmp` with 2, after removing a stale one, and makes it durable with `F_FULLFSYNC`. `Fs::replace_format` renames it over `format.json`, then the gate syncs the data folder and calls `F_FULLFSYNC`. If `format.json` already says 2, the gate syncs it and the data folder and calls `F_FULLFSYNC`, because the rename can still sit in the drive's cache after a failed sync or after the process that made it exited. A gate failure fails only the docs that need a new segment, and they retry like any failed write. Later batches of the lifetime skip the gate's I/O. A load, rebuild, or compaction holds a doc's file lock while it reads or changes the doc's files. The batch leaves that doc out with its queue as it was, and releasing the lock wakes the writer, so the doc goes in the next batch. Each session, each write error, and each rebuild starts a new segment. So does a merged load that reads the open segment back torn or corrupt. The writer never appends after a torn tail or damage. `flush(deadline)` waits for everything appended before it, and first lets every failing doc retry at once, whatever its backoff. A failing doc left out for a busy lock keeps that retry until the lock frees. `close(deadline)` flushes, then releases the folder, or hands the store back with the docs still pending. Dropping a store is for process exit.

**Update decoding.** Before yrs allocates, an iterative walker checks each v1 count against the bytes remaining. It mirrors yrs 0.28.0, pinned in a compatibility test, and allocates nothing. It preserves overlong varints and trailing bytes. The store accepts at most 1,024 nested Any values. Decode measured about 350 stack bytes per level in release. Storage reserves 2 MiB of each 8 MiB stack for other frames and budgets 16 times that measured cost per level, then rounds the available depth down to a power of two. This also leaves headroom for debug builds. The compactor and writer have explicit 8 MiB stacks. Public `load` and `append` run their decode, apply, merge, and encode work on scoped 8 MiB threads, so Tauri's synchronous main-thread commands and other callers need no larger stack. Records already on disk that exceed the nesting limit are skipped and remain byte for byte in their files. Coverage refuses to retire those files. The payload-size limit stays at the format's `u32::MAX`; a lower cap could reject valid updates.

**Directory syncs.** A directory is synced whenever an entry in it may not be durable yet, not only right after this process created the entry, because an earlier attempt may have created it and failed to sync it. Every open syncs the data folder and its parent, and the format gate syncs the data folder again before each store lifetime's first `.ylog2`. A doc's first successful batch each session syncs `docs/`. Any sync of a doc's folder first syncs its quarantine folders, so a quarantined file's removal never becomes durable before its new entry.

**Failures.** A doc whose write or sync fails is left out of the batch's full sync, so other docs still acknowledge. Its updates stay queued and retry into a new segment after 100 ms, doubling to 5 s. `health()` and `subscribe_health` report `Failing { reason, since, pending_docs }` until they land. The writer remembers each segment a failed write tore this session, and how many of its bytes were acknowledged before the failure. A panic during a writer step fails every doc with queued updates for the reason `Internal`, and they retry the same way. Each subscriber call is isolated, so a panicking subscriber stops neither the writer nor the other subscribers.

**Compaction.** When a doc's unsnapshotted logs pass 1 MB or 10,000 records, the compactor thread rotates new appends to generation N + 1, writes `snapshot-(N+1).ysnap2.tmp`, or a `.ysnap.tmp` before the format gate is ready, reads it back and verifies it, and renames it into place. Only then does it retire older files. No file leaves the doc's folder unless the new snapshot holds every block ID and deletion in every record the file yields. A file with changes the snapshot lacks stays, whether it reads clean, torn, or corrupt. If merging it back would change the doc, the doc goes stale at once, so the next `load` or `append` rebuilds it with that file. A held file that reads torn or corrupt, or that a load or rebuild found corrupt, moves to `quarantine/`, with two exceptions that hold no acknowledged bytes and are deleted. One is a `.ylog` shorter than its 16-byte header whose magic bytes match the expected prefix. The other is a log this session's failed write tore, when the bytes acknowledged before the failure read clean. A `.ylog2` has no header, so only the second applies to it. An empty `.ylog2` reads clean. So a disk that fails for hours adds nothing to `quarantine/` per retry, while a log torn in an earlier session still moves there. Otherwise only a clean held file, or a leftover `.tmp`, is deleted. A doc rebuilt while the compaction ran goes stale when it ends, since the rebuild can lack what the new snapshot holds. A compaction asked for after the running one rotated queues the doc once more when it ends. A snapshot over the payload limit skips compaction: before rotating for a doc without pending structs, and after it for a merge. A panic abandons that doc's compaction, leaves its files, and returns it to idle.

**Pending structs.** yrs keeps an update whose dependencies are missing as pending structs. Its full-state encoding of such a doc can fail to apply, renumber them, or load with different operations, and it can leave them pending even after every dependency arrives. So `load` of a doc with pending structs, and its snapshot, merge every update the store holds for it instead: each valid snapshot, each log's records, then the queue, read under the doc's file lock so none moves between the two. yrs's merge re-sorts every input at each step, so it merges 32 at a time: about 7 ms for a 10,000-record, 1 MB doc, against about 1.5 ms to encode one from memory. A doc without pending structs keeps that faster path.

**Verification.** A doc's operations are the ID of every block, deleted or not, and every deletion, pending structs included. They come from yrs's plain encoding of the integrated blocks plus its pending update, because a client's blocks can start past its state vector. A doc can hold more deletions than its updates carry, since yrs deletes what it integrates into a deleted parent. yrs's encoding of a doc without pending structs verifies when it holds exactly the doc's operations at rotation, loads into a doc that holds exactly those too, and matches its fingerprint: state vector, delete set, and the structure of every root and nested type, read as text runs with their marks, sequence values, and map entries or XML attributes, with each nested type's kind and ID. A type that holds text is read only as text runs, which leave out values, binaries and subdocs and show a string embed as text. So its runs must spell its text and account for its whole length; otherwise the doc has no fingerprint, and compaction skips it and keeps its logs. Attribute and mark order never matters, and no type is compared through a serialized string. A merge verifies when it holds exactly the operations of the updates in it and loads into a doc that holds all of them, and perhaps more deletions as structs left pending integrate. Compaction reads the merge after rotating, so it holds at least what the doc held then. A snapshot that fails verification is removed, and the doc tries again after another limit's worth of logs. Nothing compares encoded bytes.

**Loading.** A doc's first `load` or `append` applies every valid snapshot and every log. It reads them under the doc's file lock alone, not its state lock, which every append and every batch take. A batch leaves out a doc whose file lock is busy, so a slow read holds up no other doc's acknowledgement. A rebuild reads the same way. Later loads encode the doc in memory, unless it has pending structs. Yjs updates are idempotent and commutative, so applying more is safe. A log ends cleanly, with a torn tail, or with corruption. A complete log header with damaged magic still uses its salt to check records, and the file counts as corrupt. If the header's salt yields no records, reading walks the records by their unsalted lengths and solves the CRC state after the salt from each stored CRC and payload. A replacement state needs agreement from at least two records. Every record loaded with a recovered state, or after magic damage, must pass its salted CRC and decode as an update. The file counts as corrupt, so compaction preserves its bytes in quarantine after a verified snapshot covers the recovered records. A damaged salt in a one-record log cannot be recovered in format 1. A record that fails where one should start first gets its length recovered if possible. Reading hashes forward from its payload and takes the first length whose payload matches the stored salted CRC, decodes as exactly one update with no bytes left over, and ends at the end of the file or at a record that passes. Only an update's true length decodes exactly, so a recovered record never covers the records after it. It loads, and its file counts as corrupt, never torn. Each reading pass gives length recovery a budget of 64 bytes of hashing and decoding per byte of the file, then stops it for the rest of that pass, so a payload that knows the salt can't force a pass, or a CRC match, every few bytes. Otherwise reading looks at where the bad record's length says it ends. If that is the end of the file or a record that passes, and the payload up to it decodes as exactly one update, only the record's CRC or payload is damaged. Reading resumes there without looking inside, where only a frame planted in note text could pass. If the payload doesn't decode exactly, reading resumes at the first offset inside whose record passes its salted CRC, decodes as an update, and starts a chain of passing records that ends exactly where the bad record says it ends, or there if no offset does. After any other bad record, even one whose length runs past the end of the file, reading resumes at the next offset whose record passes its salted CRC and decodes as an update. So every valid record loads, before and after the damage. A tail is torn only when no valid record follows the first bad one, and that record runs to or past the end of the file or only zeros follow it. Anything else is corruption. Neither file is appended to again. Neither is deleted outside the two exceptions under "Compaction", since a flipped bit in the CRC or payload of an acknowledged last record also reads as torn. A scan never hashes the payload a length claims. It keeps the CRC of every 64th prefix of the file and combines two prefixes to get a range's salted CRC, so a candidate costs about the same whatever length it claims, and note text can neither slow a scan down nor stop it. Each reading pass lets a scan decode at most 64 bytes per byte of the file, which only a payload that knows the salt can spend. A file it can't afford to decode past counts as corrupt, never torn. A `.ylog2` reads torn when its intact frames are followed by what a torn write could leave of one more frame, then nothing or only zeros. Reading accepts three shapes there. One is fewer than 20 bytes of 0x80 or above. Another is a head that passes its check, followed by no more bytes below 0x80 than its body holds and, after a whole body, fewer than 20 bytes of 0x80 or above. The last is a body that passes its CRC-64, followed by fewer than 20 bytes that start the tail its head implies. Anything else is corruption. Damage can leave the same bytes as such a tear, such as zeros over the end of the last tail or bit 7 cleared on a final tail byte of 0x80. That file reads torn too, its update still loads, and like a torn `.ylog` it is deleted only under the exceptions in "Compaction". Files below the newest valid snapshot, a damaged newest snapshot, or a leftover `.tmp` make that load compact again. An update that decodes but fails to apply leaves the doc stale: compaction skips it, and the next `load` or `append` rebuilds it from its files and queue. A panic doesn't stop storage: every lock recovers from poisoning, and a doc whose state lock was poisoned goes stale too, since yrs may have half-applied an update. A rebuild or a merged load keeps any corruption it finds for quarantine. A rebuild queues the compaction again if the logs are over the limit or a compaction was interrupted.

**Testing seams.** `Store` runs over the `Fs` and `Clock` traits. The `test-support` feature exports `SimDisk`, a `SimFs` on the system clock, for tests outside the crate. Only `src-tauri`'s dev-dependencies turn it on, so it never reaches a release build. `OsFs` and the system clock are production. `SimFs` models written, drive-cache, and durable layers, crashes with torn prefixes, injects errors, panics, damaged writes, and latency, holds an operation until the test releases it, and checks the append-only rules on every operation. It reads each log with the parser its name selects. It allows a delete only of a `.tmp`, or of a file a durable newer snapshot holds that is clean, cut off inside a log header, or torn by a write that failed this session after bytes that read clean. It allows a move out of a doc's folder only of a file a durable newer snapshot holds. Plain rename refuses an existing entry; `Fs::replace(root, RootFile::Format)` can replace `format.json`, from a complete durable `format.json.tmp` marker. Creating a `.ylog2`, `.ysnap2`, or `.ysnap2.tmp` requires the folder's durable `format.json` to say 2. With `Driver::Manual` a test steps the writer and compactor itself.

## Workspace records

The workspace doc, `DocId::workspace()`, has one root, the Y.Map `pages` (DATA-2). Each key is a page ID and each value is that page's record, a nested Y.Map. The key is the record's ID, so no field repeats it. A `PageId` is a `DocId` other than `workspace`, so a page's body doc is the doc with the same ID.

| Key | Value |
|---|---|
| `title` | A string. Empty means untitled; the UI shows "Untitled" (TREE-7) |
| `placement` | An object `{ parentId: string or null, order: integer }`. One value, so parent and order change together |
| `favorite` | `null` or absent when not a favorite, else an object `{ order: integer }` |
| `trash` | `null` or absent when active, else `{ originalParentId: string or null, originalOrder: integer, batchId: string, trashedAt: integer }`. One value, so a trash or restore is whole. `placement` keeps its value while trashed |
| `createdAt`, `updatedAt` | Integers, milliseconds since the Unix epoch |

Every integer is a whole number from 0 to 2^53 - 1, the range a JavaScript number holds exactly. Yjs stores one as an integer or a float by size, and both read. The coupled fields are plain Any objects rather than nested Y.Maps, so concurrent edits can't leave half a placement or half a trash record. Readers ignore keys and object fields they don't know, and the writers in `fixture.rs` keep them, so fields can be added without a migration. A record that breaks the table fails with a `RecordError` naming the page and the field, never the title. A missing or empty `pages` root has no records. A `pages` root that holds sequence content, or a populated root of another shared type, fails the whole workspace with a `RecordError`, so the index never reads it as a workspace without pages and never replaces its cache for it. Readers never create a root or write a default back.

`tree::forest` decides each page's parent and whether it is active. A parent with no record makes its page top-level, and a parent cycle is cut at its smallest page ID, so no page disappears. A page is active when it and every ancestor have no `trash`, so a descendant left active under a trashed page is still hidden from the tree, favorites, search, and backlinks (TREE-9). The trash list holds each trashed page whose parent is not in its batch, newest first (TREE-10). An erased page has no record (TREE-14). The index no longer reads that body doc, even if its files remain.

## Page bodies

A page's body is the Y.XmlFragment root `prosemirror` of its doc, as `@blocknote/core/yjs` 0.55.0 (MPL-2.0) writes it through y-prosemirror 1.3.7 (MIT). The editor must bind that same fragment. The layout is ProseMirror's document without the `doc` node:

```text
Y.XmlFragment "prosemirror"
  blockGroup
    blockContainer  id="<block ID>"
      <content node>  paragraph, heading, codeBlock, image, …, with its props as attributes
        Y.XmlText  text runs; marks are formatting attributes
        pageLink  targetPageId="<page ID>"   an empty custom inline node (LINK-1)
        hardBreak
      blockGroup    optional, the block's children
```

`body::read_body` returns `Body { blocks, problem }` and walks with an explicit stack, parents before children. A block has a typed ID and text or page-link segments. Marks do not affect index text. Hard breaks become `\n`, code keeps its newlines, images contribute captions, and dividers contribute none. URL links keep their text and do not become backlinks. Literal `[[…]]` remains text. Unknown elements contribute contained text, while attributes remain metadata.

A missing block ID skips that block and still reads its children. An invalid page link contributes a space, so its neighbouring words cannot become a false token. Content under another root is diagnosed when the supported body is empty. The index salvages readable blocks and reports `PageIssue { page_id, problem, indexed_blocks }`. A `prosemirror` root, or an element under it, that holds text, values or map entries instead of XML reports `NotAFragment`, whether the doc typed the root or decoded it from an update. A stray value never hides the nodes after it. Problem precedence is undecodable update, unrecognized root, non-XML content, missing block ID, then invalid page link. Unknown additive metadata does not erase a supported projection.

`fixtures/index/blocknote-body.yupdate` was written by the actual binding. Rust readers and frontend body fixtures check its IDs, text and links. Before mounting BlockNote, the frontend checks that its schema can preserve every stored node, mark and attribute. Unsupported bodies open read-only. Populated canonical docs open without appends. A pre-input DeleteSet guard refuses destructive mount updates. Editing sends raw Yjs deltas and never recreates a doc from editor JSON. The page-link inline node stores only `targetPageId`; displayed labels remain derived.

The editor starts source delivery inside the input event. It adds no frame or timer before a Yjs update. The version-pinned `prosemirror-view` patch changes only `DecorationSet.forChild` lookup over sorted child triples from a linear scan to bisection. The `y-prosemirror` patch preserves stored attributes while retaining upstream selection and undo behavior on the binding oracle fixtures. A faster post-frame model-read experiment was rejected because it widened the interval before source delivery. The chosen immediate path retains synchronous stored-body assertions. Unknown mark keys, including null-valued keys that ProseMirror would discard, make the body read-only before binding.

## The index

`aelomi_core::index` keeps `index.sqlite` in the resolved local data folder, beside `format.json` and `docs/`. It uses rusqlite 0.40.2, bundled SQLite 3.53.2 and FTS5. SQLite holds only projections rebuildable from Y.Docs. The schema version is `PRAGMA user_version = 2`, independent of the source storage format. An exact schema comparison and a version-pinned CRC-32 guard detect schema drift.

**Schema.** `pages` caches DATA-2 fields and normalized active membership. Each page has a private ordered integer key. `blocks` stores an occurrence ID, ordinal and strictly decoded segments. `title_text` and `block_text` contain active pages only. Their FTS tables use `unicode61 remove_diacritics 2` and prefix lengths 1, 2 and 3. Guarded triggers update FTS only when text changes. `links` associates target page IDs with block occurrences. Duplicate block IDs still receive separate occurrences.

Page keys ascend by `(created_at, reverse page_id)`. Descending FTS row IDs therefore yield newest-created pages first, with page ID ascending for ties. A block row ID combines its page key and a 20-bit occurrence slot. Inserting pages allocates gaps without re-keying existing rows. Exhausted keys or slots, changed creation times, and invalid stored key order replace the cache.

**Ownership.** `Workspace` owns Store and Index. An append goes to Store first, then advances the tree revision for a workspace edit and takes one short index mark. Rejected appends mark nothing. The index adds no decoding, SQL or callback to that path. `load` and `flush` do not wait for SQLite. Index Source exposes only `load`. The source-backed tree is memoized as `Arc<Tree>` against the accepted workspace revision. Projection holds no memo lock, and generation checks prevent an older tree from replacing a newer one. A caller whose revision is already being projected waits for that projection instead of repeating it; a guard releases the waiters however the projection ends. The projection works on record positions in page ID order.

**Startup and rebuild.** One `aelomi-index` thread owns the only writer. Queries use an independent reader off the main thread. Opening Workspace returns while its index is Rebuilding. The worker:

1. Validates schema version, exact schema, SQLite integrity and both FTS integrity checks. Missing, garbage, corrupt, older or newer-schema caches are disposable.
2. Removes exactly `index.sqlite`, `-wal`, `-shm` and `-journal` when replacement is required, main file first. Leftover sidecars are removed before opening a missing main file. Symlinks are removed as entries and SQLite uses NOFOLLOW. Directories at those names fail instead of being removed.
3. Reads workspace records and every page body from Source, even when the existing cache passed validation. This repairs either side of a source/cache crash divergence. Unchanged rows are not rewritten.
4. Applies marks accepted during reconciliation, publishes per-page issues and Ready, then emits content-free startup timings.

The tree and raw docs remain usable throughout. A failed worker spawn emits Failed once from the opening thread, the exception to worker-thread index events. Store, tree, append and flush remain usable.

**Incremental updates.** A workspace mark runs at once. Body work runs 25 ms after its first unindexed edit. Later edits do not extend the delay. The worker takes marks before source reads, handles metadata before bodies, and commits up to 128 pages per transaction. Marks arriving during load or commit remain for the next pass. Source reads, parsing and SQL stay on the worker. Only changed block projections are written. Each completed batch emits `IndexChanged { workspace, pages }`.

**Links and erasure.** Link text uses the current target title, Untitled for an empty title, and Deleted page when its record is absent. Renames rerender incoming blocks without editing their docs. If any cached page record disappears, the worker replaces the whole cache before writing that workspace pass. No writer removes a `pages` row while retaining the cache file. This membership signal survives interruption without an erase flag. Rebuilding preserves target IDs and removes old title, body and inbound-label bytes from the four disposable files. Tests scan raw, folded and inner marker bytes plus FTS vocabulary through interruption and restart, with source bytes unchanged after the authorized fixture removal.

This completes the derived-cache prerequisite for TREE-14. Source-doc deletion, source-history cleanup, quarantine and assets belong to permanent erase. Previously written exports remain unchanged under SPEC. The future erase owner removes a batch's workspace records in one append before deleting its page-doc files.

**Queries.** `search(&SearchQuery { text, limit, scope })` and backlinks use one read transaction. Freshness checks before and after a query reject pending workspace changes or cache repair as `Reply::Indexing`. Body edits may lag by the coalescing delay. Failed status returns its typed reason. Ready replies carry results and every active page's issue, sorted by page ID, including empty queries.

All scope returns title matches first, then body matches. Content scope excludes up to 200 distinct page IDs before counting its limit. Both tiers use the fixed page order. Content selects the matching block with the lowest document ordinal. All title-tier hits have no block. The last whitespace phrase's last token is a prefix, using the escaped FTS expression. A row-ID cursor closes each selected page group before stopping, with no BM25 scoring, SQL sorter or arbitrary candidate cap.

Only selected texts are highlighted. `tokenizer.rs` borrows SQLite's unicode61 tokenizer from the reader connection, validates token offsets, catches callback unwinding and drops handles on that thread. `highlight.rs` returns original text parts with safe highlighted ranges. It creates neither HTML nor marker strings. Tests compare complete parts against SQLite highlight on Unicode, punctuation, overlapping terms and the large-page fixture.

**Failures.** Malformed page bodies produce explicit partial-result issues while healthy pages remain queryable. Repair clears the issue. A malformed workspace is Failed because active membership is unknown. Invalid stored IDs, ordinals, keys or segments and database corruption request in-session cache replacement. A stored field that cannot decode into its Rust type, including invalid UTF-8, also counts as corrupt cache data. Queries answer Indexing during repair. Damage repairs share one budget across Ready transitions. The first runs at once. Damage that recurs before the index has stayed Ready for 30 s publishes Failed, naming repeated repair and the problem, and waits 100 ms, doubling to 30 s, before the next repair. Queries answer Failed during the wait and the repair after it, and appends and flushes never wait on either. A cache outdated by the source or another build, such as after an erasure, is replaced at once and leaves the budget unchanged. A workspace change ends the wait at once, because it may erase pages whose bytes only the repair removes from the cache. Body edits do not end it. File, source-load or worker failures publish Failed and retry with bounded backoff. No index path changes source files. Listener panics are isolated.

**Verification and budgets.** The index tests cover exact rebuild equality, active membership, both literal parent-cycle cases, a source-doc backlink oracle with random moves, typed failures, spawn errors, scrub crash points, tree memo races and ordered search goldens. `scripts/check-search.ts` gates actual public release queries at 5 ms p95 across fixed, common and broad-prefix inputs, limits 20 and 200, exclusions, cold readers, and the 10k fixture with an additional 5,000-block page. It records matched cardinality, rows walked, consumed output, profile and load. CI and check:all also run both LINK-8 release tests serially. The debug progress test names `HELD_EDITS = 2` and retains timeout and producer-panic checks. Earlier schema-1 measurements remain in `docs/evidence/data-5/` as historical evidence. The [populated native record](docs/evidence/data-5-followups-9d7a9d4/README.md) measures rebuild, startup and memory. The [final native record](docs/evidence/data-5-final-native-9d7a9d4/README.md) proves visible indexing, automatic results and immediate edit survival through every quit route, and records the accepted 618 ms startup baseline.

## Storage in the shell

`open_storage` in `src-tauri/src/lib.rs` runs in Tauri's `setup` hook, on the main thread, after the window exists. It resolves the data folder, opens a `Workspace` on the resolved path, and prints one line to stdout, such as `storage status=Ready path=/…/Aelomi`. SQLite work runs on the index thread. The earlier store-only implementation measured 97 ms from Tauri's `setup` to page load start on an Apple M1, both before and after storage was added. The cold-start harness checks the complete app with the index enabled. The app manages a `Storage`, which holds the workspace or the `StorageStatus` that says why it did not open: `CloudPath { resolved }`, `Locked`, `NewerFormat`, `UnknownFormat`, or `Io`. The app starts either way.

A doc call goes from the UI to the store like `ping` does:

1. The caller encodes the update with `toBase64` from `src/ipc/base64.ts` and calls `commands.docAppend(docId, update)`.
2. Synchronous `doc_append` decodes it, checks the doc ID and calls `Workspace::append`. It returns the accepted sequence before durability. Every local Yjs update invokes it immediately.
3. Async `doc_load` performs Store loading and base64 encoding on the blocking pool. `fromBase64` gives the editor the raw update.

Both return `Unavailable` when the store did not open. `storage_status` returns the `StorageStatus`, and `storage_health` returns `Health`. Each change of health also arrives as the `StorageHealthChanged` event, from `Store::subscribe_health`.

**The index in the shell** (SHELL-12). `index_status` returns `Ready`, `Rebuilding`, or `Failed { reason }`, and each change arrives as `IndexStatusChanged`. The webview subscribes before reading its startup status and discards snapshots superseded by events. `IndexChanged { workspace, pages }` follows each batch the worker commits after `Ready`, so tree, search, and backlink views know to read again. `page_tree` returns active pages depth-first with their parents, favorites, and the trash list, read from the workspace doc, so the tree loads while the index rebuilds. `search(query, limit, scope)` returns `"Indexing"` or `{ Ready: { hits, issues } }`. `backlinks(pageId)` returns the same state with `{ backlinks, issues }`. The three are `async` and run on Tauri's blocking pool, so the main thread never waits on a doc load or SQLite. Errors are typed as Unavailable, InvalidPageId, InvalidQuery, Failed or Io. Corruption being repaired answers Indexing. Times cross IPC as numbers of milliseconds. The shell prints `index cache=created`, `reused`, or `replaced reason=…`, and `index status=…` on each change. When storage did not open, `index_status` is `Failed` with the reason "the data folder is not open". The palette shows Indexing… for Rebuilding or an Indexing query reply, reruns on Ready and committed index changes, and shows a quiet partial-index notice for page issues.

**Bytes cross IPC as base64.** The storage probe measured each encoding over real IPC on an Apple M1, with release Rust and the dev frontend: the median of 20 loads of a 1 MB doc and of 50 appends of a 99-byte update, at WebKit's 1 ms timer resolution.

| Encoding | `doc_load`, 1 MB | `doc_append`, 99 bytes |
|---|---|---|
| JSON array of numbers, tauri-specta's default for `Vec<u8>` | 18 ms | under 1 ms |
| Base64 string | 6 ms | under 1 ms |
| Raw IPC: a `tauri::ipc::Response`, and a raw request body | 2 ms | under 1 ms |

Appends, the keystroke path, cost the same in every encoding. Base64 loads 3 times faster than numbers and stays inside the generated bindings. Raw IPC is faster again, but specta has no type for `tauri::ipc::Response`, so the command would need a hand-written invoke outside `commands::builder()`. Revisit that when a page load shows up in a budget.

**Quitting drains and flushes** (SHELL-9). `QuitStatus { revision, phase }` has Open, Draining, Asking with pending docs, and Confirmed phases. The webview installs its quit listener, reads the status and calls `frontend_ready` before editing is enabled. A registered frontend's normal quit first enters Draining and refuses native termination without blocking the main thread. The frontend finishes pending input, freezes editing, retries refused updates and awaits source delivery. `complete_quit(revision)` then flushes documents and settings on the blocking pool with the existing 2 s deadline. Success confirms and exits. Delivery or flush failure asks the user.

Repeated requests during a drain retain its revision. Stale acknowledgements do nothing. A 5 s drain timeout asks rather than assuming undelivered edits were saved. Its pending list can be empty because the webview may hold edits Store never received. Keep app open answers only the current question and unfreezes editing. Quit anyway confirms once and explicitly allows exit. Before frontend registration, the existing Store-only flush behavior remains. All native routes share the gate. Final Exit still flushes accepted updates. A dead web content process cannot acknowledge a drain, so a normal quit remains refused and force quit remains available.

| Quit path | Handlers it reaches, in order | Can stay open |
|---|---|---|
| ⌘Q, or **Quit Aelomi** in the app menu | `ExitRequested`, `Exit` | Yes |
| ⌘W, or the window's close button | `CloseRequested`, `ExitRequested`, `Exit` | Yes |
| Logout, **Quit** in the Dock, or AppleScript `quit` | `applicationShouldTerminate:`, `Exit` | Yes |
| `perf_report` calling `app.exit(0)` | `ExitRequested`, `Exit` | Yes |

Logout, the Dock, and AppleScript send the quit Apple event, which AppKit turns into `terminate:`. tao's app delegate has no `applicationShouldTerminate:`, so `terminate:` would go straight to `RunEvent::Exit`. `macos.rs` adds that method to the delegate's class at setup, and it runs the same flush (`via=Terminate`). With updates pending it answers `NSTerminateCancel`, which also cancels a logout. Confirming **Quit anyway** exits Aelomi but does not resume the cancelled logout. Before frontend registration it flushes on the main thread. After registration it refuses the termination while the shared drain completes. `NSTerminateLater` would free the thread, but while AppKit waits for the reply it handles no input and drops key presses. The logout row comes from sending the quit event, with the logout reason, to the app's PID, not from a real logout. The default menu's Quit item also sends `terminate:`. `macos.rs` still replaces it with an item that calls `app.exit(0)`.

`on_run_event` in `lib.rs` handles `CloseRequested`, `ExitRequested`, and `Exit`, and is generic over the runtime. Its tests run Tauri's `MockRuntime` over a `SimDisk` whose appends fail, and check that the window stays open and `QuitStatusChanged` names the pending doc. A refusal with no listener stays in the gate after health recovers, until `keep_open` answers it and publishes the answer. The mock runtime calls only the run callback, never window listeners, so `CloseRequested` is handled there too.

`MockRuntime::request_exit` is unimplemented. Command tests run the same confirmation body with only the exit operation replaced. They verify continuing failures, all holds after confirmation, duplicate confirmation, and the final Exit flush. Recovery tests crash and reopen `SimDisk` and assert the exact text, including an append between confirmation and Exit. AppKit tests verify `TerminateCancel` before confirmation and `TerminateNow` afterward.

