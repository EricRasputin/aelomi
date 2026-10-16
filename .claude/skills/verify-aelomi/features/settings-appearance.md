# Settings appearance

Settings opens inside the main window. Its Appearance control switches the app between System, Light and Dark immediately. The menu follows macOS. On macOS 27.0.1, release screenshots show the title bar following an explicit app theme.

## Sub-features

- `settings-modal` opens one modal by ⌘, or Aelomi > Settings… and closes it by Escape, ⌘W or Close Settings, with focus restored (SHELL-6, SHELL-7).
- `settings-palette` closes an open palette before Settings opens and blocks palette activation until Settings closes (SHELL-6).
- `settings-theme` applies the chosen appearance to the page, modal and palette without animation (SHELL-5).
- `settings-system` matches the current macOS appearance and follows its later changes (SHELL-5).
- `settings-durable-theme` reopens with the stored choice on the first content frame (DATA-4, SHELL-8).
- `settings-recovery-theme` loads the stored choice when a refusal becomes a ready workspace without a relaunch (SHELL-5, SHELL-11).
- `settings-failure` reports retryable failures with their reason, pauses attempts while the quit question is open, and retries immediately after Keep app open (SHELL-8, SHELL-10).
- `settings-disabled` dims Appearance and shows the editor's paused copy before registration and through a quit. Enabling focuses the selected radio (SHELL-8).
- `settings-native-close` closes About with ⌘W while keeping the main window and app running (SHELL-7).

## How to get to it (user POV)

- Press ⌘, or choose Aelomi > Settings… in the native menu.
- Select System, Light or Dark under Appearance. Arrow keys change the focused radio.
- Press Escape, press ⌘W or click Close Settings to return to the page.

## Driving it with app.sh and mock-drive

Preconditions: Acquire the keyboard lock with `$S/ui-lock.sh acquire <label>` before launching through `app.sh` on a fresh synthetic data folder, then pass doctor. Hold it before every native key, click, menu press, focus change, or transient screenshot. Run `$S/app.sh frontmost "$RUN"` before every key or click and send nothing if it fails. Release the lock with `$S/ui-lock.sh release <label>` on every path. Read macOS appearance with `defaults read -g AppleInterfaceStyle`; a missing key means Light. Keep that setting unchanged.

- **Shortcut.** Run `$S/app.sh keys "$RUN" , command`. Expect Settings, Appearance, System, Light, Dark and Close Settings in the tree. Capture settings-light. Send Escape by PID-targeted System Events key code 53 and expect Settings gone.
- **Menu.** AXPress Aelomi in the owned process's menu bar, then Settings… in its first menu. Expect the same Settings modal.
- **Close Window.** Open Settings, send `$S/app.sh keys "$RUN" w command`, and expect Settings gone with the page and app process still present. Type a marker immediately afterward to prove focus returned to the editor. Repeat through File > Close Window. Open Aelomi > About Aelomi, press ⌘W, and require the About panel gone, the same app PID alive and the main page intact.
- **Dark.** Click Dark with the mouse, move the pointer off the segment, and capture the selection without a focus ring. The selected fill must remain distinct from the track and hover. Use `$S/app.sh press "$RUN" "Dark"`. Capture settings-dark. Close Settings, capture the page, open ⌘K and capture the dark palette. Compare the native title bar and menu to the unchanged macOS appearance.
- **Light and System.** Reopen Settings and click Light with the mouse. Capture without a focus ring. Capture the light modal. Press System and expect the app to match the appearance read above. Capture settings-system. Read `$RUN/data/settings.json` and require theme System.
- **Bullets.** Type `- ` followed by a line in the page. Open Settings, pick Dark and Light, and capture the bullet page in each theme. The marker changes immediately with the editor. WebKit’s automated check requires `document.getAnimations().length === 0` after each pick.
- **Persistence.** Choose Dark, close Settings and quit normally with ⌘Q. Require a successful quit flush in shell.log. Release the UI lock. Take the lock again before relaunching through `AELOMI_DATA_DIR="$RUN/data" $S/app.sh launch app`. With the lock held, capture two immediate screenshots, then open Settings and require Dark selected. Read the same settings.json and require theme Dark.
- **Recovery.** Start on Finish moving with a synthetic destination that has not arrived. The app falls back to System. Put an existing synthetic workspace with stored Dark at that destination, then confirm Create empty workspace. Require the app to use Dark without a relaunch and Settings to select Dark. WebKit covers this transition in `tests/playwright/storage-refusal.spec.ts`.
- **Failure.** Close Settings and remove write permission from the owned synthetic data directory. Reopen Settings and choose Light. Close it and expect the save-failure banner. Restore permission and expect the banner to clear. Require theme Light in settings.json.
- **Disabled during quit.** Open Settings on the owned synthetic folder, remove its directory write permission, and pick another theme. Request ⌘Q with Settings open. Capture the dimmed Appearance group and paused copy during Draining, before the quit question appears. Restore permissions, choose Keep app open, and require the selected radio focused and the stored theme equal to the pick. Release the keyboard lock on every failure path.
- **Mock.** After native cleanup, run `$S/mock-drive.mjs --flow "$S/flows/settings.mjs"`. The local Demo data / Worst case toggle switches the mock seed to unknown theme, 1,000 pages, long and unbreakable titles, empty titles, Unicode and emoji, and deep nesting. Expect every Appearance choice and Close Settings reachable at 320, 960 and 2560 pixels, plus doubled text at the minimum native window size. The flow checks horizontal overflow and captures both themes.

## Gotchas

- The sidebar assertion belongs to the sibling sidebar PR. This feature map covers the current page and overlays.
- An unknown stored theme displays System and remains untouched until an explicit theme pick.
- The native keyboard helper types text. Send Escape with System Events key code 53 against the owned PID.
- Settings closes the palette before opening. Closing Settings returns focus to the page target that opened the palette.
- Base UI hides background landmarks while Settings is open. Run doctor with Settings closed.
- On macOS 27.0.1, the native menu stayed Light while the title bar became dark under Dark and white under Light. The release title bar follows the explicit app theme. This conflicts with SHELL-5 and remains a product decision for the orchestrator. No native appearance setter is used.
- Keep the real data folder listing from before the first launch and after the last launch. Report any difference.
