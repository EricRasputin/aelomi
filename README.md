# Aelomi

Use Node 24 from `.node-version`, pnpm from `package.json`'s `packageManager`,
and Rust 1.98.1 from `rust-toolchain.toml`.
Run the macOS app with `pnpm install --frozen-lockfile` and `pnpm tauri dev`.
`pnpm dev:app` serves the UI on `AELOMI_DEV_PORT` or a free port instead of 1420, so several dev instances can run at once.
Dev builds keep their data in `~/Library/Application Support/Aelomi Dev`, apart from the release app's `Aelomi`. `AELOMI_DATA_DIR` points any build at another folder.
Build the ad-hoc signed app with `pnpm tauri build --bundles app`.

After changing a Rust command, run `pnpm bindings` and `pnpm typecheck`.
`cargo nextest run --workspace` checks the committed bindings against the command builder.

Run `pnpm lint`, `pnpm typecheck`, `pnpm test`, `cargo nextest run --workspace`,
`cargo deny check`, `pnpm audit --audit-level moderate`, and `pnpm check-licenses` before committing.

For native screenshots and shell logs, follow [Native verification evidence](docs/native-verification-evidence.md) before requesting review. Evidence stays local by default, and screenshots go on the PR.

The license policy allows Unicode-3.0 for the ICU and unicode-ident dependencies,
Zlib for foldhash, and Unlicense for isbot. Each is scoped to the packages named in `deny.toml` or `scripts/check-licenses.mjs`.
`deny.toml` names two unmaintained transitive crates with no patched releases.

## CI

`.github/workflows/ci.yml` runs on every pull request, including stacked PRs,
and on pushes to `main`. Its eleven check IDs are `biome`, `tsc`, `vitest`,
`nextest`, `cargo-deny`, `pnpm-audit`, `build-app`, `e2e`, `playwright`,
`cold-start`, and `keystroke`.
The `pnpm-audit` job also runs `pnpm check-licenses`.
The `nextest` job includes the `bindings_are_current` test.
Cargo steps run with `--locked`, so a `Cargo.toml` change without its `Cargo.lock` fails.

A repository admin runs `./scripts/protect-main.sh` to make every job a required check on `main`.
Rerun it after adding or renaming a job.

Every job uses `macos-15`, GitHub's [Apple Silicon M1 arm64 runner](https://docs.github.com/en/actions/reference/runners/github-hosted-runners#standard-github-hosted-runners-for-public-repositories).
The pinned OS label keeps the machine class stable for later performance gates.
GitHub still updates the runner image within that label.
CI caches the pnpm store and Rust builds.
Actions use full commit SHAs. cargo-nextest 0.9.148 and cargo-deny 0.20.2 use
prebuilt binaries with source-install fallbacks disabled.

`build-app` uploads `Aelomi-app` with a 7-day retention period.
The artifact contains `Aelomi-app.zip`, made with `ditto --keepParent` to preserve
the ad-hoc signed `Aelomi.app` bundle. The `cold-start` job downloads this artifact.

## Browser tests

Run `pnpm exec e2e-web install webkit`, then `pnpm build:mock`,
`pnpm test:e2e`, and `pnpm test:playwright`.
Both suites run against the production React build in `dist-mock/`.
`pnpm preview:mock` serves that build on `127.0.0.1`.
`pnpm test:playwright` runs the `webkit` project. The `perf` project is separate
(see Performance gates).
TesterArmy selects a free port. Playwright uses port 4173 and fails if it is occupied.
No model or API key is needed. The e2e script and CI disable telemetry.

`src/ipc/mock.ts` implements the generated command type in memory.
Mock builds expose `window.__aelomiMock` with `reset()` and a typed `calls` log.
Each browser test starts in a fresh context with an empty mock.
The app imports commands through `src/ipc/index.ts`.
Vite folds the IPC module's mode switch and selects the mock only in `mock` mode.
Run `pnpm build` and `pnpm check:no-mock` to verify the production bundle.
CI runs that check after the Tauri build.

Acceptance tests import `test` from `e2e/fixtures.ts`.
Its automatic fixture aborts non-local requests and fails with their URLs.
Acceptance retries are disabled so a later clean attempt cannot hide remote traffic.
Biome rejects direct test imports that skip the fixture.
Playwright collects only `.spec.ts` files covered by the import restriction.
Its fixture also covers WebSockets and remote redirect hops.
TesterArmy's route guard cannot observe those two cases.
Both browser configurations block service workers.
Both suites allow only loopback hosts and `data:`, `blob:`, and `about:` URLs.
CI caches their shared Playwright 1.63.0 WebKit build and uploads test reports.

The license policy selects BSD-3-Clause for `json-schema`'s
`AFL-2.1 OR BSD-3-Clause` dual license.

## Performance gates

`perf/budgets.json` holds every number the gates use, each with a `why`.
The `specMs` and `p95Ms` entries are the local ceilings for the SPEC.md budgets.
Do not edit them, or the budgets in SPEC.md, to make a gate pass.

**Cold start.** `pnpm perf:cold` launches `target/release/bundle/macos/Aelomi.app`
directly (build it with `pnpm tauri build --bundles app`), discards one warm-up,
times 10 runs, and gates the median against the 500 ms budget. The median of an
even count is the mean of the 5th and 6th sorted runs. With
`AELOMI_PERF_REPORT=<path>` set, the app arms a probe that records the index
screen's first content frame (the earlier of rAF plus MessageChannel and double
rAF), writes JS marks, Rust stamps, and the kernel process start to the path,
and exits. Without the env var the probe is never armed.
Hosted `macos-15` runners cannot meet 500 ms: WebContent startup alone takes
about 930 ms there against about 96 ms on an M1. The `cold-start` CI job
therefore runs the harness with `--ci`, which gates `hostedCiMs` (end to end)
and `hostedNavToPaintMs` (navigation start to first content frame, the part the
frontend controls) and prints the 500 ms verdict as information. Exit code 2
means the harness could not measure (for example "no paint marker in 10 s"),
exit code 1 means a budget miss.

**Keystroke-to-paint.** `pnpm perf:keystroke` runs the Playwright `perf` project
against `dist-mock/`. The mock build adds `/perf/keystroke/empty` and
`/perf/keystroke/page-5000-blocks`, plain `contenteditable` surfaces that stand
in for the editor until BlockNote lands. The test presses a key and records five
stamps on the page clock: the key event, the end of the `input` handler, the
frame timestamp (the argument WebKit passes to every rAF callback of a rendering
update, read from `performance.now()` before the first callback runs), the start
of the recorder's own rAF callback, and the MessageChannel task after that rAF,
which is the first frame. Each sample splits into app-owned time (key to input,
plus frame timestamp to MessageChannel) and the tick wait between the input
handler and the frame timestamp. Work the app queues in its own rAF runs before
the recorder's callback and counts as app-owned. The two parts must add up to
the full key-to-paint, and the test prints that check. It repeats 150 times per
page load with 100 ms between keys, over 3 loads, and keeps 135 samples per load.
Local mode gates the full key-to-paint p95 of the surface named by
`keystroke.gate` against `p95Ms`, the local ceiling. With `AELOMI_PERF_CI=1`,
which the `keystroke` CI job sets, the test gates the app-owned p95 against
`hostedAppOwnedP95Ms` instead and prints the full key-to-paint verdict as
information, because on hosted `macos-15` the first rendering update after
input waits for the runner's next 17 ms display tick. The other surface is
reported in both modes.
Did-it-happen guards fail the test when samples are missing or the text did not
grow by one character per key.

**Busy machine.** When the 1-minute load average per core is above
`noise.maxLoadPerCore`, both harnesses print a `NOISY: load ...` line at the
start and at the end, and append it to the verdict line. The label is
information only. It never changes a verdict or an exit code, and the harness
never waits for the load to fall.

**Fixtures.** `pnpm fixtures` regenerates `fixtures/generated/` from the seeded
generator. See `fixtures/README.md`.

## Handle an unfixable npm advisory

CI fails on moderate, high, or critical advisories in all dependencies.
First update the affected dependency or use an override to select a patched version.
If no usable fix exists, add only that advisory's GHSA ID to
[`audit.ignore`](https://pnpm.io/cli/audit#auditignore) in `pnpm-workspace.yaml`.
Add a comment beside the ID that names the dependency, explains why no fix is usable,
and links the advisory. Remove the ignore when a fix becomes usable.

The configuration shape below is a template. Replace the placeholder with the
actual advisory ID and its specific reason only when an advisory fires.

```yaml
audit:
  ignore:
    - GHSA-xxxx-xxxx-xxxx
```
