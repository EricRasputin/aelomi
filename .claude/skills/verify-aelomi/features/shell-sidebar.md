# Sidebar, empty state, and editable title

The sidebar opens pages from the source tree. New page opens an untitled page with its title focused. The title updates the sidebar immediately. Move to trash chooses the next active page or the empty state.

## Sub-features

- `shell-sidebar-toggle` covers SHELL-1 and SHELL-6, with no toggle animation.
- `shell-page-create` covers TREE-1 and SHELL-4 through the sidebar, empty state, palette, and File menu.
- `shell-title` covers TREE-7, EDIT-1, and SHELL-8, including title-only Undo across page navigation, unknown-title protection, and final composition input before quit.
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

Preconditions: launch only through app.sh with a fresh synthetic `AELOMI_DATA_DIR`. Read `ls -la ~/Library/Application\ Support/Aelomi` before the first launch and after the last. Keep that folder read-only. Serialize keys, focus, AXPress and transient screenshots through `/tmp/aelomi-ui.lock`. The portable protocol claims the directory with atomic `mkdir`, records the process, UTC time and run label in `owner`, waits while another owner holds it, and removes only its own lock on every exit path. When a coordinator supplies an acquire/release helper for this protocol, use that helper. Acquire before interactive steps. Build and launch outside the lock, and hold it for at most about 20 minutes. For nesting, seed a separate synthetic folder with `seed_index <folder> missing` and launch through app.sh with that folder as AELOMI_DATA_DIR.

- **Initial state.** Run `$S/app.sh doctor "$RUN"`, then `tree`. Expect Sidebar, Getting started, Page title, Page body, and the IPC status landmark.
- **Create and rename.** Press New page with `press`, then type `D1 notes` with `keys`. Expect Page title and the sidebar row to read D1 notes. Capture before and after.
- **Create outage.** Record the synthetic docs folder's permission mode, then remove write permission before creating a new page. Press New page and type a title. Expect the new page open immediately, its title focused and editable, a matching sidebar row, the save-failure banner, and an empty body slot. Restore the recorded mode on every exit path. Expect the body to appear without moving title focus. Type into the body, quit and relaunch the same folder. Check the exact title and body.
- **Immediate typing.** Focus Page body, press ⌘N and type immediately. Expect Page title to hold the exact text and the old body to stay unchanged. While creation is held, Return stays in the title. Recovery keeps the same title and caret. Undo steps back to the empty title and Redo restores it. Repeat from the empty state.
- **Repeated creation.** While a page is still creating or failed and its title is empty, repeat the button, ⌘N and File menu action. Expect one new page and the stable New page label. Type One, press ⌘N and type Two. Expect two separate rows with those exact titles after recovery. With tree reads failing, create an untitled page, enter body text, then press ⌘N. Expect a separate new page and the first body unchanged.
- **Failed creation.** On a terminal failure, expect the title kept read-only, a row with a visible Not created suffix and matching accessibility description, and This page couldn't be created. Its title is kept here. Try again reuses that page and focuses the title. A nonempty failed title must trigger quit confirmation. Use the mock for terminal-error injection; chmod proves the native retained-write case.
- **Title history across pages.** Rename A, open B, reopen A, focus Page title, press ⌘Z, then ⇧⌘Z. Expect the exact original and revised title.
- **Safe command selection.** Press ⌘K, type `to`, then Return. Expect the page to remain active, without a trash operation. Return may choose a safe content result. To trash, query Move to trash, then press Down and Return to select it explicitly.
- **Quit navigation.** Hold a create delivery, start Quit, and try another sidebar row and ⌘K. Expect the current route unchanged and Page title read-only. Recover storage, relaunch the synthetic folder, and assert the exact title.
- **Loading body and title.** Open a listed page row and immediately focus Page title and type. After the body appears, keep typing. Expect the exact full title, title focus, and the original body unchanged. Repeat when trash opens the fallback page. In the mock, hold and release that page's docLoad around title typing.
- **Held Return.** While creation is held, place the caret mid-title, press Return, and type. Expect insertion at that caret and the same selection after recovery. A failed read-only title must not queue a focus change for a later retry.
- **Status clearance.** Use a tree tall enough to scroll, open its last row, and hold a trash request. Expect the status in the header and every visible row clear of it. In the mock, fail a create and check the last failed row does not intersect the status. Search its title in the palette and expect the Not created accessibility description.
- **Late body.** Hold New page, focus another sidebar row without opening it, and recover storage. Press ⌘\ while that row has focus, return to Page title, type, then recover storage. Expect the body to appear with Page title still focused and all text in that title. Opening another page while creation waits must keep the chosen page and focus after recovery.
- **Pending opening.** While creation is held, open its row, palette result and direct route. Each shows its editable title and an empty body slot. In the mock, assert zero docLoad and docAppend calls for its ID before final success.
- **Accessibility.** Capture the empty shell status in the accessibility tree before a message arrives. Then capture a trash or closing message in the same mounted region. WebKit checks node identity and that the title's aria-describedby points to its visible reason. Leave VoiceOver off.
- **Pending trash.** Hold Move to trash. Expect its title and any opened descendant title to be read-only with Moving to trash.
- **Palette fallback.** Hold Move to trash on the open page, reopen ⌘K and query Move to trash, then recover storage. Expect the route and palette focus unchanged and no Move to trash command. Close the palette. Expect the fallback page only then. Repeat with Settings when available.
- **Missing record.** In the mock, leave an untitled terminally failed create, fail tree reads and revisit its route. Expect no Page body, zero docLoad or docAppend for that ID, and a linked refresh explanation under Page title.
- **Read retry.** Fail a tree refresh. Expect the previous rows and title, an explanatory notice, and Try again. Retry and automatic backoff recover the list without changing the title.
- **Title Return.** Send Return with the native key-code helper. Type `body token`. Expect Page body to contain body token.
- **Toggle.** Run `$S/app.sh keys "$RUN" '\' command` twice. Capture the hidden and restored states. The editor occupies the released width.
- **Trash.** On a folder containing only one active page, open the palette with `$S/app.sh keys "$RUN" k command`, type Move to trash, explicitly select it with Down, and send Return. Expect No page open and New page. Press New page and type a title without focusing another field. Expect the title and row to update.
- **Disclosure restart.** On the nested fixture, press Projects children and check `aria-expanded` becomes false. Quit with ⌘Q, wait for the owned PID to exit, and relaunch the same synthetic folder through app.sh. Expect Projects children to be collapsed and no child rows. The settings file has no Projects ID in expandedRows.
- **Unknown title.** Inject a source-read failure only in the synthetic run. Expect Page title to retain its text but refuse input and Undo. Recover the read and check title Undo still reaches its original baseline. Reopen the synthetic folder after quit and assert the exact title.
- **Title and palette.** Capture the palette over a one-line title and a wrapped title. Dismiss it, scroll the body, and type near the scrollport top. The title scrolls out and the caret remains visible.
- **Selection.** Open a page near the end of a large tree by palette or ⌘N. Expect its row in view, with ancestors revealed. Hover a different row and capture the Mac's current appearance. Use app Settings or browser emulation for another theme. Never change macOS appearance. Selection must be stronger.
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

- New page opens its real title immediately. Only its body waits for durable creation. Success never navigates or moves focus. Pending Move to trash keeps the page open until final success. A successful trash reply removes affected rows even if tree reads fail. The banner includes retained operations and terminal failures.
