# Aelomi verification map

This directory is the maintained source for verifying Aelomi's user-facing behavior. Read this index before you drive the app, then use the matching feature file as the recipe. `../SKILL.md` documents the helpers that the recipes call.

## Baseline preconditions

- Run commands from the repo root with `S=.claude/skills/verify-aelomi/scripts`.
- Launch the real app with `RUN=$($S/app.sh launch dev)`, or with `launch app` after `pnpm tauri build --bundles app`.
- Run `$S/app.sh doctor "$RUN"` and require every line to read `ok`.
- Each launch stores its data in a new `<run dir>/data` folder (`AELOMI_DATA_DIR` in `../SKILL.md`), so it starts empty. The status line starts at `seq 1`.
- Never drive an instance that this verification run did not start.

## Driving conventions

- Start every recipe from a fresh launch unless its preconditions say otherwise.
- On the real app, find elements by accessibility name with `$S/app.sh tree "$RUN"`. Never use screen coordinates.
- On the real app, act with `press`. Use `focus` and `keys` only to prove a keyboard path, because they bring Aelomi to the front.
- On the mock browser, use Playwright role and name locators in a flow passed to `$S/mock-drive.mjs --flow`.
- Wait for state with `wait-status` or `expect`, never a fixed sleep.
- Treat quoted names and texts as literal. The status separator is a middle dot (`·`).
- Run `$S/app.sh cleanup "$RUN"` after every attempt. It keeps the evidence.

## Proof and skip reporting

- Capture the state before the action and after it, not only the final screen.
- Real-app proof is a pair of screenshots with their `.ax.txt` files, plus the matching lines in `shell.log`.
- Mock-browser proof is the flow's screenshots and `.aria.yml` files, plus `summary.json` with the mock's calls and an empty `nonLocalRequests`.
- Look at each screenshot and say what it shows.
- Record the sub-feature ID and the surface with each artifact.
- A mock-browser pass does not verify the Rust side. Report it as mock-only.
- Report an unreachable path with the command you ran and the precondition that failed.

## Feature entry contract

Each feature file starts with an H1 title and one paragraph describing the user-visible behavior. It then uses exactly four H2 sections in this order.

1. `Sub-features` lists short IDs with one line for each behavior.
2. `How to get to it (user POV)` lists every user entry point.
3. `Driving it with app.sh and mock-drive` starts with `Preconditions:` and uses labeled bullets that pair each user action with an exact command and an observable result.
4. `Gotchas` lists traps that can waste or invalidate a verification run.

Keep implementation details out of the map. Name only user paths, stable handles, required state, commands, and observable proof.

## Features

- [Sidebar, empty state, and editable title](./shell-sidebar.md) covers page creation, title input and Undo, disclosure persistence, sidebar toggle, and open-page trash.

- [Getting started](./getting-started.md) covers the fresh-folder seed, editable body, and reuse after relaunch.

- [App window](./app-window.md) covers the single window, its macOS controls, the native menu bar, and the welcome content.
- [Shortcuts and menu commands](./shortcut-menu.md) covers the palette, native menu, editor history, field history, and Quit.
- [IPC status line](./ipc-status.md) covers the status line, the **Ping again** round trip to the Rust core, and the button's tooltip.
- [Save failure and quit confirmation](./save-failure.md) covers the banner for failing writes, its recovery, and the dialog that guards every quit path while an edit is pending.
- [Settings appearance](./settings-appearance.md) covers the modal, app themes, native chrome and first-frame persistence.
- [Settings persistence](./settings.md) covers the dev probe, durable session writes, and close-and-reopen equality.
- [Storage startup and URL links](./storage-startup-links.md) covers refusal screens and browser handoff.
