# Aelomi

A lightweight, fast, local-first alternative to Notion for macOS. Open source (MIT).

Aelomi is built entirely by AI agents. The maintainer sets direction and taste and uses the app daily as QA; nobody reads the code line by line. CI, the test suites, and a separate reviewer agent are the review gate. Product and design questions go to the maintainer. Engineering choices inside the stack below are yours to make.

`SPEC.md` lists MVP features, behavior IDs, and their tests. `ARCHITECTURE.md` maps the crates, modules, and test layers. Planned, not yet written: `TASTE.md` (design rules). Until it exists, this file is the source of truth for design rules.

## Hard rules

- **Never lose user data.** Storage writes are append-only. Migrations are additive only; never drop or rewrite user data in place. Every migration ships with a test that opens an older database or backup. The only exceptions: **Delete permanently** and trash expiry erase what the user deleted, completely, and compaction replaces stored history with a verified snapshot. See `SPEC.md` (TREE-13, TREE-14, DATA-6).
- **No GPL code.** Never add `@blocknote/xl-*` packages (GPL-3.0 or commercial). Use BlockNote core only. Check the license of every new dependency; MIT, Apache-2.0, BSD, ISC and MPL-2.0 are fine.
- **Local-first.** The app works fully offline. The only network calls allowed are opt-out telemetry, optional AI features, sync (later), and update checks.
- **Telemetry never contains user content.** No note text, titles, page names, or file paths. Anonymous hashed install ID and usage metadata only.
- **macOS only** (macOS 14+, Apple Silicon). Keep all Mac-specific code in one isolated module so community ports stay possible.
- **The decided stack wins.** If a skill, library, or your own judgment suggests replacing part of the stack, open a proposal for the maintainer. Never switch silently.
- **Don't re-propose ruled-out options** (list at the bottom) unless something material has changed.
- **Never store SQLite or app data in iCloud Drive.** iCloud file sync corrupts SQLite.

## Stack

| Layer | Choice |
|---|---|
| Desktop shell | Tauri 2 |
| Core | Rust: storage, indexing, search, markdown export, MCP server, sync client (later) |
| UI | TypeScript (strict, no `any`) + React |
| Editor | BlockNote (core only) |
| Routing | TanStack Router, code-based routes only (no file-based route codegen) |
| Database views (v2) | TanStack Table |
| Long lists | TanStack Virtual (sidebar, search results, table rows) |
| UI state | Zustand stores with selectors |
| UI ↔ Rust calls | tauri-specta (types generated from Rust commands; mismatches fail the build) |
| Styling | Tailwind v4 + Base UI |
| Tooling | Vite, pnpm, Biome (lint + format), Vitest, cargo-nextest |
| MCP server | `rmcp` (official Rust MCP SDK), inside the Rust core |

## Data model

- **Yjs is the source of truth** for page content and database rows: one Y.Doc per page, one Y.Doc per database with a Y.Map per row.
- **SQLite (rusqlite, FTS5) is a derived index** for search, backlinks, the page tree, and database view queries. It must always be rebuildable from Yjs.
- **Data flow:** the Rust core updates SQLite and emits a change event over Tauri IPC ("page tree changed", "search results ready"). Zustand stores subscribe and update; only components using that data re-render. Page content never takes this path; BlockNote edits Yjs directly.
- **Location:** `~/Library/Application Support/Aelomi`.
- **Backup:** scheduled markdown export to a folder the user picks.

## Performance budgets

- Cold start under 500 ms.
- Keystroke-to-paint under 16 ms.
- A 5,000-block page stays smooth to scroll and edit.

GitHub's hosted Mac runners are too slow and too noisy to time these numbers, and the project uses no paid or self-hosted runners (decided 2026-10-11). CI gates runner-calibrated budgets from `perf/budgets.json`, which catch regressions. `pnpm perf:cold` and `pnpm perf:keystroke` check the numbers above on Apple Silicon. A change that fails a CI perf gate doesn't merge.

## Testing

| Layer | Tool | Covers |
|---|---|---|
| Features | TesterArmy `e2e` | Acceptance tests for every feature, written as plain-English steps. UI runs in a browser with Tauri IPC mocked. |
| Browser gaps + timing | Playwright (WebKit) | Key flows in WebKit (closest to the app's WKWebView) and the performance budgets |
| Rust core | cargo-nextest, property tests | Storage, migrations, indexing, export, data safety |
| Sync (v3) | Fuzz/property tests | Simulated devices converge after random concurrent edits |
| Pure TS logic | Vitest | View filtering/sorting, markdown conversion, keyboard handling |
| React components | none | Covered by `e2e`. Don't write component unit tests. |

Data-safety checks end in exact assertions, never an AI judgment step. The `verify-aelomi` skill drives the real app window through the macOS accessibility tree to prove a change works. It is evidence for a PR, not a CI gate, so the Rust tests and the maintainer's daily use still cover the real app.

Before publishing native verification evidence or handing a PR to review, follow [Publish native verification evidence](docs/native-verification-evidence.md). Attach future screenshots to the PR and commit the text record. Use the documented committed fallback when uploads fail, and verify remote proof links before local runs disappear.

**Definition of done:** the feature's `e2e` tests pass in CI, logic has unit tests, budgets are green, dependency checks (`cargo-deny`, `pnpm audit`) pass, and the reviewer agent approves.

## Workflow

- **pstack** (`github.com/backnotprop/pstack`) runs the engineering process. Use `/poteto-mode` and its playbooks: feature → PR → verify → merge. One branch and one PR per feature. Never commit directly to `main`.
- **Emil Kowalski's skills** (`github.com/emilkowalski/skill`) are the design and motion rulebook. Not used: `animate-expo`, `mobile-native`, `write-swift`.
- **pen.dev** (`pen-design` skill, `@pen.dev/cli`) can prototype new UI designs to explore and verify ideas before implementation when useful. Judge the prototypes against Emil's rules. Its skill is proprietary, so `scripts/vendor-skills.sh` fetches it from npm at a pinned version into a gitignored `.claude/skills/pen-design/`; it is never committed.
- Both skill sets are vendored into `.claude/skills/` (pstack's subagents into `.claude/agents/`), pinned to commits in `scripts/vendor-skills.sh`. Licenses and the pinned source live in `.claude/vendor/`. Update deliberately: bump the ref (or pen.dev's version), rerun the script, review the diff. Never hand-edit vendored files. Keep `.claude/` out of Biome, tsc, and Vitest (pstack ships its own TS scripts).
- pstack's model roles live in `~/.agents/pstack-models.md`, written by `/setup-pstack`.
- **UI work goes to Claude Opus 5.5 only.** Claude Opus 5.5 (`claude-opus-5-5`) does every UI design and implementation task: pen.dev prototypes, React components, styles, UI dependencies, and UI tests. No other model writes or edits UI code. For UI work this overrides the `feature` role and the other code roles in `~/.agents/pstack-models.md`, which stays unchanged. The Fable review gate still applies.
- **Review gate.** After CI is green on a PR, the agent that opened or updated it runs a fresh `reviewer` subagent (`.claude/agents/reviewer.md`) on Claude Fable. Builder subagents never use Fable. Findings go back to the builder, and the loop repeats on each new head until the verdict is APPROVE for the current head SHA. The maintainer merges only PRs whose latest reviewer verdict is APPROVE for the head SHA.

## Design direction

Linear-like: minimal, calm, dense but quiet, easy on the eyes. Keyboard-first, with ⌘K as the main entry point. Dark mode is a first-class theme. Motion is fast and purposeful (follow Emil's skills). Native macOS conventions: system font, native menus, standard window controls.

## Roadmap

- **MVP (confirmed 2026-10-11):** page tree (nesting, favorites, trash), BlockNote editor (slash menu, markdown shortcuts, todos, code, images, drag to reorder), `[[links]]` and backlinks, ⌘K search and commands, scheduled markdown export. `SPEC.md` has the details.
- **Right after MVP:** built-in MCP server.
- **v2:** Notion import (markdown + CSV), databases with table, board and calendar views.
- **v3:** multi-device sync, auth, sharing.
- **Later:** in-app AI features; community ports to other platforms.

## Later-phase decisions

- **Sync:** Yjs over y-sweet or y-partyserver (Cloudflare Durable Objects); check y-sweet's maintenance status first. Documents stored in Cloudflare R2. Cloudflare D1 holds server metadata (accounts, sharing) only, never the app's primary data.
- **Auth:** WorkOS AuthKit with OAuth + PKCE via the system browser and an `aelomi://` deep link. Keep auth pluggable so self-hosters can use a simple access token. The local app has no login.
- **AI:** ship the MCP server first. Later, optional and off by default: bring-your-own subscription (drive the user's installed Codex / Claude Code CLIs) or API keys, in an isolated module. Check Anthropic's current terms before shipping any Claude-subscription feature.
- **Telemetry:** PostHog, anonymous hashed install ID, usage metadata only. Opt-out via an env var, a Settings toggle, and `DO_NOT_TRACK=1`. Documented in `docs/telemetry.md`.

## Distribution and infrastructure

- **No paid Apple Developer account.** Builds are ad-hoc signed.
- **Homebrew is the only supported install path:** tap repo `aelomi/homebrew-aelomi`, a cask whose `postflight` clears the quarantine flag. The release asset is a zipped `.app` on GitHub Releases (no DMG). A release workflow in this repo updates the cask's version and sha256. Updates via `brew upgrade`.
- **CI:** GitHub Actions on macOS runners.
- **Domain and site:** `aelomi.com` on Cloudflare (registrar + DNS). Marketing site in `site/`: static Astro on Cloudflare Workers static assets, deployed by GitHub Actions. It follows the same design rules as the app.
- **Cloudflare tooling:** use Cloudflare's `cf` CLI (npm package `cf`) for API work such as DNS; Wrangler only for Workers deploys.
- **Contact:** `hello@aelomi.com`.

## Ruled out

- **GPUI, Kotlin/Compose:** no rich-text block editor ecosystem; Compose is also heavy.
- **Electron:** too heavy.
- **tinyjs:** too young, QuickJS backend; revisit at 1.0.
- **Effect:** poor agent fluency, adds little to a UI layer.
- **Postgres:** needs a server process; SQLite fits a local app.
- **D1 as primary storage:** network-bound; local-first instead.
- **TanStack Query:** built for network data; **TanStack Start, Form, DB:** not needed.
- **Clerk:** smaller free tier, awkward for native desktop sign-in.
- **Midscene:** vision-based testing declined.
- **DMG installs, Apple notarization:** Homebrew only, no paid account.
