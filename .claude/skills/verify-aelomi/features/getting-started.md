# Getting started

A fresh data folder opens one page titled **Getting started** with three editable paragraphs. Reopening that folder keeps the same page and edits. SHELL-2 does not seed an empty workspace after its pages are trashed or erased.

## Sub-features

- `seed-first-open` shows **Getting started** and the three instructions on first launch.
- `seed-editable` accepts typing in **Page body**.
- `seed-deferred` keeps the app open without a first page when its first body write fails, with the normal save-failure banner and retry. Corrupt workspace history never creates a first page.
- `seed-relaunch` opens the same page with the typed text after a normal quit and relaunch.

## How to get to it (user POV)

- Start Aelomi with a fresh data folder.
- Quit with ⌘Q, then open Aelomi again on that folder.

## Driving it with app.sh and mock-drive

Preconditions:

- Launch the real app with a fresh isolated folder and require every doctor check to pass.
- Use synthetic text only. The mock starts empty according to each test's setup and does not run the native folder seed.

- **First open.** Run `$S/app.sh tree "$RUN"`. Expect **Getting started**, **Page body**, and `⌘K to search and run commands`, `/ to insert a block`, and `[[ to link a page`. Capture `$S/app.sh screenshot "$RUN" before`.
- **Edit.** Run `$S/app.sh focus "$RUN" "Page body"`, then `$S/app.sh keys "$RUN" " seed proof"`. Capture `$S/app.sh screenshot "$RUN" after` and check that the body contains the synthetic text.
- **Relaunch.** Quit with `$S/app.sh keys "$RUN" q command`. Require the quit log to show `pending=[]`, then clean up the run. Launch again with `AELOMI_DATA_DIR="$RUN/data" $S/app.sh launch dev`. Run doctor and capture `relaunch`. Expect the same title, instructions, and typed text.
- **Page identity.** The page route in the second run must name the same ID as the first. The core test for two opens also asserts exactly one stored page and reads the three lines from its body.

## Gotchas

- A seed page opens only on the native fresh-folder path. An empty mock workspace stays empty.
- Move and restore have no commands or UI yet. Create, rename, and trash have commands; later sidebar and title UI provide their user paths.
- A read-only notice or a missing **Page body** editor fails the editability proof.
