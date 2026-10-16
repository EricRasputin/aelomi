# Settings persistence

The dev Settings probe shows the cached session settings and writes a synthetic session through real IPC. Settings survive quitting and reopening the app. The modal and theme controls are covered in [Settings appearance](settings-appearance.md). Session restoration UI arrives in a later PR.

## Sub-features

- DATA-4 stores the theme, open page and block anchor, recent pages, and expanded rows in settings.json.
- SHELL-9 returns from the settings write after the snapshot is durable. Normal quit waits for settings invocations from dispatch through durability, including calls started during document delivery. After completion starts, new settings writes return Quitting until Keep app open.
- SHELL-10 retries failed settings and names settings.json in quit confirmation.
- DATA-4 defaults when configuration is missing and preserves a written session across launches.

## How to get to it (user POV)

- In a dev build on a fresh synthetic folder, click **Settings probe** in the dev header.
- Click **Write session settings** to save the synthetic session.

## Driving it with app.sh and mock-drive

Preconditions: Use a fresh dev launch and pass doctor. This probe does not exist in production builds. Use the real app for persistence proof.

- Open the probe with `$S/app.sh press "$RUN" "Settings probe"`. The status shows theme System, no open page, and empty recent and expanded lists. Capture `$S/app.sh screenshot "$RUN" settings-before`.
- Write with `$S/app.sh press "$RUN" "Write session settings"`. The status shows theme Dark, page probe-page, anchor probe-block, recent pages probe-page and older-page, and expanded rows parent-page and probe-page. Capture `$S/app.sh screenshot "$RUN" settings-written`.
- Read `$RUN/data/settings.json` and compare its JSON fields to the visible status. The shell log has an aelomi-dev-settings-probe updated line with the same snapshot.
- For the immediate-quit proof, press **Write session settings** and send ⌘Q in the same accessibility script, without waiting for the write reply. Relaunch and compare the full snapshot to settings.json.
- Quit with `$S/app.sh keys "$RUN" q command`, wait for the owned app PID to exit, then run `$S/app.sh cleanup "$RUN"`.
- Relaunch with `AELOMI_DATA_DIR="$RUN/data" $S/app.sh launch dev`. Run doctor on the new run. Open Settings probe and capture settings-reopened. Its status must equal the written snapshot without another write. Read the original settings.json again and require the same fields.
- For a failure proof, remove write permission from the owned synthetic data directory, then write settings. Expect the save-failure banner. Quit and expect the confirmation to say Settings changes have not been saved. The shell quit status has no document IDs and reports pending_settings true. Choose Keep app open, restore permission, and expect the banner to clear without another update.
- Cleanup both owned runs. Keep their paths and screenshots in the PR report.

## Gotchas

- Expanded rows are a set and appear in sorted order. Recent pages retain the first occurrence and at most 50 distinct IDs.
- The probe stores synthetic page IDs. It does not create or open pages.
- A relaunch creates a new run directory while the explicit data override still points at the first run's folder.
- Settings commands return Unavailable for a refused folder. Never use a real data directory for this proof.
- `app.sh status` reads the shell status first on this route. Use `app.sh tree` to inspect the probe snapshot.
