# Sidebar, empty state, and editable title

The sidebar opens pages from the source tree. New page opens an untitled page with its title focused. The title updates the sidebar immediately. Move to trash chooses the next active page or the empty state.

## Sub-features

- `shell-sidebar-toggle` covers SHELL-1 and SHELL-6, with no toggle animation.
- `shell-page-create` covers TREE-1 and SHELL-4 through the sidebar, empty state, palette, and File menu.
- `shell-title` covers TREE-7, EDIT-1, and SHELL-8, including title-only Undo.
- `shell-disclosure` covers TREE-6 and persisted expanded rows.
- `shell-trash-open` covers TREE-9's sibling, ancestor, recent-page, and empty fallbacks.

## How to get to it (user POV)

- Launch with a fresh synthetic folder. Getting started appears in the sidebar.
- Click New page in the sidebar, choose File > New page, or press ⌘N.
- Type in Page title. Press Return to start the body.
- Press ⌘K, type `trash`, and choose Move to trash.
- Click a disclosure arrow to expand or collapse a parent.

## Driving it with app.sh and mock-drive

Preconditions: follow the keyboard lock and synthetic data rules in the builder brief. Use a fresh verify-aelomi run. For nesting, seed a separate synthetic folder with `seed_index <folder> missing` and launch through app.sh with that folder as AELOMI_DATA_DIR.

- **Initial state.** Run `$S/app.sh doctor "$RUN"`, then `tree`. Expect Sidebar, Getting started, Page title, Page body, and the IPC status landmark.
- **Create and rename.** Press New page with `press`, then type `D1 notes` with `keys`. Expect Page title and the sidebar row to read D1 notes. Capture before and after.
- **Title Return.** Send Return with the native key-code helper. Type `body token`. Expect Page body to contain body token.
- **Toggle.** Run `$S/app.sh keys "$RUN" '\' command` twice. Capture the hidden and restored states. The editor occupies the released width.
- **Trash.** On a folder containing only one active page, open the palette with `$S/app.sh keys "$RUN" k command`, type Move to trash, and send Return. Expect No page open and New page. Press New page and type a title without focusing another field. Expect the title and row to update.
- **Disclosure restart.** On the nested fixture, press Collapse Projects. Quit with ⌘Q, wait for the owned PID to exit, and relaunch the same synthetic folder through app.sh. Expect Expand Projects and no child rows. The settings file has no Projects ID in expandedRows.
- **Worst-case browser.** Open `/dev/sidebar-stress` in a mock build. Its picker has Demo, Worst case, Empty, One, and 10,000 rows. Check long and unbreakable titles, CJK, diacritics, RTL text, emoji, line breaks, and deep nesting in light and dark at the actual sidebar width, 320 px, and 200% zoom.

## Gotchas

- The page commands are absent from the palette with no active page. New page remains available through the empty state, sidebar, and ⌘N.
- Favorites, Trash navigation, row menus, drag, and tree keyboard navigation belong to section 2.
- `Page title` is a textarea. Use its accessibility name, not a heading, to focus it.
- The stress picker exists only in mock builds. It clears synthetic mock data when changing datasets.
- Native proof uses real buttons and keys. Read settings afterward to confirm persistence.
