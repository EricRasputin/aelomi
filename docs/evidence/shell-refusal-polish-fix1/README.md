# Refusal recovery, fix round 1

Source commit `5fb8fb008f712ac72117b71d6a67ca28d6ccf6bb`. Merge base against `origin/main` is `6d64d683d5bf02bea2f67e8038428caef1ab4d62`. Verbatim patch-id is `954478497f2131627cd3edd8dd64165b3046115f`.

Proven pathspecs are `src/storage-copy.ts`, `src/router.tsx`, `src-tauri/src/commands.rs`, `src-tauri/src/lib.rs`, `src/ipc/mock.ts`, `e2e/cloud-folder.e2e.ts`, `src/storage-copy.test.ts`, and `tests/playwright/storage-refusal.spec.ts`.

## Shape and scope

Model the Domain retains Rust's three `CloudRecovery` variants and the ordered `RecoveryStep` text-or-folder union.
The leaf check is a folder step on `CloudPath.configured`, so it checks the path Aelomi opens rather than a Finder window.
Quit registration uses the existing opened-workspace state. No new state flag or quit phase is added.

The refusal branch no longer mounts `QuitConfirmation`. It cannot hold edits because no workspace opened. Ready workspaces still ask after a drain timeout even when Store's pending list is empty, since the webview may hold undelivered edits. This is smaller than adding refusal-specific dialog copy or weakening the ready-workspace timeout.

Copy now asks for icon view, puts copying before opening Go to Folder, and says to move the new window aside. The conditional rename is unchanged. The check prompt and failure remedy match the round-1 brief exactly. The feature map again permits equivalent Finder actions.

## Automated proof

- `src/storage-copy.test.ts`, `maps safe recovery steps for $name`, passed all equal-name, different-name, debug-name, shared-link and override mappings. The focused file passed 11 tests.
- `e2e/cloud-folder.e2e.ts`, `SHELL-11 acceptance 7: a cloud folder shows its resolved path and how to move it back`, checks the configured-path chip, the exact failure remedy and SHELL-7 isolation. The ready-phase cancelable ⌘K returns `false`; the refused phase returns `true`. Real `Meta+k` and the mock's native menu activation leave no palette, dialog, editor or document loads on refusal.
- The required mutation moved `useShortcutCommands()` from `WorkspaceLayout` into `RootLayout`. The test failed at the refusal cancellation assertion with `ASSERTION_FAILED: expected false to be true`. The change was reverted. `.local-gate/e2-fix1/hoisted-hook.log` retains the failure.
- The separate `SHELL-9/SHELL-10: a ready workspace asks on drain timeout but a refused folder quits immediately` acceptance case checks ready timeout choices, exactly one Quit anyway call, and refused quit with no completion call. Both it and Rust's new refusal test failed before the registration fix and passed afterward. All four focused acceptance cases passed.
- Native Rust `frontend_ready_keeps_refused_storage_on_store_only_quit` calls the real readiness command twice and checks Store-only quit for ExitRequested, CloseRequested and Terminate, no quit events, and no created cloud data folder. The existing `a_registered_webview_drains_every_route_before_the_app_exits` now registers through that command and checks the ready path, exact events, one exit and exact content after a simulated crash. All 66 shell tests passed, with one existing ignored test.
- `tests/playwright/storage-refusal.spec.ts` passed its two light/dark leaf layout cases at 320 px with 100% and 200% text. They check path wrapping, selection and the configured-path check. Redundant shared-link and override string-only cases were removed; their acceptance and exact mapping tests remain.

The proof-only stress toggle uses the same mock setup boundary and never enters production. The run `~/.local/state/aelomi-verify/20261016-012425-mock-vWAjVz` passed all four fixtures in both themes at widths 320, 960 and 1440 with 100% and 200% text. Long unbreakable names, deep paths, Japanese text and emoji produced no horizontal overflow or clipped path field. The network guard saw no remote requests and the preview port was free after cleanup. Empty names and arbitrary step counts cannot come from native recovery input.

No Comments was applied directly because the builder brief forbids nested agents. No comment deletion or suppression was needed. The restored service-worker reason comment is about external `@e2e-dev/web` behavior, checked in its installed `dist/surface.js`. Public command contract docs were regenerated through `pnpm bindings`.

## Native proof

PASS on macOS 27.0.1 build 26A434, Apple Silicon. The synthetic base was `/private/tmp/ae90-r1-nFJn1Y`. Every app launch used `app.sh launch dev` with explicit synthetic `AELOMI_DATA_DIR` and `AELOMI_CLOUD_ROOTS`.

- leaf-ui run is `~/.local/state/aelomi-verify/20261016-070316-dev-nt4J`.
- leaf run is `~/.local/state/aelomi-verify/20261016-070538-dev-pR7r`.
- recovered run is `~/.local/state/aelomi-verify/20261016-070625-dev-89vZ`.
- shared run is `~/.local/state/aelomi-verify/20261016-070655-dev-XNNq`.
- override run is `~/.local/state/aelomi-verify/20261016-070742-dev-MQ7k`.

The leaf UI run captured both themes. Its native AX-focus-and-copy attempt did not copy the field, so it stopped before changing any folder. The successful recovery run read the three displayed path chips from the accessibility tree, asserted each exact value, and put that value on the clipboard, as pr86-fix4 did. No native one-click-copy claim is made.

The recovery started with one owned Finder window. One existing window was preserved. The destination held Archive, Design, OtherApp and Research alongside the equal-name Aelomi link. Finder's ⌘⌫ trashed only that link. View > as Icons left the destination window open. File > New Finder Window opened a second owned window on the fake cloud parent. ⌘C followed by Move Item Here with ⌘⌥V in the step-1 window moved Aelomi into the destination. Equal names produced no rename instruction. Go to Folder on the check chip opened `/private/tmp/ae90-r1-nFJn1Y/Recovery Destination/Aelomi` and showed the original docs and format marker.

The before/after manifest comparison checked every original workspace file's SHA-256 before reopening the app. Every byte matched. Only Finder's `.DS_Store` metadata was permitted as an addition. Both owned Finder windows were closed through their accessibility close buttons.

The successful proof printed these exact lines in `.local-gate/e2-fix1/native.log`:

```text
PASS: only the synthetic link was trashed; the cloud workspace remains
PASS: Finder Move Item Here moved the equal-name folder into the step-1 window
PASS: configured-folder check opened the data folder; every original workspace file is byte-identical
PASS: refused app quit without a frontend drain or dialog
PASS: relaunched app shows exact original page text: Original pages survived the equal-name Finder recovery, round 1.
PASS: all owned Finder windows closed; all app processes cleaned; appearance and clipboard restored
```

Refused leaf, shared-link and override runs each logged `quit flush via=ExitRequested pending=None`, with no frontend-ready, draining or CompleteQuit line. Their ⌘Q path exited without a dialog. The ready recovery run logged:

```text
storage status=Ready path=/private/tmp/ae90-r1-nFJn1Y/Recovery Destination/Aelomi
quit frontend ready
quit draining via=ExitRequested revision=1
quit flush via=CompleteQuit pending=None
quit confirmed via=drain
quit allowed via=ExitRequested confirmed=true
quit flush via=Exit pending=None
```

The recovered page's accessibility text was asserted against the complete original marker. The screenshot shows the same page title and its single original line. Both themes of all three refusal variants were inspected and show the complete instructions and paths. The real-data folder was listed read-only before launch and after cleanup; the listings were identical.

The prior record documents failed drag automation on this harness. This round used the explicitly authorized Finder keyboard equivalents. Two initial app-level Finder scripting attempts also stalled at an automation permission prompt for Dia and stopped before folder movement. System Events keyboard commands avoided that path. Read-only Finder diagnostic probes during this debugging ran outside this task's owned lock, a deviation from the strict Finder-step rule. Every key, click, screenshot and folder operation in the successful proof ran under the owned lock. No permission prompt was approved. All owned helper processes were stopped.

The deferred shared-link and override recovery redesigns remain outside this PR. The exact-path check and fallback are proven; real drag ergonomics and native automatic path selection are not claimed.


## Reproduce

Copy these text scripts into `.local-gate/e2-fix1/`, removing only the final `.txt` suffix. Keep `seed.rs.txt` as text. From the repository root, run `bash .local-gate/e2-fix1/prepare-proof.sh`, then `bash .local-gate/e2-fix1/native-proof.sh`. Do not run browser suites during the dev proof. Both use only a fresh synthetic data folder and fake cloud root.

`native-proof.sh` takes `/tmp/aelomi-ui.lock` before keys, clicks, Finder operations and screenshots. It releases the owned lock between launches and on failures. It closes only the Finder window IDs it created, restores appearance and clipboard, and runs `app.sh cleanup` for each owned launch. The read-only real-data-folder listing is compared before the first launch and after the last.

Run the stress flow with `.claude/skills/verify-aelomi/scripts/mock-drive.mjs --flow .local-gate/e2-fix1/stress.mjs`. Run `hoisted-hook-proof.sh` only with no native dev run active, then rebuild the mock before positive checks. The mutation script restores the source on exit.

## Published screenshots

[Native fix-round proof](https://github.com/aelomi/aelomi/pull/90#issuecomment-6206056209) contains the nine screenshots below. Each original was inspected. The cached GitHub CLI 2.102.0 executable matched its checksum-verified archive. Every attachment was downloaded without credentials, opened as a PNG through `sips`, and matched the original byte count and SHA-256. Other Finder shots stay local because their sidebars include the home-folder name.

| Screenshot | Bytes | SHA-256 |
| --- | ---: | --- |
| [leaf-dark.png](https://github.com/user-attachments/assets/067610b3-ab6d-42f0-90c8-0f62b662f6f2) | 302214 | `6cb7e3a2b13d7c7ef80632763c38231db7ca06585a9d4eea4e4c9bb5eadae796` |
| [leaf-light.png](https://github.com/user-attachments/assets/caaf481e-b8cb-42db-a9b1-4afa88f95272) | 321513 | `90e5d9e671d03a83373717fb54c23e42d0d8adb44519441ab5aee3e1dbf4434b` |
| [shared-dark.png](https://github.com/user-attachments/assets/899bbffc-c487-4bfd-83e3-40d3a1515a9f) | 196501 | `f35b08171715676fbca6d373ab755cd051a96cc029f540a627bf5e0266620d3b` |
| [shared-light.png](https://github.com/user-attachments/assets/2a0624cc-8c9a-4c0b-b4d7-06f92c8906ad) | 207222 | `2d4008a8ff2d5d8a5ded19bdd5589dd8436af8166f3a7863428c8a15d92640ca` |
| [override-dark.png](https://github.com/user-attachments/assets/ffd33f1a-02de-404f-900c-a141047fa5f5) | 158061 | `cc1d79a4f860f51ff980355db5c442d02400c951c129294898ffe30dec7338f8` |
| [override-light.png](https://github.com/user-attachments/assets/c6f9bd1a-8cb7-4bb6-868b-1c38ed4b8b4d) | 165297 | `dd7c761c935dc79a80585f6e50fb0e9f67be79cf7efc7757d878db3eab48395a` |
| [link-trashed-icons.png](https://github.com/user-attachments/assets/90727aaf-69a1-4d99-82b9-abb212c5542b) | 565142 | `c7a0b4b92f2231aa37d5ac585e1f0a184eb58889dad37bc8f102ba18d880d129` |
| [configured-folder-check.png](https://github.com/user-attachments/assets/b684b882-b0db-4f42-915a-85e725413a5c) | 546082 | `03404aba1033a505c1eebd10b205e5f6189dd3e64aa442f8da66295c97589394` |
| [original-page-recovered.png](https://github.com/user-attachments/assets/fdc77d11-1501-4252-84e6-c79f1034a1b2) | 89698 | `22e50e20460c1a727a2dc15ffac1394e855605700f0557618b08ace9cf59ed69` |


The final committed head is checked with `pnpm check:all`. Its `.local-gate/<head>.checks.json` record and result are cited in PR #90's Verification section. No performance improvement is claimed.
