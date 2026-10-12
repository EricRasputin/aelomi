# SHELL-10 native evidence

These screenshots come from the real Tauri WKWebView, driven with `verify-aelomi` through macOS accessibility. Each run used its own `AELOMI_DATA_DIR` under `/tmp/aelomi-verify`; no normal app data was touched. The app source was unchanged from `6ef389f` during verification. Later commits changed tests, verification helpers, and docs.

## Save failure and recovery

Run: `/tmp/aelomi-verify/20261012-234458-dev-J4MK`.

1. Launch and require all doctor checks to pass. The initial screenshot shows no banner.
2. Open **Storage probe**. Before its first write, remove write permission from the run's `data/docs` folder with `chmod a-w`.
3. Press **Run probe**. The probe reads its own newly appended marker from memory, and the shell emits `Failing` with `PermissionDenied` and pending doc `probe`.
4. Press Command-Q. The shell's two-second flush returns `probe`; the window stays open and the quit dialog appears. `wait-dialog` and modal `doctor` both pass.
5. Capture both themes, choose **Keep app open**, restore write permission with `chmod u+w`, and wait for the `Healthy` event. The banner disappears.
6. Press Command-Q again. Both `ExitRequested` and final `Exit` flush with `pending=[]`.

The original dark appearance was restored after the light-theme captures. The test processes were cleaned up.

| State | Screenshot | Accessibility tree |
| --- | --- | --- |
| Healthy | [healthy-dark.png](healthy-dark.png) | [tree](healthy-dark.ax.txt) |
| Failed save, dark | [failure-dark.png](failure-dark.png) | [tree](failure-dark.ax.txt) |
| Failed save, light, after Keep app open | [failure-light.png](failure-light.png) | [tree](failure-light.ax.txt) |
| Quit dialog, dark | [dialog-dark.png](dialog-dark.png) | [tree](dialog-dark.ax.txt) |
| Quit dialog, light | [dialog-light.png](dialog-light.png) | [tree](dialog-light.ax.txt) |
| Recovered | [recovered-light.png](recovered-light.png) | [tree](recovered-light.ax.txt) |

Each image was inspected. The banner sits below the header and above the existing probe page. Both dialogs show the loss warning and a focus ring on **Keep app open**. The recovered view has neither a banner nor a dialog.

Evidence: [complete shell log](recovery-shell.log), [actions](recovery-actions.log), [doctor](recovery-doctor.txt).

## Window close, termination, and confirmed quit

Run: `/tmp/aelomi-verify/20261012-235412-dev-VRbH`.

Starting with fresh isolated data, make the first probe write fail as above. Command-W returns `quit flush via=CloseRequested pending=[DocId(probe)]` and shows the dialog. **Keep app open** closes it and leaves the banner. A quit AppleEvent sent to this exact PID returns `reply_error=-128`, logs `via=Terminate`, and shows the same dialog. Repeat with the logout reason to prove `NSTerminateCancel` without logging out the Mac.

Choose **Quit anyway** from that last dialog. The app process exits. The log has exactly one `quit confirmed`, then `quit allowed via=ExitRequested confirmed=true` and `quit flush via=Exit pending=[DocId(probe)]`. No `QuitBlocked` follows confirmation. The test `docs` folder has no entries after exit, as expected for the explicit discard choice. Restore its write permission and clean up the run.

| State | Screenshot | Accessibility tree |
| --- | --- | --- |
| Fresh launch | [run3-start.png](run3-start.png) | [tree](run3-start.ax.txt) |
| Probe before edit | [run3-probe-idle.png](run3-probe-idle.png) | [tree](run3-probe-idle.ax.txt) |
| Failed save | [run3-failing.png](run3-failing.png) | [tree](run3-failing.ax.txt) |
| Command-W dialog | [run3-close-dialog.png](run3-close-dialog.png) | [tree](run3-close-dialog.ax.txt) |
| Keep app open after close | [run3-close-kept.png](run3-close-kept.png) | [tree](run3-close-kept.ax.txt) |
| Quit AppleEvent dialog | [run3-terminate-dialog.png](run3-terminate-dialog.png) | [tree](run3-terminate-dialog.ax.txt) |
| Keep app open after termination | [run3-terminate-kept.png](run3-terminate-kept.png) | [tree](run3-terminate-kept.ax.txt) |
| Logout-reason dialog before Quit anyway | [run3-logout-dialog.png](run3-logout-dialog.png) | [tree](run3-logout-dialog.ax.txt) |

Each image was inspected. The dialog warnings and safe focus are visible; the kept states show the banner with no dialog.

Evidence: [complete shell log](quit-shell.log), [actions](quit-actions.log), [doctor](quit-doctor.txt), [AppleEvent replies and exit checks](termination-events.log).

The sender used only the supplied PID. Its source is preserved as a [text artifact](quit-event.swift.txt). To repeat the termination checks, copy it to a temporary `.swift` file, compile with `swiftc`, and run the resulting executable with your run's `APP_PID`, with or without the `logout` argument. This sends a quit event to Aelomi; it does not log out the computer. The Dock menu itself was not clicked. Its shared `applicationShouldTerminate:` path is covered by the AppleEvent proof and Rust tests.

## Reproduction limits

The scaffold has no editor. Native proof uses its existing dev-only **Run probe** button; acceptance test 5 appends `x` through the IPC mock. The mock suites prove UI behavior, while native logs prove the Rust command and event path.

Changing only the parent `docs` directory must happen before the first probe write. Once `docs/probe` exists, its existing files can remain writable.

macOS can hide the window from accessibility when another Space is active. The helper now brings the test process forward and waits within the same command when that happens. The initial screenshot attempts encountered this and were superseded by the successful runs above.

Do not regenerate browser test reports during a dev-window run. A report HTML write reloaded an intermediate run's frontend, invalidating its dialog evidence. The final proof uses a fresh instance with browser tests stopped. The Rust pending queue survives a frontend reload, but the frontend's transient dialog state does not.
