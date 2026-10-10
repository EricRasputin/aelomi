# Aelomi

Use Node 24 from `.node-version`, pnpm from `package.json`'s `packageManager`,
and Rust 1.98.1 from `rust-toolchain.toml`.
Run the macOS app with `pnpm install --frozen-lockfile` and `pnpm tauri dev`.
Build the ad-hoc signed app with `pnpm tauri build --bundles app`.

After changing a Rust command, run `pnpm bindings` and `pnpm typecheck`.
`cargo nextest run --workspace` checks the committed bindings against the command builder.

Run `pnpm lint`, `pnpm typecheck`, `pnpm test`, `cargo nextest run --workspace`,
`cargo deny check`, `pnpm audit --audit-level moderate`, and `pnpm check-licenses` before committing.

The license policy allows Unicode-3.0 for the ICU and unicode-ident dependencies,
Zlib for foldhash, and Unlicense for isbot. Each is scoped to the packages named in `deny.toml` or `scripts/check-licenses.mjs`.
`deny.toml` names two unmaintained transitive crates with no patched releases.

## CI

`.github/workflows/ci.yml` runs on every pull request, including stacked PRs,
and on pushes to `main`. Its nine check IDs are `biome`, `tsc`, `vitest`,
`nextest`, `cargo-deny`, `pnpm-audit`, `build-app`, `e2e`, and `playwright`.
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
the ad-hoc signed `Aelomi.app` bundle. The cold-start gate can download this artifact.

## Browser tests

Run `pnpm exec e2e-web install webkit`, then `pnpm build:mock`,
`pnpm test:e2e`, and `pnpm test:playwright`.
Both suites run against the production React build in `dist-mock/`.
`pnpm preview:mock` serves that build on `127.0.0.1`.
TesterArmy selects a free port. Playwright uses port 4173 and fails if it is occupied.
No model or API key is needed. The e2e script and CI disable telemetry.

`src/ipc/mock.ts` implements the generated command type in memory.
Tests can use `window.__aelomiMock.reset()` and its typed `calls` log.
Each browser test starts in a fresh context with an empty mock.
The app imports commands through `src/ipc/index.ts`.
Vite selects the mock only in `mock` mode.
Run `pnpm build` and `pnpm check:no-mock` to verify the production bundle.
CI runs that check after the Tauri build.

Acceptance tests import `test` from `e2e/fixtures.ts`.
Its automatic fixture aborts non-local requests and fails with their URLs.
Biome rejects direct test imports that skip the fixture.
The Playwright fixture also covers WebSockets and blocks service workers.
Both suites allow only loopback hosts and `data:`, `blob:`, and `about:` URLs.
CI caches their shared Playwright 1.63.0 WebKit build and uploads test reports.

The license policy selects BSD-3-Clause for `json-schema`'s
`AFL-2.1 OR BSD-3-Clause` dual license.

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
