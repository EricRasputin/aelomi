# Getting started

A fresh data folder opens one page titled **Getting started** with three editable paragraphs. Reopening that folder keeps the same page and edits. SHELL-2 does not seed an empty workspace after its pages are trashed or erased.

## Sub-features

- `seed-first-open` shows **Getting started** and the three instructions on first launch.
- `seed-editable` accepts typing in **Page body**.
- `seed-deferred` keeps the app open without a first page when its first body write fails, with the normal save-failure banner and retry. Corrupt workspace history never creates a first page.
- `record-quit-retry` retains a failed rename across repeated normal quits, shows the save-failure banner for pre-acceptance failures, retries after storage recovers, clears the banner after durable recovery, and preserves the rename after relaunch.
- `record-terminal-failure` keeps a terminal retry failure in the save-failure banner until the user dismisses it or a later edit of that page succeeds.
- `record-settings-quit` keeps a rename and a settings probe write issued together before ⌘Q after relaunch.
- `record-quit` keeps a page rename issued just before ⌘Q after relaunch.
- `seed-relaunch` opens the same page with the typed text after a normal quit and relaunch.

## How to get to it (user POV)

- Start Aelomi with a fresh data folder.
- Quit with ⌘Q, then open Aelomi again on that folder.

## Driving it with app.sh and mock-drive

Preconditions:

- Hold `/tmp/aelomi-ui.lock` for every key, focus, press and transient screenshot step when other agents can drive the Mac. Release it between scenarios and on failure.
- Launch the real app with a fresh isolated folder and require every doctor check to pass.
- Use synthetic text only. The mock starts empty according to each test's setup and does not run the native folder seed.

- **First open.** Run `$S/app.sh tree "$RUN"`. Expect **Getting started**, **Page body**, and `⌘K to search and run commands`, `/ to insert a block`, and `[[ to link a page`. Capture `$S/app.sh screenshot "$RUN" before`.
- **Edit.** Run `$S/app.sh focus "$RUN" "Page body"`, then `$S/app.sh keys "$RUN" " seed proof"`. Capture `$S/app.sh screenshot "$RUN" after` and check that the body contains the synthetic text.
- **Relaunch.** Quit with `$S/app.sh keys "$RUN" q command`. Require the quit log to show `pending=[]`, then clean up the run. Launch again with `AELOMI_DATA_DIR="$RUN/data" $S/app.sh launch dev`. Run doctor and capture `relaunch`. Expect the same title, instructions, and typed text.
- **Rename and quit.** In a dev build, open **Storage probe**, run `$S/app.sh rename-quit "$RUN"` to press **Rename Getting started** and send native ⌘Q without waiting for its reply. Relaunch on that data folder. Expect **Quit rename proof**, the same page ID, and the typed body. The quit log must show `pending=[]`.
- **Retried quit.** On the dev Storage probe, record the mode of the synthetic `data/docs/workspace` folder, then run `chmod 000` on that folder. Press **Rename Getting started** and send ⌘Q. Expect the save-failure banner with its record I/O reason and **Quit with unsaved changes?**. Send ⌘Q again while the folder remains inaccessible. Expect the dialog again and no confirmation. Restore the exact original folder mode, allow up to 10 seconds for retry, require the banner to clear, then send normal ⌘Q. Expect `quit confirmed via=drain`. Relaunch on the same data folder and expect **Quit rename proof**, the same page ID, and the typed body. Restore the mode on every failure path.
- **Terminal retry failure.** In the mock browser, refuse record commands with an I/O error, then change the refusal to an inactive-page error. Expect the save-failure banner to stay visible and **Dismiss failed change** to appear. Make record commands succeed again. Rename another page and expect the notice to remain. Press **Dismiss failed change** and expect it to clear. The WebKit case `SHELL-10 a terminal retry failure stays visible until dismissed` drives this path.
- **Settings and rename.** Open **Settings probe** from the dev header. Run `$S/app.sh both-quit "$RUN"` to press both buttons and send native ⌘Q without waiting for either reply. Relaunch on the same synthetic folder. Expect **Settings and rename quit proof**, the same typed body, and settings.json with theme Dark, open page probe-page, anchor probe-block, recent pages probe-page and older-page, and expanded rows parent-page and probe-page.
- **Page identity.** The page route in the second run must name the same ID as the first. The core test for two opens also asserts exactly one stored page and reads the three lines from its body.

## Gotchas

- A seed page opens only on the native fresh-folder path. An empty mock workspace stays empty.
- Move and restore have no commands or UI yet. Create, rename, and trash are available through the [sidebar and title](./shell-sidebar.md).
- A read-only notice or a missing **Page body** editor fails the editability proof.
- A warm workspace can accept record edits from memory while its files are inaccessible. To exercise a record failure before acceptance, quit the synthetic instance, record the workspace folder mode, set that folder to `000`, then relaunch with the same explicit `AELOMI_DATA_DIR`. The dev header still exposes **Storage probe**. After both normal quits ask, restore the exact original mode and require the retry to update the indexed title before the final normal quit.
