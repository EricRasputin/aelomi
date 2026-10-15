# Storage startup and URL links

Aelomi opens pages only after its data folder is ready. A refused cloud folder shows its resolved path, explains database corruption, and gives steps to move it back to this Mac. Clicking a URL in a page opens the default browser while the Aelomi page stays open.

## Sub-features

- `cloud-refusal` shows the configured and resolved paths, Finder instructions, symbolic-link removal, and a warning against overwriting a folder. The refusal title receives focus.
- `storage-refusals` shows specific messages for a locked folder, a newer or unknown format, and a read error.

- `url-browser` opens http, https, and mailto links in their default application.
- `navigation-guard` keeps the window on the app when another origin tries to navigate it.

## How to get to it (user POV)

- Launch Aelomi with its data folder inside cloud storage.
- Launch a second Aelomi process on the same folder to see the locked message.

- Click a URL link in the page body.

## Driving it with app.sh and mock-drive

Preconditions:

- Use synthetic folders and a dev build. Read the launch instructions in `../SKILL.md`.
- Create a fake cloud folder outside the checkout. Create a sibling symbolic link to its child folder. Set `AELOMI_CLOUD_ROOTS` to the fake cloud root and `AELOMI_DATA_DIR` to the link when you run `$S/app.sh launch dev`.

- **Refusal.** Run `$S/app.sh doctor "$RUN"`, then `$S/app.sh tree "$RUN"`. Expect the heading `Move your data folder out of cloud storage`, both complete paths, Finder’s Go to Folder, symbolic-link removal, the overwrite precaution, and the quit, move, and reopen steps. The editor is absent.
- **Appearance.** Capture `$S/app.sh screenshot "$RUN" cloud-dark` and `cloud-light` with the Mac in each appearance. Restore the original appearance. Both shots show the complete path and readable instructions.
- **URL link.** Launch on a synthetic folder with one page and a URL link to `https://example.com`. Run `$S/app.sh tree "$RUN"` to find its link label. Capture `url-before`, press the link with `$S/app.sh press "$RUN" "Example"`, and capture `url-after`. Expect `url opener launched=true` in the shell log. This log proves the OS opener launched, not browser receipt. Observe the browser and confirm Aelomi still shows the same page.
- **Selection.** Double-click the link with native pointer input. Expect only one new opener line. Drag-select its text and expect no new opener line.
- **Context menu.** In a packaged build, right-click a link targeting a synthetic loopback request counter. Expect no WebKit link menu and no request at the counter. `allowLinkPreview` must be false in the bundle configuration. Capture the app and the request count.
- **Navigation guard.** In the dev window’s Web Inspector console, assign a synthetic loopback URL to `location.href`. Expect `url navigation refused=true`, no request at the loopback server, and the original page still visible. This action must bypass the captured link handler.
- **Quit survival.** Type a unique synthetic marker, quit with ⌘Q, relaunch with the same isolated data folder, and confirm the exact marker in the editor.
- **Mock URL link.** Run `pnpm test:e2e e2e/url-links.e2e.ts`. Expect the open-URL call to receive exactly `https://example.com`, and the page heading and body to remain.
- **Cleanup.** Run `$S/app.sh cleanup "$RUN"`. Keep the named evidence directory.

## Gotchas

- `AELOMI_CLOUD_ROOTS` applies only in debug builds. Never move or modify the real data folder for this check.
- A refused startup still has an accessible IPC status for the launch helper. No editor is mounted.
