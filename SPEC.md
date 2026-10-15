# Aelomi MVP specification

Status: **confirmed scope** (2026-10-11). The maintainer confirmed the MVP scope below. Choices marked **(default)** are proposals the maintainer can override. Open questions are listed at the end.

This file lists what the MVP does and how each behavior is tested. Each behavior has an ID, such as `TREE-3`. Tests, PRs, and commit messages cite these IDs.

## How to read the tests

Each feature has three kinds of tests. They map to the testing table in `CLAUDE.md`.

- **Acceptance** tests are plain-English steps for TesterArmy `e2e`. They run in a browser with Tauri IPC mocked. Each one ends in something a person can see on screen.
- **Core** tests are cargo-nextest and property tests in the Rust core. Data-safety tests end in exact assertions, never an AI judgment.
- **Logic** tests are Vitest tests for pure TypeScript functions.
- **Perf** tests are Playwright (WebKit) tests that measure timing or inspect the IPC mock directly, except cold start, which runs the built `.app` on the macOS CI runner.

Rules for every test:

- The IPC mock is one in-memory TypeScript fake of the tauri-specta bindings, shared by all acceptance tests. Every Rust behavior an acceptance test relies on also has a core test, so a test never only checks the mock.
- Each acceptance test starts from an empty mock workspace plus what its **Setup** line lists. Tests do not share state or depend on order.
- A test that expects something to be absent first shows it present, in the same test.
- Acceptance tests cannot quit and relaunch the app. Anything that must survive a relaunch is a core test, plus the maintainer's daily use.

A feature is done when every test listed for it passes in CI. Every behavior has at least one test. The full definition of done is in `CLAUDE.md`.

## Scope

The MVP has six features:

1. [App shell and data safety](#1-app-shell-and-data-safety)
2. [Page tree](#2-page-tree)
3. [Editor](#3-editor)
4. [Links and backlinks](#4-links-and-backlinks)
5. [Command palette and search](#5-command-palette-and-search)
6. [Markdown export](#6-markdown-export)

These are out of scope for the MVP: the MCP server, databases and table views, Notion import, sync, accounts, sharing, AI features, telemetry, comments, page history UI, multiple windows, tabs, and file attachments other than images.

## Where data lives

`CLAUDE.md` makes Yjs the source of truth and SQLite a derived index. These rules say which data goes where, so the index can always be rebuilt and nothing the user made lives only in the index.

- **DATA-1.** Each page's body is one Y.Doc.
- **DATA-2.** One workspace Y.Doc holds a record for every page: page ID, title, parent ID, sibling order, favorite flag and favorite order, trash state, and created and updated times. Trash state is active, or trashed with the original parent, a trash batch ID, and the trash time. An erased page has no record (TREE-14).
- **DATA-3.** Images are files in the data folder, named by the hash of their bytes. An image file is deleted only when the last page that refers to it is erased.
- **DATA-4.** Settings and session state live in one `settings.json` file, replaced atomically on each write. It holds the theme, the export folder and schedule, export history, expanded tree rows, recent pages, and the open page with its scroll anchor. It is configuration, not user data: the append-only rule in `CLAUDE.md` covers the Y.Docs and image files. Losing it loses no page content.
- **DATA-5.** SQLite holds only what can be rebuilt from the Y.Docs: the tree query cache, the full-text index, and backlinks. It is not user data. On an index schema change or a corrupt index, the app deletes it and rebuilds it.
- **DATA-6.** Each page's Y.Doc, and the workspace Y.Doc, is stored as a snapshot file plus newer update files. When a doc's update files pass 1 MB or 10,000 updates, the app compacts it: it writes a new snapshot file, checks that it loads to the same document, and only then deletes the old files **(default)**. Compaction never changes what the user sees. The page history it removes has no UI in the MVP.

## 1. App shell and data safety

The window, theme, keyboard model, and the guarantees about user data.

### Behavior

- **SHELL-1.** The app opens one window with a sidebar on the left and the current page on the right. The window uses standard macOS controls and a native menu bar.
- **SHELL-2.** When the app starts with no data folder, it creates the data folder and one page titled "Getting started". The page body has three lines: ⌘K to search and run commands, `/` to insert a block, and `[[` to link a page **(default)**. A workspace with no active pages does not create it again.
- **SHELL-3.** On later launches, the app reopens the page that was open when it quit, scrolled to the same top visible block. If that page is not active, the most recently opened active page opens instead.
- **SHELL-4.** When no active page exists, the editor area shows an empty state with a **New page** button.
- **SHELL-5.** The theme follows the macOS appearance setting by default. **Settings** offers **System**, **Light**, and **Dark**. The theme applies to the app's own UI. The native menu bar and window chrome always follow macOS.
- **SHELL-6.** ⌘\ shows and hides the sidebar. ⌘, opens **Settings** as a modal inside the main window.
- **SHELL-7.** Keyboard shortcuts are handled in the webview. One shortcut table in TypeScript lists every command and its shortcut. The webview bindings and the native menu items are both built from it, so a shortcut is never bound twice. Native menu items show the shortcut and call the same command. **Edit → Undo** and **Edit → Redo** call the editor's undo and redo.
- **SHELL-8.** Every edit is saved without a save action, and no UI says "saving" or "saved". This covers typing, paste, drag, rename, move, trash, restore, and settings.
- **SHELL-9.** While writes succeed, each edit is on disk, flushed with `F_FULLFSYNC`, within 100 ms **(default)**. Quitting with ⌘Q, closing the window, or logging out writes every pending edit before the app exits. A crash of the app or the Mac can lose only the edits from the last 100 ms, unless writes were already failing (SHELL-10).
- **SHELL-10.** If a write fails, pending edits stay in memory and the app retries them. A banner says that changes are not being saved, and why. The banner clears when every pending edit is on disk. Pending edits are never discarded while the app runs. Quitting with pending edits asks for confirmation first.
- **SHELL-11.** Data lives in `~/Library/Application Support/Aelomi`. The app resolves symlinks on that path and on every parent folder. If the resolved path is inside `~/Library/Mobile Documents` (iCloud Drive) or `~/Library/CloudStorage` (Dropbox, Google Drive, OneDrive), the app does not open the data. It shows the resolved path, explains that cloud sync corrupts the database, and says how to move the folder back.
- **SHELL-12.** If the index is missing, corrupt, or on an old schema, the app rebuilds it (DATA-5). The tree and pages load from the Y.Docs at once. Search shows "Indexing…" until the rebuild finishes. No error appears unless the rebuild fails.
- **SHELL-13.** The app makes no network calls in the MVP. A content security policy blocks every remote origin. Clicking a URL link opens it in the default browser. The app window never navigates away.

### Acceptance tests

1. **Theme.** Setup: one page. Set the theme to **Dark** in **Settings**. Expect the sidebar, editor, settings modal, and command palette to use the dark theme.
2. **Toggle sidebar.** Setup: one page. Press ⌘\. Expect the sidebar hidden and the editor filling the window. Press ⌘\ again. Expect the sidebar back.
3. **Settings shortcut.** Press ⌘,. Expect the **Settings** modal. Press Escape. Expect it closed.
4. **Empty state.** Setup: one page, "A". Move "A" to trash. Expect the empty state with **New page**. Click **New page**. Expect a new untitled page open with the title focused.
5. **Save failure.** Setup: one page, and the mock fails every write. Type "x". Expect the banner within 2 seconds. Make the mock accept writes. Expect the banner gone.
6. **System theme.** Setup: theme **System**, and the mock reports a dark macOS appearance. Expect the dark theme. Make the mock report light. Expect the light theme.
7. **Cloud folder.** Setup: the mock reports the data path resolves into `~/Library/Mobile Documents`. Expect the refusal screen showing that resolved path.
8. **Indexing.** Setup: one page, and the mock reports the index is rebuilding. Expect the page open and editable. Press ⌘K and type "a". Expect "Indexing…".
9. **URL link.** Setup: a page with a link to `https://example.com`. Click it. Expect the mock's open-URL command to receive that URL and the page still shown.

### Core tests

- A new data folder, opened twice, contains exactly one page, "Getting started", with one page ID.
- Writes are append-only: no stored update is ever rewritten. Old update files are deleted only by compaction (DATA-6) or erasure (TREE-14).
- Interrupt compaction at every step. On each reopen, the doc loads and equals the doc before compaction.
- With a storage layer that drops every byte not yet flushed, edits are applied, the layer "crashes", and the store reopens. Every edit that was acknowledged as flushed is present. This repeats for 1,000 random crash points.
- With a controllable clock, every edit is acknowledged as flushed within 100 ms of being applied, for edits of every kind SHELL-8 lists.
- With writes failing for one page and succeeding for another, the failed page's edits stay pending until its writes succeed.
- Quitting with pending edits flushes them: after quit and reopen, the document includes every edit made before quit.
- Inject a write error, queue edits, clear the error. Every queued edit is on disk, in order.
- Session state round-trips: after close and reopen, the open page, top visible block, theme, expanded rows, and recent pages are the same.
- Index rebuild: build a fixture with nesting, favorites, a trashed subtree, an erased page, links, and text. Delete the index, reopen, and compare the tree, the backlinks of every page, and the results of a fixed list of searches with the results before. They must be equal. Repeat with a garbage index file and with an index on an old schema.
- Migrations: `fixtures/` keeps a data folder from every released version. Each migration test opens every one and checks page content, tree position, favorites, and trash state against the expected values.
- The data path check rejects a symlinked data folder and a symlinked parent that resolve into `~/Library/Mobile Documents` or `~/Library/CloudStorage`, and accepts the normal path.

### Logic tests

- The shortcut table has no shortcut assigned twice, and every native menu item and webview binding comes from it.

### Perf tests

- A Playwright check fails if the app makes any request to a non-local origin during the full acceptance suite.
- Type "x". The mock's persist command receives an update containing "x" within 100 ms.
- Type "x" and trigger quit at once. The persist command receives "x" before the quit completes.

## 2. Page tree

The sidebar shows **Favorites**, then the page tree, then a **Trash** row.

### Behavior

- **TREE-1.** ⌘N creates a page at the end of the top level and opens it with the title focused. **New page** in the sidebar does the same.
- **TREE-2.** Clicking a row opens the page and moves focus to the editor. ⌘0 moves focus to the sidebar, and Escape returns it to the editor **(default)**. When the sidebar has focus, ↑ and ↓ move between rows, → expands, ← collapses, and Return opens the page.
- **TREE-3.** Each row has a menu, opened from its **…** button or by right-click: **Add subpage**, **Rename**, **Add to favorites** or **Remove from favorites**, and **Move to trash**. The command palette offers the same actions for the current page (CMD-6).
- **TREE-4.** **Add subpage** creates a page as the last child and opens it with the title focused.
- **TREE-5.** Dragging a row between two rows moves the page to that position. Dragging a row onto another row makes it that page's last child. A page cannot move under itself or its descendants. **Move page to…** in the palette moves the current page to be the last child of the chosen page, or to the end of the top level.
- **TREE-6.** Pages with children show a disclosure arrow. Expanded and collapsed state persists across launches.
- **TREE-7.** The editor title and the sidebar row always show the same title. A page with an empty title shows "Untitled".
- **TREE-8.** **Add to favorites** pins a page to **Favorites**. Dragging inside **Favorites** reorders favorites only. Removing a favorite does not change the page.
- **TREE-9.** **Move to trash** moves the page and all its descendants to trash. They disappear from the tree, favorites, search, backlinks, and the `[[` picker. If the open page becomes trashed, the next active sibling opens, else the nearest active ancestor, else the most recently opened active page, else the empty state (SHELL-4).
- **TREE-10.** The **Trash** row opens the trash view. It lists only the pages you trashed, not their descendants, newest first.
- **TREE-11.** A trashed page opens read-only, with a **Restore** button. Page commands other than **Restore** are disabled for it.
- **TREE-12.** **Restore** returns the page and the descendants trashed with it to their original parent, sibling position, expanded state, and favorites. If the original parent is not active, the page goes to the end of the top level.
- **TREE-13.** A trashed page is erased 30 days after it was trashed **(default)**. **Settings → Trash** offers **7 days**, **30 days**, **90 days**, and **Never**. The trash view shows how many days each page has left. The app checks for expired pages at launch and every hour.
- **TREE-14.** **Delete permanently** erases the page and the descendants trashed with it, after a confirmation that says this cannot be undone. Erasing deletes the page's Y.Doc files, removes its record from the workspace doc and compacts that doc so the title is gone from disk, deletes images no other page uses, and removes the page from the index. Exports already written are not changed. They age out under EXP-9.
- **TREE-15.** ⌘Z in the editor never undoes tree actions. Trash has its own undo, **Restore**.

### Acceptance tests

1. **Create and nest.** Press ⌘N once. Expect exactly one new page, shown as "Untitled". Type "Projects". Open its row menu and choose **Add subpage**. Type "Aelomi". Expect "Aelomi" indented under "Projects".
2. **Reorder by drag.** Setup: top-level pages "A", "B", and "C". Drag "C" between the top of the list and "A". Expect the order C, A, B.
3. **Nest by drag.** Setup: top-level pages "A" and "B". Drag "B" onto "A". Expect "B" as the child of "A".
4. **Illegal move.** Setup: "Projects" with child "Aelomi". Drag "Projects" onto "Aelomi". Expect "Projects" still at the top level with "Aelomi" under it.
5. **Keyboard navigation.** Setup: "Projects" with child "Aelomi", collapsed. Press ⌘0, select the "Projects" row with ↓ if needed, then press →, ↓, and Return. Expect "Aelomi" open.
6. **Favorites.** Setup: "Projects" with child "Aelomi". Choose **Add to favorites** on "Aelomi". Expect it under **Favorites** and still under "Projects". Choose **Remove from favorites**. Expect it gone from **Favorites** only.
7. **Trash and restore.** Setup: "Projects" with child "Aelomi", and "Aelomi" a favorite. Press ⌘K and type "Aelomi". Expect a result. Move "Projects" to trash. Expect both pages gone from the sidebar and **Favorites**, and only "Projects" listed in the trash view. Press ⌘K and type "Aelomi". Expect no result. Restore "Projects". Expect both back in place and "Aelomi" back in **Favorites**.
8. **Trash the open page.** Setup: "Projects" with children "A" and "B", and "A" open. Move "A" to trash. Expect "B" open. Move "Projects" to trash. Expect the empty state.
9. **Rename from the menu.** Setup: pages "Inbox draft" (open) and "Other". Choose **Rename** on the "Inbox draft" row and type "Inbox". Expect the editor title to read "Inbox".
10. **Delete permanently.** Setup: "Old" in trash. Choose **Delete permanently**. Expect a confirmation that says it cannot be undone. Confirm. Expect "Old" gone from the trash view.
12. **Trash expiry setting.** Setup: "Old" trashed 10 days ago. Open the trash view. Expect "20 days left". Set **Settings → Trash** to **7 days**. Expect "Old" gone from the trash view.
11. **Undo does not restore.** Setup: "A" (open) and "B". Type "x" in "A", then move "A" to trash. Press ⌘Z. Expect "A" still in trash.

### Core tests

- A property test applies random sequences of create, move, reorder, favorite, unfavorite, trash, restore, and erase. After each step, active pages form a tree with no cycles, each active page has one active parent or is at the top level, sibling and favorite orders have no duplicates, and trashed pages keep their original parent.
- Trash then restore returns the workspace doc to its prior tree, sibling order, and favorites, compared field by field.
- Trashed pages and their descendants never appear in search results, backlinks, or the `[[` picker list.
- Erase a page whose title and body contain a unique marker string, and which has an image no other page uses. Afterwards, no file in the data folder contains the marker, and the image file is gone. An image another page uses is kept.
- With a controllable clock and the 30-day setting, a page trashed 31 days ago is erased at launch and one trashed 29 days ago is not. With **Never**, a page trashed 400 days ago is kept.
- Tree structure, sibling order, favorites, and expanded state are the same after close and reopen.

### Perf tests

- With 10,000 pages, scrolling the sidebar for 5 seconds has a p95 frame time under 16.7 ms and no frame over 50 ms **(default)**.

## 3. Editor

The page editor is BlockNote core. This section lists the blocks and interactions the MVP supports.

### Behavior

- **EDIT-1.** A page has a title and a body. Return in the title moves the cursor to the first block of the body.
- **EDIT-2.** The editor schema has exactly these blocks: paragraph, heading 1 to 3, bulleted list, numbered list, to-do, quote, code, divider, and image. BlockNote's other default blocks are removed.
- **EDIT-3.** Typing `/` opens the slash menu, which lists only EDIT-2 blocks. Typing filters it. ↑ and ↓ move the selection, Return inserts the block, and Escape closes the menu and keeps the typed text.
- **EDIT-4.** Markdown shortcuts at the start of a block convert it: `#`, `##`, and `###` for headings, `-` or `*` for bullets, `1.` for numbered lists, `[]` for to-dos, `>` for quotes, ```` ``` ```` for code, and `---` for a divider.
- **EDIT-5.** Inline marks: ⌘B bold, ⌘I italic, ⌘U underline, ⌘E inline code, and ⇧⌘S strikethrough. The markdown forms `**bold**`, `*italic*`, and `` `code` `` also work as you type. Pasting a URL over selected text turns the text into a link. ⌘K stays reserved for the command palette (CMD-1).
- **EDIT-6.** Clicking a to-do checkbox toggles it. Checked to-dos show struck-through text.
- **EDIT-7.** Code blocks have a language picker and syntax highlighting. Tab inside a code block inserts two spaces.
- **EDIT-8.** Pasting or dropping a local image inserts an image block. The image bytes are on disk (DATA-3) before the page refers to them.
- **EDIT-9.** Each block has a drag handle on hover. Dragging it moves the block with its children. ⌘⇧↑ and ⌘⇧↓ move the current block up and down by keyboard.
- **EDIT-10.** Tab and ⇧Tab indent and outdent list items and to-dos. On other blocks outside code, Tab does nothing.
- **EDIT-11.** ⌘Z and ⇧⌘Z undo and redo editor content. Typing groups into one undo step until a pause of 500 ms or a move to another block. Paste, image insert, checkbox toggle, indent, outdent, and block move are each one undo step of their own. Each page keeps its undo history until the app quits.
- **EDIT-12.** Paste uses HTML when the clipboard has it, and otherwise parses plain text as markdown. Inside a code block, paste always inserts plain text. Structure outside EDIT-2, such as a table, becomes paragraphs with its text kept. Pasted text is never dropped.
- **EDIT-13.** A remote image in pasted content becomes a link to its URL. The app does not fetch it (SHELL-13).
- **EDIT-14.** A 5,000-block page stays within the performance budgets while scrolling and typing.

The markdown form of each block is defined once, in a table in the converter fixtures. Paste (TypeScript) and export (Rust) both run against the same fixtures:

| Block | Markdown |
|---|---|
| Heading 1 to 3 | `#`, `##`, `###` |
| Bulleted list | `- item` |
| Numbered list | `1. item` |
| To-do | `- [ ] item` and `- [x] item` |
| Quote | `> text` |
| Code | fenced with ```` ``` ```` and the language name |
| Divider | `---` |
| Image | `![](_assets/<hash>.<ext>)` |
| Page link | see EXP-4 |

### Acceptance tests

1. **Title to body.** Setup: one empty page with the title focused. Type "Plan" and press Return. Type "first". Expect the title "Plan" and a first block "first".
2. **Slash menu.** Setup: one empty page. Type `/head`. Expect the menu to show the heading options. Press Return. Expect a heading 1 block.
3. **Slash menu escape.** Setup: one empty page. Type `/head`, then press Escape. Expect the menu closed and "/head" in the block.
4. **Markdown shortcuts.** Setup: one empty page. Type each shortcut from EDIT-4 at the start of a new block. Expect each block type.
5. **Inline marks.** Setup: a page with "word". Select "word" and press ⌘B. Expect "word" bold. Repeat for each mark in EDIT-5.
6. **To-do.** Setup: one empty page. Type `[] ` then "Buy milk". Click its checkbox. Expect it checked and struck through.
7. **Code block.** Setup: one empty page. Type ```` ``` ```` and pick "Rust". Type `fn main() {}`. Expect keywords highlighted. Press Tab. Expect two spaces inserted.
8. **Image.** Setup: one empty page. Paste an image from the clipboard. Expect the image shown in the page.
9. **Drag to reorder.** Setup: blocks "one", "two" with an indented child "two-a", and "three". Drag "two" above "one". Expect the order two, two-a, one, three.
10. **Move by keyboard.** Setup: blocks "one" and "two", cursor in "two". Press ⌘⇧↑. Expect the order two, one.
11. **Indent.** Setup: bullets "a" and "b", cursor in "b". Press Tab. Expect "b" nested under "a". Press ⇧Tab. Expect it back.
12. **Undo grouping.** Setup: one empty page. Type "hello", wait 1 second, type " world", and press ⌘Z. Expect "hello". Press ⇧⌘Z. Expect "hello world".
13. **Paste markdown.** Setup: one empty page. Paste plain text `# Title\n- a\n- b`. Expect a heading and a two-item bulleted list.
14. **Paste a table.** Setup: one empty page. Paste HTML for a 2-by-2 table with cells "a", "b", "c", "d". Expect four paragraphs reading "a", "b", "c", and "d".
15. **Remote image.** Setup: one empty page. Paste HTML containing `<img src="https://example.com/x.png">`. Expect a link to that URL and no image request.

### Core tests

- Interrupt image insertion at every step. On reopen, the page either has no reference to the image or refers to bytes that are on disk.
- An inserted image survives deletion of the original file: after reopen, the page loads the same bytes.
- The Rust converter turns each fixture in the shared table into the expected markdown, byte for byte.

### Logic tests

- Each shared fixture parses to the expected block tree, checked block by block, including nesting and inline marks. Round-tripping each fixture also returns the same markdown.
- HTML paste fixtures (a table, nested lists, an unsupported element) produce the expected blocks, with all text kept.

### Perf tests

- On a 5,000-block page, keystroke-to-paint is under 16 ms at p95 while typing at the end of the page. Scrolling for 5 seconds has a p95 frame time under 16.7 ms and no frame over 50 ms **(default)**.

## 4. Links and backlinks

Pages link to each other with `[[`, and each page lists the pages that link to it.

### Behavior

- **LINK-1.** A page link is an inline node that stores only the target page ID. Its text is the target's current title, read when the link is shown or exported.
- **LINK-2.** Typing `[[` outside code opens a page picker. It lists active pages and filters by title as you type. When two results share a title, each shows its parent path in muted text. If the paths are also equal, each also shows its last-updated date. Return inserts a link to the selected page. Escape closes the picker and keeps the typed text. Inside a code block or inline code, `[[` is plain text.
- **LINK-3.** If no page matches, the picker offers **Create "<typed text>"**. Choosing it creates a page with that title at the end of the top level and inserts a link to it.
- **LINK-4.** Clicking a link opens the target page. ⌘[ and ⌘] go back and forward through visited pages.
- **LINK-5.** A link to a trashed page shows struck through and muted. Clicking it opens the page read-only in trash (TREE-11).
- **LINK-6.** A link to an erased page shows muted text, "Deleted page", and does nothing on click.
- **LINK-7.** Below the page body, a read-only **Backlinks** footer lists each active page that links to this page, most recently updated first. Each entry shows the page title and, for each linking block, that block's text on one line. The footer is not part of the page's Y.Doc, is not exported, and is not searched. It is not shown when no page links here.
- **LINK-8.** Backlinks update within 250 ms of the linking edit **(default)**.

### Acceptance tests

1. **Insert a link.** Setup: pages "Daily" (open) and "Aelomi". Type `[[Aelo` and press Return. Expect a link reading "Aelomi". Click it. Expect "Aelomi" open. Press ⌘[. Expect "Daily" open. Press ⌘]. Expect "Aelomi" open.
2. **Create from link.** Setup: page "Daily" (open). Type `[[Reading list` and choose **Create "Reading list"**. Expect "Reading list" in the sidebar and a link to it in "Daily".
3. **Rename updates links.** Setup: "Daily" (open) with a link to "Aelomi". Rename "Aelomi" to "Aelomi app". Expect the link to read "Aelomi app".
4. **Duplicate titles.** Setup: "Work" with child "Notes", and "Home" with child "Notes". Type `[[Notes`. Expect two results, showing "Work" and "Home" as their parents.
5. **No picker in code.** Setup: pages "Daily" (open, with a code block) and "Aelomi". Type `[[` inside the code block. Expect no picker.
6. **Backlinks.** Setup: pages "Aelomi" and "Daily", where "Daily" has a block "see ", a page link to "Aelomi", and " today". Open "Aelomi". Expect **Backlinks** listing "Daily" with "see Aelomi today". Remove the link from "Daily". Expect no **Backlinks** footer on "Aelomi".
7. **Trashed target.** Setup: "Daily" (open) linking to "Reading list". Expect a normal link. Move "Reading list" to trash. Expect the link struck through and muted. Click it. Expect "Reading list" read-only with **Restore**. Press ⌘K and type `rename`. Expect no **Rename page**.
8. **Erased target.** Setup: "Daily" (open) linking to "Old", and "Old" in trash. Delete "Old" permanently. Expect the link to read "Deleted page".

### Core tests

- After random link inserts, deletes, renames, trashes, and restores across pages, the backlink index equals a backlink set computed from scratch from the Y.Docs.
- A linking edit is reflected in the backlink index within 250 ms.

## 5. Command palette and search

⌘K is the main way to move around the app.

### Behavior

- **CMD-1.** ⌘K opens the command palette from anywhere. Escape closes it and returns focus to where it was.
- **CMD-2.** With an empty query, the palette lists recently opened pages, newest first, without the current page.
- **CMD-3.** With a query, results appear in three groups, in this order: pages whose title matches, matching commands, and pages whose content matches. A page that matches by title is not repeated under content.
- **CMD-4.** Title matching runs in TypeScript over the page list. The query matches if its characters appear in the title in order, ignoring case. Rank: exact match, then title prefix, then word starts, then any other match. Ties go to the most recently opened page.
- **CMD-5.** Content matching runs in the Rust core with SQLite FTS5. The last word of the query matches as a prefix. Each result shows the matching text with the query terms highlighted. Content results order newest-created first, then by page ID. Each page appears once, using its first matching block in document order.
- **CMD-6.** Commands: **New page**, **Add subpage**, **Rename page**, **Move page to…**, **Add to favorites** or **Remove from favorites**, **Move to trash**, **Toggle theme**, **Export now**, **Open settings**, and **Open trash**. Page commands act on the open page. They are not listed when no active page is open.
- **CMD-7.** The first result is selected when results change. ↑ and ↓ move the selection. Return runs the selected item. For a content match, the editor scrolls to the matching block and highlights it for 1 second.
- **CMD-8.** **Toggle theme** sets the opposite of the theme on screen: **Light** when it shows dark, **Dark** when it shows light.
- **CMD-9.** ⌘K is the only palette shortcut. There is no separate quick switcher.
- **CMD-10.** Trashed and erased pages never appear in results.

### Acceptance tests

1. **Recent pages.** Setup: pages "A", "B", and "C". Open "A", then "B", then "C". Press ⌘K. Expect "B" first, then "A", and no "C".
2. **Title search.** Setup: pages "Aelomi app" and "Apple". Press ⌘K and type `aelm`. Expect "Aelomi app" and not "Apple". Press Return. Expect "Aelomi app" open.
3. **Select a later result.** Setup: pages "Notes 1" and "Notes 2". Press ⌘K and type `notes`. Press ↓ and Return. Expect the second result open.
4. **Content search.** Setup: "Daily" with a block "quarterly roadmap" far down the page. Press ⌘K and type `roadm`. Expect "Daily" with "roadmap" highlighted. Press Return. Expect the editor scrolled to that block.
5. **Commands.** Setup: theme set to **Light**. Press ⌘K and type `theme`. Press Return on **Toggle theme**. Expect the dark theme.
6. **Page commands.** Setup: "Projects" and "Aelomi" (open). Press ⌘K, run **Move page to…**, and choose "Projects". Expect "Aelomi" under "Projects".
7. **Escape.** Setup: one page with the cursor in its body. Press ⌘K, then Escape. Type "x". Expect "x" in the body.

### Core tests

- On the search fixture, a fixed list of content queries returns the expected page IDs. The fixture is 10,000 pages from a seeded generator, checked into `fixtures/`, with the expected IDs listed next to it.
- A content query on the search fixture takes under 5 ms at p95 **(default)**.

### Logic tests

- A checked-in table of titles and queries returns the ranked order CMD-4 defines.

### Perf tests

- With the search fixture loaded into the mock, the palette shows results within 16 ms of each keystroke at p95, one frame at 60 Hz **(default)**. This measures the UI. The Rust query budget above covers the core. The real IPC hop between them is covered by the maintainer's daily use.

## 6. Markdown export

A scheduled copy of every active page as plain markdown files, in a folder the user picks. It is the user's backup if Aelomi's own data is damaged, and their way out if they stop using Aelomi. The files open in any text editor.

### Behavior

- **EXP-1.** **Settings → Export** has a folder picker, a schedule (**Off**, **Daily**, or **Weekly**), and an **Export now** button. The schedule starts as **Off**. The first time a folder is picked, it changes to **Daily** **(default)**.
- **EXP-2.** Scheduled exports run while the app is open. An export is due when no export to the current folder has succeeded yet, or when the time since the last successful one is longer than the schedule interval. Changing the folder starts its history over. The app checks at launch, on wake from sleep, and every minute.
- **EXP-3.** The export reads one consistent snapshot of all pages, taken when it starts. Edits made during the export go into the next one.
- **EXP-4.** Each export is a new folder, `<folder>/Aelomi exports/<date> <time>/`, for example `2026-10-11 0930`. In it, each active page is one `.md` file. A page with children also has a folder with the same name, next to its file, holding the children. Links become relative markdown links to the target page's file. A link to a page that is not exported becomes its plain title text.
- **EXP-5.** Images are copied into an `_assets` folder inside each export, named by hash, and referenced by relative path. An image unchanged since the previous export is an APFS clone of that copy, so it takes no extra disk space. On other file systems it is a plain copy.
- **EXP-6.** Each file starts with front matter holding the page ID, created time, and updated time.
- **EXP-7.** File and folder names come from the page title. Characters the file system does not allow become `-`. An empty title becomes "Untitled". Names are cut to 200 bytes. Names are compared after Unicode NFC normalization and ignoring case, because macOS file systems do by default. Siblings get names in order of age, by created time, then page ID. Each takes the first free name of its title, then the title plus " (2)", " (3)", and so on. Names stay the same between exports unless an older sibling with the same name is added or removed. `_assets` and names starting with `.` are reserved, so a page with such a title gets a leading `-`.
- **EXP-8.** `<folder>/Aelomi exports/.aelomi-export` holds the workspace ID, and so does each export folder. If `<folder>/Aelomi exports`, in any letter case, exists without that marker or with another workspace's ID, the export stops and reports why. It does not touch that folder.
- **EXP-9.** The export writes into `<folder>/Aelomi exports/.staging/`, then renames it to its dated name in one step. The app keeps the 7 newest exports and deletes older ones **(default)**. It deletes only folders that hold this workspace's marker. A leftover `.staging` folder is removed before the next export.
- **EXP-10.** The export creates, changes, or deletes nothing in the picked folder outside `Aelomi exports/`.
- **EXP-11.** **Settings → Export** shows the time of the last successful export and the error from the last failed one, if any. After a failed export, a small indicator shows in the sidebar footer until an export succeeds.

### Acceptance tests

1. **Export now.** Setup: one page, and an export folder picked. Click **Export now**. Expect **Settings** to show the time of the last successful export.
2. **Default schedule.** Setup: no export folder. Expect the schedule **Off**. Pick a folder. Expect **Daily**.
3. **Failure.** Setup: an export folder, and the mock fails exports with "Disk full". Click **Export now**. Expect "Disk full" in **Settings** and the indicator in the sidebar footer. Make the mock succeed and click **Export now**. Expect the indicator gone.

Acceptance tests cannot read exported files. The core tests below cover the output.

### Core tests

- Export a fixture workspace and compare the output folder byte for byte with a checked-in expected folder. The fixture covers every block in the shared table, nested pages, images, links to active, trashed, and erased pages, and a trashed page.
- Interrupt the export at every step. Each time, every dated export folder is complete, and the older exports are unchanged.
- An existing `<folder>/aelomi exports/` without a marker, containing a file, is unchanged after an export attempt, and the export reports an error. A folder without the marker inside `Aelomi exports/` is never deleted by the 7-export limit.
- After 9 exports, exactly the 7 newest remain. An unchanged image in two exports shares disk blocks on APFS.
- A file placed in the picked folder outside `Aelomi exports/` is unchanged after export.
- Name fixtures produce valid, distinct, stable names for: `/`, `:`, leading dots, emoji, an empty title, a 300-byte title, "Notes" and "notes" as siblings, two Unicode forms of "é", two top-level "Projects" pages that each have a child "Notes", a title "Notes (2)" next to two pages titled "Notes", and a page titled "_assets".
- Renaming and moving pages while an export runs still produces an export whose links all resolve.
- With the schedule **Daily** and the last success 25 hours ago, launch runs an export. With the last success 23 hours ago, it does not. With **Weekly**, the cutoff is 7 days.

## Performance budgets

The budgets in `CLAUDE.md` apply to every feature above. CI gates runner-calibrated versions of them, and local commands check the exact numbers on Apple Silicon (see `CLAUDE.md`). This spec defines how each is measured:

- **Cold start under 500 ms.** From process launch to the open page's first content paint, measured by launching the built `.app`, median of 10 runs.
- **Keystroke-to-paint under 16 ms.** From the key event to the first frame that shows the typed character, at p95, in Playwright WebKit (EDIT-14).
- **A 5,000-block page stays smooth.** Defined in the editor perf tests.

This spec adds four more budgets, all **(default)**: the sidebar at 10,000 pages (section 2), content search under 5 ms and palette results under 16 ms (section 5), backlinks within 250 ms (LINK-8), and edits on disk within 100 ms (SHELL-9). A ballpark run on the maintainer's Mac on 2026-10-11 informed them: FTS5 content search over 10,000 synthetic pages took 0.2 to 0.3 ms at p50 and 0.4 to 1 ms at p95, fuzzy title matching over 10,000 titles in Node took 1.3 to 1.9 ms at p50 and 1.7 to 4.1 ms at p95, and one `F_FULLFSYNC` of a small append took about 4 ms at p50 and 4 to 6 ms at p95. Each figure is the range over 5 runs, on a machine with a load average near 8. Remeasure on the real app once it exists.

## Open questions

None right now.

Decided on 2026-10-11:

- **Delete permanently** truly erases (TREE-14). This is the one exception to the append-only rule, and the user asks for it.
- Trash empties after 30 days by default, with a setting (TREE-13).
- Export runs daily once a folder is picked and keeps the 7 newest copies (EXP-1, EXP-9).
- Faster budgets, informed by a ballpark measurement (Performance budgets).
- ⌘K is the only palette shortcut (CMD-9).
- Stored history is compacted when it grows large (DATA-6).
