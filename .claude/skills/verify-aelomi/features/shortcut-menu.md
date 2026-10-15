# Shortcuts and menu commands

The native menu and webview keyboard commands share one shortcut table. Search and commands opens the palette. Undo and Redo use the page editor's history, while a plain text field keeps its own native history. Quit drains pending edits before exit.

## Sub-features

- `shortcut-palette` opens the palette once with ⌘K or View > Search and commands (CMD-1, SHELL-7).
- `shortcut-editor-history` undoes and redoes one editor typing group with ⌘Z, ⇧⌘Z, Edit > Undo, and Edit > Redo (SHELL-7).
- `shortcut-field-history` keeps keyboard and menu Undo in the focused palette input without changing the page (SHELL-7).
- `shortcut-history-focus` leaves page history unchanged when palette chrome or a row has focus (SHELL-7).
- `shortcut-modal` blocks keyboard and menu commands during input composition and the quit confirmation (SHELL-7).
- `shortcut-native-menu` shows Aelomi, File, Edit, View, Window, and Help with native shortcut labels (SHELL-1, SHELL-7).
- `shortcut-quit` drains edits through both ⌘Q and Aelomi > Quit Aelomi (SHELL-9).

## How to get to it (user POV)

- Open a page, type text, and use the keyboard or Edit menu to undo and redo it.
- Press ⌘K or choose View > Search and commands. Type in Search pages and undo there.
- Choose native menu commands in the menu bar.
- Press ⌘Q or choose Aelomi > Quit Aelomi.

## Driving it with app.sh and mock-drive

Preconditions:

- Launch with an isolated synthetic data folder that contains an editable page. `cargo run -p aelomi-core --features test-support --example seed_index -- <new-folder> valid` creates one. Pass that folder as `AELOMI_DATA_DIR` to `launch dev`.
- `$S/app.sh doctor "$RUN"` reports every line as `ok`.
- `$S/app.sh tree "$RUN"` shows `Page body`.

- **Menu bar.** Run `$S/app.sh menus "$RUN"`. Expect Apple, Aelomi, File, Edit, View, Window, and Help. Inspect Edit for Undo ⌘Z and Redo ⇧⌘Z, View for Search and commands ⌘K and Toggle Sidebar ⌘\, and Aelomi for Settings… ⌘, below About Aelomi.
- **Window and Help.** Open Window with PID-targeted System Events AXPress. Expect the current Aelomi window below the static items and capture the open menu. Open Help and expect its Search field, then capture it. Capture the full display or menu window because `app.sh screenshot` captures only the app window.
- **Palette key.** Run `$S/app.sh keys "$RUN" k command`. The tree shows `Command palette` and `Search pages`. Capture `palette-key.png`. Press Escape with a PID-targeted System Events key code 53 and expect the palette closed.
- **Native menu click.** Use System Events on the PID from `$RUN/run.env`. Call `AXPress` first on the named menu-bar item, then on the named item under its first menu. Choosing View > Search and commands opens the same palette. Do not synthesize its shortcut to prove a menu click.
- **Editor history.** Focus Page body with `$S/app.sh focus "$RUN" "Page body"`. Type two distinct groups separated by a cursor move. Capture the populated page, then use `$S/app.sh keys "$RUN" z command` and `$S/app.sh keys "$RUN" z command,shift`. Expect one group removed and restored. Repeat through Edit > Undo and Edit > Redo and capture each result.
- **Field history.** Open the palette, type two groups in Search pages, then undo and redo by key and through Edit. Expect the input value to change and the page body to retain its text. Close the palette and capture the unchanged page.
- **History without input focus.** Open the palette and click its Pages heading to move focus off Search pages. Press ⌘Z and choose Edit > Undo. Close the palette and expect the page text unchanged. Repeat with a result row focused without activation.
- **Quit dialog.** Follow `save-failure.md` to open the quit confirmation. Press ⌘K and choose View > Search and commands, then Edit > Undo. Expect only the quit dialog and the unchanged page after Keep app open.
- **Composition.** Start an IME composition in the page or palette input. Try ⌘K and the View menu command, then Edit > Undo. Expect no palette or page-history change until composition ends. The WebKit shortcut spec also sends composition events through the window listeners and then activates the mock menu event.
- **Quit.** Type a final marker and quit immediately by key or menu. Wait for the owned app PID to exit. `shell.log` must show Draining, Confirmed, and the final Exit flush with no pending documents. Relaunch on the same isolated folder and expect the marker. Repeat the other route.
- **Mock acceptance.** Run `pnpm build:mock` then `pnpm test:e2e e2e/shortcuts.e2e.ts`. It covers palette focus return and editor Undo and Redo after a workspace record edit. Run it only after cleanup of any dev instance.

## Gotchas

- Toggle Sidebar and Settings… have table entries but no handler until their section-1 follow-up PRs. Their keys must remain uncancelled here.
- A palette input owns its native history. Keyboard Undo must never affect the page behind it.
- Field keyboard history runs directly in the webview and cancels the key. The menu event listener is not on that path.
- Two separate typing groups expose a double undo. A single group cannot distinguish one undo from two.
- `app.sh menus` prints top-level titles only. Use the PID-targeted System Events menu tree to inspect accelerators and activate items.
- Native menu items exist before the webview starts. A webview-owned item cannot act until its handler connects. Native roles and Quit still work.
