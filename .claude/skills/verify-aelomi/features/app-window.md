# App window

Aelomi opens one window titled `Aelomi` with standard macOS window controls and a native menu bar. The window keeps an `Aelomi` header, the IPC status line, and a **Ping again** button. A fresh data folder opens the editable **Getting started** page. The sidebar belongs to a later change.

## Sub-features

- `window-open` opens exactly one window titled `Aelomi`, 960 by 640 points by default.
- `window-controls` shows the close, minimize, and full screen buttons.
- `window-menu` shows the native menu bar with `Aelomi`, `File`, `Edit`, `View`, `Window`, and `Help`.
- `window-content` shows the `Aelomi` header, the status line, and **Ping again**. Dev builds also show a **Storage probe** link to the dev-only `/dev/storage` route.

## How to get to it (user POV)

- Start the app with `pnpm dev:app` (any free port) or `pnpm tauri dev` (port 1420).
- Open the built `Aelomi.app`.

## Driving it with app.sh and mock-drive

Preconditions:

- `RUN=$($S/app.sh launch dev)` printed a run directory.
- `$S/app.sh doctor "$RUN"` reports every line as `ok`.

- **Window opens.** Launch the app. Run `$S/app.sh screenshot "$RUN" window`. The first line of `window.ax.txt` reads `title="Aelomi"` with `w=960 h=640`, and `window.png` shows one window with the title `Aelomi`.
- **Window controls.** Look at `window.png`. The red, yellow, and green buttons are at the top left of the title bar.
- **Menu bar.** Run `$S/app.sh menus "$RUN"`. The output is `Apple`, `Aelomi`, `File`, `Edit`, `View`, `Window`, and `Help`, one per line.
- **Window content.** Run `$S/app.sh tree "$RUN"`. The tree has `AXGroup/AXLandmarkBanner` containing `AXStaticText "Aelomi"`, and `AXGroup/AXLandmarkMain` containing the **Getting started** page and **Page body** editor. The status group and `AXButton "Ping again"` remain visible.
- **Mock browser.** Run `$S/mock-drive.mjs`. `before.aria.yml` lists `banner: Aelomi`, the `status` line, and `button "Ping again"`.
- **Proof.** Keep `window.png`, `window.ax.txt`, and the `menus` output. The screenshot shows the title bar, the header, and the welcome section.

## Gotchas

- `screenshot` fails with `No on-screen window for PID <pid>. Is it minimized?` while the window is minimized. Restore the window before you capture.
- The window's close, minimize, and full screen buttons are in the accessibility tree of the window, not of the web content, so `tree` does not list them. Prove them from the screenshot.
- `menus` lists the menu titles only. The menu is built from the shared shortcut table. **Quit Aelomi** (⌘Q) remains the app's drain-aware item. [Shortcuts and menu commands](./shortcut-menu.md) covers its items.
