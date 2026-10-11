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
| `crates/core/src/storage/tests/` | Store tests: crash points, compaction, failures, acknowledgement time, refusals, the fixture. `measure.rs` holds ignored measurements: `cargo nextest run -p aelomi-core --cargo-profile release --run-ignored only measure --no-capture` |
| `src-tauri/` | `aelomi`, the Tauri 2 shell (library name `aelomi_lib`) |
| `src-tauri/src/lib.rs` | `run()` builds the Tauri app and registers the commands |
| `src-tauri/src/commands.rs` | Tauri commands, the tauri-specta `builder()`, and the `bindings_are_current` test |
| `src-tauri/src/macos.rs` | All macOS-specific code |
| `src-tauri/src/perf.rs` | The cold-start probe, armed only by `AELOMI_PERF_REPORT` |
| `src-tauri/tauri.conf.json` | Window, content security policy, and the ad-hoc signed `.app` bundle |
| `src/main.tsx` | UI entry point, loaded by `index.html` |
| `src/router.tsx` | Routes and the screens they render |
| `src/stores/` | Zustand stores |
| `src/ipc/` | Generated bindings, the IPC mock, and the switch between them |
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
| `scripts/check-no-mock.mjs` | Fails if the IPC mock is in the production bundle in `dist/` |
| `scripts/protect-main.sh` | Makes each CI job a required check on `main` |
| `scripts/tauri-dev.ts` | `pnpm dev:app`, which runs `tauri dev` with the UI on `AELOMI_DEV_PORT` or a free port |
| `scripts/vendor-skills.sh` | Vendors the pinned pstack and Emil skills into `.claude/`, and fetches the gitignored pen.dev skill |
| `deny.toml` | cargo-deny policy: Rust licenses, advisories, and sources |
| `vite.config.ts` | Vite config. Mock builds go to `dist-mock/`, production builds to `dist/` |
| `tsconfig.json` | Strict TypeScript settings for `src/`, `e2e/`, and `tests/` |
| `biome.json` | Lint and format rules, including the test import restrictions |
| `.claude/agents/reviewer.md` | The reviewer agent that gates each PR |
| `.claude/skills/verify-aelomi/` | The skill agents use to launch, drive, and screenshot the app to prove a change works, with its feature map |

## Rust workspace

The workspace has two crates. Both use the version, edition, and license from `[workspace.package]` in `Cargo.toml`.

`crates/core` is the package `aelomi-core`. `CLAUDE.md` assigns storage, indexing, search, markdown export, the MCP server, and later the sync client to it. Today it holds the storage engine, `aelomi_core::storage`, and the constant `VERSION`. It depends on no Tauri crate, so its tests need no window.

`src-tauri` is the package `aelomi`, the desktop shell. `src-tauri/src/main.rs` calls `aelomi_lib::run()`. `run()` in `src-tauri/src/lib.rs` passes the command list from `commands::builder()` to Tauri's `invoke_handler` and calls `macos::setup` from Tauri's `setup` hook.

## macOS-specific code lives in one module

`src-tauri/src/macos.rs` holds all code that works only on macOS. `src-tauri/src/lib.rs` declares it with `#[cfg(target_os = "macos")] mod macos;` and calls `macos::setup(app)` under the same `cfg`. Today `setup` installs the default native menu, and `process_start_us` reads the kernel's process start time for the cold-start probe. A port to another platform replaces this one module. The `bundle.macOS` block in `src-tauri/tauri.conf.json` sets the macOS 14 minimum and ad-hoc signing.

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

`src/router.tsx` builds the route tree in code with `createRootRoute` and `createRoute`. There is no file-based route generation. The root layout wraps every screen in Radix `Tooltip.Provider`. The app route is `/`, which renders `IndexScreen`. Mock builds add `/perf/keystroke/empty` and `/perf/keystroke/page-5000-blocks` for the keystroke gate.

`src/stores/` holds the Zustand stores. Components read stores through selectors, such as `usePingStore((state) => state.status)`.

`src/ipc/` holds the IPC layer:

- `bindings.ts` is the generated tauri-specta client.
- `mock.ts` is the in-memory fake of every command. Mock builds expose `window.__aelomiMock`, which has a typed `calls` log and `reset()`.
- `index.ts` picks one of the two. In `mock` mode (`import.meta.env.MODE === "mock"`) it calls `installMock()`. Otherwise it exports the generated commands.
- `format-ping.ts` is a pure formatter with a Vitest test beside it.

`pnpm build:mock` builds with `--mode mock` into `dist-mock/`, which both browser suites serve with `pnpm preview:mock`. `pnpm build` builds the production bundle into `dist/`, where Vite drops the mock. `pnpm check:no-mock` fails if `__aelomiMock` appears in `dist/`.

## Tests by layer

| Layer | Tool | Location | CI job |
|---|---|---|---|
| Pure TypeScript logic | Vitest | `*.test.ts` in `src/`, `tests/`, `perf/`, and `fixtures/`, configured in `vitest.config.ts` | `vitest` |
| Rust core and shell | cargo-nextest | `#[cfg(test)]` modules in each crate | `nextest` |
| Acceptance | TesterArmy `e2e` | `e2e/*.e2e.ts`, configured in `e2e.config.ts` | `e2e` |
| WebKit flows and the network check | Playwright, `webkit` project | `tests/playwright/*.spec.ts`, configured in `playwright.config.ts` | `playwright` |
| Keystroke-to-paint | Playwright, `perf` project | `tests/playwright/perf/` | `keystroke` |
| Cold start | Node harness on the built `.app` | `perf/cold-start.ts` | `cold-start` |

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

- **Cold start.** `perf/cold-start.ts` (`pnpm perf:cold`) launches the built `.app` directly, discards one warm-up, and takes the median of 10 runs. The app reports its first content frame only when `AELOMI_PERF_REPORT=<path>` is set: `src/perf/cold-start.ts` marks the index commit and calls the `perf_report` command, and `src-tauri/src/perf.rs` writes the report and exits. CI mode gates end-to-end time and navigation-to-paint time against the hosted budgets.
- **Keystroke-to-paint.** The `perf` project in `playwright.config.ts` runs `tests/playwright/perf/keystroke.spec.ts` (`pnpm perf:keystroke`) against mock-only routes under `/perf/keystroke/` (`src/perf/`). `perf/keystroke.ts` splits each sample into the app's own time and the wait for the next frame. Local mode gates full key-to-paint p95. CI mode gates the app's own time. The gated surface moves to the real editor when BlockNote lands.
- **Fixtures.** `fixtures/generator.ts` (`pnpm fixtures`) writes `fixtures/generated/`: 10,000 pages for search, their expected query results, and a 5,000-block BlockNote page. `fixtures/generator.test.ts` regenerates them and compares hashes, so a hand edit fails Vitest.
- **CI.** The `cold-start` job downloads the `build-app` artifact. The `keystroke` job runs the `perf` project. Both are in `CHECKS` in `scripts/protect-main.sh`.

## Data model and storage

Yjs is the source of truth (DATA-1, DATA-2). Each page body is one Y.Doc, and one workspace Y.Doc, `DocId::workspace()`, holds every page's record. Each database in v2 will be one Y.Doc with a Y.Map per row. `aelomi_core::storage::Store` keeps them.

Not built yet: the Tauri wiring that opens the store at launch, and the SQLite index. SQLite (rusqlite with FTS5) will be a derived index for search, backlinks, and the page tree, always rebuildable from the Y.Docs (DATA-5). The Rust core will update it and emit change events over Tauri IPC for Zustand stores. Page content skips that path, because BlockNote edits Yjs directly.

**Data folder.** `~/Library/Application Support/Aelomi`, checked by `check_data_path` before opening: it resolves every symlink and refuses a path inside `~/Library/Mobile Documents` or `~/Library/CloudStorage` (SHELL-11).

```text
Aelomi/
  format.json                       {"format": 1}, written once through a .tmp rename
  .lock                             exclusive lock while a Store has the folder open
  docs/<doc-id>/
    snapshot-<gen>.ysnap            the doc's full state when generation <gen> began
    log-<gen>-<seg>.ylog            updates appended during generation <gen>
  quarantine/<doc-id>/              files that failed validation, never deleted
```

`<gen>` and `<seg>` are 10-digit zero-padded numbers. A log is an 8-byte magic header, then records of a `u32` length, a `u32` CRC-32, and one Yjs v1 update. A snapshot is a magic header, length, CRC-32, and the doc's state as one update.

**Refusals.** `Store::open` returns `Locked` if another process holds `.lock`, `NewerFormat` if the `format` in `format.json` is newer than this build, whatever other keys it has (checked before anything is written), and `UnknownFormat` if there is data but no readable `format.json`. Opening does no per-doc work.

**Write path.** `append` checks that the bytes decode, applies them to the doc in memory, queues them, and returns a `Seq` without waiting. One writer thread takes every queued update, appends each doc's records to its current segment with one write per file, plain-`fsync`s each touched file and each directory with a new entry, then calls `F_FULLFSYNC` once for the whole batch. Only then are the updates acknowledged. Each session, each write error, and each rebuild starts a new segment. So does a merged load that reads the open segment back torn or corrupt. The writer never appends after a torn tail or damage. `flush(deadline)` waits for everything appended before it, and first lets every failing doc retry at once, whatever its backoff. `close(deadline)` flushes, then releases the folder, or hands the store back with the docs still pending. Dropping a store is for process exit.

**Directory syncs.** A directory is synced whenever an entry in it may not be durable yet, not only right after this process created the entry, because an earlier attempt may have created it and failed to sync it. Every open syncs the data folder and its parent. A doc's first successful batch each session syncs `docs/`. Any sync of a doc's folder first syncs its quarantine folders, so a quarantined file's removal never becomes durable before its new entry.

**Failures.** A doc whose write or sync fails is left out of the batch's full sync, so other docs still acknowledge. Its updates stay queued and retry into a new segment after 100 ms, doubling to 5 s. `health()` and `subscribe_health` report `Failing { reason, since, pending_docs }` until they land. A panic during a writer step fails every doc with queued updates for the reason `Internal`, and they retry the same way. Each subscriber call is isolated, so a panicking subscriber stops neither the writer nor the other subscribers.

**Compaction.** When a doc's unsnapshotted logs pass 1 MB or 10,000 records, the compactor thread rotates new appends to generation N + 1, writes `snapshot-(N+1).ysnap.tmp`, reads it back and verifies it, and renames it into place. Only then does it retire older files. It deletes one only when it reads clean and every block ID and deletion in it is already in the new snapshot. A file with changes the snapshot lacks stays, and the doc goes stale at once, so the next `load` or `append` rebuilds it with that file. Files that failed validation move to `quarantine/` instead. That includes any a load or rebuild found, and any torn or corrupt file whose readable records the new snapshot holds. Only a clean file the snapshot holds, or a leftover `.tmp`, is ever deleted. A doc rebuilt while the compaction ran goes stale when it ends, since the rebuild can lack what the new snapshot holds. A compaction asked for after the running one rotated queues the doc once more when it ends. A snapshot over the payload limit skips compaction: before rotating for a doc without pending structs, and after it for a merge. A panic abandons that doc's compaction, leaves its files, and returns it to idle.

**Pending structs.** yrs keeps an update whose dependencies are missing as pending structs. Its full-state encoding of such a doc can fail to apply, renumber them, or load with different operations, and it can leave them pending even after every dependency arrives. So `load` of a doc with pending structs, and its snapshot, merge every update the store holds for it instead: each valid snapshot, each log's records, then the queue, read under the doc's file lock so none moves between the two. yrs's merge re-sorts every input at each step, so it merges 32 at a time: about 7 ms for a 10,000-record, 1 MB doc, against about 1.5 ms to encode one from memory. A doc without pending structs keeps that faster path.

**Verification.** A doc's operations are the ID of every block, deleted or not, and every deletion, pending structs included. They come from yrs's plain encoding of the integrated blocks plus its pending update, because a client's blocks can start past its state vector. A doc can hold more deletions than its updates carry, since yrs deletes what it integrates into a deleted parent. yrs's encoding of a doc without pending structs verifies when it holds exactly the doc's operations at rotation, loads into a doc that holds exactly those too, and matches its fingerprint: state vector, delete set, and root contents. A merge verifies when it holds exactly the operations of the updates in it and loads into a doc that holds all of them, and perhaps more deletions as structs left pending integrate. Compaction reads the merge after rotating, so it holds at least what the doc held then. A snapshot that fails verification is removed, and the doc tries again after another limit's worth of logs. Nothing compares encoded bytes.

**Loading.** A doc's first `load` or `append` applies every valid snapshot and every log. Later loads encode the doc in memory, unless it has pending structs. Yjs updates are idempotent and commutative, so applying more is safe. A log ends cleanly, with a torn tail, or with corruption. A tail is torn only when the first bad record runs to the end of the file or only zeros follow it. The file is never appended to. Anything else is corruption: every valid record loads, before and after the damage. Either way the file is quarantined at the next compaction, never deleted, since a flipped bit in an acknowledged last record also reads as torn. After a bad record, reading resumes at the next offset whose record passes its CRC and decodes as an update. That scan hashes at most 1,024 bytes per byte of the file. A file it can't afford to read past counts as corrupt, never torn. Files below the newest valid snapshot, or a leftover `.tmp`, make that load compact again. An update that decodes but fails to apply leaves the doc stale: compaction skips it, and the next `load` or `append` rebuilds it from its files and queue. A panic doesn't stop storage: every lock recovers from poisoning, and a doc whose state lock was poisoned goes stale too, since yrs may have half-applied an update. A rebuild or a merged load keeps any corruption it finds for quarantine. A rebuild queues the compaction again if the logs are over the limit or a compaction was interrupted.

**Testing seams.** `Store` runs over the `Fs` and `Clock` traits. `OsFs` and the system clock are production. `SimFs` models written, drive-cache, and durable layers, crashes with torn prefixes, injects errors, panics, damaged writes, and latency, and checks the append-only rules on every operation. It allows a delete only of a `.tmp` or of a clean file a durable newer snapshot holds. With `Driver::Manual` a test steps the writer and compactor itself.
