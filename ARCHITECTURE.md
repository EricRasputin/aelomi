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
| `crates/core/src/storage/fs.rs` | The `Fs` seam and `OsFs`, with the `F_FULLFSYNC` sources |
| `crates/core/src/storage/sim_fs.rs` | `SimFs`, the crash-simulating filesystem that checks the append-only rules |
| `crates/core/src/storage/data_path.rs` | `check_data_path`, the cloud folder check (SHELL-11) |
| `crates/core/src/storage/test_support.rs` | `SimDisk`, a crash-simulating disk for the shell's tests, behind the `test-support` feature |
| `crates/core/src/storage/tests/` | Store tests: crash points, compaction, failures, acknowledgement time, refusals, the fixture. `measure.rs` holds ignored measurements: `cargo nextest run -p aelomi-core --cargo-profile release --run-ignored only measure --no-capture` |
| `src-tauri/` | `aelomi`, the Tauri 2 shell (library name `aelomi_lib`) |
| `src-tauri/src/lib.rs` | `run()` builds the Tauri app, registers the commands and events, opens the store, and flushes it before quitting |
| `src-tauri/src/commands.rs` | Tauri commands, the tauri-specta `builder()`, and the `bindings_are_current` test |
| `src-tauri/src/storage.rs` | `Storage`, the open store or why it did not open, the data folder overrides, the flush before quitting, and the storage IPC types |
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
| `tests/is-local-url.ts` | The loopback allow list both browser suites share |
| `.github/workflows/ci.yml` | Every CI job |
| `scripts/check-licenses.mjs` | Fails on an npm license outside the allow list |
| `scripts/check-no-mock.mjs` | Fails if the IPC mock or a dev route is in the production bundle in `dist/` |
| `scripts/protect-main.sh` | Makes each CI job a required check on `main` |
| `scripts/tauri-dev.ts` | `pnpm dev:app`, which runs `tauri dev` with the UI on `AELOMI_DEV_PORT` or a free port |
| `scripts/vendor-skills.sh` | Vendors the pinned pstack and Emil skills into `.claude/`, and fetches the gitignored pen.dev skill. It unhides the pstack skills `how` and `why` for agents and records that change in `.claude/vendor/pstack/PATCHES` |
| `deny.toml` | cargo-deny policy: Rust licenses, advisories, and sources |
| `vite.config.ts` | Vite config. Mock builds go to `dist-mock/`, production builds to `dist/` |
| `tsconfig.json` | Strict TypeScript settings for `src/`, `e2e/`, and `tests/` |
| `biome.json` | Lint and format rules, including the test import restrictions |
| `.claude/agents/reviewer.md` | The reviewer agent that gates each PR |
| `.claude/skills/verify-aelomi/` | The skill agents use to launch, drive, and screenshot the app to prove a change works, with its feature map |

## Rust workspace

The workspace has two crates. Both use the version, edition, and license from `[workspace.package]` in `Cargo.toml`.

`crates/core` is the package `aelomi-core`. `CLAUDE.md` assigns storage, indexing, search, markdown export, the MCP server, and later the sync client to it. Today it holds the storage engine, `aelomi_core::storage`, and the constant `VERSION`. It depends on no Tauri crate, so its tests need no window.

`src-tauri` is the package `aelomi`, the desktop shell. `src-tauri/src/main.rs` calls `aelomi_lib::run()`. `run()` in `src-tauri/src/lib.rs` passes the command list from `commands::builder()` to Tauri's `invoke_handler`. Tauri's `setup` hook mounts the events, calls `macos::setup`, and opens the store (see "Storage in the shell").

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

`src/main.tsx` renders `RouterProvider` with the router from `src/router.tsx`, inside `StrictMode`.

`src/router.tsx` builds the route tree in code with `createRootRoute` and `createRoute`. There is no file-based route generation. The root layout wraps every screen in Radix `Tooltip.Provider`. The app route is `/`, which renders `IndexScreen`. Mock builds add `/perf/keystroke/empty` and `/perf/keystroke/page-5000-blocks` for the keystroke gate. Dev builds (`import.meta.env.DEV`) add `/dev/storage`, the storage probe, and a **Storage probe** link to it on the index screen.

`src/stores/` holds the Zustand stores. Components read stores through selectors, such as `usePingStore((state) => state.status)`.

`src/ipc/` holds the IPC layer:

- `bindings.ts` is the generated tauri-specta client.
- `mock.ts` is the in-memory fake of every command and event. Mock builds expose `window.__aelomiMock`, which has a typed `calls` log, `reset()`, and `storage`: `failWrites(on)`, `setStatus(status)`, and `updates(docId)`.
- `index.ts` picks one of the two. In `mock` mode (`import.meta.env.MODE === "mock"`) it calls `installMock()`. Otherwise it exports the generated commands and events.
- `base64.ts` encodes doc bytes for `doc_load` and `doc_append`, with a Vitest test beside it.
- `format-ping.ts` is a pure formatter with a Vitest test beside it.

`pnpm build:mock` builds with `--mode mock` into `dist-mock/`, which both browser suites serve with `pnpm preview:mock`. `pnpm build` builds the production bundle into `dist/`, where Vite drops the mock and the dev routes. `pnpm check:no-mock` fails if `__aelomiMock` or the storage probe's marker, `aelomi-dev-storage-probe`, appears in `dist/`.

## Tests by layer

| Layer | Tool | Location | CI job |
|---|---|---|---|
| Pure TypeScript logic | Vitest | `*.test.ts` in `src/`, `tests/`, `perf/`, and `fixtures/`, configured in `vitest.config.ts` | `vitest` |
| Rust core and shell | cargo-nextest | `#[cfg(test)]` modules in each crate | `nextest` |
| Acceptance | TesterArmy `e2e` | `e2e/*.e2e.ts`, configured in `e2e.config.ts` | `e2e` |
| WebKit flows and the network check | Playwright, `webkit` project | `tests/playwright/*.spec.ts`, configured in `playwright.config.ts` | `playwright` |
| Keystroke-to-paint | Playwright, `perf` project | `tests/playwright/perf/` | `keystroke` |
| Cold start | Node harness on the built `.app` | `perf/cold-start.ts` | `cold-start` |

`.config/nextest.toml` terminates tests after 60 s and gives the ignored storage measurements 10 minutes.

Acceptance tests import `test` from `e2e/fixtures.ts`, and Playwright tests import `test` and `expect` from `tests/playwright/fixtures.ts`. Both fixtures fail a test that makes a non-local request (SHELL-13). Biome rejects a test file that imports `test` from the library directly. There are no React component unit tests. `e2e` covers components.

## CI jobs

`.github/workflows/ci.yml` runs on every pull request and on pushes to `main`. Every `ci.yml` job runs on `macos-15`. `review-gate.yml` runs `review-gate` on `ubuntu-latest`, because it only calls the GitHub API.

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
| `reviewer-approved` | Required commit status posted by `review-gate` on Ubuntu for the latest reviewer verdict on the PR head. Keep this context in `CHECKS`. |

To add a job, add its ID to `CHECKS` in `scripts/protect-main.sh` and rerun the script, so `main` requires it.

## Performance gates

`CLAUDE.md` sets the budgets, and `SPEC.md` ("Performance budgets") defines how each is measured. Every number the gates read lives in `perf/budgets.json`, each with a `why`. Hosted macOS runners cannot reach the `CLAUDE.md` numbers, so each gate has two modes. Local mode enforces the `CLAUDE.md` budget on Apple Silicon. CI mode (`--ci` or `AELOMI_PERF_CI=1`) enforces what a hosted runner can measure and prints the local verdict as information.

- **Cold start.** `perf/cold-start.ts` (`pnpm perf:cold`) launches the built `.app` directly, discards one warm-up, and takes the median of 10 runs. Every launch opens one throwaway data folder through `AELOMI_DATA_DIR`, so the timed runs open an existing folder, as a returning user does. The app reports its first content frame only when `AELOMI_PERF_REPORT=<path>` is set: `src/perf/cold-start.ts` marks the index commit and calls the `perf_report` command, and `src-tauri/src/perf.rs` writes the report and exits. CI mode gates end-to-end time and navigation-to-paint time against the hosted budgets.
- **Keystroke-to-paint.** The `perf` project in `playwright.config.ts` runs `tests/playwright/perf/keystroke.spec.ts` (`pnpm perf:keystroke`) against mock-only routes under `/perf/keystroke/` (`src/perf/`). `perf/keystroke.ts` splits each sample into the app's own time and the wait for the next frame. Local mode gates full key-to-paint p95. CI mode gates the app's own time. The gated surface moves to the real editor when BlockNote lands.
- **Fixtures.** `fixtures/generator.ts` (`pnpm fixtures`) writes `fixtures/generated/`: 10,000 pages for search, their expected query results, and a 5,000-block BlockNote page. `fixtures/generator.test.ts` regenerates them and compares hashes, so a hand edit fails Vitest.
- **CI.** The `cold-start` job downloads the `build-app` artifact. The `keystroke` job runs the `perf` project. Both are in `CHECKS` in `scripts/protect-main.sh`.

## Data model and storage

Yjs is the source of truth (DATA-1, DATA-2). Each page body is one Y.Doc, and one workspace Y.Doc, `DocId::workspace()`, holds every page's record. Each database in v2 will be one Y.Doc with a Y.Map per row. `aelomi_core::storage::Store` keeps them.

Not built yet: the SQLite index. SQLite (rusqlite with FTS5) will be a derived index for search, backlinks, and the page tree, always rebuildable from the Y.Docs (DATA-5). The Rust core will update it and emit change events over Tauri IPC for Zustand stores. Page content skips that path, because BlockNote edits Yjs directly.

**Data folder.** `~/Library/Application Support/Aelomi`, or `Aelomi Dev` in debug builds, so `pnpm dev:app` never touches daily-use data. `AELOMI_DATA_DIR` replaces the path in every build. `check_data_path` checks it before opening: it resolves every symlink and refuses a path inside `~/Library/Mobile Documents` or `~/Library/CloudStorage` (SHELL-11). Debug builds also read `AELOMI_CLOUD_ROOTS`, a `:`-separated list that replaces those two roots, so a test can prove the refusal against a fake folder. The store opens the resolved path.

```text
Aelomi/
  format.json                       {"format": 1}, written once through a .tmp rename
  .lock                             exclusive lock while a Store has the folder open
  docs/<doc-id>/
    snapshot-<gen>.ysnap            the doc's full state when generation <gen> began
    log-<gen>-<seg>.ylog            updates appended during generation <gen>
  quarantine/<doc-id>/              files that failed validation, never deleted
```

`<gen>` and `<seg>` are 10-digit zero-padded numbers. A log is the 8-byte magic `AELOG\0\0\x02`, a random 8-byte salt, then records of a `u32` length, a `u32` CRC-32 of the salt and the payload, and the payload, one Yjs v1 update. Payloads can't know the salt, so a record framed inside one, in note text say, never passes for a real record. A snapshot is a magic header, length, CRC-32 of the payload, and the doc's state as one update.

**Refusals.** `Store::open` returns `Locked` if another process holds `.lock`, `NewerFormat` if the `format` in `format.json` is newer than this build, whatever other keys it has (checked before anything is written), and `UnknownFormat` if there is data but no readable `format.json`. Opening does no per-doc work.

**Write path.** `append` checks that the bytes decode, applies them to the doc in memory, queues them, and returns a `Seq` without waiting. One writer thread takes every queued update, appends each doc's records to its current segment with one write per file, plain-`fsync`s each touched file and each directory with a new entry, then calls `F_FULLFSYNC` once for the whole batch. Only then are the updates acknowledged. Each session, each write error, and each rebuild starts a new segment. So does a merged load that reads the open segment back torn or corrupt. The writer never appends after a torn tail or damage. `flush(deadline)` waits for everything appended before it, and first lets every failing doc retry at once, whatever its backoff. `close(deadline)` flushes, then releases the folder, or hands the store back with the docs still pending. Dropping a store is for process exit.

**Directory syncs.** A directory is synced whenever an entry in it may not be durable yet, not only right after this process created the entry, because an earlier attempt may have created it and failed to sync it. Every open syncs the data folder and its parent. A doc's first successful batch each session syncs `docs/`. Any sync of a doc's folder first syncs its quarantine folders, so a quarantined file's removal never becomes durable before its new entry.

**Failures.** A doc whose write or sync fails is left out of the batch's full sync, so other docs still acknowledge. Its updates stay queued and retry into a new segment after 100 ms, doubling to 5 s. `health()` and `subscribe_health` report `Failing { reason, since, pending_docs }` until they land. The writer remembers each segment a failed write tore this session, and how many of its bytes were acknowledged before the failure. A panic during a writer step fails every doc with queued updates for the reason `Internal`, and they retry the same way. Each subscriber call is isolated, so a panicking subscriber stops neither the writer nor the other subscribers.

**Compaction.** When a doc's unsnapshotted logs pass 1 MB or 10,000 records, the compactor thread rotates new appends to generation N + 1, writes `snapshot-(N+1).ysnap.tmp`, reads it back and verifies it, and renames it into place. Only then does it retire older files. No file leaves the doc's folder unless the new snapshot holds every block ID and deletion in every record the file yields. A file with changes the snapshot lacks stays, whether it reads clean, torn, or corrupt. If merging it back would change the doc, the doc goes stale at once, so the next `load` or `append` rebuilds it with that file. A held file that reads torn or corrupt, or that a load or rebuild found corrupt, moves to `quarantine/`, with two exceptions that hold no acknowledged bytes and are deleted. One is a log cut off inside its 16-byte header. The other is a log this session's failed write tore, when the bytes acknowledged before the failure read clean. So a disk that fails for hours adds nothing to `quarantine/` per retry, while a log torn in an earlier session still moves there. Otherwise only a clean held file, or a leftover `.tmp`, is deleted. A doc rebuilt while the compaction ran goes stale when it ends, since the rebuild can lack what the new snapshot holds. A compaction asked for after the running one rotated queues the doc once more when it ends. A snapshot over the payload limit skips compaction: before rotating for a doc without pending structs, and after it for a merge. A panic abandons that doc's compaction, leaves its files, and returns it to idle.

**Pending structs.** yrs keeps an update whose dependencies are missing as pending structs. Its full-state encoding of such a doc can fail to apply, renumber them, or load with different operations, and it can leave them pending even after every dependency arrives. So `load` of a doc with pending structs, and its snapshot, merge every update the store holds for it instead: each valid snapshot, each log's records, then the queue, read under the doc's file lock so none moves between the two. yrs's merge re-sorts every input at each step, so it merges 32 at a time: about 7 ms for a 10,000-record, 1 MB doc, against about 1.5 ms to encode one from memory. A doc without pending structs keeps that faster path.

**Verification.** A doc's operations are the ID of every block, deleted or not, and every deletion, pending structs included. They come from yrs's plain encoding of the integrated blocks plus its pending update, because a client's blocks can start past its state vector. A doc can hold more deletions than its updates carry, since yrs deletes what it integrates into a deleted parent. yrs's encoding of a doc without pending structs verifies when it holds exactly the doc's operations at rotation, loads into a doc that holds exactly those too, and matches its fingerprint: state vector, delete set, and root contents. A merge verifies when it holds exactly the operations of the updates in it and loads into a doc that holds all of them, and perhaps more deletions as structs left pending integrate. Compaction reads the merge after rotating, so it holds at least what the doc held then. A snapshot that fails verification is removed, and the doc tries again after another limit's worth of logs. Nothing compares encoded bytes.

**Loading.** A doc's first `load` or `append` applies every valid snapshot and every log. It reads them under the doc's file lock alone, not its state lock, which each batch takes for every loaded doc, so a slow read holds up no other doc's acknowledgement. A rebuild reads the same way. Later loads encode the doc in memory, unless it has pending structs. Yjs updates are idempotent and commutative, so applying more is safe. A log ends cleanly, with a torn tail, or with corruption. After any bad record, even one whose length runs past the end of the file, reading resumes at the next offset whose record passes its salted CRC and decodes as an update, so every valid record loads, before and after the damage. A tail is torn only when no valid record follows the first bad one, and that record runs to or past the end of the file or only zeros follow it. Anything else is corruption. Neither file is appended to again. Neither is deleted outside the two exceptions under "Compaction", since a flipped bit in an acknowledged last record also reads as torn. The scan hashes at most 1,024 bytes per byte of the file. A file it can't afford to read past counts as corrupt, never torn. Files below the newest valid snapshot, or a leftover `.tmp`, make that load compact again. An update that decodes but fails to apply leaves the doc stale: compaction skips it, and the next `load` or `append` rebuilds it from its files and queue. A panic doesn't stop storage: every lock recovers from poisoning, and a doc whose state lock was poisoned goes stale too, since yrs may have half-applied an update. A rebuild or a merged load keeps any corruption it finds for quarantine. A rebuild queues the compaction again if the logs are over the limit or a compaction was interrupted.

**Testing seams.** `Store` runs over the `Fs` and `Clock` traits. The `test-support` feature exports `SimDisk`, a `SimFs` on the system clock, for tests outside the crate. Only `src-tauri`'s dev-dependencies turn it on, so it never reaches a release build. `OsFs` and the system clock are production. `SimFs` models written, drive-cache, and durable layers, crashes with torn prefixes, injects errors, panics, damaged writes, and latency, holds an operation until the test releases it, and checks the append-only rules on every operation. It allows a delete only of a `.tmp`, or of a file a durable newer snapshot holds that is clean, cut off inside a log header, or torn by a write that failed this session after bytes that read clean. It allows a move out of a doc's folder only of a file a durable newer snapshot holds. With `Driver::Manual` a test steps the writer and compactor itself.

## Storage in the shell

`open_storage` in `src-tauri/src/lib.rs` runs in Tauri's `setup` hook, on the main thread, after the window exists. It resolves the data folder, opens the store on the resolved path, and prints one line to stdout, such as `storage status=Ready path=/…/Aelomi`. Opening an existing folder adds nothing measurable to cold start: on an Apple M1 the median from Tauri's `setup` to page load start was 97 ms before and after. The app manages a `Storage`, which holds the store or the `StorageStatus` that says why it did not open: `CloudPath { resolved }`, `Locked`, `NewerFormat`, `UnknownFormat`, or `Io`. The app starts either way.

A doc call goes from the UI to the store like `ping` does:

1. The caller encodes the update with `toBase64` from `src/ipc/base64.ts` and calls `commands.docAppend(docId, update)`.
2. `doc_append` in `src-tauri/src/commands.rs` decodes it, checks the doc ID, and calls `Store::append`. It returns the update's sequence number once the update is queued, before it is durable.
3. `doc_load` returns the doc's full state as base64, and `fromBase64` turns it back into bytes.

Both return `Unavailable` when the store did not open. `storage_status` returns the `StorageStatus`, and `storage_health` returns `Health`. Each change of health also arrives as the `StorageHealthChanged` event, from `Store::subscribe_health`.

**Bytes cross IPC as base64.** The storage probe measured each encoding over real IPC on an Apple M1, with release Rust and the dev frontend: the median of 20 loads of a 1 MB doc and of 50 appends of a 99-byte update, at WebKit's 1 ms timer resolution.

| Encoding | `doc_load`, 1 MB | `doc_append`, 99 bytes |
|---|---|---|
| JSON array of numbers, tauri-specta's default for `Vec<u8>` | 18 ms | under 1 ms |
| Base64 string | 6 ms | under 1 ms |
| Raw IPC: a `tauri::ipc::Response`, and a raw request body | 2 ms | under 1 ms |

Appends, the keystroke path, cost the same in every encoding. Base64 loads 3 times faster than numbers and stays inside the generated bindings. Raw IPC is faster again, but specta has no type for `tauri::ipc::Response`, so the command would need a hand-written invoke outside `commands::builder()`. Revisit that when a page load shows up in a budget.

**Quitting flushes first** (SHELL-9). Each quit path below waits up to 2 s for `Store::flush`, inside the event handler. With nothing pending it lets the quit go on. With updates still pending it stops the quit and emits `QuitBlocked` with the pending docs, so the window stays open for the confirmation that the app shell UI will add. `RunEvent::Exit` can't be stopped, so there the flush is a last resort. Each flush prints `quit flush via=<handler> pending=[…]`.

| Quit path | Handlers it reaches, in order | Can stay open |
|---|---|---|
| ⌘Q, or **Quit Aelomi** in the app menu | `ExitRequested`, `Exit` | Yes |
| ⌘W, or the window's close button | `CloseRequested`, `ExitRequested`, `Exit` | Yes |
| Logout, **Quit** in the Dock, or AppleScript `quit` | `applicationShouldTerminate:`, `Exit` | Yes |
| `perf_report` calling `app.exit(0)` | `ExitRequested`, `Exit` | Yes |

Logout, the Dock, and AppleScript send the quit Apple event, which AppKit turns into `terminate:`. tao's app delegate has no `applicationShouldTerminate:`, so `terminate:` would go straight to `RunEvent::Exit`. `macos.rs` adds that method to the delegate's class at setup, and it runs the same flush (`via=Terminate`). With updates pending it answers `NSTerminateCancel`, which also cancels a logout. It flushes on the main thread, as ⌘Q does. `NSTerminateLater` would free the thread, but while AppKit waits for the reply it handles no input and drops key presses. The logout row comes from sending the quit event, with the logout reason, to the app's PID, not from a real logout. The default menu's Quit item also sends `terminate:`. `macos.rs` still replaces it with an item that calls `app.exit(0)`.

`on_run_event` in `lib.rs` handles `CloseRequested`, `ExitRequested`, and `Exit`, and is generic over the runtime. Its tests run Tauri's `MockRuntime` over a `SimDisk` whose appends fail, and check that the window stays open and `QuitBlocked` names the pending doc. The mock runtime calls only the run callback, never window listeners, so `CloseRequested` is handled there too.

