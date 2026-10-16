# Sidebar, empty state, and editable title

The sidebar opens pages from the source tree. New page opens an untitled page with its title focused. The title updates the sidebar immediately. Move to trash chooses the next active page or the empty state.

## Sub-features

- `shell-sidebar-toggle` covers SHELL-1 and SHELL-6, with no toggle animation.
- `shell-page-create` covers TREE-1 and SHELL-4 through the sidebar, empty state, palette, and File menu.
- `shell-title` covers TREE-7, EDIT-1, and SHELL-8, including title-only Undo, unknown-title protection, and final composition input before quit.
- `shell-current-page` covers selected versus hovered rows, active-row reveal, and scroll visibility.
- `shell-disclosure` covers TREE-6 and persisted expanded rows.
- `shell-trash-open` covers TREE-9's sibling, ancestor, recent-page, and empty fallbacks.

## How to get to it (user POV)

- Launch with a fresh synthetic folder. Getting started appears in the sidebar.
- Click New page in the sidebar, choose File > New page, or press ⌘N.
- Type in Page title. Press Return to start the body.
- Press ⌘K, type `trash`, and choose Move to trash.
- Click a disclosure arrow to expand or collapse a parent.

## Driving it with app.sh and mock-drive

Preconditions: launch only through app.sh with a fresh synthetic `AELOMI_DATA_DIR`. Read `ls -la ~/Library/Application\ Support/Aelomi` before the first launch and after the last. Keep that folder read-only. Before keys, focus, AXPress, hover, or a transient screenshot, acquire `/tmp/aelomi-ui.lock` with an atomic `mkdir`, write your PID, UTC time, and run name to its `owner` file, and release your lock on every exit path. Wait up to 45 minutes if occupied. Remove another owner's lock only when its owner file is older than 30 minutes. Hold your lock for at most 20 minutes, with builds outside it. Use a fresh verify-aelomi run. For nesting, seed a separate synthetic folder with `seed_index <folder> missing` and launch through app.sh with that folder as AELOMI_DATA_DIR.

- **Initial state.** Run `$S/app.sh doctor "$RUN"`, then `tree`. Expect Sidebar, Getting started, Page title, Page body, and the IPC status landmark.
- **Create and rename.** Press New page with `press`, then type `D1 notes` with `keys`. Expect Page title and the sidebar row to read D1 notes. Capture before and after.
- **Title Return.** Send Return with the native key-code helper. Type `body token`. Expect Page body to contain body token.
- **Toggle.** Run `$S/app.sh keys "$RUN" '\' command` twice. Capture the hidden and restored states. The editor occupies the released width.
- **Trash.** On a folder containing only one active page, open the palette with `$S/app.sh keys "$RUN" k command`, type Move to trash, and send Return. Expect No page open and New page. Press New page and type a title without focusing another field. Expect the title and row to update.
- **Disclosure restart.** On the nested fixture, press Projects children and check `aria-expanded` becomes false. Quit with ⌘Q, wait for the owned PID to exit, and relaunch the same synthetic folder through app.sh. Expect Projects children to be collapsed and no child rows. The settings file has no Projects ID in expandedRows.
- **Unknown title.** Inject a source-read failure only in the synthetic run. Expect Page title to retain its text but refuse input and Undo. Recover the read and check title Undo still reaches its original baseline. Reopen the real data after quit and assert the exact title.
- **Title and palette.** Capture the palette over a one-line title and a wrapped title. Dismiss it, scroll the body, and type near the scrollport top. The title scrolls out and the caret remains visible.
- **Selection.** Open a page near the end of a large tree by palette or ⌘N. Expect its row in view, with ancestors revealed. Hover a different row and capture both light and dark. Selection must be stronger.
- **Composition and quit.** Begin title composition, then send native Quit immediately. Reopen the synthetic folder and assert the final title exactly. Identify synthetic composition events separately from a hardware IME run.
- **Worst-case browser.** Open `/dev/sidebar-stress` in a mock build. Its picker has Demo, Worst case, Empty, One, and 10,000 rows. Check long and unbreakable titles, CJK, diacritics, RTL text, emoji, line breaks, and deep nesting in light and dark at the actual sidebar width, 320 px, and 200% zoom.

## Gotchas

- Empty palette queries list no commands. Query New page even when no page is open. Only Move to trash requires an active page.
- Settings loading or failure leaves root rows visible and collapsed. Disclosure controls remain disabled until settings load.
- A retained rename stays a draft without a per-title error. Background acknowledgement plus the matching source title clears the draft. A final refusal keeps the draft and shows a failure.
- Favorites, Trash navigation, row menus, drag, and tree keyboard navigation belong to section 2.
- `Page title` is a textarea. Use its accessibility name, not a heading, to focus it.
- The stress picker exists only in mock builds. It clears synthetic mock data when changing datasets.
- Native proof uses real buttons and keys. Read settings afterward to confirm persistence.

- A retrying New page navigates to the returned page ID and shows Creating page while its record is absent. On recovery, its title becomes focused. A retrying Move to trash waits for source change without a final failure message. Final error replies show the action failure and preserve the source. Both use #84's record-command store and typed results; D1 adds no observer API.
