# Refusal recovery, fix round 2

`attempted-finder-proof.sh.txt` and `capture-ui-only.sh.txt` change the Mac's appearance and must not be rerun as is. Follow the current [verify-aelomi rules](../../../.claude/skills/verify-aelomi/SKILL.md#rules) instead.

Source commit `d79deecba3840d29098c4fd821bf7e66fc31d6f2`. Merge base `6d64d683d5bf02bea2f67e8038428caef1ab4d62` against `origin/main`. Verbatim patch-id `c11d6ca7151c0972e9969634f813c59469e368e9`.

Proven pathspecs are `src/storage-copy.ts`, `src/storage-copy.test.ts`, `e2e/cloud-folder.e2e.ts`, `tests/playwright/storage-refusal.spec.ts`, `src-tauri/src/commands.rs`, `src-tauri/src/storage.rs`, `src-tauri/src/lib.rs`, `src-tauri/src/tests.rs`, `src-tauri/src/macos.rs`, `src/ipc/mock.ts`, and `src/stores/quit.ts`.

## Shape and changes

Rust's three `CloudRecovery` variants still select ordered `RecoveryStep` text or folder values. The configured-path check stays a folder step. Its copy now says to copy that path, names the expected folder and files, and distinguishes a rename mistake, a misplaced move and a move that never happened. The misplaced-move remedy names Undo Move of the resolved folder and accounts for a prior rename. The conditional rename remains conditional.

Step 2 explains cloud removal warnings. Shared-link copy identifies a link into cloud storage, explains the local-folder requirement, and warns that turning off sync can remove local files, including other apps' files. It gives no move or sync-setting instruction and keeps restart last.

The shell tests moved unchanged into `src-tauri/src/tests.rs`, apart from rustfmt and a local `DocId` import. The module and shared helpers retain `pub(crate)` access. `lib.rs` is 258 lines. `Storage::register_frontend` owns the opened-workspace check and logs refused registration. The mock registration is sticky, matching Rust. The frontend's registration comment now describes that contract.

The superseded `shell-refusal-polish` record is deleted. The fix1 README now claims only its historical equal-name success path. Its negative checks use explicit failure branches and portable extended grep alternation.

## Automated proof

- `src/storage-copy.test.ts`, `maps safe recovery steps for $name`, passed all five exact mappings. The file passed 11 tests, including equal names, differing names, a debug name, shared links and overrides.
- `e2e/cloud-folder.e2e.ts` passed all four cases. Acceptance 7 checks the new leaf remedy and removal warning. Its SharedLink case checks the link chip, unchanged-data statement, local-folder requirement and sync-removal warning. Every variant checks the keep-open and separate restart steps.
- Acceptance 7's ready ⌘K control cancels the event. Its refused control leaves the event uncanceled. Real Meta+k and the mock menu leave no palette, dialog, editor or document loads on refusal. The required hoisted-hook mutation failed with `ASSERTION_FAILED: expected false to be true`, then the source was restored. The subsequent positive acceptance run passed.
- The separate `SHELL-9/SHELL-10: a ready workspace asks on drain timeout but a refused folder quits immediately` case passed. It checks both timeout choices, one Quit anyway confirmation and no completion call on refused quit.
- `cargo nextest run -p aelomi --lib` passed 66 tests, with one existing skipped test. This includes `tests::frontend_ready_keeps_refused_storage_on_store_only_quit`, `tests::a_registered_webview_drains_every_route_before_the_app_exits` and `macos::tests::terminating_waits_for_a_registered_webview_without_blocking`. Comparing the extracted module with the old module after rustfmt showed no other changes.
- `tests/playwright/storage-refusal.spec.ts` passed its two light/dark cases at 320 px with normal and doubled text. The configured-path chip uses the new copy-first label, and the exact failure remedy remains readable.
- The stress run `~/.local/state/aelomi-verify/20261016-025023-mock-EjDLrO` passed all four fixtures in both themes at 320, 960 and 1440 px with normal and doubled text. Long unbreakable names, differing names, deep paths, Japanese text and emoji produced no horizontal overflow or clipped path fields. All eight saved screenshots were inspected. The mock saw no non-local requests and released its port. The fixture toggle stays outside production.
- Injecting forbidden text into each historical negative check returned exit 1. Clean text returned exit 0. The check also caught BSD grep's unsupported basic alternation, which is now explicit `grep -E`.

One early browser result was discarded because the mutation rebuild overlapped it. The sequential positive run passed. No Comments was applied directly because the brief forbids nested agents. No comment or suppression deletion was needed. The service-worker reason comment describes installed `@e2e-dev/web` behavior at `dist/surface.js:264`. Public command and state-contract comments remain. No motion or dependency was added.

## Native result and incomplete proof

INCOMPLETE. The user explicitly stopped further Finder interaction after the driver could no longer create a window. Finder was not restarted. The environment was macOS 27.0.1 build 26A434 on Apple Silicon. Every Aelomi launch used `app.sh launch dev` with an explicit synthetic `AELOMI_DATA_DIR` and fake `AELOMI_CLOUD_ROOTS`.

The furthest differing-name run was `~/.local/state/aelomi-verify/20261016-082657-dev-YLj5`, on `/private/tmp/ae90-r2-4BrBtP`. A later attempt repeated the misplaced-folder check at `~/.local/state/aelomi-verify/20261016-082852-dev-TO6m`.

The recipe started from one owned Finder window and preserved the existing `local` window. The destination also held Archive, Design, OtherApp and Research. Finder's Move to Trash removed only the synthetic link. Move Item Here moved `Aelomi Notes` into OtherApp, and File > Rename changed it to `Aelomi`. The check copied the displayed configured path. Go to Folder stayed open without finding that destination. The screenshot shows the unresolved configured path and the misplaced folder behind it.

The first failure attempt selected Undo Rename and restored the old name. It then stopped because the next menu read still showed Undo Rename. The next attempt also stopped there. Later window creation commands stopped taking effect. The retained logs include:

```text
PASS: only the synthetic link was trashed; the cloud workspace remains
PASS: differing-name folder was moved into OtherApp and renamed, leaving the configured path absent
PASS: Go to Folder could not open the configured path after the misplaced move
Undo Rename
execution error: Expected named Undo Move, got Undo Rename (-2700)
```

The incomplete steps are choosing the named Undo Move, confirming the folder returned to the cloud source, retrying the move into the destination, renaming there, passing the configured-path check, comparing the original file bytes, and relaunching on that configured path with an exact original-page assertion. None of those steps is claimed as proven. The historical fix1 record proves its older equal-name success path only.

Finder's last observed state was one pre-existing window titled `local`. A one-second sample placed its main thread in `NSMenuItem(Accessibility) accessibilityPerformAction`, `NSCocoaMenuImpl openWithOptions` and `NSMenuTrackingSession startRunningMenuEventLoop`. Keys, AXCancel and folder-open requests did not create another visible window. All owned Finder windows from the partial recovery attempts had been closed. Finder was left running, and no further keys or Finder calls were sent after the user's stop instruction. The driver did not restart or kill Finder.

During diagnosis, one read-only Finder inventory ran after its diagnostic lock had released. A preceding read-only window probe also lacked a fresh ownership check. This violated the brief's lock rule. No keys, clicks or file operation occurred in either probe. All input, screenshots and synthetic-folder operations ran under the owned lock. The user was told about the inventory deviation. The later screenshot-only script contains no Finder calls or keystrokes.

Real drag ergonomics, cloud-provider removal prompts and native automatic path selection remain unproven. The synthetic cloud root cannot produce a real iCloud or provider removal prompt. The shared-link and override recovery redesigns and Finder tab behavior remain outside this PR.

## Screenshots and cleanup

[Six native refusal screenshots](https://github.com/aelomi/aelomi/pull/90#issuecomment-6207215859) show every variant in light and dark. Each original was inspected. The leaf screenshots show the path chips, removal warning and all four steps. The shared-link screenshots show the new link, local-folder and sync-removal copy. The override screenshots show the keep-open and separate restart steps.

The screenshot-only runs were:

- leaf at `~/.local/state/aelomi-verify/20261016-083954-dev-RYhI`.
- shared at `~/.local/state/aelomi-verify/20261016-084052-dev-jqlo`.
- override at `~/.local/state/aelomi-verify/20261016-084150-dev-Rw2y`.

The screenshot-only script completed with:

```text
PASS: all refusal variants captured in light and dark without keystrokes or Finder calls; owned processes stopped; appearance restored
```

All three shell logs include `quit frontend ready skipped: storage refused`. The native runs were stopped with the launch helper cleanup, without another Finder or keyboard step. The final read-only real-data-folder listings were identical. The cleanup audit found no live recorded native helper, and no UI lock was held.

GitHub CLI 2.102.0 was downloaded and its archive matched the published SHA-256 `da922c20d1792e5b2cbf375593d7a658acf034c12c84e007e71c76ef959c337e`. Every uploaded image was downloaded without credentials, opened as a PNG with `sips`, and matched its original bytes. Finder screenshots stay local because their sidebars contain the home-folder name.

| Screenshot | Bytes | SHA-256 | Public image |
| --- | ---: | --- | --- |
| `20261016-083954-dev-RYhI/leaf-dark.png` | 365364 | `f90214d7f2d7840c907a9aa2d8fe3b0d91eb2b93d7e8600b631b12491d55fc29` | [Image](https://github.com/user-attachments/assets/203c9483-e79d-4e27-a42e-dcea0649a9c8) |
| `20261016-083954-dev-RYhI/leaf-light.png` | 393084 | `41418b424aa22e9a4c01939191bc71b58280a3125886452697c4f29bda0f9dca` | [Image](https://github.com/user-attachments/assets/df1b6922-1a7d-4b54-b81b-8821f5ac4174) |
| `20261016-084052-dev-jqlo/shared-dark.png` | 208704 | `e3fc6201390a9643d323b30991f1d15bfa8d1b82adf0ded001d64f3b6869a66c` | [Image](https://github.com/user-attachments/assets/45f89ab6-0a61-4fb4-b192-ffdca5b767e2) |
| `20261016-084052-dev-jqlo/shared-light.png` | 219580 | `a6d305108eb70ec0a0edcd878eebcfa30c2cc4c63fc02d2b135986778a19bd1c` | [Image](https://github.com/user-attachments/assets/00424a3f-3dbe-4c00-ae49-399dbb1c2d32) |
| `20261016-084150-dev-Rw2y/override-dark.png` | 158481 | `fff364f5d7c221139d3eec9f68ec44515942a2b59ff0f69eec67ac6a70195c79` | [Image](https://github.com/user-attachments/assets/217b53d6-8d34-4527-a27c-77031045d5ca) |
| `20261016-084150-dev-Rw2y/override-light.png` | 165769 | `29ccc0f275aba22fd6429c60233ddeb8fecac4203cfa017a5dda1963f5243aa7` | [Image](https://github.com/user-attachments/assets/ad0a18de-029b-4ce5-a4be-490e4cff3196) |

## Reproduce

(see the warning above)

Copy the text scripts into `.local-gate/e2-fix2/`, dropping the final `.txt` suffix except from `seed.rs.txt`. `capture-ui-only.sh` creates fresh synthetic cloud paths and captures every refusal variant without sending keys or calling Finder. `stress.mjs` runs through `mock-drive.mjs`. Run `hoisted-hook-proof.sh` only when no native dev instance is active, then rebuild the mock before positive checks.

`prepare-proof.sh` and `attempted-finder-proof.sh` preserve the attempted recovery driver for diagnosis. The latter takes the keyboard lock, closes only its owned Finder windows and stops owned Aelomi launches on failure. It has not completed successfully. Do not run it while the user's stop instruction applies. The record makes no passing recovery claim for it.

`audit-record.py` checks the recorded source paths, patch-id, unchanged product files, evidence deletion, screenshot hashes and both negative-check controls. The committed final head's full `pnpm check:all` result is cited in PR #90's Verification section.
