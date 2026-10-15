# Storage startup and URL links

Aelomi opens pages only after its data folder is ready. A refused cloud folder shows its resolved path, explains database corruption, and gives steps to move it back to this Mac. Clicking a URL in a page opens the default browser while the Aelomi page stays open.

## Sub-features

- `cloud-refusal` shows where the data really is and says cloud sync corrupts the database. Nothing in the folder was changed. The title receives focus.
- `cloud-leaf-link` keeps the screen open while Finder steps open the link's parent, remove only the link, open the cloud folder's parent, and move the data folder into the first window. A different folder name gets an explicit rename. Quit and reopen comes last.
- `cloud-shared-link` names the synced folder, says other apps and files may use it, and directs the user to turn off syncing in the cloud app's settings. It gives no removal or movement instruction.
- `cloud-override` tells the user to set `AELOMI_DATA_DIR` outside cloud storage or unset it, then reopen.
- `cloud-path-copy` selects a complete path chip with one click or keyboard focus for copying with ⌘C.
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
- Create a fake cloud folder outside the checkout. Set `AELOMI_CLOUD_ROOTS` to it for each dev launch. Use an absolute canonical path. Never use the real cloud folders.
- For the leaf case, create a sibling link named `Aelomi` to `<fake cloud>/Aelomi Notes`. Set `AELOMI_DATA_DIR` to the link. Also try a trailing slash and `missing/../Aelomi`.
- For the shared case, create a synthetic `Library/Application Support` link to the fake cloud folder and configure `<link>/Aelomi Dev`. For the override case, configure `<fake cloud>/Aelomi Dev` directly.

- **Leaf refusal.** Run `$S/app.sh doctor "$RUN"`, then `$S/app.sh tree "$RUN"`. Expect the heading `Move your data folder out of cloud storage`, the resolved path under `Where your data really is`, and separate path chips for both Finder parent folders. The first step trashes only the `Aelomi` link. The second moves `Aelomi Notes` into the first window and renames it `Aelomi`. The last step is `Quit Aelomi and open it again.` The editor is absent.
- **Shared refusal.** Expect the synthetic `Application Support` link, the warning that other apps and files may use it, and cloud-app settings guidance to make it a regular folder on this Mac. No step says to trash, move, or replace anything.
- **Override refusal.** Expect only the override and restart steps, with no Finder movement instructions.
- **Copy a path.** Click a parent-path chip, press ⌘C, and compare the clipboard with the exact displayed path. Focus another chip by keyboard and repeat. The whole path is selected, including spaces and non-Latin characters.
- **Appearance.** For each of the three variants, capture `$S/app.sh screenshot "$RUN" cloud-dark` and `cloud-light` with the Mac in each appearance. Restore the original appearance. Both shots show the complete path and readable instructions.
- **URL link.** Launch on a synthetic folder with one page and a URL link to `https://example.com`. Run `$S/app.sh tree "$RUN"` to find its link label. Capture `url-before`, press the link with `$S/app.sh press "$RUN" "Example"`, and capture `url-after`. Expect `url opener launched=true` in the shell log. This log proves the OS opener launched, not browser receipt. Observe the browser and confirm Aelomi still shows the same page.
- **Pointer.** Click the link with native pointer input and expect exactly one new `url opener launched=true` line.
- **Modified click.** Hold Shift while clicking the link and expect no new opener line. Repeat with Option, Control, and Command when available.
- **Selection.** Double-click the link with native pointer input. Expect only one new opener line. Then make a later single click on the selected link and expect one more opener line, exactly two for that sequence. Drag-select its text and expect no new opener line.
- **Context menu.** In a packaged build, right-click a link targeting a synthetic loopback request counter. Expect no WebKit link menu and no request at the counter. `allowLinkPreview` must be false in the bundle configuration. Capture the app and the request count.
- **Navigation guard.** In the dev window’s Web Inspector console, assign a synthetic loopback URL to `location.href`. Expect `url navigation refused=true`, no request at the loopback server, and the original page still visible. This action must bypass the captured link handler.
- **Quit survival.** Type a unique synthetic marker, quit with ⌘Q, relaunch with the same isolated data folder, and confirm the exact marker in the editor.
- **Mock URL link.** Run `pnpm test:e2e e2e/url-links.e2e.ts`. Expect the open-URL call to receive exactly `https://example.com`, and the page heading and body to remain.
- **Cleanup.** Run `$S/app.sh cleanup "$RUN"`. Keep the named evidence directory.

## Gotchas

- `AELOMI_CLOUD_ROOTS` applies only in debug builds. Never move or modify the real data folder for this check.
- AXPress produces a zero-detail click. Prove it separately from pointer input.
- A refused startup still has an accessible IPC status for the launch helper. No editor is mounted.
