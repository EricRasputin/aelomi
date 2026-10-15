# Save failure and quit confirmation

When a write to the data folder fails, Aelomi keeps the edit in memory and shows a banner under the header with `Changes aren't being saved.`, the reason, and `Your changes are kept in memory. Aelomi will keep trying.` The banner clears once every pending edit is on disk. Quitting while an edit is still pending opens the dialog `Quit with unsaved changes?` with **Keep app open** focused. **Keep app open** keeps the app and the pending edit. **Quit anyway** exits and loses the pending edit.

## Sub-features

- `save-banner` shows the banner with the reason while a write fails. A read-only `docs` folder gives `Aelomi doesn't have permission to write to its data folder.`
- `save-recovery` clears the banner after the folder is writable again and the retry succeeds.
- `quit-settings` names unsaved settings when a settings write is pending.
- `quit-blocked` opens the dialog when a quit finds a pending edit, after a flush of up to 2 s. **Keep app open** has focus.
- `quit-keep-open` closes the dialog, keeps the app running, and keeps the pending edit and the banner.
- `quit-reload` keeps an unanswered dialog across a reload of the app's UI, and keeps an answered one closed.
- `quit-anyway` exits once. The edit that was still pending is not on disk.
- `quit-paths` blocks every quit path: ⌘Q and the app menu's **Quit Aelomi**, ⌘W and the close button, and Dock **Quit**, an AppleScript `quit`, and logout.

## How to get to it (user POV)

- Make the data folder unwritable, then edit. The MVP has no page editor yet, so in a dev build the **Storage probe** link on the start screen opens a page whose **Run probe** button makes one edit.
- Quit with ⌘Q or the app menu's **Quit Aelomi** while the banner shows.
- Close the window with ⌘W or the red close button while the banner shows.
- Choose **Quit** from the Dock icon's menu, or log out, while the banner shows.
- In the dialog, press **Keep app open** or **Quit anyway**.

## Driving it with app.sh and mock-drive

Preconditions:

- `RUN=$($S/app.sh launch dev)` printed a run directory, and `$S/app.sh doctor "$RUN"` reports every line as `ok`. A release build has no **Storage probe** link.
- `shell.log` has `storage status=Ready path=<run dir>/data`, and **Run probe** has not been pressed yet.
- `$S/app.sh tree "$RUN"` prints the web content.

- **Open the probe.** Run `$S/app.sh press "$RUN" "Storage probe"` and `$S/app.sh wait-status "$RUN" "Storage probe idle"`.
- **Make writes fail.** Run `chmod a-w "$RUN/data/docs"` before the first **Run probe**.
- **Edit while failing.** Run `$S/app.sh press "$RUN" "Run probe"`, then wait up to 20 s with `for _ in $(seq 100); do grep -q '^storage event health={"Failing"' "$RUN/shell.log" && break; sleep 0.2; done; grep '^storage event health={"Failing"' "$RUN/shell.log"`. The last `grep` exits 1 on a timeout. `shell.log` has `storage event health={"Failing":{"reason":"PermissionDenied","pending_docs":["probe"]}}`. The status reads `storage probe present=true marker=aelomi-dev-storage-probe-<id> before=[] ...`, because the edit is held in memory.
- **Banner.** Run `$S/app.sh screenshot "$RUN" failure`. `failure.ax.txt` has an `AXGroup/AXApplicationAlert` holding `Changes aren't being saved. Aelomi doesn't have permission to write to its data folder.` and `Your changes are kept in memory. Aelomi will keep trying.` `failure.png` shows the banner between the header and the probe.
- **Quit with ⌘Q.** Run `$S/app.sh keys "$RUN" q command`, then `$S/app.sh wait-dialog "$RUN" "Quit with unsaved changes?"`. It prints the dialog's title and warning. `shell.log` has `quit flush via=ExitRequested pending=[DocId(probe)]` and `storage event quit_status={"revision":1,"pending_docs":["probe"]}`.
- **Dialog.** Run `$S/app.sh screenshot "$RUN" dialog`. `dialog.ax.txt` has `AXGroup/AXApplicationAlertDialog "Quit with unsaved changes?"` with the warning, `AXButton "Keep app open"`, and `AXButton "Quit anyway"`. `dialog.png` shows a focus ring on **Keep app open** and the banner dimmed behind the dialog.
- **Keep app open.** Run `$S/app.sh press "$RUN" "Keep app open"` and `$S/app.sh screenshot "$RUN" kept`. `kept.ax.txt` has no alert dialog and still has the banner. `shell.log` has `storage event quit_status={"revision":2,"pending_docs":null}` and no `quit confirmed`.
- **Reload.** From the failing state, quit with ⌘Q and wait for the dialog. Reload the UI with `touch index.html`, which makes the dev server reload the page without changing the file's content, then run `$S/app.sh wait-dialog "$RUN" "Quit with unsaved changes?"`. The dialog returns from Rust's retained decision, and no new `quit flush` line appears. Press **Keep app open**, reload the same way, and run `$S/app.sh wait-status "$RUN" "Storage probe idle"`. `$S/app.sh tree "$RUN"` has no alert dialog.
- **Recovery.** Run `chmod u+w "$RUN/data/docs"`, then wait up to 20 s with `for _ in $(seq 100); do grep -q '^storage event health="Healthy"' "$RUN/shell.log" && break; sleep 0.2; done; grep '^storage event health="Healthy"' "$RUN/shell.log"`. Run `$S/app.sh screenshot "$RUN" recovered`. `recovered.ax.txt` has no `AXApplicationAlert`.
- **Quit after recovery.** Run `$S/app.sh keys "$RUN" q command` and wait up to 15 s for the app PID to exit with `(. "$RUN/run.env"; for _ in $(seq 75); do kill -0 "$APP_PID" 2>/dev/null || exit 0; sleep 0.2; done; echo "app $APP_PID still running after 15 s"; exit 1)`. `shell.log` ends with `quit flush via=ExitRequested pending=[]` and `quit flush via=Exit pending=[]`.
- **Close with ⌘W.** From the failing state, run `$S/app.sh keys "$RUN" w command` and `$S/app.sh wait-dialog "$RUN" "Quit with unsaved changes?"`. `shell.log` has `quit flush via=CloseRequested pending=[DocId(probe)]` and a `storage event quit_status=` line with `"pending_docs":["probe"]`. The window stays open with the dialog. Press **Keep app open** before the next path.
- **Quit Apple event.** From the failing state, send a quit Apple event to the app PID with a PID-targeted sender (see Gotchas), then run `$S/app.sh wait-dialog "$RUN" "Quit with unsaved changes?"`. `shell.log` has `quit flush via=Terminate pending=[DocId(probe)]` and a `quit_status` line with `"pending_docs":["probe"]`. The sender gets reply error `-128` because the quit was cancelled. The same event with the logout reason gives the same lines.
- **Quit anyway.** From the dialog, run `$S/app.sh press "$RUN" "Quit anyway"` and wait up to 15 s for the app PID to exit with the loop from **Quit after recovery**. The app exited 5 s after the press in the proof run. `shell.log` has one `quit confirmed`, then `quit allowed via=ExitRequested confirmed=true`, then a final `quit flush via=Exit pending=[DocId(probe)]` while the folder is still read-only. No `quit_status` line with pending docs and no other `quit flush` line follow `quit confirmed`. `data/docs` stays empty, because the pending edit is lost.
- **Mock browser.** Run `pnpm exec playwright test tests/playwright/save-failure.spec.ts --project webkit` and `pnpm test:e2e`. Both pass. They cover Escape, outside clicks, recovery while the dialog is open, both themes, wrapping of a long reason, and a double click on **Quit anyway**. The mock injects the failure, so report these as mock-only. Run them only while no `launch dev` run is active.
- **Proof.** Keep the before and dialog screenshot of each path with its `.ax.txt` file, plus `actions.log` and `shell.log`.

## Gotchas

- Change the `docs` folder's mode before the first **Run probe**. Once `docs/probe` exists, `chmod a-w "$RUN/data/docs"` alone does not make its appends fail.
- Run `chmod u+w "$RUN/data/docs"` before `cleanup`, so the run directory stays removable.
- A blocked quit freezes the window for up to 2 s while it flushes. Accessibility can report no window during that time. Wait with `wait-dialog`, not `sleep`.
- If `screenshot` or `keys` fails because accessibility reports no window, check the Spaces gotcha in `../SKILL.md` before you suspect the app.
- While the dialog is open, the background `role="status"` region leaves the accessibility tree, so `status` and `wait-status` fail. Use `wait-dialog`.
- `tree` shows an unnamed `AXButton` on each side of the dialog. These are focus guards. Press the dialog's buttons by name.
- No helper in this repo sends a quit Apple event to one PID. `osascript -e 'tell application "Aelomi" to quit'` can reach another Aelomi instance, so don't use it. A sender must address `kAEQuitApplication` to the process ID (`typeKernelProcessID`) with `AESendMessage`, and add `kAEQuitReason` set to `kAEReallyLogOut` for the logout case. That event only asks Aelomi to quit. It does not log the Mac out. Without such a sender, report the Apple event and logout paths as unreachable. Dock **Quit** also logs `via=Terminate`, but it needs the person's pointer. **Quit anyway** after a blocked logout quits only Aelomi and does not resume the logout.
- Never run the WebKit suite, `pnpm test:e2e`, `pnpm build:mock`, or `mock-drive.mjs` during a `launch dev` run. They rewrite files under `playwright-report/`, `.e2e/`, and `dist-mock/`. When an HTML file there changes, the dev server reloads the app's UI, which clears in-memory UI state such as an open quit dialog. The edit stays pending in Rust, but the run no longer proves the dialog.
- The app follows the macOS appearance. To capture both themes, switch System Settings > Appearance between shots, and restore the setting afterwards.
