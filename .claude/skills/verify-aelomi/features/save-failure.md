# Save failure and quit confirmation

When a write to the data folder fails, Aelomi keeps the edit in memory and shows a banner under the header with `Changes aren't being saved.`, the reason, and `Your changes are kept in memory. Aelomi will keep trying.` The banner clears once every pending edit is on disk. Quitting while an edit is still pending opens the dialog `Quit with unsaved changes?` with **Keep app open** focused. **Keep app open** keeps the app and the pending edit. **Quit anyway** exits and loses the pending edit.

## Sub-features

- `save-banner` shows the banner with the reason while a write fails. A read-only `docs` folder gives `Aelomi doesn't have permission to write to its data folder.`
- `save-recovery` clears the banner after the folder is writable again and the retry succeeds.
- `quit-blocked` opens the dialog when a quit finds a pending edit, after a flush of up to 2 s. **Keep app open** has focus.
- `quit-keep-open` closes the dialog, keeps the app running, and keeps the pending edit and the banner.
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
- Aelomi's window is on the current Space, and `$S/app.sh tree "$RUN"` prints the web content.

- **Open the probe.** Run `$S/app.sh press "$RUN" "Storage probe"` and `$S/app.sh wait-status "$RUN" "Storage probe idle"`.
- **Make writes fail.** Run `chmod a-w "$RUN/data/docs"` before the first **Run probe**.
- **Edit while failing.** Run `$S/app.sh press "$RUN" "Run probe"`, then wait with `until grep -q '^storage event health={"Failing"' "$RUN/shell.log"; do sleep 0.2; done`. `shell.log` has `storage event health={"Failing":{"reason":"PermissionDenied","pending_docs":["probe"]}}`. The status reads `storage probe present=true marker=aelomi-dev-storage-probe-<id> before=[] ...`, because the edit is held in memory.
- **Banner.** Run `$S/app.sh screenshot "$RUN" failure`. `failure.ax.txt` has an `AXGroup/AXApplicationAlert` holding `Changes aren't being saved. Aelomi doesn't have permission to write to its data folder.` and `Your changes are kept in memory. Aelomi will keep trying.` `failure.png` shows the banner between the header and the probe.
- **Quit with ⌘Q.** Run `$S/app.sh keys "$RUN" q command`, then `$S/app.sh wait-dialog "$RUN" "Quit with unsaved changes?"`. It prints the dialog's title and warning. `shell.log` has `quit flush via=ExitRequested pending=[DocId(probe)]` and `storage event quit_blocked={"pending_docs":["probe"]}`.
- **Dialog.** Run `$S/app.sh screenshot "$RUN" dialog`. `dialog.ax.txt` has `AXGroup/AXApplicationAlertDialog "Quit with unsaved changes?"` with the warning, `AXButton "Keep app open"`, and `AXButton "Quit anyway"`. `dialog.png` shows a focus ring on **Keep app open** and the banner dimmed behind the dialog.
- **Keep app open.** Run `$S/app.sh press "$RUN" "Keep app open"` and `$S/app.sh screenshot "$RUN" kept`. `kept.ax.txt` has no alert dialog and still has the banner. `shell.log` has no `quit confirmed`.
- **Recovery.** Run `chmod u+w "$RUN/data/docs"`, then wait with `until grep -q '^storage event health="Healthy"' "$RUN/shell.log"; do sleep 0.2; done`. Run `$S/app.sh screenshot "$RUN" recovered`. `recovered.ax.txt` has no `AXApplicationAlert`.
- **Quit after recovery.** Run `$S/app.sh keys "$RUN" q command` and wait for the app PID to exit with `(. "$RUN/run.env"; while kill -0 "$APP_PID" 2>/dev/null; do sleep 0.2; done)`. `shell.log` ends with `quit flush via=ExitRequested pending=[]` and `quit flush via=Exit pending=[]`.
- **Close with ⌘W.** From a fresh failing state, run `$S/app.sh keys "$RUN" w command` and `$S/app.sh wait-dialog "$RUN" "Quit with unsaved changes?"`. `shell.log` has `quit flush via=CloseRequested pending=[DocId(probe)]`, and the window stays open.
- **Quit anyway.** From the dialog, run `$S/app.sh press "$RUN" "Quit anyway"` and wait for the app PID to exit. `shell.log` has `quit confirmed`, `quit allowed via=ExitRequested confirmed=true`, and a final `quit flush via=Exit pending=[DocId(probe)]` while the folder is still read-only.
- **Mock browser.** Run `pnpm exec playwright test tests/playwright/save-failure.spec.ts --project webkit` and `pnpm test:e2e`. Both pass. They cover Escape, outside clicks, recovery while the dialog is open, both themes, wrapping of a long reason, and a double click on **Quit anyway**. The mock injects the failure, so report these as mock-only.
- **Proof.** Keep `failure`, `dialog`, `kept`, and `recovered` with their `.ax.txt` files, plus `actions.log` and `shell.log`.

## Gotchas

- Change the `docs` folder's mode before the first **Run probe**. Once `docs/probe` exists, `chmod a-w "$RUN/data/docs"` alone does not make its appends fail.
- Run `chmod u+w "$RUN/data/docs"` before `cleanup`, so the run directory stays removable.
- A blocked quit freezes the window for up to 2 s while it flushes. Accessibility can report no window during that time. Wait with `wait-dialog`, not `sleep`.
- If `screenshot` or `keys` fails because accessibility reports no window, check the Spaces gotcha in `../SKILL.md` before you suspect the app.
- While the dialog is open, the background `role="status"` region leaves the accessibility tree, so `status` and `wait-status` fail. Use `wait-dialog`.
- `tree` shows an unnamed `AXButton` on each side of the dialog. These are focus guards. Press the dialog's buttons by name.
- No helper in this repo sends a quit Apple event to one PID. `osascript -e 'tell application "Aelomi" to quit'` can reach another Aelomi instance, so don't use it. Without a PID-targeted sender, report Dock **Quit** and logout as unreachable. Dock **Quit** and logout log `quit flush via=Terminate`. **Quit anyway** after a blocked logout quits only Aelomi and does not resume the logout.
- The app follows the macOS appearance. To capture both themes, switch System Settings > Appearance between shots, and restore the setting afterwards.
