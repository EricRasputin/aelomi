# Aelomi architecture

This file maps the code. `CLAUDE.md` has the rules and the stack, `SPEC.md` has the behaviors, and `README.md` has the commands to run, build, and test.

## Find a file

| Path | Contents |
|---|---|
| `Cargo.toml` | Cargo workspace with members `crates/core` and `src-tauri`, edition 2024 |
| `crates/core/` | `aelomi-core`, the Rust core |
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
| `tests/is-local-url.ts` | The loopback allow list both browser suites share |
| `.github/workflows/ci.yml` | Every CI job |
| `scripts/check-licenses.mjs` | Fails on an npm license outside the allow list |
| `scripts/check-no-mock.mjs` | Fails if the IPC mock is in the production bundle in `dist/` |
| `scripts/protect-main.sh` | Makes each CI job a required check on `main` |
| `scripts/vendor-skills.sh` | Vendors the pinned pstack and Emil skills into `.claude/` |
| `deny.toml` | cargo-deny policy: Rust licenses, advisories, and sources |
| `vite.config.ts` | Vite config. Mock builds go to `dist-mock/`, production builds to `dist/` |
| `tsconfig.json` | Strict TypeScript settings for `src/`, `e2e/`, and `tests/` |
| `biome.json` | Lint and format rules, including the test import restrictions |
| `.claude/agents/reviewer.md` | The reviewer agent that gates each PR |
| `.claude/skills/verify-aelomi/` | The skill agents use to launch, drive, and screenshot the app to prove a change works, with its feature map |

## Rust workspace

The workspace has two crates. Both use the version, edition, and license from `[workspace.package]` in `Cargo.toml`.

`crates/core` is the package `aelomi-core`. `CLAUDE.md` assigns storage, indexing, search, markdown export, the MCP server, and later the sync client to it. Today it holds one constant, `VERSION`. It depends on no Tauri crate, so its tests need no window.

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

Import `commands` from `src/ipc/index.ts`. A direct import of `commands` from `bindings.ts` skips the mock swap, so Biome rejects it outside `src/ipc/`. Type imports from `bindings.ts` are fine.

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
| Pure TypeScript logic | Vitest | `src/`, `perf/`, and `fixtures/` `*.test.ts`, configured in `vitest.config.ts` | `vitest` |
| Rust core and shell | cargo-nextest | `#[cfg(test)]` modules in each crate | `nextest` |
| Acceptance | TesterArmy `e2e` | `e2e/*.e2e.ts`, configured in `e2e.config.ts` | `e2e` |
| WebKit flows and the network check | Playwright, `webkit` project | `tests/playwright/*.spec.ts`, configured in `playwright.config.ts` | `playwright` |
| Keystroke-to-paint | Playwright, `perf` project | `tests/playwright/perf/` | `keystroke` |
| Cold start | Node harness on the built `.app` | `perf/cold-start.ts` | `cold-start` |

Acceptance tests import `test` from `e2e/fixtures.ts`, and Playwright tests import `test` and `expect` from `tests/playwright/fixtures.ts`. Both fixtures fail a test that makes a non-local request (SHELL-13). Biome rejects a test file that imports `test` from the library directly. There are no React component unit tests. `e2e` covers components.

## CI jobs

`.github/workflows/ci.yml` runs on every pull request and on pushes to `main`. Every job runs on `macos-15`.

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

To add a job, add its ID to `CHECKS` in `scripts/protect-main.sh` and rerun the script, so `main` requires it.

## Performance gates

`CLAUDE.md` sets the budgets, and `SPEC.md` ("Performance budgets") defines how each is measured. Every number the gates read lives in `perf/budgets.json`, each with a `why`. Hosted macOS runners cannot reach the `CLAUDE.md` numbers, so each gate has two modes. Local mode enforces the `CLAUDE.md` budget on Apple Silicon. CI mode (`--ci` or `AELOMI_PERF_CI=1`) enforces what a hosted runner can measure and prints the local verdict as information.

- **Cold start.** `perf/cold-start.ts` (`pnpm perf:cold`) launches the built `.app` directly, discards one warm-up, and takes the median of 10 runs. The app reports its first content frame only when `AELOMI_PERF_REPORT=<path>` is set: `src/perf/cold-start.ts` marks the index commit and calls the `perf_report` command, and `src-tauri/src/perf.rs` writes the report and exits. CI mode gates end-to-end time and navigation-to-paint time against the hosted budgets.
- **Keystroke-to-paint.** The `perf` project in `playwright.config.ts` runs `tests/playwright/perf/keystroke.spec.ts` (`pnpm perf:keystroke`) against mock-only routes under `/perf/keystroke/` (`src/perf/`). `perf/keystroke.ts` splits each sample into the app's own time and the wait for the next frame. Local mode gates full key-to-paint p95. CI mode gates the app's own time. The gated surface moves to the real editor when BlockNote lands.
- **Fixtures.** `fixtures/generator.ts` (`pnpm fixtures`) writes `fixtures/generated/`: 10,000 pages for search, their expected query results, and a 5,000-block BlockNote page. `fixtures/generator.test.ts` regenerates them and compares hashes, so a hand edit fails Vitest.
- **CI.** The `cold-start` job downloads the `build-app` artifact. The `keystroke` job runs the `perf` project. Both are in `CHECKS` in `scripts/protect-main.sh`.

## Data model (planned, not built)

None of this section exists in code yet. It records the direction from `CLAUDE.md` and `SPEC.md` ("Where data lives") so new code fits it.

- Yjs is the source of truth. Each page body is one Y.Doc, and one workspace Y.Doc holds every page's record (DATA-1, DATA-2). Each database in v2 is one Y.Doc with a Y.Map per row.
- Each Y.Doc is stored as a snapshot file plus newer update files. Writes only append, and compaction replaces old files with a verified snapshot (DATA-6).
- SQLite (rusqlite with FTS5) is a derived index for search, backlinks, and the page tree. The app can always delete it and rebuild it from the Y.Docs (DATA-5).
- The Rust core updates SQLite and emits change events over Tauri IPC. Zustand stores subscribe to them. Page content skips this path, because BlockNote edits Yjs directly.
- The data folder is `~/Library/Application Support/Aelomi`. No data path may resolve into iCloud Drive.
