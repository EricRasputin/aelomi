---
name: verify-aelomi
description: Launch and drive Aelomi, the macOS notes app in this repo, to prove a change works. The primary surface is the real Tauri 2 window, driven through the macOS accessibility tree, with window screenshots and the shell log as evidence. The secondary surface is the mock-IPC production build in Playwright WebKit. Use before you call a UI or IPC change done, when asked to run, click through, or screenshot the app, or when tests pass and you still need to see the behavior.
---

# Verify Aelomi

Aelomi has two surfaces an agent can drive. Prove every change on the real app. Add the mock browser when a flow needs DOM locators, a hover, the IPC mock's call log, or the network guard.

| Surface | What runs | Driven by | Proves |
|---|---|---|---|
| Real app | `pnpm dev:app` or the built `Aelomi.app`. A WKWebView in a Tauri 2 window, with real IPC to the Rust shell | `scripts/app.sh`, through the macOS accessibility tree | The UI, the IPC round trip, and the Rust side together |
| Mock browser | The production React build from `pnpm build:mock`, with the in-memory IPC mock from `src/ipc/mock.ts` | `scripts/mock-drive.mjs`, with Playwright WebKit | UI behavior against the mock, and no non-local requests (SHELL-13) |

No WebDriver exists for WKWebView on macOS. System Events can still reach the window's web content through the accessibility tree, and `AXPress` on a button runs its click handler.

Read [`features/README.md`](features/README.md) before you drive. Then read the feature file for the behavior you are proving.

Run every command from the repo root. The examples use `S=.claude/skills/verify-aelomi/scripts`. If your shell does not keep variables between commands, write the paths out.

## Rules

- Acquire the keyboard lock with `$S/ui-lock.sh acquire <label>` before `app.sh launch`, because a launch takes focus. Hold it before every key, click, menu press, focus change, or transient screenshot. Use a unique run label and release with `$S/ui-lock.sh release <label>` on every success, failure, and interruption path. Build outside the lock and release it between scenarios that need a rebuild.
- Run `$S/app.sh frontmost "$RUN"` immediately before every key or click, including `press`, `focus`, native menu AXPress, and direct System Events input. It compares the frontmost process PID with the run's app PID. If it fails, send nothing. A process name such as `aelomi` cannot identify the owned instance.
- Use only `ui-lock.sh` to manage `/tmp/aelomi-ui.lock`. It writes the owner only after its own `mkdir` succeeds, reclaims locks older than 30 minutes, and waits at most 45 minutes. Report a timeout as INCONCLUSIVE. Keep each interactive session under about 20 minutes.
- Never change the Mac's appearance or any other system setting, even to restore it. The maintainer uses this Mac during verification.
- If a system setting changes unexpectedly during your run or was left changed by a failed script, report it. Do not restore it. For appearance evidence, capture the current state and name it, even if historical evidence shows another appearance.
- Never touch an open system prompt. If one appears, run `$S/app.sh cleanup "$RUN"` for every owned run, release the keyboard lock, stop, and report it.
- If a required permission is missing or denied, run `$S/app.sh cleanup "$RUN"` before you stop, release the lock, and report it. Do not grant permission. Signs include `osascript` error `-1743`, "not allowed assistive access", or a failed doctor System Events check. Limit each Apple event to 60 seconds. If one hangs for 60 seconds, terminate only that owned helper, clean up the run, and stop.
- Before each variant's launch, run the appearance mapping below. After launch and any app-theme selection, capture an unlabeled check screenshot and inspect it before assigning a light or dark label. Never infer appearance from a filename.
- For either light or dark captures, use the app's theme controls where the branch has them. SHELL-5 specifies **System**, **Light**, and **Dark** in **Settings**. CMD-8 specifies **Toggle theme** in ⌘K. Label these as app-theme captures. The native menu bar and title bar still follow the Mac's appearance.
- Alternatively, use the mock browser with Playwright's `page.emulateMedia({ colorScheme: "light" })` or `page.emulateMedia({ colorScheme: "dark" })`. If neither route is available, capture only the current appearance and name it in the evidence.

## Before the first run

- Run `pnpm install --frozen-lockfile`.
- The app that runs your shell (Terminal, an IDE, or an agent host) must already have **Accessibility**, **Screen Recording**, and **Automation (System Events)** permissions. `app.sh doctor` runs the System Events (Accessibility and Automation) check. Screen Recording has no check, so look at every screenshot. If a screenshot does not show the Aelomi window, Screen Recording is missing. For missing permissions or a macOS permission prompt, stop and report as required by the [rules](#rules).
- For the mock browser, install WebKit once with `pnpm exec e2e-web install webkit`.

## Launch the real app

Build first without the keyboard lock, for example with `pnpm build` and `cargo build -p aelomi`. Before each variant's launch, map the current Mac appearance without changing it:

```bash
MAC_APPEARANCE=$(defaults read -g AppleInterfaceStyle 2>/dev/null || true)
case "$MAC_APPEARANCE" in
  Dark) MAC_SCHEME=dark ;;
  "") MAC_SCHEME=light ;;
  *) echo "Unrecognized appearance: $MAC_APPEARANCE" >&2; exit 1 ;;
esac
printf 'System theme maps to %s\n' "$MAC_SCHEME"
```

System uses that mapping. An explicit app theme uses its selected Light or Dark value. Confirm either mapping against an unlabeled check screenshot after launch.

To launch the dev build, acquire the lock and install the release trap first. Run the session in a shell that exits on failed commands, such as a subshell with `set -e`:

```bash
set -e
LABEL="verify-$$"
$S/ui-lock.sh acquire "$LABEL"
trap '$S/ui-lock.sh release "$LABEL"' EXIT
trap 'exit 1' HUP INT TERM
RUN=$($S/app.sh launch dev)
trap '$S/app.sh cleanup "$RUN"; $S/ui-lock.sh release "$LABEL"' EXIT
```

`launch dev` runs `pnpm dev:app`, which prints `AELOMI_DEV_PORT=N` and runs `tauri dev` with the UI on port N. `launch` waits for the line `ping seq=1` in the shell log. The UI sends its first `ping` command when it mounts, and the Rust handler prints that line. So the line means the window, the UI, and the IPC round trip are up. `launch` then waits until the web content appears in the accessibility tree, and prints the run directory on stdout, for example `~/.local/state/aelomi-verify/20261011-064618-dev-ik2v`. Pass that directory to every other command.

Each launch sets `AELOMI_RECOVERY_DIR=<run dir>/recovery` unless explicitly supplied. Pass the same recovery directory when relaunching a refused synthetic path. The directory stays outside the data folder.

Each launch sets `AELOMI_DATA_DIR=<run dir>/data`, so every run, dev or release, keeps its data in its own folder and never touches `~/Library/Application Support`. The shell log's first storage line names the resolved folder, for example `storage status=Ready path=$HOME/.local/state/aelomi-verify/<run>/data`. To launch on an existing folder, set the variable yourself. For example, `AELOMI_DATA_DIR=$OLD_RUN/data $S/app.sh launch dev` relaunches on an earlier run's data, or, while that run is still open, logs `storage status=Locked`. Debug builds also read `AELOMI_CLOUD_ROOTS`, a `:`-separated list that replaces `~/Library/Mobile Documents` and `~/Library/CloudStorage`. Point it at a fake folder to prove the cloud folder refusal without touching the real `~/Library`.

The first build compiles about 350 crates and took 59 s on an Apple Silicon Mac. Later launches reuse it. `launch` waits up to 900 s. Change the limit with `--timeout S`. It fails early, and prints the end of the log, if the launch process exits, Rust fails to compile, or the app panics.

Each `launch dev` serves the UI on its own free port and records it as `DEV_PORT` in `run.env`, so several dev instances can run at once, from one checkout or several. To choose the port, run `AELOMI_DEV_PORT=1500 $S/app.sh launch dev`. If that port is in use, Vite exits and the launch fails within seconds. The port is compiled into the app, so a launch on a new port relinks the `aelomi` crate, which took 5 to 7 s. Launches from one checkout share `target/`, so a second launch waits for the first build.

To launch the release build, run:

```bash
set -e
pnpm tauri build --bundles app
LABEL="verify-$$"
$S/ui-lock.sh acquire "$LABEL"
trap '$S/ui-lock.sh release "$LABEL"' EXIT
trap 'exit 1' HUP INT TERM
RUN=$($S/app.sh launch app)
trap '$S/app.sh cleanup "$RUN"; $S/ui-lock.sh release "$LABEL"' EXIT
```

`launch app` runs the executable inside `target/release/bundle/macos/Aelomi.app` directly, so its stdout lands in the shell log and its PID is known. `open Aelomi.app` loses both. The release app uses no port, so it can run beside a dev instance. `doctor` fails when a source file is newer than the bundle.

## Check the instance with doctor

```bash
$S/app.sh doctor "$RUN"
```

Run it after launch and whenever a command behaves oddly. It changes nothing, writes `doctor.txt` into the run directory, and exits 1 if any check fails. It checks that:

- The launch process and the app process still run the commands this run started.
- The System Events (Accessibility and Automation) check succeeds.
- The app window is on screen, and its web content has a `role="status"` region or an alert dialog. A modal dialog hides the background status from accessibility while it is open.
- For `dev`, a process from this run listens on the run's `DEV_PORT`. For `app`, the bundle is newer than `src/`, `src-tauri/src/`, `crates/`, and `index.html`.
- The shell log has `ping seq=1`.

If the System Events (Accessibility and Automation) check fails, run cleanup before you stop and report as required by the [rules](#rules). A relaunch cannot grant permission. For other failed checks, do not drive the instance. Run cleanup and launch again. Apply the [rules](#rules) if a permission prompt appears during any check.

## Drive the real app

`app.sh` finds the window by the app's PID, never by process name, so it cannot drive another Aelomi instance by mistake. System Events returns child elements as references through the process name, which every Aelomi instance shares, so `ax.js` builds each child reference by index from the PID instead.

| Command | Effect |
|---|---|
| `$S/app.sh frontmost "$RUN"` | Exit 0 only when the frontmost process has this run's app PID. On failure, send no input |
| `$S/app.sh tree "$RUN"` | Print the web content's accessibility tree |
| `$S/app.sh status "$RUN"` | Print the text of the `role="status"` region |
| `$S/app.sh wait-status "$RUN" TEXT [S]` | Wait up to S seconds (default 10) until the status reads TEXT. On timeout, exit 1 and print the last text |
| `$S/app.sh wait-dialog "$RUN" TITLE [S]` | Wait up to S seconds for the alert dialog with TITLE. Use it after a native quit, before a screenshot |
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

Prefer `press` for named elements. Although AXPress can act on a background window, require the lock and a successful `frontmost` check before acting. The `ax.js` commands that inspect a window or act on its content then make Aelomi frontmost and wait up to 5 s for its window. Use `focus` and `keys` only to prove a keyboard path. `keys` brings Aelomi to the front and types into the focused element. Tab and Option-Tab did not move focus to the button in this WKWebView, so set focus with `focus` first.

Wait for state with `wait-status`, not `sleep`. The IPC reply arrives after `press` returns.

This example proves the IPC round trip from [`features/ipc-status.md`](features/ipc-status.md):

```bash
(
set -e
S=.claude/skills/verify-aelomi/scripts
LABEL="ipc-proof-$$"
$S/ui-lock.sh acquire "$LABEL"
trap '$S/ui-lock.sh release "$LABEL"' EXIT
trap 'exit 1' HUP INT TERM
RUN=$($S/app.sh launch dev)
trap '$S/app.sh cleanup "$RUN"; $S/ui-lock.sh release "$LABEL"' EXIT
$S/app.sh doctor "$RUN"
$S/app.sh wait-status "$RUN" "IPC ok · core 0.1.0 · seq 1"
$S/app.sh screenshot "$RUN" before
$S/app.sh frontmost "$RUN"
$S/app.sh press "$RUN" "Ping again"
$S/app.sh wait-status "$RUN" "IPC ok · core 0.1.0 · seq 2"
$S/app.sh screenshot "$RUN" after
grep '^ping seq=' "$RUN/shell.log"
)
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

Every run gets its own directory under `~/.local/state/aelomi-verify/` (or `$XDG_STATE_HOME/aelomi-verify/`), which survives a reboot, sits outside every checkout, and is never synced by iCloud. Set `AELOMI_VERIFY_ROOT` to use another root. Cleanup never deletes a run directory. Delete runs you no longer need yourself with `rm -rf`, but keep any run a PR names until that PR merges. Run directories are local evidence. [Native verification evidence](../../../docs/native-verification-evidence.md) says what to attach to the PR and what, if anything, to commit.

A real-app run directory holds these files:

| File | Contents |
|---|---|
| `shell.log` | Everything `pnpm dev:app` or the app printed, including one `ping seq=N` line per IPC call |
| `doctor.txt` | The latest doctor result |
| `actions.log` | Each `press`, `focus`, `keys`, and `screenshot`, with the time |
| `NAME.png` | The window, captured with `screencapture -l <window id>` |
| `NAME.ax.txt` | The window ID, title, and bounds, and the accessibility tree when the screenshot was taken |
| `run.env`, `pids` | The launch state that `doctor` and `cleanup` read |
| `data/` | The run's data folder, unless the launch set `AELOMI_DATA_DIR` |

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

With the lock held, run `$S/app.sh frontmost "$RUN"` first. If it succeeds, run `$S/app.sh keys "$RUN" q command` and wait for the app PID to exit. The shell log then shows `quit flush via=ExitRequested pending=[]`. `cleanup` is not a quit: its SIGTERM skips that flush.

`cleanup` stops every process the launch started: pnpm, the `dev:app` launcher, the Tauri CLI, Vite, and the app. It reads the process list that `launch` recorded, and it signals a PID only while that PID still runs the recorded command. It sends SIGTERM, and SIGKILL to anything left after 10 s. It keeps the run directory and lists its files. Running it twice is safe. Run it after every attempt, including a failed launch, before you launch again. For a failed launch, use the run directory printed in its error. Release the keyboard lock after cleanup.

Never kill Aelomi, Vite, or node by name. Another agent or the maintainer may be running their own instance.

`mock-drive.mjs` stops its own server and browser, also on Ctrl-C.

## Helpers

- `scripts/ui-lock.sh` manages the keyboard lock and requires Python 3. `status` prints `free` or the owner label. For isolated tests, set `AELOMI_UI_LOCK` to a temporary path and optionally shorten `AELOMI_UI_LOCK_WAIT_SECONDS`. A persistent sibling `.guard` file serializes filesystem transitions. Its advisory lock releases automatically if a helper exits.
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
- A native quit can block the main thread for its 2 s flush. Accessibility can report no window during that interval. Wait with `wait-dialog` before capturing a blocked quit.
- Accessibility lists only windows on the current Space. When Aelomi's window is on another Space, `ax.js` commands that inspect a window or act on its content make Aelomi frontmost, which switches Spaces, and wait up to 5 s for the window. The switch took about 2 s in the proof run. Another app can take the front back between two commands, so a command can still fail with `The window has no web content yet.` Rerun it. This is not a product failure. `screencapture -l` captures the window from any Space, but the tree in `NAME.ax.txt` needs the switch.
- During a `launch dev` run, the dev server reloads the app's UI when an HTML file under `playwright-report/`, `.e2e/`, or `dist-mock/` changes. The reload clears in-memory UI state, such as an open dialog. Don't run the WebKit suite, `pnpm test:e2e`, `pnpm build:mock`, or `mock-drive.mjs` until that run is cleaned up.
- `pnpm build:mock` empties `dist-mock/` before it writes the new build. Do not pass `--rebuild` while another mock run in the same checkout is serving `dist-mock/`.
- The mock browser renders light unless the flow emulates dark. By default, the real app follows the Mac's current appearance. Read it without changing it with `defaults read -g AppleInterfaceStyle`. The command prints `Dark` in dark mode and exits non-zero in light mode. For capture choices, follow the [rules](#rules).
- An accessibility query takes 1 to 2 s, because each element read is a separate System Events call. Expect `tree` to slow down as pages grow.

To keep the feature map current as the app changes, use `/maintain-verification-skill`.
