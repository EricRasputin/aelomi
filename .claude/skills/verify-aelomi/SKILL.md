---
name: verify-aelomi
description: Launch and drive Aelomi, the macOS notes app in this repo, to prove a change works. The primary surface is the real Tauri 2 window, driven through the macOS accessibility tree, with window screenshots and the shell log as evidence. The secondary surface is the mock-IPC production build in Playwright WebKit. Use before you call a UI or IPC change done, when asked to run, click through, or screenshot the app, or when tests pass and you still need to see the behavior.
---

# Verify Aelomi

Aelomi has two surfaces an agent can drive. Prove every change on the real app. Add the mock browser when a flow needs DOM locators, a hover, the IPC mock's call log, or the network guard.

| Surface | What runs | Driven by | Proves |
|---|---|---|---|
| Real app | `pnpm tauri dev` or the built `Aelomi.app`. A WKWebView in a Tauri 2 window, with real IPC to the Rust shell | `scripts/app.sh`, through the macOS accessibility tree | The UI, the IPC round trip, and the Rust side together |
| Mock browser | The production React build from `pnpm build:mock`, with the in-memory IPC mock from `src/ipc/mock.ts` | `scripts/mock-drive.mjs`, with Playwright WebKit | UI behavior against the mock, and no non-local requests (SHELL-13) |

No WebDriver exists for WKWebView on macOS. System Events can still reach the window's web content through the accessibility tree, and `AXPress` on a button runs its click handler.

Read [`features/README.md`](features/README.md) before you drive. Then read the feature file for the behavior you are proving.

Run every command from the repo root. The examples use `S=.claude/skills/verify-aelomi/scripts`. If your shell does not keep variables between commands, write the paths out.

## Before the first run

- Run `pnpm install --frozen-lockfile`.
- In System Settings > Privacy & Security, give the app that runs your shell (Terminal, an IDE, or an agent host) **Accessibility** and **Screen Recording**. `app.sh doctor` checks Accessibility. Screen Recording has no check, so look at every screenshot. If a screenshot does not show the Aelomi window, Screen Recording is missing.
- For the mock browser, install WebKit once with `pnpm exec e2e-web install webkit`.

## Launch the real app

To launch the dev build, run:

```bash
RUN=$($S/app.sh launch dev)
```

`launch dev` runs `pnpm tauri dev` and waits for the line `ping seq=1` in the shell log. The UI sends its first `ping` command when it mounts, and the Rust handler prints that line. So the line means the window, the UI, and the IPC round trip are up. `launch` then waits until the web content appears in the accessibility tree, and prints the run directory on stdout, for example `/tmp/aelomi-verify/20261011-064618-dev-ik2v`. Pass that directory to every other command.

The first build compiles about 350 crates and took 59 s on an Apple Silicon Mac. Later launches reuse it. `launch` waits up to 900 s. Change the limit with `--timeout S`. It fails early, and prints the end of the log, if the launch process exits, Rust fails to compile, or the app panics.

`launch dev` refuses to start if port 1420 is in use. `vite.config.ts` serves the dev UI on 1420 with `strictPort`, and `src-tauri/tauri.conf.json` points the window at it, so only one dev instance can run per machine. If one of your runs holds the port, clean it up. If you did not start it, leave it alone and use the built app or the mock browser.

To launch the release build, run:

```bash
pnpm tauri build --bundles app
RUN=$($S/app.sh launch app)
```

`launch app` runs the executable inside `target/release/bundle/macos/Aelomi.app` directly, so its stdout lands in the shell log and its PID is known. `open Aelomi.app` loses both. The release app uses no port, so it can run beside a dev instance. `doctor` fails when a source file is newer than the bundle.

## Check the instance with doctor

```bash
$S/app.sh doctor "$RUN"
```

Run it after launch and whenever a command behaves oddly. It changes nothing, writes `doctor.txt` into the run directory, and exits 1 if any check fails. It checks that:

- The launch process and the app process still run the commands this run started.
- System Events has Accessibility access.
- The app window is on screen, and its web content has a `role="status"` region.
- For `dev`, a process from this run listens on port 1420. For `app`, the bundle is newer than `src/`, `src-tauri/src/`, `crates/`, and `index.html`.
- The shell log has `ping seq=1`.

If a check fails, do not drive the instance. Run cleanup and launch again.

## Drive the real app

`app.sh` finds the window by the app's PID, never by process name, so it cannot drive another Aelomi instance by mistake.

| Command | Effect |
|---|---|
| `$S/app.sh tree "$RUN"` | Print the web content's accessibility tree |
| `$S/app.sh status "$RUN"` | Print the text of the `role="status"` region |
| `$S/app.sh wait-status "$RUN" TEXT [S]` | Wait up to S seconds (default 10) until the status reads TEXT. On timeout, exit 1 and print the last text |
| `$S/app.sh press "$RUN" NAME` | Send `AXPress` to the first element named NAME |
| `$S/app.sh focus "$RUN" NAME` | Focus the first element named NAME |
| `$S/app.sh keys "$RUN" TEXT [MODS]` | Bring Aelomi to the front and type TEXT. MODS is a comma list of `command`, `shift`, `option`, and `control` |
| `$S/app.sh menus "$RUN"` | Print the native menu bar titles |
| `$S/app.sh screenshot "$RUN" NAME` | Save the window to `NAME.png`, and its bounds and tree to `NAME.ax.txt` |

Use `tree` to find names. In this app, WebKit maps the DOM to these accessibility roles:

| DOM | Accessibility tree |
|---|---|
| `<header>` | `AXGroup/AXLandmarkBanner` |
| `<main>` | `AXGroup/AXLandmarkMain` |
| `<h1>` | `AXHeading` with value `1` |
| `role="status"` | `AXGroup/AXApplicationStatus` |
| `<button>Ping again</button>` | `AXButton "Ping again"` |
| Text | `AXStaticText` |

`press` and `focus` match an element's accessibility name or description and skip static text. If nothing matches, they exit 1 and list the named elements.

Prefer `press`. It works while the window is in the background, and it does not take focus from the person at the keyboard. Use `focus` and `keys` only to prove a keyboard path. `keys` brings Aelomi to the front and types into the focused element. Tab and Option-Tab did not move focus to the button in this WKWebView, so set focus with `focus` first.

Wait for state with `wait-status`, not `sleep`. The IPC reply arrives after `press` returns.

This example proves the IPC round trip from [`features/ipc-status.md`](features/ipc-status.md):

```bash
S=.claude/skills/verify-aelomi/scripts
RUN=$($S/app.sh launch dev)
$S/app.sh doctor "$RUN"
$S/app.sh wait-status "$RUN" "IPC ok · core 0.1.0 · seq 1"
$S/app.sh screenshot "$RUN" before
$S/app.sh press "$RUN" "Ping again"
$S/app.sh wait-status "$RUN" "IPC ok · core 0.1.0 · seq 2"
$S/app.sh screenshot "$RUN" after
grep '^ping seq=' "$RUN/shell.log"
$S/app.sh cleanup "$RUN"
```

The `0.1.0` is `aelomi_core::VERSION`, the workspace version in `Cargo.toml`. If the version changes, read the current text with `status` first.

## Drive the mock browser

```bash
$S/mock-drive.mjs [--port N] [--flow FILE] [--out DIR] [--rebuild] [--headed]
```

`mock-drive.mjs` does the whole run:

1. It picks a free port, or uses `--port N`. It refuses port 4173, which `playwright.config.ts` uses, and any port already in use.
2. It runs `pnpm build:mock` if `dist-mock/index.html` is missing or older than `src/`, `index.html`, `vite.config.ts`, or `package.json`. `--rebuild` forces the build.
3. It serves `dist-mock/` on `127.0.0.1` with the Vite command from the `preview:mock` script.
4. It opens Playwright WebKit with service workers blocked, and installs the non-local request guard from `tests/playwright/fixtures.ts`, using `tests/is-local-url.ts`.
5. It runs the flow, saves the mock's call log, and fails if the page made any non-local request.
6. It stops the server and records in `summary.json` whether the port is free again.

It prints `PASS` or `FAIL` and the evidence directory, and exits 0 or 1. Each run serves its own port, so several mock runs and a real-app run can go side by side.

The shipped flows live in `scripts/flows/`:

- `ping.mjs` is the default. It checks `seq 1`, clicks **Ping again**, checks `seq 2`, and checks that the mock received `ping` with `seq` 1 and then 2.
- `tooltip.mjs` hovers **Ping again** and checks the tooltip text.

To prove something else, write a flow module and pass it with `--flow`. Its default export receives `{ page, expect, shot, out }`:

- `page` is a Playwright page whose base URL is the preview server.
- `expect` is Playwright's `expect`.
- `shot(name)` saves `name.png` and `name.aria.yml`, an ARIA snapshot of the page.
- `out` is the evidence directory.

A thrown error or a failed `expect` fails the run.

```js
export default async function start({ page, expect, shot }) {
  await page.goto("/");
  await expect(page.getByRole("status")).toHaveText("IPC ok · core 0.1.0-mock · seq 1");
  await shot("start");
}
```

Use role and name locators, as the browser suites do. In mock builds the status reads `core 0.1.0-mock`, from `src/ipc/mock.ts`. Keep a one-off flow outside the repo. Add a flow to `scripts/flows/` when a feature file uses it.

## Capture evidence

Every run gets its own directory under `/tmp/aelomi-verify/`. Set `AELOMI_VERIFY_ROOT` to use another root. Cleanup never deletes a run directory.

A real-app run directory holds these files:

| File | Contents |
|---|---|
| `shell.log` | Everything `pnpm tauri dev` or the app printed, including one `ping seq=N` line per IPC call |
| `doctor.txt` | The latest doctor result |
| `actions.log` | Each `press`, `focus`, `keys`, and `screenshot`, with the time |
| `NAME.png` | The window, captured with `screencapture -l <window id>` |
| `NAME.ax.txt` | The window ID, title, and bounds, and the accessibility tree when the screenshot was taken |
| `run.env`, `pids` | The launch state that `doctor` and `cleanup` read |

`screencapture -l` captures the window itself, even when other windows cover it. On a Retina display the image is at 2x, so the 960 by 640 window gives a 1920 by 1280 PNG.

A mock-browser run directory holds `NAME.png`, `NAME.aria.yml`, `preview.log`, and `summary.json`, which has the result, the port, the mock's calls, and any non-local requests. A failed flow also leaves `failure.png`.

Hold every proof to these standards:

- Drive the real user path. Press the button the user presses. Do not call stores, commands, or `window.__aelomiMock` to cause a change. Reading the mock's call log afterwards is fine.
- Capture the state before the action and after it, not only the final screen.
- Check the side effect next to what is visible. For an IPC call, that is the `ping seq=N` line in `shell.log` on the real app, or the mock's calls in `summary.json`.
- Look at every screenshot yourself and say what it shows. A file that exists is not proof.
- The mock browser proves the UI against a fake. A claim about the Rust side needs the real app.
- Name the run directory and its files in your report.

## Clean up

```bash
$S/app.sh cleanup "$RUN"
```

`cleanup` stops every process the launch started: pnpm, the Tauri CLI, Vite, and the app. It reads the process list that `launch` recorded, and it signals a PID only while that PID still runs the recorded command. It sends SIGTERM, and SIGKILL to anything left after 10 s. It keeps the run directory and lists its files. Running it twice is safe. Run it after every attempt, including a failed launch, before you launch again.

Never kill Aelomi, Vite, or node by name. Another agent or the maintainer may be running their own instance.

`mock-drive.mjs` stops its own server and browser, also on Ctrl-C.

## Helpers

- `scripts/app.sh` drives the real app. Run `$S/app.sh` with no arguments for usage.
- `scripts/ax.js` is the JXA script behind `app.sh`. It takes a PID instead of a run directory, for example `osascript -l JavaScript $S/ax.js press <pid> "Ping again"`. Run it with no arguments for usage.
- `scripts/mock-drive.mjs` runs one mock-browser flow. Run it with `--help` for usage.
- `scripts/flows/ping.mjs` and `scripts/flows/tooltip.mjs` are the shipped flows.

## Gotchas

- pnpm starts each script shell in its own process group, so killing the launch's process group leaves Vite and the app running. `cleanup` uses the recorded process tree instead.
- If Vite dies first, the Tauri CLI exits and the app keeps running with no parent. The recorded list still covers it.
- After a Rust compile error, `tauri dev` keeps running and waits for a fix. `launch` stops waiting when the log shows `could not compile`, so clean up, fix the code, and launch again.
- The first accessibility query after launch can find no web content. `launch` retries until the status region appears.
- `src/main.tsx` wraps the app in `StrictMode`, which runs effects twice in dev. The ping store ignores a ping while one is pending, so the dev log still starts with one `ping seq=1`.
- `pnpm build:mock` empties `dist-mock/` before it writes the new build. Do not pass `--rebuild` while another mock run in the same checkout is serving `dist-mock/`.
- The mock browser renders the light theme, Playwright's default color scheme. The real app follows the macOS appearance, which was dark in this repo's proof run.
- An accessibility query takes 1 to 2 s, because each element read is a separate System Events call. Expect `tree` to slow down as pages grow.

To keep the feature map current as the app changes, use `/maintain-verification-skill`.
