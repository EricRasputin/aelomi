# Aelomi architecture

This file maps the code. `CLAUDE.md` has the rules and the stack, `SPEC.md` has the behaviors, and `README.md` has the commands to run, build, and test.

## Find a file

| Path | Contents |
|---|---|
| `Cargo.toml` | Cargo workspace with members `crates/core` and `src-tauri`, edition 2024 |
| `crates/core/` | `aelomi-core`, the Rust core |
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
| `crates/core/src/index/` | The SQLite index (DATA-5): `mod.rs` holds the API and the marks, `worker.rs` the only writer, `db.rs` the schema, startup check, and projection writes, `query.rs` search and backlinks |
| `crates/core/src/index/tests/` | Index tests: rebuild equality, races, failures, latency, the backlink property test, and the 10,000-page search fixture |
| `crates/core/examples/seed_index.rs` | Seeds a new data folder with the index fixture and a missing, garbage, old-schema, or valid index, for native checks |
| `src-tauri/` | `aelomi`, the Tauri 2 shell (library name `aelomi_lib`) |
| `src-tauri/src/lib.rs` | `run()` builds the Tauri app, registers the commands and events, opens the workspace, logs and emits index events, and flushes the store before quitting |
| `src-tauri/src/commands.rs` | Tauri commands, the tauri-specta `builder()`, and the `bindings_are_current` test |
| `src-tauri/src/storage.rs` | `Storage`, the open workspace or why it did not open, the data folder overrides, the flush before quitting, and the storage IPC types |
| `src-tauri/src/quit.rs` | `QuitGate`, the quit decision `Storage` holds, and its `QuitStatus` and `QuitStatusChanged` IPC types |
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
| `src/perf/` | The cold-start mark and the mock-only keystroke surfaces |
| `src/styles.css` | Tailwind v4 import and the light and dark color variables |
| `e2e/` | TesterArmy acceptance tests and their guarded fixture |
| `tests/playwright/` | Playwright WebKit tests and their guarded fixture |
| `perf/` | Perf harnesses and `budgets.json`, the numbers every perf gate reads |
| `fixtures/` | The seeded fixture generator and its output in `fixtures/generated/` |
| `fixtures/data-folders/` | A data folder per storage format, for migration tests |
| `fixtures/index/` | A page body the real BlockNote binding wrote, its generator, and its provenance |
| `tests/is-local-url.ts` | The loopback allow list both browser suites share |
| `.github/workflows/ci.yml` | Every CI job |
| `scripts/check-all.ts` | `pnpm check:all`, which runs every CI job locally on a clean committed head and records a pass in `.local-gate/` |
| `scripts/check-all-stages.ts` | The stage list `check:all` runs, one per CI job; `check-all.test.ts` fails when its jobs or commands drift from `ci.yml` |
| `scripts/check-licenses.mjs` | Fails on an npm license outside the allow list |
| `scripts/check-no-mock.mjs` | Fails if the IPC mock or a dev route is in the production bundle in `dist/` |
| `scripts/protect-main.sh` | Makes each CI job a required check on `main` |
| `scripts/tauri-dev.ts` | `pnpm dev:app`, which runs `tauri dev` with the UI on `AELOMI_DEV_PORT` or a free port |
| `scripts/vendor-skills.sh` | Vendors the pinned pstack and Emil skills into `.claude/`, and fetches the gitignored pen.dev skill. It unhides the pstack skills `how` and `why` for agents and records that change in `.claude/vendor/pstack/PATCHES` |
| `deny.toml` | cargo-deny policy: Rust licenses, advisories, and sources |
| `vite.config.ts` | Vite config. Mock builds go to `dist-mock/`, production builds to `dist/` |
| `tsconfig.json` | Strict TypeScript settings for `src/`, `e2e/`, and `tests/` |
| `biome.json` | Lint and format rules, including the test import restrictions |
| `.claude/agents/reviewer.md` | The reviewer agent that gates each PR, with a local mode for branches not yet pushed |
| `.claude/skills/verify-aelomi/` | The skill agents use to launch, drive, and screenshot the app to prove a change works, with its feature map |

## Rust workspace

The workspace has two crates. Both use the version, edition, and license from `[workspace.package]` in `Cargo.toml`.

`crates/core` is the package `aelomi-core`. `CLAUDE.md` assigns storage, indexing, search, markdown export, the MCP server, and later the sync client to it. Today it holds the storage engine, `aelomi_core::storage`, the source model and its owner, `aelomi_core::workspace`, the SQLite index, `aelomi_core::index`, and the constant `VERSION`. It depends on no Tauri crate, so its tests need no window.

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

`src/main.tsx` connects the storage event listeners before rendering `RouterProvider` with the router from `src/router.tsx`, inside `StrictMode`. HMR disposes the connection. `src/stores/storage-events.ts` registers health and quit listeners independently, then reads `storageHealth()` and `quitStatus()` for startup and reconnects. A health event received by that connection takes precedence over its health snapshot. Quit events, the quit snapshot, and Keep app open replies all carry a revision, and the quit store applies only a newer one. Disposal ignores late snapshots and unregisters late listeners. A failed snapshot leaves successful subscriptions active.

`src/router.tsx` builds the route tree in code with `createRootRoute` and `createRoute`. There is no file-based route generation. The root layout wraps every screen in Base UI `Tooltip.Provider`. `src/components/storage-notices.tsx` puts a non-dismissable save-failure banner below the header and a Base UI `AlertDialog` above every route. The safe **Keep app open** button receives focus. Escape chooses it, and outside clicks leave the dialog open. Both buttons disable while the quit command is pending. The dialog opens instantly because native keyboard shortcuts can trigger it. `src/storage-copy.ts` holds the exact warning and reason strings. The app route is `/`, which renders `IndexScreen`. Mock builds add `/perf/keystroke/empty` and `/perf/keystroke/page-5000-blocks` for the keystroke gate. Dev builds (`import.meta.env.DEV`) add `/dev/storage`, the storage probe, and a **Storage probe** link to it on the index screen.

`src/stores/` holds the Zustand stores. Components read stores through selectors, such as `usePingStore((state) => state.status)`.

`storage-health.ts` tracks the latest `Health`. `quit.ts` tracks the independent quit decision as `open`, `asking`, or `quitting`, each with the revision of the newest Rust status it applied. Health recovery clears the banner without dismissing a quit decision. **Keep app open** closes the dialog before IPC and sends the revision it answers. Confirmation changes the decision to `quitting` before IPC, so repeated events and confirmations cannot submit another quit, and a newer status keeps it `quitting`. A rejected Keep app open restores the question with an error only if no newer status arrived. A rejected quit command restores choices with an error and the latest pending docs.

`src/ipc/` holds the IPC layer:

- `bindings.ts` is the generated tauri-specta client.
- `mock.ts` is the in-memory fake of every command and event. Mock builds expose `window.__aelomiMock`, which has a typed `calls` log and `reset()`. Its storage controls include `failWrites(on, reason)`, `setStatus(status)`, `updates(docId)`, and `text(docId)`. `type(docId, text)` builds a Yjs append through `docLoad` and `docAppend`. Its quit controls include `request()`, which asks through a copy of the Rust quit gate for pending docs, and `exited`, which records an allowed exit. `quitStatus` reads the gate. `keepOpen` and `quitAnyway` change it synchronously and emit `QuitStatusChanged` as Rust does. `quitAnyway()` preserves pending updates and document data. The SHELL-10 acceptance test appends through this mock because the page editor is not built yet.
- `index.ts` picks one of the two. In `mock` mode (`import.meta.env.MODE === "mock"`) it calls `installMock()`. Otherwise it exports the generated commands and events.
- `base64.ts` encodes doc bytes for `doc_load` and `doc_append`, with a Vitest test beside it.
- `format-ping.ts` is a pure formatter with a Vitest test beside it.

`pnpm build:mock` builds with `--mode mock` into `dist-mock/`, which both browser suites serve with `pnpm preview:mock`. `pnpm build` builds the production bundle into `dist/`, where Vite drops the mock and the dev routes. `pnpm check:no-mock` fails if `__aelomiMock` or the storage probe's marker, `aelomi-dev-storage-probe`, appears in `dist/`.

## Tests by layer

| Layer | Tool | Location | CI job |
|---|---|---|---|
| Pure TypeScript logic | Vitest | `*.test.ts` in `src/`, `tests/`, `perf/`, `fixtures/`, and `scripts/`, configured in `vitest.config.ts` | `vitest` |
| Rust core and shell | cargo-nextest | `#[cfg(test)]` modules in each crate | `nextest` |
| Acceptance | TesterArmy `e2e` | `e2e/*.e2e.ts`, configured in `e2e.config.ts` | `e2e` |
| WebKit flows and the network check | Playwright, `webkit` project | `tests/playwright/*.spec.ts`, configured in `playwright.config.ts` | `playwright` |
| Keystroke-to-paint | Playwright, `perf` project | `tests/playwright/perf/` | `keystroke` |
| Cold start | Node harness on the built `.app` | `perf/cold-start.ts` | `cold-start` |

`.config/nextest.toml` terminates tests after 60 s and gives the ignored storage measurements 10 minutes.

Acceptance tests import `test` from `e2e/fixtures.ts`, and Playwright tests import `test` and `expect` from `tests/playwright/fixtures.ts`. Both fixtures fail a test that makes a non-local request (SHELL-13). Biome rejects a test file that imports `test` from the library directly. There are no React component unit tests. `e2e` covers components.

## CI jobs

`.github/workflows/ci.yml` runs on every pull request and on pushes to `main`. Every `ci.yml` job runs on `macos-15`.

| Job | Runs |
|---|---|
| `biome` | `pnpm lint` |
| `tsc` | `pnpm typecheck` |
| `vitest` | `pnpm test` |
| `nextest` | `cargo nextest run --workspace`, including `bindings_are_current` |
| `cargo-deny` | `cargo deny check` |
| `pnpm-audit` | `pnpm audit --audit-level moderate` and `pnpm check-licenses` |
| `build-app` | `pnpm tauri build --bundles app` and `pnpm check:no-mock`, then uploads the zipped `.app` as the `Aelomi-app` artifact |
| `e2e` | `pnpm build:mock` and `pnpm test:e2e` |
| `playwright` | `pnpm build:mock` and `pnpm test:playwright` |
| `cold-start` | Downloads `Aelomi-app` and runs `perf/cold-start.ts --ci` |
| `keystroke` | `pnpm build:mock` and the `perf` Playwright project in CI mode |

To add a job, add its ID to `CHECKS` in `scripts/protect-main.sh` and rerun the script, so `main` requires it. Add a matching stage to `scripts/check-all-stages.ts` so `pnpm check:all` runs it too.

## Performance gates

`CLAUDE.md` sets the budgets, and `SPEC.md` ("Performance budgets") defines how each is measured. Every number the gates read lives in `perf/budgets.json`, each with a `why`. Hosted macOS runners cannot reach the `CLAUDE.md` numbers, so each gate has two modes. Local mode enforces the `CLAUDE.md` budget on Apple Silicon. CI mode (`--ci` or `AELOMI_PERF_CI=1`) enforces what a hosted runner can measure and prints the local verdict as information.

- **Cold start.** `perf/cold-start.ts` (`pnpm perf:cold`) launches the built `.app` directly, discards one warm-up, and takes the median of 10 runs. Every launch opens one throwaway data folder through `AELOMI_DATA_DIR`, so the timed runs open an existing folder, as a returning user does. The app reports its first content frame only when `AELOMI_PERF_REPORT=<path>` is set: `src/perf/cold-start.ts` marks the index commit and calls the `perf_report` command, and `src-tauri/src/perf.rs` writes the report and exits. CI mode gates end-to-end time and navigation-to-paint time against the hosted budgets.
- **Keystroke-to-paint.** The `perf` project in `playwright.config.ts` runs `tests/playwright/perf/keystroke.spec.ts` (`pnpm perf:keystroke`) against mock-only routes under `/perf/keystroke/` (`src/perf/`). `perf/keystroke.ts` splits each sample into the app's own time and the wait for the next frame. Local mode gates full key-to-paint p95. CI mode gates the app's own time. The gated surface moves to the real editor when BlockNote lands.
- **Fixtures.** `fixtures/generator.ts` (`pnpm fixtures`) writes `fixtures/generated/`: 10,000 pages for search, their expected query results, and a 5,000-block BlockNote page. `fixtures/generator.test.ts` regenerates them and compares hashes, so a hand edit fails Vitest.
- **CI.** The `cold-start` job downloads the `build-app` artifact. The `keystroke` job runs the `perf` project. Both are in `CHECKS` in `scripts/protect-main.sh`.

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

**Verification.** A doc's operations are the ID of every block, deleted or not, and every deletion, pending structs included. They come from yrs's plain encoding of the integrated blocks plus its pending update, because a client's blocks can start past its state vector. A doc can hold more deletions than its updates carry, since yrs deletes what it integrates into a deleted parent. yrs's encoding of a doc without pending structs verifies when it holds exactly the doc's operations at rotation, loads into a doc that holds exactly those too, and matches its fingerprint: state vector, delete set, and root contents. A merge verifies when it holds exactly the operations of the updates in it and loads into a doc that holds all of them, and perhaps more deletions as structs left pending integrate. Compaction reads the merge after rotating, so it holds at least what the doc held then. A snapshot that fails verification is removed, and the doc tries again after another limit's worth of logs. Nothing compares encoded bytes.

**Loading.** A doc's first `load` or `append` applies every valid snapshot and every log. It reads them under the doc's file lock alone, not its state lock, which every append and every batch take. A batch leaves out a doc whose file lock is busy, so a slow read holds up no other doc's acknowledgement. A rebuild reads the same way. Later loads encode the doc in memory, unless it has pending structs. Yjs updates are idempotent and commutative, so applying more is safe. A log ends cleanly, with a torn tail, or with corruption. A complete log header with damaged magic still uses its salt to check records, and the file counts as corrupt. If the header's salt yields no records, reading walks the records by their unsalted lengths and solves the CRC state after the salt from each stored CRC and payload. A replacement state needs agreement from at least two records. Every record loaded with a recovered state, or after magic damage, must pass its salted CRC and decode as an update. The file counts as corrupt, so compaction preserves its bytes in quarantine after a verified snapshot covers the recovered records. A damaged salt in a one-record log cannot be recovered in format 1. A record that fails where one should start first gets its length recovered if possible. Reading hashes forward from its payload and takes the first length whose payload matches the stored salted CRC, decodes as exactly one update with no bytes left over, and ends at the end of the file or at a record that passes. Only an update's true length decodes exactly, so a recovered record never covers the records after it. It loads, and its file counts as corrupt, never torn. Each reading pass gives length recovery a budget of 64 bytes of hashing and decoding per byte of the file, then stops it for the rest of that pass, so a payload that knows the salt can't force a pass, or a CRC match, every few bytes. Otherwise reading looks at where the bad record's length says it ends. If that is the end of the file or a record that passes, and the payload up to it decodes as exactly one update, only the record's CRC or payload is damaged. Reading resumes there without looking inside, where only a frame planted in note text could pass. If the payload doesn't decode exactly, reading resumes at the first offset inside whose record passes its salted CRC, decodes as an update, and starts a chain of passing records that ends exactly where the bad record says it ends, or there if no offset does. After any other bad record, even one whose length runs past the end of the file, reading resumes at the next offset whose record passes its salted CRC and decodes as an update. So every valid record loads, before and after the damage. A tail is torn only when no valid record follows the first bad one, and that record runs to or past the end of the file or only zeros follow it. Anything else is corruption. Neither file is appended to again. Neither is deleted outside the two exceptions under "Compaction", since a flipped bit in the CRC or payload of an acknowledged last record also reads as torn. A scan never hashes the payload a length claims. It keeps the CRC of every 64th prefix of the file and combines two prefixes to get a range's salted CRC, so a candidate costs about the same whatever length it claims, and note text can neither slow a scan down nor stop it. Each reading pass lets a scan decode at most 64 bytes per byte of the file, which only a payload that knows the salt can spend. A file it can't afford to decode past counts as corrupt, never torn. A `.ylog2` reads torn when its intact frames are followed by what a torn write could leave of one more frame, then nothing or only zeros. Reading accepts three shapes there. One is fewer than 20 bytes of 0x80 or above. Another is a head that passes its check, followed by no more bytes below 0x80 than its body holds and, after a whole body, fewer than 20 bytes of 0x80 or above. The last is a body that passes its CRC-64, followed by fewer than 20 bytes that start the tail its head implies. Anything else is corruption. Damage can leave the same bytes as such a tear, such as zeros over the end of the last tail or bit 7 cleared on a final tail byte of 0x80. That file reads torn too, its update still loads, and like a torn `.ylog` it is deleted only under the exceptions in "Compaction". Files below the newest valid snapshot, a damaged newest snapshot, or a leftover `.tmp` make that load compact again. An update that decodes but fails to apply leaves the doc stale: compaction skips it, and the next `load` or `append` rebuilds it from its files and queue. A panic doesn't stop storage: every lock recovers from poisoning, and a doc whose state lock was poisoned goes stale too, since yrs may have half-applied an update. A rebuild or a merged load keeps any corruption it finds for quarantine. A rebuild queues the compaction again if the logs are over the limit or a compaction was interrupted.

**Testing seams.** `Store` runs over the `Fs` and `Clock` traits. The `test-support` feature exports `SimDisk`, a `SimFs` on the system clock, for tests outside the crate. Only `src-tauri`'s dev-dependencies turn it on, so it never reaches a release build. `OsFs` and the system clock are production. `SimFs` models written, drive-cache, and durable layers, crashes with torn prefixes, injects errors, panics, damaged writes, and latency, holds an operation until the test releases it, and checks the append-only rules on every operation. It reads each log with the parser its name selects. It allows a delete only of a `.tmp`, or of a file a durable newer snapshot holds that is clean, cut off inside a log header, or torn by a write that failed this session after bytes that read clean. It allows a move out of a doc's folder only of a file a durable newer snapshot holds. Plain rename refuses an existing entry; only `Fs::replace_format(root)` can replace `format.json`, from a complete durable `format.json.tmp` marker. Creating a `.ylog2`, `.ysnap2`, or `.ysnap2.tmp` requires the folder's durable `format.json` to say 2. With `Driver::Manual` a test steps the writer and compactor itself.

## Workspace records

The workspace doc, `DocId::workspace()`, has one root, the Y.Map `pages` (DATA-2). Each key is a page ID and each value is that page's record, a nested Y.Map. The key is the record's ID, so no field repeats it. A `PageId` is a `DocId` other than `workspace`, so a page's body doc is the doc with the same ID.

| Key | Value |
|---|---|
| `title` | A string. Empty means untitled; the UI shows "Untitled" (TREE-7) |
| `placement` | An object `{ parentId: string or null, order: integer }`. One value, so parent and order change together |
| `favorite` | `null` or absent when not a favorite, else an object `{ order: integer }` |
| `trash` | `null` or absent when active, else `{ originalParentId: string or null, originalOrder: integer, batchId: string, trashedAt: integer }`. One value, so a trash or restore is whole. `placement` keeps its value while trashed |
| `createdAt`, `updatedAt` | Integers, milliseconds since the Unix epoch |

Every integer is a whole number from 0 to 2^53 - 1, the range a JavaScript number holds exactly. Yjs stores one as an integer or a float by size, and both read. The coupled fields are plain Any objects rather than nested Y.Maps, so concurrent edits can't leave half a placement or half a trash record. Readers ignore keys and object fields they don't know, and the writers in `fixture.rs` keep them, so fields can be added without a migration. A record that breaks the table fails with a `RecordError` naming the page and the field, never the title. Readers never create a root or write a default back.

`tree::forest` decides each page's parent and whether it is active. A parent with no record makes its page top-level, and a parent cycle is cut at its smallest page ID, so no page disappears. A page is active when it and every ancestor have no `trash`, so a descendant left active under a trashed page is still hidden from the tree, favorites, search, and backlinks (TREE-9). The trash list holds each trashed page whose parent is not in its batch, newest first (TREE-10). An erased page has no record (TREE-14), so its body doc, even if its files remain, is never read again.

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

`body::read_body` walks it with an explicit stack and returns each block in document order, parents before children, as its ID and segments. A segment is text or a link to a page ID. Text comes from the text runs, with marks dropped; a hard break is `\n`; a code block keeps its raw newlines; an image contributes its `caption`; a divider has none. URL links keep their text and are not backlinks, and literal `[[…]]` is text. Unknown elements contribute the text inside them, and attributes are never text. A blockContainer without a string `id`, or a `pageLink` without a valid `targetPageId`, fails with a `BodyError`. A doc with no `prosemirror` content but content under another root fails too, so a body stored elsewhere never reads as empty. `fixtures/index/blocknote-body.yupdate` was written by the real binding; a test checks every block the reader finds in it.

## The index

`aelomi_core::index` keeps `index.sqlite` in the resolved data folder: rusqlite 0.40.2 with its bundled SQLite 3.53.2 and FTS5 (MIT and public domain). It holds only what the Y.Docs rebuild: a row per page record with every DATA-2 field and whether the page is active, each block's segments and rendered text, one link row per block and target, and FTS5 indexes over titles and block text (`unicode61` with diacritics removed). Triggers keep both FTS indexes in step with their content tables. The schema version is `PRAGMA user_version`, independent of `format.json`.

**Ownership.** `Workspace` owns the `Store` and the `Index`. `append` calls `Store::append`, then marks the doc with one short lock: a workspace mark, or the page's first unindexed change time. An update the store refuses marks nothing. No decoding, SQL, or callback runs on that path, and `load` and `flush` never touch the index. The index reads docs only through `Source::load`, which can't append.

**The worker.** One thread, `aelomi-index`, owns the only writer connection and opens, validates, and rebuilds the cache. Queries use an independent read connection off the main thread. `Workspace::open` starts the worker and returns with the status `Rebuilding`. On every launch the worker:

1. Checks the file. It is missing, or it must open as a database at the current `user_version`, match the schema exactly, and pass `PRAGMA integrity_check` and the FTS5 `integrity-check` of both indexes. Otherwise it deletes `index.sqlite` and its `-wal`, `-shm`, and `-journal` sidecars, and only those, and creates the schema again. Sidecars left without an index file are deleted before a new one is created. A symlink or special file at one of those names is removed as an entry, never followed; SQLite opens the file with `SQLITE_OPEN_NOFOLLOW`. A folder at one of those names is never removed and fails the index.
2. Reconciles the whole cache with the docs, even when the file passed: a structurally valid cache can hold an edit a crash lost from the store, or lack one the store flushed. It takes the workspace mark, reads the records, writes every page row, and deletes rows of pages without a record. Then it marks every page and replaces each body projection, 128 pages per transaction.
3. Takes the marks that arrived meanwhile and applies them, then opens the read-only connection and publishes `Ready`.

After that it waits for marks. A workspace mark runs at once. A page runs 25 ms after its first unindexed change; later edits don't extend that, so typing can't starve it. Each round applies the workspace first, because titles, membership, and trash change other pages' rows. A workspace pass diffs the page rows against the last committed ones, rerenders every block that links to a page whose title appeared, changed, or went away, and marks pages that gained a record. A body pass loads the page, reads it, and writes only the blocks whose segments, text, or position changed, so one edit on a 5,000-block page stays cheap. The worker takes a doc's mark before it loads the doc, and the store applies an update before `append` marks it, so an edit that lands during a load or a commit is marked again and never lost. Dropping the workspace stops the worker between pages and joins it before the store releases its folder.

**Links render from current titles.** A page link stores only its target (LINK-1). The index renders it as the target's title, "Untitled" for an empty one, or "Deleted page" for an erased one, into the block text it searches and returns. A rename rerenders the blocks that link to the page; it never touches their docs.

**Queries.** `search` and `backlinks` run on the reader connection in one read transaction. Each answers `Reply::Indexing` unless the status is `Ready` and no workspace change is marked or being applied, checked before and after the query against a generation counter. So a query never shows a page whose trash or erasure the index has not applied. Body edits may lag by the coalescing delay. A Failed status answers `QueryError::Failed` with its reason.

- `search` quotes each word of the query, makes the last one a prefix, and matches titles and block text of active pages. It returns one hit per page, best rank first, ties by page ID: the raw title split into highlighted parts, and the best matching block with its ID and highlighted parts. Highlights are marked with private-use characters the text doesn't contain, then split into parts, so no HTML is built. A query with no word returns nothing. The palette's fuzzy title matching (CMD-4) stays in TypeScript.
- `backlinks` returns each active page that links to an active target, most recently updated first, ties by page ID, with each linking block's ID and text in document order. A block that links twice appears once. Self-links are included, matching LINK-7's rule that every active linking page appears.

**Failures.** A doc that breaks the record or body schema sets the status to `Failed` with a reason naming the doc and the problem. A page body that breaks it loses its block rows; a workspace doc keeps the last rows that read. The next edit to that doc retries it. A SQLite or file error, a failed doc load, or a panic in the worker publishes `Failed`, drops the reader, and rebuilds from step 1 after 100 ms, doubling to 30 s. Nothing in the index writes a doc or a store file, and the docs keep loading, appending, and flushing whatever its status. Each listener call is caught, so a panicking listener stops nothing.

**Not yet.** Erasure (TREE-14) must also scrub the index: `secure_delete` and FTS5 `secure-delete`, an FTS `optimize`, and a WAL checkpoint, proven by the byte scan TREE-14 specifies. The startup reconcile reads every page and keeps it loaded in the store; skipping unchanged pages by fingerprint waits for a measurement on a large folder.

**Measured** on an Apple M1 with a load average near 7, 2026-10-14. A linking edit, from `append` to the backlink a query returns, took at most 40 ms on a 200-block page over 20 edits and 51 ms on a 5,000-block page over 5, in a release build. A debug build took 45 ms and 122 ms. LINK-8's budget is 250 ms, and 25 ms of each figure is the coalescing delay. On the 10,000-page search fixture, served from memory, the four fixture queries took 0.07 ms at p50 and 0.15 ms at p95 over 1,000 runs, highlights included, against CMD-5's 5 ms. Words in thousands of pages are slower: common words and a two-letter prefix took 26 ms at p50 and 63 ms at p95, so palette work should measure them in place. `cargo nextest run -p aelomi-core --cargo-profile release --run-ignored only measure_content_query --no-capture` repeats the search measurement, and `cargo nextest run -p aelomi-core latency --no-capture` the latency.

## Storage in the shell

`open_storage` in `src-tauri/src/lib.rs` runs in Tauri's `setup` hook, on the main thread, after the window exists. It resolves the data folder, opens a `Workspace` on the resolved path, and prints one line to stdout, such as `storage status=Ready path=/…/Aelomi`. SQLite work runs on the index thread. The earlier store-only implementation measured 97 ms from Tauri's `setup` to page load start on an Apple M1, both before and after storage was added. The cold-start harness checks the complete app with the index enabled. The app manages a `Storage`, which holds the workspace or the `StorageStatus` that says why it did not open: `CloudPath { resolved }`, `Locked`, `NewerFormat`, `UnknownFormat`, or `Io`. The app starts either way.

A doc call goes from the UI to the store like `ping` does:

1. The caller encodes the update with `toBase64` from `src/ipc/base64.ts` and calls `commands.docAppend(docId, update)`.
2. `doc_append` in `src-tauri/src/commands.rs` decodes it, checks the doc ID, and calls `Store::append`. It returns the update's sequence number once the update is queued, before it is durable.
3. `doc_load` returns the doc's full state as base64, and `fromBase64` turns it back into bytes.

Both return `Unavailable` when the store did not open. `storage_status` returns the `StorageStatus`, and `storage_health` returns `Health`. Each change of health also arrives as the `StorageHealthChanged` event, from `Store::subscribe_health`.

**The index in the shell** (SHELL-12). `index_status` returns `Ready`, `Rebuilding`, or `Failed { reason }`, and each change arrives as `IndexStatusChanged`. A launch starts `Rebuilding`, so a webview reads the status with `index_status` before it listens. `IndexChanged { workspace, pages }` follows each batch the worker commits after `Ready`, so tree, search, and backlink views know to read again. `page_tree` returns active pages depth-first with their parents, favorites, and the trash list, read from the workspace doc, so the tree loads while the index rebuilds. `search(query, limit)` and `backlinks(pageId)` return `"Indexing"` or `{ Ready: … }`. The three are `async` and run on Tauri's blocking pool, so the main thread never waits on a doc load or SQLite. Errors are `QueryError`: `Unavailable`, `InvalidPageId`, `Failed { reason }`, or `Io`. Times cross IPC as numbers of milliseconds. The shell prints `index cache=created`, `reused`, or `replaced reason=…`, and `index status=…` on each change. When storage did not open, `index_status` is `Failed` with the reason "the data folder is not open". The "Indexing…" text in the palette waits for the palette itself.

**Bytes cross IPC as base64.** The storage probe measured each encoding over real IPC on an Apple M1, with release Rust and the dev frontend: the median of 20 loads of a 1 MB doc and of 50 appends of a 99-byte update, at WebKit's 1 ms timer resolution.

| Encoding | `doc_load`, 1 MB | `doc_append`, 99 bytes |
|---|---|---|
| JSON array of numbers, tauri-specta's default for `Vec<u8>` | 18 ms | under 1 ms |
| Base64 string | 6 ms | under 1 ms |
| Raw IPC: a `tauri::ipc::Response`, and a raw request body | 2 ms | under 1 ms |

Appends, the keystroke path, cost the same in every encoding. Base64 loads 3 times faster than numbers and stays inside the generated bindings. Raw IPC is faster again, but specta has no type for `tauri::ipc::Response`, so the command would need a hand-written invoke outside `commands::builder()`. Revisit that when a page load shows up in a budget.

**Quitting flushes first** (SHELL-9). Each quit path below waits up to 2 s for `Store::flush`, inside the event handler. With nothing pending it lets the quit go on. With updates still pending it stops the quit and records the question in `Storage`'s `QuitGate`, so the window stays open for confirmation. The gate is `Open`, `Asking` with the pending docs, or `Confirmed`, and a `revision` grows with each change. Every change emits `QuitStatusChanged` with the full status, after the gate is written. A webview that connects late reads the same status with `quit_status`, so a quit refused before its listener registered still asks. Health never changes the gate. **Keep app open** calls `keep_open` with the revision it answers, which reopens the gate only if that question is still the newest, and leaves the writer retrying. **Quit anyway** invokes the generated `quit_anyway` command, which confirms the gate once and requests exit once. Confirmation is sticky: later answers and refusals change nothing. All cancellable handlers share `flush_then_allow_quit`, which bypasses their flush and hold after confirmation. A repeated quit while the gate asks flushes again, and asks a newer question if writes still fail. `RunEvent::Exit` always makes a final 2 s flush of the latest appends, including those queued after confirmation. It cannot stop exit if writes still fail. Each flush prints `quit flush via=<handler> pending=[…]`. Confirmation prints `quit confirmed`, and a bypass prints `quit allowed via=<handler> confirmed=true`.

| Quit path | Handlers it reaches, in order | Can stay open |
|---|---|---|
| ⌘Q, or **Quit Aelomi** in the app menu | `ExitRequested`, `Exit` | Yes |
| ⌘W, or the window's close button | `CloseRequested`, `ExitRequested`, `Exit` | Yes |
| Logout, **Quit** in the Dock, or AppleScript `quit` | `applicationShouldTerminate:`, `Exit` | Yes |
| `perf_report` calling `app.exit(0)` | `ExitRequested`, `Exit` | Yes |

Logout, the Dock, and AppleScript send the quit Apple event, which AppKit turns into `terminate:`. tao's app delegate has no `applicationShouldTerminate:`, so `terminate:` would go straight to `RunEvent::Exit`. `macos.rs` adds that method to the delegate's class at setup, and it runs the same flush (`via=Terminate`). With updates pending it answers `NSTerminateCancel`, which also cancels a logout. Confirming **Quit anyway** exits Aelomi but does not resume the cancelled logout. It flushes on the main thread, as ⌘Q does. `NSTerminateLater` would free the thread, but while AppKit waits for the reply it handles no input and drops key presses. The logout row comes from sending the quit event, with the logout reason, to the app's PID, not from a real logout. The default menu's Quit item also sends `terminate:`. `macos.rs` still replaces it with an item that calls `app.exit(0)`.

`on_run_event` in `lib.rs` handles `CloseRequested`, `ExitRequested`, and `Exit`, and is generic over the runtime. Its tests run Tauri's `MockRuntime` over a `SimDisk` whose appends fail, and check that the window stays open and `QuitStatusChanged` names the pending doc. A refusal with no listener stays in the gate after health recovers, until `keep_open` answers it and publishes the answer. The mock runtime calls only the run callback, never window listeners, so `CloseRequested` is handled there too.

`MockRuntime::request_exit` is unimplemented. Command tests run the same confirmation body with only the exit operation replaced. They verify continuing failures, all holds after confirmation, duplicate confirmation, and the final Exit flush. Recovery tests crash and reopen `SimDisk` and assert the exact text, including an append between confirmation and Exit. AppKit tests verify `TerminateCancel` before confirmation and `TerminateNow` afterward.

