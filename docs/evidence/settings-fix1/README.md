# Settings quit safety and modal fixes

This record proves source commit `29589c1b482e7f24a6ef03972fd740c44bdf9244` for PR #94.
Its merge base is `7a285d85bca3da5da66990f546471d703850d812`.
Its patch-id is `0dc015409aa9b4e79949b982f271d78f650f246f`.

The proven pathspecs are `src/stores/settings.ts`, `src/stores/theme.ts`, `src/ipc/pending-commands.ts`, `src/editor/quit-drain.ts`, `src-tauri/src/commands.rs`, `src-tauri/src/settings.rs`, `src/components/settings-dialog.tsx`, `src/components/storage-notices.tsx`, `src/shortcuts.ts`, and `src/use-shortcut-commands.ts`.

## Environment

macOS 27.0.1, Apple Silicon arm64. The Mac stayed Light. `defaults read -g AppleInterfaceStyle` returned a missing-key error. The release build used only a synthetic `AELOMI_DATA_DIR`. Read-only listings of the real Aelomi folder before and after the native run matched exactly.

## Model and checks

The shared settings job holds the latest wanted value per patch field and allows one write in flight. Omitted or null fields leave pending values intact. A failed attempt ends its tracked promise and enters the existing retained-command registry with a settings marker. The frontend drain retries the job immediately, then names settings in the quit question if delivery still fails. Native `Write` replies leave retry ownership with Store.

Run [verify.sh.txt](./verify.sh.txt) as the commands it contains. The source commit passed all 297 Vitest cases, all 12 Settings WebKit cases, and the three Settings acceptance cases. Startup-theme and frontend-only settings failure Rust cases passed. A full shell test run then detected a generated documentation mismatch, which `pnpm bindings` corrected before this source commit. The final full local gate is reported in the PR body.

The exact regression outcomes are:

- SHELL-8 registration. `Appearance waits for quit registration before accepting a pick` attempts a disabled Dark pick, checks zero updates and stored System, releases registration, then checks stored Dark.
- SHELL-8 and SHELL-10 retention. `latest field values survive an outage with one write in flight` checks both calls and the complete stored snapshot. Dark with page-a becomes Light with page-c and recent page-b. A null recent-pages field preserves the pending page-b value.
- SHELL-9 drain retry. `drain retries a retained settings failure %j without waiting for backoff` covers Io, Internal and Quitting, checks exactly two Dark calls and the complete stored Dark snapshot.
- SHELL-9 and SHELL-10 quit question. `an Io settings failure asks promptly with settings and retries through the next drain` advances the mocked clock by 1 ms, checks `quitDrainFailed(1, [], true)`, the visible settings warning, exactly three Dark calls over refusal and recovery, and the complete stored Dark snapshot.
- Native quit reporting. `shell_10_frontend_only_settings_failure_asks_without_native_pending_edits` checks the exact Asking status with `pending_settings: true` while Store has no pending edits.
- SHELL-5 motion. `theme switches leave bullet markers without animations` checks `document.getAnimations().length === 0` after Dark and Light picks on a bullet page.
- SHELL-6 and SHELL-7 modal close. `Close Window dismisses Settings and the palette before closing the main window` checks keyboard and menu dismissal, no native close calls while a modal is open, then exactly one close call for the main window.
- SHELL-5 selection. `a mouse-selected Appearance option has contrast without a focus ring in both themes` checks the exact fills, label weight, absent outline, root class, color scheme, light and dark surfaces, and at least 3:1 selected-fill contrast.
- SHELL-5 startup. `shell_5_initial_theme_uses_system_for_refused_storage`, `shell_5_initial_theme_uses_system_on_read_error_and_retries_later`, `shell_5_initial_theme_uses_system_for_missing_and_malformed_settings`, and `shell_5_initial_theme_preserves_unknown_and_serializes_the_script` check exact themes and initialization scripts, including quotes, a newline and U+2028.

The WebKit cases live in `tests/playwright/settings.spec.ts`. The pure write-job cases live in `src/stores/settings.test.ts`. Native quit status is tested in `src-tauri/src/tests.rs`, and startup themes in `src-tauri/src/settings/tests.rs`. The acceptance cases in `e2e/settings.e2e.ts` are `SHELL-5 acceptance 1: Theme applies Dark to the editor, Settings and command palette`, `SHELL-6 acceptance 3: Settings shortcut opens the modal and Escape closes it`, and `SHELL-5 acceptance 6: System theme follows changes in the mock macOS appearance`. The existing `data_4_session_state_round_trips_through_the_owner_on_a_real_folder` covers durable Dark after reopen.

## Native proof

Release run `~/.local/state/aelomi-verify/20261016-135627-app-Cz0e` passed doctor. The builder opened Settings through ⌘, and Aelomi > Settings…, clicked Dark and Light with the mouse, and captured each with no focus ring. ⌘W and File > Close Window dismissed Settings while the owned PID and editor remained present. Escape also closed it. The selected System matched the unchanged Light Mac, and the settings file contained exactly theme System before the later Dark pick.

The builder typed `- ` followed by `A native bullet`, switched the page between Light and Dark, and opened the Dark palette. The screenshots show the expected marker and UI colors. The zero-animation assertion is a WebKit check, since native accessibility snapshots cannot measure a 200 ms transition.

A normal ⌘Q ended the native run. These are the relevant shell lines:

```text
quit draining via=ExitRequested revision=1
quit flush via=CompleteQuit pending=None
quit confirmed via=drain
quit allowed via=ExitRequested confirmed=true
quit flush via=Exit pending=None
```

The layout stress run `~/.local/state/aelomi-verify/20261016-082546-mock-4btSGg` passed the existing settings flow at 320, 960 and 2560 pixels, plus doubled text. It seeded an unknown theme, 1,000 pages, long and unbreakable titles, Unicode, emoji and deep nesting. All nine screenshots were inspected. The controls remained reachable without horizontal overflow.

## Screenshots

[The PR evidence comment](https://github.com/aelomi/aelomi/pull/94#issuecomment-6211456013) holds the screenshots. Each published image was downloaded without authentication and matched the local PNG byte for byte.

| Screenshot | Bytes | SHA-256 |
|---|---:|---|
| [settings-dark-mouse.png](https://github.com/user-attachments/assets/aeb421b1-5f57-44b0-8336-e06d82c5fc39) | 140078 | `3e1f8281452acf15bfd2aac7e597827b1e1f1620c4da56f06f54408a55a8b5df` |
| [settings-light-mouse.png](https://github.com/user-attachments/assets/b0a97d08-23bf-4ba4-9f80-6b8c5c4f228c) | 159109 | `69c205df41bd2cf56d06abccda55d832ab280ff99cff55070038bd0de6db9ee0` |
| [after-command-w.png](https://github.com/user-attachments/assets/7bc4bfd7-1608-4c4e-805e-097cc9202f86) | 95192 | `3c3b9159fbfe4df39bfc54c7116cb66a08df066a3947aa861673c847e7283890` |
| [bullet-light.png](https://github.com/user-attachments/assets/26e7c821-419a-4e43-b694-e3b6f919e22e) | 105747 | `9ff46cf3f5c7659c238e892dc196a9cf59f6bf83e71502453eebbe3aa0bf6f19` |
| [bullet-dark.png](https://github.com/user-attachments/assets/2b1dd5de-2212-4ea3-987f-3c3c8f9d97cc) | 99834 | `8f7934305fd32b9fe20faf474370b24b3724e1a706ff5d264387699e0691a22f` |
| [palette-dark.png](https://github.com/user-attachments/assets/a4145c9f-55a9-4566-925c-b0c0ce5f7291) | 117645 | `0e06eb229883895b07e193e13fcc52932a331beed4a61ad8c1d0b88344a0344c` |

## Remaining scope

On this Mac, the Dark screenshot has a dark title bar and the Light screenshot has a white title bar. This conflicts with SHELL-5. No native appearance setter was added, and the inline `colorScheme` probe was not attempted in this round. The orchestrator owns the product decision.

PR #89 was still open at this record. Its expanded-rows writer moves to `updateSettings` when that branch rebases. The sidebar theme assertion also belongs to that integration.
