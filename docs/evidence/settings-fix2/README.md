# Settings fix round 2

The lock path in the historical scripts is an old orchestrator-private path. Use `.claude/skills/verify-aelomi/scripts/ui-lock.sh` instead.

This record proves source commit `d422ffce9a3480f2c78d45718276bad24c1aa8e7` for [PR #94](https://github.com/aelomi/aelomi/pull/94).
The merge base is `7a285d85bca3da5da66990f546471d703850d812`.
The patch-id is `2115d02a1da4d14b87d4fd879fab66692fe2c024`.

The proven pathspecs are `src-tauri/src/macos.rs`, `src/stores/settings.ts`, `src/ipc/command-retry.ts`, `src/ipc/pending-commands.ts`, `src/ipc/record-commands.ts`, `src/components/settings-dialog.tsx`, `src/components/storage-notices.tsx`, `src/use-shortcut-commands.ts`, `src/command-dispatcher.ts`, `src/router.tsx`, and `src/styles.css`.

## Environment and model

macOS 27.0.1, Apple Silicon arm64. The Mac stayed Light. `defaults read -g AppleInterfaceStyle` reported a missing key. Every launch used `app.sh` with a fresh synthetic `AELOMI_DATA_DIR`. Read-only listings of the real Aelomi folder before the first launch and after the final native quit matched exactly. Keyboard locks were acquired and released only through the orchestrator's `ui-lock.sh`.

The settings job holds a latest-value-per-field `SettingsPatch` and one in-flight write. A reported failure is either retrying or terminal. The shared `retryCommandError` classifies I/O, uncertain transport results and quit refusals of already-retained work as retryable. Record operations keep their FIFO and durability confirmation. Settings keep field merging and transfer native Write retry ownership to Store. A generic runner would need to preserve those different lifetimes, so only classification and the transport error type are shared.

Terminal failures leave the quit tracker. InvalidPageId removes the invalid page-ID fields, retains valid fields and lets newer pending fields win. Other terminal refusals drop the attempted patch while leaving newer pending work intact. Closed admission pauses settings timer attempts and preserves the earlier reason. Keep app open triggers an immediate attempt.

## Reproduction and checks

The requested `pnpm test:playwright tests/playwright/save-failure.spec.ts` invocation hit Playwright's greedy `--project webkit` argument parsing. The equivalent `pnpm exec playwright test tests/playwright/save-failure.spec.ts --project=webkit` reproduced the hosted colour failure locally in the dark case. The test read colours before the media-query listener applied the new theme. The mock does not stub matchMedia in this case; its appearance fallback reads the real query. Waiting for `data-theme` on initial load and both directions of emulateMedia removed the race. Three repeated runs passed all 18 cases.

The native parent reproduction used `~/.local/state/aelomi-verify/20261016-145147-dev-rOH4`. About opened from the app menu, then Command-W exited the app. Its log included:

```text
quit draining via=CloseRequested revision=1
quit flush via=CompleteQuit pending=None
quit confirmed via=drain
quit flush via=Exit pending=None
```

Regression tests were run red before the retry and disabled-state fixes. The final focused runs passed all 15 Settings WebKit cases and 31 settings/record Vitest cases. The record test files were unchanged. The extracted settings mock passed its existing round-trip and health tests. `mock.ts` is now 980 lines. The final complete local gate is cited in the PR body.

The changed or added cases are listed below. Each retains exact stored values or exact call lists where saving is involved.

| Behavior | File | Case |
|---|---|---|
| SHELL-5, SHELL-10 | `tests/playwright/save-failure.spec.ts` | `SHELL-10 light warnings wrap and preserve the quit decision after recovery`; dark variant. Both now wait for applied themes and check the original background on return. |
| SHELL-8 | `src/stores/settings.test.ts` | `SHELL-8 a terminal settings rejection %s settles without retry`, for InvalidPageId, Unavailable, Internal and first-attempt Quitting. |
| SHELL-8 | `src/stores/settings.test.ts` | `SHELL-8 InvalidPageId drops the rejected fields and saves the pending theme`. |
| SHELL-8 | `src/stores/settings.test.ts` | `SHELL-8 a merged invalid field cannot discard valid settings after a retained outage`. |
| SHELL-9 | `src/stores/settings.test.ts` | `SHELL-9 drain retries a retained settings failure %j without waiting for backoff`, for Io. |
| SHELL-10 | `src/stores/settings.test.ts` | `SHELL-10 closed admission preserves the Io reason and reopening retries immediately`. |
| SHELL-10 | `src/stores/settings.test.ts` | `SHELL-10 a retained transport failure survives a quit refusal until storage accepts it`. |
| SHELL-8 | `tests/playwright/settings.spec.ts` | `SHELL-8 Appearance waits for quit registration before accepting a pick`, extended with paused copy, disabled opacity, inert hover/press and selected-radio focus after enabling. |
| SHELL-6, SHELL-7 | `tests/playwright/settings.spec.ts` | `SHELL-6 Close Window dismisses Settings and the palette before closing the main window`, extended with editor-focus assertions after keyboard and menu dismissal. |
| SHELL-9, SHELL-10 | `tests/playwright/settings.spec.ts` | `SHELL-9 SHELL-10 an Io settings failure asks promptly with settings and retries through the next drain`, now including the immediate Keep app open attempt. |
| SHELL-10 | `tests/playwright/settings.spec.ts` | `SHELL-10 Keep app open preserves the settings error while asking and retries at once`. |
| SHELL-8 | `tests/playwright/settings.spec.ts` | `SHELL-8 terminal settings errors are dismissible and later theme picks save`. |
| SHELL-7 | `tests/playwright/settings.spec.ts` | `SHELL-7 refused storage keeps Close Window registered without workspace commands`. |

The original acceptance cases remain in `e2e/settings.e2e.ts`: `SHELL-5 acceptance 1: Theme applies Dark to the editor, Settings and command palette`, `SHELL-6 acceptance 3: Settings shortcut opens the modal and Escape closes it`, and `SHELL-5 acceptance 6: System theme follows changes in the mock macOS appearance`. DATA-4 persistence remains covered by `data_4_session_state_round_trips_through_the_owner_on_a_real_folder` in `crates/core/src/storage/settings_tests.rs`.

## Native proof

Release run `~/.local/state/aelomi-verify/20261016-145958-app-sFx9` passed doctor against the source commit above. The About panel closed on Command-W while PID 72439 stayed alive and its main page remained intact. No quit drain appeared in the log for that action. The About screenshot used a native window capture because the panel has no web content for the main-window tree helper.

Command-W dismissed Settings. Typing immediately afterward inserted `D2-key-focus` into Page body. File > Close Window also dismissed Settings; after observing dismissal, typing inserted `MENU-FOCUS-OK` into the same editor. Neither route closed the app. Dark and Light were selected with actual pointer events aimed at the radio's accessibility bounds. Their screenshots show distinct selected fills without focus rings. System matched the unchanged Light Mac, and settings.json contained theme System at that point.

Write permission was removed only from this run's synthetic data directory. Picking Dark produced the permission-denied save banner. Command-Q with Settings open disabled Appearance and replaced its hint with the editor's paused copy. `settings-disabled-draining.png` was captured before the quit question. The log immediately after capture ended with the Draining line. The following flush confirmed pending settings:

```text
quit draining via=ExitRequested revision=1
quit flush via=CompleteQuit pending=Some(Pending { docs: [], settings: true })
```

After permission was restored, Keep app open re-enabled Appearance. An accessibility read reported Dark as the focused radio with value 1. The stored settings snapshot was exactly theme Dark, null openPage, and empty recentPages and expandedRows. A later normal quit completed:

```text
quit draining via=ExitRequested revision=4
quit flush via=CompleteQuit pending=None
quit confirmed via=drain
quit allowed via=ExitRequested confirmed=true
quit flush via=Exit pending=None
```

The worst-case layout run `~/.local/state/aelomi-verify/20261016-092659-mock-s6NEo5` passed the existing settings flow. It exercised unknown themes, 1,000 pages, long and unbreakable titles, empty titles, Unicode, emoji and deep nesting at 320, 960 and 2560 pixels, plus doubled text. All nine screenshots were inspected. The Appearance choices and close control remained reachable, without horizontal overflow.

[verify.sh.txt](./verify.sh.txt) is a replay recipe assembled from the native commands above. Run from this worktree's repository root with a fresh release bundle. [native.js.txt](./native.js.txt) contains the PID-targeted menu, focus-read and pointer helpers. The recipe is kept as text; the recorded proof used its individual commands.

## Published screenshots

[The screenshot comment](https://github.com/aelomi/aelomi/pull/94#issuecomment-6212658133) contains the eight PNGs below. Every public URL was downloaded without credentials. All eight files opened as PNGs and matched the local image byte for byte.

| Screenshot | Bytes | SHA-256 |
|---|---:|---|
| [about-before-close.png](https://github.com/user-attachments/assets/dd4b2a10-5cec-4e2b-8797-07a6c1c7efb1) | 39157 | `9c5534847f632eeec04ab692ee0e1933a282cab006b5e6518c2cbe31e96a26c5` |
| [about-after-close.png](https://github.com/user-attachments/assets/e5088a7b-6fda-40cb-b26a-1358cab25a80) | 101138 | `179a0d3465acfc9e5fb8310d2f477aaa30499e8c9c3de701fbf4fb4c97834b98` |
| [settings-dark-mouse.png](https://github.com/user-attachments/assets/2467ceab-f327-4de7-8ce9-a87e871e0e15) | 152252 | `7ce8e4a5f157cfe4c09a2b625f01087538f18d67ffdef4ee9f0f6e9a1fcb999e` |
| [settings-light-mouse.png](https://github.com/user-attachments/assets/e01cabac-1471-4814-b411-ee687d1f696b) | 168778 | `28d51627849c91266f88fd218917a170b93c9c2c99b19ef52733066b85256271` |
| [settings-after-command-w.png](https://github.com/user-attachments/assets/67ef9d83-f6da-4cc4-9493-7bdf6fb906d6) | 104190 | `3fa1cf245c8e9c7d16667d1394e5177a758999f8b2862d4ea7769ffce70a2d74` |
| [settings-after-menu-close.png](https://github.com/user-attachments/assets/6e21ff27-cf5d-4ae7-9155-16661b3bcd4f) | 113410 | `c414554298d1ef5d8dda10c37f38a1709ac855bc27ebbc32c924d19ab4f06023` |
| [settings-disabled-draining.png](https://github.com/user-attachments/assets/0a441ec3-ea09-4b3c-9957-334eb18cabcc) | 160462 | `bf7958a7b3e4115986c0c0b07fbe8833e0be95eb58f4af84690d069c2b0e11bd` |
| [settings-after-keep-open.png](https://github.com/user-attachments/assets/1281fa40-705c-4a8d-b55d-3a009a9ee6e0) | 152731 | `dd043add10d01708bfb6a957afea72b43ad6ab7f64848e65441d66e523052db6` |

## Remaining scope

The title bar still follows an explicit app theme on this Mac. That SHELL-5 conflict remains the orchestrator's product decision. No native appearance setter or system-setting change was made, and the inline colorScheme probe was not attempted. The review's Noted design and typing items remain deferred to #87.

PR #89 was still open and main remained at the recorded merge base. Its expanded-rows writer and the sidebar theme assertion move onto this work when that branch rebases. No SPEC text was changed. The builder ran no review gate or independent verifier and used no subagents.
