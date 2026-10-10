# Aelomi MVP specification

Status: **draft**. The MVP scope in `CLAUDE.md` is still pending maintainer confirmation. Choices marked **(default)** are proposals the maintainer can override. Open questions are listed at the end.

This file lists what the MVP does and how each behavior is tested. Each behavior has an ID, such as `TREE-3`. Tests, PRs, and commit messages cite these IDs.

## How to read the tests

Each feature has three kinds of tests. They map to the testing table in `CLAUDE.md`.

- **Acceptance** tests are plain-English steps for TesterArmy `e2e`. They run in a browser with Tauri IPC mocked. Each one ends in something a person can see on screen.
- **Core** tests are cargo-nextest and property tests in the Rust core. Data-safety tests end in exact assertions, never an AI judgment.
- **Logic** tests are Vitest tests for pure TypeScript functions.

Acceptance tests cannot quit and relaunch the app, because IPC is mocked. Anything that must survive a relaunch is a core test, plus the maintainer's daily use.

A feature is done when every test listed for it passes in CI. The full definition of done is in `CLAUDE.md`.

## Scope

The MVP has six features:

1. [App shell and data safety](#1-app-shell-and-data-safety)
2. [Page tree](#2-page-tree)
3. [Editor](#3-editor)
4. [Links and backlinks](#4-links-and-backlinks)
5. [Command palette and search](#5-command-palette-and-search)
6. [Markdown export](#6-markdown-export)

These are out of scope for the MVP: the MCP server, databases and table views, Notion import, sync, accounts, sharing, AI features, telemetry, comments, page history UI, multiple windows, tabs, and file attachments other than images.

## 1. App shell and data safety

The window, theme, keyboard model, and the guarantees about user data.

### Behavior

- **SHELL-1.** The app opens one window with a sidebar on the left and the current page on the right. The window uses standard macOS controls and a native menu bar.
- **SHELL-2.** On first launch, the app creates one page titled "Getting started" and opens it.
- **SHELL-3.** On later launches, the app reopens the page that was open when it quit, at the same scroll position.
- **SHELL-4.** The theme follows the macOS appearance setting by default. **Settings** offers **System**, **Light**, and **Dark**.
- **SHELL-5.** ⌘\ shows and hides the sidebar. ⌘, opens **Settings**.
- **SHELL-6.** Every edit is saved without a save action. No UI says "saving" or "saved".
- **SHELL-7.** Data lives in `~/Library/Application Support/Aelomi`. If that path is inside iCloud Drive, the app refuses to start and explains why.
- **SHELL-8.** If the app or the Mac crashes, the app loses at most the last 1 second of typing **(default)**.
- **SHELL-9.** If the search index is missing or corrupt at startup, the app rebuilds it from page content and keeps working. It shows no error unless the rebuild fails.
- **SHELL-10.** The app makes no network calls in the MVP.

### Acceptance tests

1. **First launch.** Start the app with no data. Expect one page, "Getting started", open in the editor and listed in the sidebar.
2. **Theme.** Set the theme to **Dark** in **Settings**. Expect dark backgrounds everywhere, including menus and the command palette.
3. **Toggle sidebar.** Press ⌘\. Expect the sidebar to hide and the editor to fill the window. Press ⌘\ again. Expect the sidebar back.

### Core tests

- Writes are append-only: no stored update is ever rewritten or dropped in place. If compaction exists, it writes new files and keeps the old ones until the new ones are verified to load the same document.
- Session state round-trips: the open page, its scroll position, and every setting are the same after close and reopen.
- Kill the process at a random point during a write, 1,000 times. On every reopen, the document equals the state after the last fully written update. No reopen fails.
- Delete the SQLite index, reopen, and compare the rebuilt index row by row with the original. They must be equal.
- Every migration has a test that opens a database file from the previous schema version, checked into `fixtures/`, and asserts its pages are unchanged.
- The data path check rejects paths under `~/Library/Mobile Documents`.

## 2. Page tree

The sidebar shows every page as a tree, with **Favorites** above it and **Trash** below it.

### Behavior

- **TREE-1.** ⌘N creates a page at the top level and opens it with the title focused. **New page** in the sidebar does the same.
- **TREE-2.** Each page row has an **Add subpage** action that creates a child page.
- **TREE-3.** You can drag a page to reorder it among its siblings or to move it under another page. A page cannot move under itself or its descendants.
- **TREE-4.** Pages with children show a disclosure arrow. Expanded and collapsed state persists across launches.
- **TREE-5.** You can rename a page by editing its title in the editor or by double-clicking its row. Both stay in sync.
- **TREE-6.** A page without a title shows "Untitled".
- **TREE-7.** **Add to favorites** pins a page to the **Favorites** section. Favorites can be reordered by drag. Removing a favorite does not change the page.
- **TREE-8.** **Move to trash** moves the page and all its descendants to **Trash**. They disappear from the tree, favorites, search, and backlinks.
- **TREE-9.** **Trash** lists only the pages you trashed, not their descendants. **Restore** moves a trashed page back to its original parent, with the descendants that were trashed with it. If that parent is gone or trashed, the page goes to the top level.
- **TREE-10.** Trashed pages stay in **Trash** until you delete them **(default: no automatic expiry)**.
- **TREE-11.** **Delete permanently** asks for confirmation, then removes the page from **Trash**. See open question Q1 for what this does to stored data.
- **TREE-12.** The sidebar stays smooth with 10,000 pages.

### Acceptance tests

1. **Create and nest.** Press ⌘N and type "Projects". Use **Add subpage** on "Projects" and type "Aelomi". Expect "Aelomi" indented under "Projects" in the sidebar.
2. **Reorder by drag.** Create pages "A", "B", and "C" at the top level. Drag "C" above "A". Expect the order C, A, B.
3. **Illegal move.** Drag "Projects" onto its child "Aelomi". Expect the move to be refused and the tree unchanged.
4. **Favorites.** Add "Aelomi" to favorites. Expect it under **Favorites** and still under "Projects". Remove it from favorites. Expect it gone from **Favorites** only.
5. **Trash and restore.** Move "Projects" to trash. Expect "Projects" and "Aelomi" gone from the sidebar, and only "Projects" listed in **Trash**. Search for "Aelomi". Expect no result. Restore "Projects". Expect both back in their original places.
6. **Rename sync.** Double-click a page row and rename it "Inbox". Expect the editor title to read "Inbox".

### Core tests

- A property test applies random sequences of create, move, reorder, trash, and restore. After each step, the tree has no cycles, every page has exactly one parent or is at the top level, and sibling order has no duplicates.
- Trash and restore round-trip: trash then restore returns the tree to an exact equal of its prior state.
- Tree structure, sibling order, favorites, and expanded state are the same after close and reopen.

## 3. Editor

The page editor is BlockNote core. This section lists the blocks and interactions the MVP supports.

### Behavior

- **EDIT-1.** A page has a title and a body. Enter in the title moves the cursor to the first block of the body.
- **EDIT-2.** Supported blocks: paragraph, heading 1 to 3, bulleted list, numbered list, to-do, quote, code, divider, and image.
- **EDIT-3.** Typing `/` opens the slash menu. Typing filters it. Arrow keys and Enter pick a block. Escape closes it.
- **EDIT-4.** Markdown shortcuts at the start of a block convert it: `#`, `##`, and `###` for headings, `-` or `*` for bullets, `1.` for numbered lists, `[]` for to-dos, `>` for quotes, ```` ``` ```` for code, and `---` for a divider.
- **EDIT-5.** Inline marks: ⌘B bold, ⌘I italic, ⌘U underline, ⌘E inline code, and ⇧⌘S strikethrough. Pasting a URL over selected text turns the text into a link. ⌘K stays reserved for the command palette (CMD-1). The markdown forms `**bold**`, `*italic*`, and `` `code` `` also work as you type.
- **EDIT-6.** Clicking a to-do checkbox toggles it. Checked to-dos show struck-through text.
- **EDIT-7.** Code blocks have a language picker and syntax highlighting. Tab inside a code block inserts indentation.
- **EDIT-8.** Pasting or dropping an image inserts an image block. The image file is copied into the app's data folder, so deleting the original does not break the page.
- **EDIT-9.** Each block has a drag handle on hover. Dragging it moves the block, with its children.
- **EDIT-10.** Tab and ⇧Tab indent and outdent list items and to-dos.
- **EDIT-11.** ⌘Z and ⇧⌘Z undo and redo, per page.
- **EDIT-12.** Pasting markdown text converts it to blocks. Pasting HTML keeps the structure EDIT-2 supports and drops the rest.
- **EDIT-13.** A 5,000-block page scrolls and edits within the performance budgets.

### Acceptance tests

1. **Slash menu.** In an empty block, type `/head`. Expect the menu to show the heading options. Press Enter. Expect a heading 1 block.
2. **Markdown shortcuts.** Type `[] ` then "Buy milk". Expect a to-do block with "Buy milk". Click its checkbox. Expect it checked and struck through.
3. **Code block.** Type ```` ``` ```` and pick "Rust". Type `fn main() {}`. Expect keywords highlighted.
4. **Image.** Paste an image from the clipboard. Expect the image shown in the page.
5. **Drag to reorder.** Create blocks "one", "two", and "three". Drag "three" above "one". Expect the order three, one, two.
6. **Undo.** Type "hello", then press ⌘Z. Expect "hello" removed.
7. **Paste markdown.** Paste `# Title\n- a\n- b`. Expect a heading and a two-item bulleted list.

### Core tests

- An inserted image is copied into the data folder. After the original file is deleted and the app reopens, the page still loads the image bytes unchanged.

### Logic tests

- Markdown-to-blocks and blocks-to-markdown conversion for every block type in EDIT-2. Round-tripping each fixture gives the same markdown.

## 4. Links and backlinks

Pages link to each other with `[[` and each page lists the pages that link to it.

### Behavior

- **LINK-1.** Typing `[[` opens a page picker that filters by title as you type. Enter inserts a link to the selected page.
- **LINK-2.** If no page matches, the picker offers **Create "<typed text>"**. Choosing it creates a top-level page with that title and inserts a link to it.
- **LINK-3.** A link shows the target page's current title. Renaming the target updates every link to it.
- **LINK-4.** Clicking a link opens the target page. ⌘[ and ⌘] go back and forward through visited pages.
- **LINK-5.** A link to a trashed page shows as struck through and muted. Clicking it opens the page in **Trash** with a **Restore** button.
- **LINK-6.** The bottom of each page shows a **Backlinks** section listing every page that links to it, with the text around each link. The section is hidden when the list is empty.
- **LINK-7.** Backlinks update within 1 second of the linking edit **(default)**.

### Acceptance tests

1. **Insert a link.** On page "Daily", type `[[Aelo` and press Enter. Expect a link reading "Aelomi". Click it. Expect the "Aelomi" page to open. Press ⌘[. Expect "Daily" again.
2. **Create from link.** Type `[[Reading list` and choose **Create "Reading list"**. Expect a new page "Reading list" in the sidebar and a link to it in "Daily".
3. **Rename updates links.** Rename "Aelomi" to "Aelomi app". Expect the link in "Daily" to read "Aelomi app".
4. **Backlinks.** Open "Aelomi app". Expect **Backlinks** to list "Daily" with the text around the link.
5. **Trashed target.** Move "Reading list" to trash. Expect its link in "Daily" struck through and muted.

### Core tests

- After random link insertions and deletions across pages, the backlink index equals a backlink set computed from scratch from page content.

## 5. Command palette and search

⌘K is the main way to move around the app.

### Behavior

- **CMD-1.** ⌘K opens the command palette from anywhere. Escape closes it and returns focus to where it was.
- **CMD-2.** With an empty query, the palette lists recently opened pages, newest first.
- **CMD-3.** Typing matches page titles with fuzzy matching and page content with full-text search. Title matches rank above content matches.
- **CMD-4.** Content matches show the matched text with the query terms highlighted.
- **CMD-5.** Enter opens the selected page. For a content match, the editor scrolls to the matching block and highlights it briefly.
- **CMD-6.** The palette also lists commands that match the query: **New page**, **Toggle theme**, **Export now**, **Open settings**, and **Open trash**.
- **CMD-7.** ⌘P opens the same palette limited to pages, as a quick switcher.
- **CMD-8.** Trashed pages never appear in results.
- **CMD-9.** Results update within 50 ms of a keystroke with 10,000 pages **(default)**.

### Acceptance tests

1. **Recent pages.** Open "Daily", then "Aelomi app". Press ⌘K. Expect "Aelomi app" first and "Daily" second.
2. **Title search.** Press ⌘K and type `aelm`. Expect "Aelomi app" in the results. Press Enter. Expect it to open.
3. **Content search.** Add the text "quarterly roadmap" to "Daily". Press ⌘K and type `roadmap`. Expect "Daily" with "roadmap" highlighted. Press Enter. Expect the editor scrolled to that block.
4. **Commands.** Press ⌘K and type `theme`. Expect **Toggle theme**. Press Enter. Expect the theme to switch.
5. **Keyboard only.** Do tests 2 to 4 without touching the mouse.

### Core tests

- Search over a fixture of 10,000 pages returns expected result IDs, in order, for a fixed list of queries.
- The 50 ms budget in CMD-9 is measured in the Playwright performance suite.

### Logic tests

- Fuzzy title ranking for a fixed list of queries and titles returns the expected order.

## 6. Markdown export

A scheduled export of every page to a folder the user picks. It is the user's backup and their way out.

### Behavior

- **EXP-1.** **Settings → Export** has a folder picker, a schedule (**Off**, **Hourly**, or **Daily**), and an **Export now** button. The default schedule is **Off** until a folder is picked, then **Daily** **(default)**.
- **EXP-2.** The export writes into `<folder>/Aelomi/`. Each page is one `.md` file. Child pages go in a folder named after the parent page.
- **EXP-3.** Images are copied into `<folder>/Aelomi/assets/` and referenced by relative path.
- **EXP-4.** `[[links]]` become relative markdown links to the target page's file.
- **EXP-5.** Each file starts with front matter holding the page ID, created time, and updated time.
- **EXP-6.** Trashed pages are not exported.
- **EXP-7.** The export writes to a temporary folder first, then swaps it in. A failed or interrupted export leaves the previous export intact.
- **EXP-8.** The export never touches files outside `<folder>/Aelomi/`.
- **EXP-9.** **Settings → Export** shows the time of the last successful export, or the error from the last failed one.
- **EXP-10.** File names are the page titles, made safe for the file system. Two sibling pages with the same title get distinct names.

### Acceptance tests

1. **Export now.** Pick a folder and click **Export now**. Expect **Settings** to show the time of the last successful export.
2. **Schedule.** Pick a folder. Expect the schedule to change from **Off** to **Daily**.

Acceptance tests cannot read the exported files because IPC is mocked. The core tests below cover the output.

### Core tests

- Export a fixture workspace and compare the output folder byte for byte with a checked-in expected folder.
- Interrupt the export at random points. Every time, the folder holds either the complete previous export or the complete new one.
- A file placed in the export folder outside `Aelomi/` is unchanged after export.
- Titles with `/`, `:`, leading dots, emoji, and duplicate names produce valid, distinct file names.

## Performance budgets

The budgets in `CLAUDE.md` are CI gates for every feature above:

- Cold start under 500 ms.
- Keystroke-to-paint under 16 ms.
- A 5,000-block page stays smooth to scroll and edit.

This spec adds two proposed budgets: CMD-9 (search under 50 ms at 10,000 pages) and TREE-12 (a smooth sidebar at 10,000 pages).

## Open questions

These need the maintainer.

- **Q1. Permanent delete.** Storage is append-only and "never lose user data" is a hard rule. Should **Delete permanently** only hide the page from **Trash** while keeping its data on disk, or truly erase it? Erasing is what users expect from the word "permanently". Keeping it is safer.
- **Q2. Trash expiry.** Should trashed pages expire after 30 days, as in Notion, or stay forever (the current default)?
- **Q3. Getting started page.** What should the first-launch page say? A short tour of ⌘K, `/`, and `[[`, or empty?
- **Q4. Export default.** Is **Daily** the right default once a folder is picked?
- **Q5. Budget numbers.** Are 50 ms for search and 1 second for backlink updates the right targets?
- **Q6. ⌘P.** CMD-7 binds ⌘P to the quick switcher, as Notion does. On macOS, ⌘P usually means Print. Keep it, or pick another shortcut?
