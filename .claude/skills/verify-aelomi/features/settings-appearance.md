# Settings appearance

Settings opens inside the main window. Its Appearance control switches the app between System, Light and Dark immediately. The native title bar and menus continue to follow macOS.

## Sub-features

- `settings-modal` opens one modal by ⌘, or Aelomi > Settings… and closes it by Escape or Close Settings, with focus restored (SHELL-6, SHELL-7).
- `settings-palette` closes an open palette before Settings opens and blocks palette activation until Settings closes (SHELL-6).
- `settings-theme` applies the chosen appearance to the page, modal and palette without animation (SHELL-5).
- `settings-system` matches the current macOS appearance and follows its later changes (SHELL-5).
- `settings-durable-theme` reopens with the stored choice on the first content frame (DATA-4, SHELL-8).
- `settings-failure` reports failed writes through the save-failure banner and retains theme picks for retry (SHELL-8, SHELL-10).

## How to get to it (user POV)

- Press ⌘, or choose Aelomi > Settings… in the native menu.
- Select System, Light or Dark under Appearance. Arrow keys change the focused radio.
- Press Escape or click Close Settings to return to the page.

## Driving it with app.sh and mock-drive

Preconditions: Launch through `app.sh` on a fresh synthetic data folder and pass doctor. Take `/tmp/aelomi-ui.lock` before any native key, click or transient screenshot. Read macOS appearance with `defaults read -g AppleInterfaceStyle`; a missing key means Light. Keep that setting unchanged.

- **Shortcut.** Run `$S/app.sh keys "$RUN" , command`. Expect Settings, Appearance, System, Light, Dark and Close Settings in the tree. Capture settings-light. Send Escape by PID-targeted System Events key code 53 and expect Settings gone.
- **Menu.** AXPress Aelomi in the owned process's menu bar, then Settings… in its first menu. Expect the same Settings modal.
- **Dark.** Press Dark with `$S/app.sh press "$RUN" "Dark"`. Capture settings-dark. Close Settings, capture the page, open ⌘K and capture the dark palette. Compare the native title bar and menu to the unchanged macOS appearance.
- **Light and System.** Reopen Settings and press Light. Capture the light modal. Press System and expect the app to match the appearance read above. Capture settings-system. Read `$RUN/data/settings.json` and require theme System.
- **Persistence.** Choose Dark, close Settings and quit normally with ⌘Q. Require a successful quit flush in shell.log. Release the UI lock. Relaunch through `AELOMI_DATA_DIR="$RUN/data" $S/app.sh launch app`. Take the lock again, capture two immediate screenshots, then open Settings and require Dark selected. Read the same settings.json and require theme Dark.
- **Failure.** Close Settings and remove write permission from the owned synthetic data directory. Reopen Settings and choose Light. Close it and expect the save-failure banner. Restore permission and expect the banner to clear. Require theme Light in settings.json.
- **Mock.** After native cleanup, run `$S/mock-drive.mjs --flow "$S/flows/settings.mjs"`. The local Demo data / Worst case toggle switches the mock seed to unknown theme, 1,000 pages, long and unbreakable titles, empty titles, Unicode and emoji, and deep nesting. Expect every Appearance choice and Close Settings reachable at 320, 960 and 2560 pixels, plus doubled text at the minimum native window size. The flow checks horizontal overflow and captures both themes.

## Gotchas

- The sidebar assertion belongs to the sibling sidebar PR. This feature map covers the current page and overlays.
- An unknown stored theme displays System and remains untouched until an explicit theme pick.
- The native keyboard helper types text. Send Escape with System Events key code 53 against the owned PID.
- Settings closes the palette before opening. Closing Settings returns focus to the page target that opened the palette.
- Base UI hides background landmarks while Settings is open. Run doctor with Settings closed.
- On macOS 27.0.1, the native menu stayed Light while the title bar became gray with dark content. The exact light-title-bar visual assertion remains unproven. Keep native appearance under macOS control.
- Keep the real data folder listing from before the first launch and after the last launch. Report any difference.
