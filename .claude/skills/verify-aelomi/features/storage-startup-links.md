# Storage startup and URL links

Aelomi opens pages only after its data folder is ready. A refused cloud folder shows its resolved path, explains database corruption, and gives steps to move it back to this Mac. Clicking a URL in a page opens the default browser while the Aelomi page stays open.

## Sub-features

- `cloud-refusal` shows the resolved cloud path and the steps to quit, move the folder, and reopen.
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
- Create a fake cloud folder outside the checkout. Set `AELOMI_CLOUD_ROOTS` to it and `AELOMI_DATA_DIR` to a child folder when you run `$S/app.sh launch dev`.

- **Refusal.** Run `$S/app.sh doctor "$RUN"`, then `$S/app.sh tree "$RUN"`. Expect the heading `Move your data folder out of cloud storage`, the resolved path, and the quit, move, and reopen steps. The editor is absent.
- **Appearance.** Capture `$S/app.sh screenshot "$RUN" cloud-dark` and `cloud-light` with the Mac in each appearance. Restore the original appearance. Both shots show the complete path and readable instructions.
- **URL link.** Launch on a synthetic folder with one page and a URL link to `https://example.com`. Run `$S/app.sh tree "$RUN"` to find its link label. Capture `url-before`, press the link with `$S/app.sh press "$RUN" "Example"`, and capture `url-after`. Expect `url opener handed_to_browser=true` in the shell log. The default browser receives the URL and Aelomi still shows the same page.
- **Mock URL link.** Run `pnpm test:e2e e2e/url-links.e2e.ts`. Expect the open-URL call to receive exactly `https://example.com`, and the page heading and body to remain.
- **Cleanup.** Run `$S/app.sh cleanup "$RUN"`. Keep the named evidence directory.

## Gotchas

- `AELOMI_CLOUD_ROOTS` applies only in debug builds. Never move or modify the real data folder for this check.
- A refused startup still has an accessible IPC status for the launch helper. No editor is mounted.
