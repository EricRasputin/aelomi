# PR 86 round 3 recovery and link proof

Source commit `122c3067059d709b474f754b8388998be97a71f8`. Merge base against `origin/main`: `fe0baa50bbb2346615e33097c8734baa2a422c68`. Verbatim patch-id: `7b3e581beb254c36bf6a00d066d6fd7182d20642`.

Proven pathspecs, from the repository root: `src-tauri/src/storage.rs`, `src/ipc/bindings.ts`, `src/storage-copy.ts`, `src/components/storage-refusal.tsx`, `src/styles.css`, `src/editor/url-links.ts`, and `src/editor/delivery.ts`. This record covers refusal without storage writes, manual recovery instructions, link activation, and recovery confirmation. The app performs no folder movement or migration.

## Shape and decisions

`CloudPath` carries configured and resolved paths plus `CloudRecovery`.
Rust lexically normalizes the configured path and distinguishes a data-folder link, a shared parent link, and a direct override.
Each variant carries its own required parts; TypeScript maps the variant to copy without splitting paths.

Both round-3 Consider items were accepted. `deliver()` retries only when refused bytes exist, so a new append cannot invalidate a confirmation that it just started. The acceptance context error now directs callers to a fresh test setup; that runner cannot restore or clear an existing context's state.

## Environment and results

macOS on Apple Silicon, system font and existing theme tokens. Native cloud runs used synthetic folders under `/private/tmp/ae86-r3-vHbG`, debug-only `AELOMI_CLOUD_ROOTS`, and port 55086. The packaged URL runs used `/private/tmp/ae86-r3-links-ogk9qkp5/data` and a loopback request counter on port 55087. Doctor passed for every retained native run. Keyboard input, pointer input, and screenshots held the shared keyboard lock; appearance and clipboard contents were restored. No new animation was added. No performance improvement is claimed.

| Variant | Native run under `~/.local/state/aelomi-verify/` | Result |
| --- | --- | --- |
| Data-folder link | `20261016-022142-dev-saDn` | Trailing-slash path selects leaf recovery. Both parent chips, explicit rename from Aelomi Notes to Aelomi, and restart last are visible in both themes. |
| Shared parent link | `20261016-022305-dev-P1hH` | Names Application Support, warns that other apps may use it, and directs the user to cloud settings. No removal or movement instruction. |
| Direct override | `20261016-022405-dev-IUeO` | Shows only the environment override and restart steps in both themes. |

The cloud script ends with `PASS: all three variants preserved exact sentinel bytes, entries, and links`. Each target still contains exactly one sentinel with its original bytes; both symbolic links remain links. Native parent-path copying passed with accessibility focus, ⌘A, then ⌘C. Accessibility focus alone did not establish selection. Whole-path pointer selection and keyboard DOM-focus selection pass in the WebKit acceptance and Playwright tests.

Packaged run `20261016-022629-app-TRYh` logged an opener event for AXPress and one for an initial pointer click. A right-click on Context check produced no native link menu and no request (`[]`). Its later AXPress control produced one loopback request with a Chrome user agent. The page stayed at `tauri://localhost/page/page-proof`.

That run typed `native-proof-86-round3` and quit through ⌘Q. Its final lines include:

```text
quit draining via=ExitRequested revision=1
quit flush via=CompleteQuit pending=None
quit confirmed via=drain
quit allowed via=ExitRequested confirmed=true
quit flush via=Exit pending=None
```

Reopen `20261016-024221-app-cdC5` showed the exact marker. A pointer click logged one `url opener launched=true`; Shift-click left the count at one; AXPress then raised it to two. The window still showed the same page. After another native quit, reading the stored Y.Doc asserted exactly `Before Example after.Context checkType here: native-proof-86-round3` and printed `PASS: exact stored body survives native quit and reopen`.

Mock run `20261015-204516-mock-8BukrL` passed all three variants with demo and worst-case data, both themes, widths 320/480/960/1440, long unbreakable names, Japanese text, emoji, and 200% text. With `field-sizing: fixed !important`, paths wrap without clipping or horizontal overflow. The committed Playwright tests also prove complete selection and a reachable final instruction. The proof-only data toggle is injected by the driver and is absent from production.

## Design review

| Before | After | Why |
| --- | --- | --- |
| One linked-path recipe could suggest changing a shared folder. | Rust selects one of three recovery variants. | Shared folders receive settings guidance only. |
| Paths embedded in prose were awkward to copy. | Each path has a separate selectable chip with the existing theme colors. | Finder's Go to Folder can receive an exact path. |
| Differing folder names were implicit. | The leaf recipe adds the exact rename only when needed. | The recovered folder has the configured name. |
| Textarea height depended on newer WebKit field sizing. | A hidden grid mirror sizes the native readonly textarea. | Older WebKit keeps the entire path visible, including at narrow widths. |

## Proving tests

- SHELL-11: `src-tauri/src/storage.rs` cases `cloud_leaf_recovery_has_parts_for_normal_trailing_slash_and_dot_dot_paths`, `cloud_shared_link_recovery_names_the_shared_folder_without_changing_data`, and `cloud_override_recovery_keeps_paths_without_changing_data` assert exact serialized parts and unchanged sentinel contents. All three were red before implementation.
- SHELL-11: `src/storage-copy.test.ts`, `maps safe recovery steps for $name`, asserts five exact mappings: leaf, differing names, shared parent, override, and debug path. `e2e/cloud-folder.e2e.ts`, `SHELL-11 acceptance 7: a cloud folder shows its resolved path and how to move it back`, proves the leaf screen and selection. `tests/playwright/storage-refusal.spec.ts` proves light/dark chips without automatic field sizing. The clipping check failed before the grid sizing fix.
- SHELL-13: `src/editor/url-links.test.ts`, `%s clicks prevent navigation without opening a link`, covers Shift, Option, Control, and Command. `e2e/url-links.e2e.ts`, `SHELL-13 shift-click makes no browser handoff`, uses a real modifier click. Modifier cases were red before the guard; detail-0, 4 px movement, double-click/later-single, and context-menu cases still pass.
- SHELL-10: `src/editor/page-sync.test.ts`, `new typing waits for its append before confirming a recovering delivery`, failed with the old unconditional retry and passes with retry limited to refused bytes. Earlier close/reopen, durability, and revision-race coverage remains intact.

## Reproduce

From the repository root, create `.local-gate/fix3/` and copy these scripts there without the final `.txt` suffix. Check free disk space before building. Run `bash .local-gate/fix3/cloud-proof.sh` in a terminal that stays alive until completion. The default command needs no pointer-proof pause. It acquires/releases the keyboard lock for each variant, captures both themes, restores global state on failure, cleans up its app, and checks the exact synthetic files and links.

For the mock matrix, run `node .claude/skills/verify-aelomi/scripts/mock-drive.mjs --flow .local-gate/fix3/layout.mjs`; consult its `--help` for a free port. The driver injects only synthetic absolute paths.

For packaged links, build with `pnpm tauri build --bundles app`, then run `node .local-gate/fix3/generate-body.mjs`. Copy `seed.rs.txt` temporarily to `crates/core/examples/pr86_seed.rs`, run `cargo run -p aelomi-core --features test-support --example pr86_seed -- <fresh-synthetic-data-folder> .local-gate/fix3/native-body.yupdate`, and remove the temporary example. Write the synthetic parent path to `.local-gate/fix3/link-base.txt`; the launcher expects its `data` child to be the seeded folder. Start `request-counter.py`, keep `link-launch.sh` running, then run `link-lock.sh` in another live terminal. Remove any old release marker before taking a new lock. Run doctor before attaching CUA; follow `native-actions.js.txt` and the feature file. A missing app must be relaunched by the isolated launcher, never by CUA.

For the quit assertion, compile `read-page.rs.txt` as another temporary core example, run it only after the app exits, and write `.local-gate/fix3/reopened-page.yupdate`; `node .local-gate/fix3/check-body.mjs` checks the exact text. Touch both release markers to release the lock and launcher, then stop the owned request-counter process. Do not hold the keyboard lock during builds.

## Limits and deviations

Repeated synthetic pointer sequences stopped producing opener events after the first successful click, despite fresh AX targets. Reopening restored a successful initial pointer click. The current native double-click/later-single sequence is therefore inconclusive; it is not counted as a pass. Exact unit and acceptance cases cover that sequence. The Shift-click result is bracketed by the initial pointer and later AXPress controls. A physical force click was not synthesized; the context-menu path was exercised instead. Native programmatic navigation remains unexercised; Rust tests cover the guard decisions.

An initial isolated launcher expired with its tool session. CUA was attached before doctor and launched a default instance. It was closed immediately without editing. Opening it may have created shell metadata; this record makes no claim that the default folder was untouched. Subsequent launches kept their parent terminal alive and required doctor before attachment. No default data was deleted or altered to clean up this deviation. An earlier cloud-copy attempt failed because accessibility focus did not select text; its trap restored appearance and clipboard, released the lock, and cleaned up the app. The successful run explicitly used ⌘A.

The first full local gate on the preceding source head was intentionally stopped when the WebKit sizing issue required another product commit. The final gate result belongs in the PR's Verification section and its `.local-gate/<head>.checks.json`, not to that interrupted run. Review and independent verification remain the orchestrator's work.

## Published screenshots

[Round-3 self-proof comment](https://github.com/aelomi/aelomi/pull/86#issuecomment-6202413453). GitHub CLI 2.102.0 was checked against the official release asset's SHA-256 before use. Every attachment was downloaded without authentication, viewed, and matched against its original byte size and SHA-256. Logs and accessibility trees remain local.

| Screenshot | Bytes | SHA-256 |
| --- | ---: | --- |
| [cloud-leaf-dark.png](https://github.com/user-attachments/assets/f32edfb0-63be-4b62-8917-663b0389da3c) | 199927 | `f5c0e76e39654467de7a57c67c63ebe843379975981e49d42041a7f0efdbc302` |
| [cloud-leaf-light.png](https://github.com/user-attachments/assets/add3f368-b1ac-4d65-a909-21af36a8373e) | 210563 | `aa3b4b496e2c80c2c4a87a554d742b38d9d0db2c4c5459faac81b60500cb5d32` |
| [cloud-shared-dark.png](https://github.com/user-attachments/assets/7620fe11-ea42-4d65-bae8-f4ae1efa0099) | 173049 | `caaf68e442bcb178b172b7cdb7e1aa480b93676ecf0f0ec859f2b6a45ce6d67f` |
| [cloud-shared-light.png](https://github.com/user-attachments/assets/042cac08-719a-4bb0-b629-3af18f3cf077) | 180927 | `de11fe02008fb29e7d516fabb832942860a7824035d2af7d3c58b46ffbf2b0ba` |
| [cloud-override-dark.png](https://github.com/user-attachments/assets/190a1ab9-e4a4-418b-82e1-d654fdc21a35) | 134488 | `d8555d91c77c36ad8c525ec87a1d3ee190ceb296166b2e1728864058b07cb8ff` |
| [cloud-override-light.png](https://github.com/user-attachments/assets/7a7415aa-167d-4e94-94d8-718d96a6d2c1) | 139806 | `d6f85f4076220fd94c05dc99995481db6978ec6089a95e38df8b668a2d34c0f2` |
| [url-context.png](https://github.com/user-attachments/assets/9a9a5e95-21f2-4e34-929d-9da39ab5d8f2) | 100936 | `4c29f3851ace3accda38d6b92955d6448574482db31f112456011b74beae61e2` |
| [url-reopened.png](https://github.com/user-attachments/assets/15b7b2f3-354a-4ea6-94ae-be2138d6a4fd) | 100041 | `1b91df6e68ead68957f51e39eaae04a426395cda70354050cd28cfc472c3c606` |
| [leaf-worst-light-320.png](https://github.com/user-attachments/assets/96587e28-6b8a-4c3b-b179-15d5303c2c88) | 74114 | `1773513872e75460501e0ffb26d6fcea2c534d57c117dbe4f626d67434344b4a` |
