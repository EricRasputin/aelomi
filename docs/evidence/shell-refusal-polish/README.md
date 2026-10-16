# Refusal recovery follow-ups

Source commit `f36bca092f98638f9f9702981d4e53e1ccf635be`. Merge base against `origin/main` is `96cdc94ccb48be742d5bdc907de1f609f4a747fe`. Verbatim patch-id is `bce7a3010ac706ff70b98da6cdb37ff01103d2eb`.

Proven pathspecs from the repository root are `src/storage-copy.ts` and `src/router.tsx`. The app does not move folders. This record proves refusal presentation. Acceptance 7 proves the existing quit dialog while storage is refused. The real Finder recovery proof is INCONCLUSIVE.

## Shape and choice

Model the Domain retains Rust's `CloudRecovery` enum and the frontend's ordered `RecoveryStep` union.
The leaf recipe gains a visible drop check, and the rename still depends only on different names.
Laziness Protocol keeps the existing quit dialog available before startup is ready. Automatically confirming an empty native pending list would assume the webview has delivered every edit.

Test Behavior, Not Implementation gives the palette absence check a positive control before reload. Acceptance 7 exercises a timeout with no pending documents or settings, Keep app open, both palette entry points, zero document loads, and Quit anyway.
No Comments was applied directly because the brief forbids nested agents. The restored reason comment describes an external dependency's service-worker behavior, confirmed in the installed engine. There are no new suppressions or unresolved comment flags.

## Automated proof

The full source gate passed on macOS 27.0.1, build 26A434, Apple Silicon. The record is `.local-gate/f36bca092f98638f9f9702981d4e53e1ccf635be.checks.json`, with all 11 configured stages complete. The focused runs stay under `.local-gate/e2/`. Acceptance uses its guarded fixture. Playwright covers the leaf variant in light and dark, including narrow layout, wrapped paths, non-Latin text, emoji, and doubled text size. The SharedLink and Override cases in this source snapshot check strings at the default viewport.

The focused checks passed 11 copy tests, three Acceptance 7 cases, six WebKit refusal cases, typecheck, and lint. The full gate passed 220 Vitest tests, 407 ordinary Rust tests, the serial release checks, 23 acceptance tests, 25 WebKit tests, all seven timing cases, the bundle, and cold start. These are configured CI gates, not a before-and-after performance claim.

`src/storage-copy.test.ts` keeps the exact `maps safe recovery steps for $name` mappings. `e2e/cloud-folder.e2e.ts` keeps the named cloud-folder Acceptance 7 case and adds the SharedLink and Override keep-open cases. The primary case also covers SHELL-7 command isolation and SHELL-9/SHELL-10 timeout choices. `tests/playwright/storage-refusal.spec.ts` covers the light and dark chip cases and each shared/override restart case.

The mock stress run is `~/.local/state/aelomi-verify/20261016-001404-mock-sEWZXB`. Its four fixtures cover all three variants and a long equal-name leaf case. Light and dark, widths 320, 960, and 1440, and 100% and 200% text produce no horizontal overflow or clipped path field. The proof-only fixture toggle is injected by the local flow and never enters production. Titles and step counts come from the fixed variants. Dynamic path fields include long names, deep paths, Japanese text, and emoji.

## Native proof

Use only fresh synthetic folders and the verify-aelomi launch script with explicit `AELOMI_DATA_DIR` and `AELOMI_CLOUD_ROOTS`. Take `/tmp/aelomi-ui.lock` before keys, clicks, Finder steps, and screenshots. Build and launch before taking it. Release between scenarios and on failure. Never launch a default instance.

Raw logs, accessibility trees, file manifests, and build outputs stay local. Screenshots are attached to the PR comment and checked through public unauthenticated downloads. No performance improvement is claimed.

## Native result and limitation

The successful leaf UI run is `~/.local/state/aelomi-verify/20261016-054613-dev-NaFJ`. SharedLink uses `~/.local/state/aelomi-verify/20261016-055447-dev-EVra`. Override uses `~/.local/state/aelomi-verify/20261016-055905-dev-ms4S`. Each launch explicitly sets a synthetic data folder and the fake cloud root.

The first detached launch, `20261016-054453-dev-ooqf`, was gone before doctor could inspect it. It is not counted as proof. A persistent terminal session kept the retry alive and every doctor check passed.

Finder started with one window on the synthetic root, then opened the step-1 parent. The destination held the equal-name `Aelomi` link and Archive, Design, Research, and OtherApp folders. The equal-name recipe showed no rename step. CUA drags to Finder's sidebar Bin did not remove the link. A Dock read returned `Computer Use server error -10005: timeoutReached`. Native CoreGraphics events then used the measured icon center `522,522`, sidebar Bin center `360,546`, and Dock Bin center `31,845`. Explicit click state, a HID event source, motion deltas, longer holds, and a refreshed Finder binding did not start a Finder drag. A held-state capture showed the pointer at Bin, the selected original alias, and no drag image.

**INCONCLUSIVE.** The link remains in place. No keyboard or shell movement equivalent was substituted. The folder drop and exact recovered-page relaunch assertion were not performed. Every original workspace file remains byte-identical. This record makes no complete-recovery claim.

The leaf shell logged `quit flush via=CompleteQuit pending=None`, `quit confirmed via=drain`, and `quit flush via=Exit pending=None`. Appearance was restored and the owned keyboard lock was released after the scenario. The read-only real-data folder listing before the first launch and after the last launch was identical. All three successful UI runs quit through the normal drain. Appearance was restored after each run, and every owned app process was cleaned up.

## Reproduce the checks

Copy the text scripts to `.local-gate/e2/` without their final `.txt` suffix. Run them from the repository root. The native scripts assume the required keyboard lock protocol and restore appearance on exit.

`generate-body.mjs` writes the synthetic marker `Original pages survived the equal-name Finder drag.` Use `docs/evidence/pr86-fix4/seed.rs.txt` as a temporary `crates/core/examples/pr90_e2_seed.rs`, seed a fresh `<base>/cloud/Aelomi` through the real Store, and remove that temporary source afterward. The recorded seed command was:

```bash
cargo run -p aelomi-core --features test-support --example pr90_e2_seed -- "$base/cloud/Aelomi" .local-gate/e2/original.yupdate
```

The base is a fresh `/private/tmp/ae90-e2-*` directory. Put its path in `.local-gate/e2/base.txt`. Create `<base>/local/{Archive,Design,Research,OtherApp}` and an equal-name `<base>/local/Aelomi` link to the cloud folder. The shared case uses `<base>/Shared Support` as a parent link to `<base>/cloud`. The override case uses `<base>/cloud/Aelomi` directly.

Launch only with the verify-aelomi script, explicit `AELOMI_DATA_DIR`, and `AELOMI_CLOUD_ROOTS=<base>/cloud`. Keep the launching terminal session alive. Save each run directory in `.local-gate/e2/{leaf,shared,override}-run.txt`. Run `bash .local-gate/e2/capture-variant.sh <variant>` from a second terminal. It takes the keyboard lock, runs doctor, captures both themes, quits normally, restores appearance, releases its owned lock, and cleans up.

The leaf Finder attempt used CUA and the local `.local-gate/e2/drag.swift` helper under the same keyboard lock. The local `native-attempts.md`, `native-drag.log`, and `drag-held.png` retain the failed input evidence. They are not a successful recovery script. Before and after the attempts, `proof-files.py` compared every original file with the recorded SHA-256 manifest.

Run the mock stress flow with:

```bash
.claude/skills/verify-aelomi/scripts/mock-drive.mjs --flow .local-gate/e2/stress.mjs
```

Empty names and arbitrary step counts are not native recovery inputs. The cases exercise the dynamic path and folder-name fields without changing the component.

## Published screenshots

[Native UI self-proof](https://github.com/aelomi/aelomi/pull/90#issuecomment-6205296653) contains the six screenshots below. GitHub CLI 2.102.0 matched its published archive checksum and the cached executable matched the archive. Each attachment was downloaded without credentials, opened as PNG bytes, and matched the recorded byte size and SHA-256. All original screenshots were inspected locally.

| Screenshot | Bytes | SHA-256 |
| --- | ---: | --- |
| [leaf-dark.png](https://github.com/user-attachments/assets/4b6cfea5-6426-4e6b-aac3-3cc41f20c648) | 264017 | `82274ca7766901518fb48c3104af91b61b42f314cca343d369ecfe9154f0624c` |
| [leaf-light.png](https://github.com/user-attachments/assets/0b4e5957-2d35-46cb-964a-923f74e45fc0) | 281050 | `6dcff88dd1343b8bc3adb3d45f64592a0632e7fbf4f646e3c29dbfb2b2068f97` |
| [shared-dark.png](https://github.com/user-attachments/assets/ad0e8b9d-6db4-49d7-a103-3e794c51a6e1) | 193062 | `d51251628bcfd5ee197dbcf14b42a117f850e8eaadf10f1a68484b8571ec45ff` |
| [shared-light.png](https://github.com/user-attachments/assets/ff158846-f738-4d1a-b93c-61346076f21b) | 201820 | `ee1b2c5d1065770853366a818c8bc9d44bb50e6dec30ac05d2ee61131d4335ea` |
| [override-dark.png](https://github.com/user-attachments/assets/3023f398-6575-41fa-915c-d71921701959) | 157696 | `62ddc4652ecfb2a991463c5eabd4b0142ab9041eef998fc9372d3e0982e0fc59` |
| [override-light.png](https://github.com/user-attachments/assets/f3952845-2dbc-40c3-b854-c809c3ad9d75) | 165003 | `bab28166b06b775ab28e3549acc748cfb8ff33c1be672c7ed24705d1922f73ce` |
