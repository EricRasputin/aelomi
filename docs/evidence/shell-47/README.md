# SHELL-10 quit decision recovery

This folder keeps only this README and its scripts, under the [evidence policy](../../native-verification-evidence.md). Every file it names is in the [complete record at 9b44d57](https://github.com/aelomi/aelomi/tree/9b44d575e43efa1170b743cc54b019b5faf7a8cf/docs/evidence/shell-47).

Issue #47 exposed a confirmation lost before frontend listener registration. The shell retained the pending edits but no queryable quit decision. The regression committed before the fix produced:

```text
FAIL src/stores/storage-events.test.ts > a quit refused before the listeners register still asks, and keeping open answers it
AssertionError: expected { kind: 'open' } to match object { kind: 'asking', …(2) }
```

The same test passes after the fix, including its Healthy startup and answered-choice reconnect assertions. Additional Vitest cases cover delayed snapshots, reordered events and replies, stale cancellation, and an answer that reaches Rust after a replacement page reads its snapshot. Rust tests cover retention with no listener, cancellation identity, publication to a late listener, sticky confirmation, and exact crash-and-reopen text after an append between confirmation and final Exit.

## Native reload proof

Opus 5.5 drove the real Tauri dev app through the macOS accessibility tree. Browser suites and build generators had finished before each launch. Every run used its own `AELOMI_DATA_DIR`, and no normal app data was touched.

Run A was `/tmp/aelomi-verify/20261013-100637-dev-JlZl`. The existing Storage probe queued an update while its test `docs` folder was read-only. Command-Q held the window and showed the confirmation. Three intentional `touch index.html` operations reloaded the webview without changing the file's bytes. Vite logged each full reload, and the probe's local status reset to `Storage probe idle`. No additional quit request occurred.

The [native event log](https://github.com/aelomi/aelomi/blob/b5e88db063ab87b0c250d37b1d34da4908f05655/docs/evidence/shell-47/reload-events.log) shows one failed quit flush before the reloads. The confirmation returned after the first reload. Restoring write permission produced Healthy and removed the banner, while the confirmation remained open. A second reload still recovered that outstanding choice. Keep app open published revision 2 with no pending decision. A third reload showed no dialog. A subsequent healthy Command-Q ran both the cancellable and final Exit flushes successfully.

Screenshots and accessibility trees remain in the local run folder. The coordinator inspected `a05-reload1-dialog.png`, `a07-reload2-healthy-dialog.png`, and `a09-reload3-open.png`. They show the restored dialog with safe focus, the restored dialog after recovery with no banner, and the answered state after reload.

## Native quit paths

Run B was `/tmp/aelomi-verify/20261013-101021-dev-5DdA`. Command-W held the window through `CloseRequested`. PID-targeted quit Apple events, including the logout reason, returned `reply_error=-128` and reached `via=Terminate`. A repeated request while the dialog was open flushed again and created a newer decision. One accessibility press failed, so run C repeated the logout and Keep app open sequence successfully.

Run C was `/tmp/aelomi-verify/20261013-101336-dev-0MkN`. Its [termination log](https://github.com/aelomi/aelomi/blob/b5e88db063ab87b0c250d37b1d34da4908f05655/docs/evidence/shell-47/termination-events.log) records cancelled logout-reason events, Keep app open, a later request, and one Quit anyway. Its [shell event log](https://github.com/aelomi/aelomi/blob/b5e88db063ab87b0c250d37b1d34da4908f05655/docs/evidence/shell-47/quit-events.log) records one confirmation, the confirmed `ExitRequested` bypass, and the unconditional final `Exit` flush with the failing update still pending. This is the explicit discard choice. The coordinator inspected `c02-logout-dialog.png` and `c03-logout-kept.png`, which show safe focus and the answered decision while the save-failure banner remains.

The sender was compiled from the existing `docs/evidence/shell-10/quit-event.swift.txt` into `/tmp`. It addressed only the supplied app PID. It did not log out the Mac. Dock Quit and a real logout were not performed; they share the tested AppKit termination hook.

## Limits and cleanup

The exact late-answer timing was tested deterministically in Vitest and Rust, rather than forced through the native UI. Native reloads prove snapshot recovery and answer delivery to a replacement page.

All three native runs exited and were cleaned with `app.sh cleanup`. Their data-folder write permissions were restored. The coordinator checked that the app PIDs and test ports were absent. Appearance stayed Dark, system permissions were unchanged, and `index.html` content was unchanged. No source behavior changed after native proof; the subsequent cleanup removed nine comments.

The project recipe in `.claude/skills/verify-aelomi/features/save-failure.md` contains the repeatable reload steps. The detailed local report is `/tmp/aelomi-47-native-proof.md`.
