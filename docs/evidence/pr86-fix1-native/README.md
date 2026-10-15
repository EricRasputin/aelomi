# PR 86 recovery and link self-proof

Source commit `06079e2917eee86acd7cc40dae939a9825252d24`. Merge base `a4db19d8ab2e1ac072dea8bc3a1cc3747af8fdae` against `origin/main`. Patch-id `b33e2776cdd015828c6d1d3d07678421e1159104`.

The proven pathspecs are `src/` and `src-tauri/`. This record covers pending-edit replay, cloud-path refusal, link gesture handling, and durability confirmation. It introduces no data migration or movement.

## Environment and results

macOS on Apple Silicon. All app runs used synthetic folders under `/private/tmp`, outside the normal data folder. The source was built with `pnpm tauri build --bundles app`. Cloud roots were injected only into a development build. Doctor passed for each retained proof run.

- `~/.local/state/aelomi-verify/20261015-234832-dev-CC50` shows a configured symbolic link and its resolved cloud target in light and dark appearances. The original appearance was restored. The heading receives focus and precedes the IPC diagnostic. Its startup line is `storage status=CloudPath { configured: "/private/tmp/aelomi-pr86-fix1-cloud/configured-data", resolved: "/private/tmp/aelomi-pr86-fix1-cloud/cloud/data" } path=/private/tmp/aelomi-pr86-fix1-cloud/cloud/data`.
- Packaged run `~/.local/state/aelomi-verify/20261015-235008-app-AJk6` starts on the Link recovery proof page. A native click opens Example Domain in Dia and logs `url opener launched=true` once. Double-click selects Example and adds one opener line. Right-click on Context check leaves no native link menu and its loopback counter records `[]`. A subsequent normal click records exactly one request with a Chrome user agent, the positive control. The window remains on the page throughout. The context-menu route was tested instead of a physical force click.
- The same run types ` native-proof-86-x` through native keys and quits with ⌘Q. Its log ends with `quit flush via=CompleteQuit pending=[]`, `quit allowed via=ExitRequested confirmed=true`, and `quit flush via=Exit pending=[]`. Relaunch `~/.local/state/aelomi-verify/20261015-235136-app-BgKL` reads the exact body `Example\n\nContext check\n\nType here:  native-proof-86-x` in the accessibility tree.
- Mock run `~/.local/state/aelomi-verify/20261015-181513-mock-y2S8f1` has no horizontal overflow with normal and long paths at 320, 480, 960, or 1440 px in both appearances. Japanese text, emoji, unbreakable names, and 200% text remain readable and scrollable. The native shots show the static-heading outline correction found in that run.
- `src/editor/page-session.test.ts` reproduces close, reopen before retry, recover, and further typing. It asserts exact editor content and stored text, including an in-flight acceptance that races a load. `src/editor/page-sync.test.ts` holds a durability acknowledgement and checks that the banner remains, then verifies an older flush cannot clear a newer refusal. The native-command test `recovery_flush_waits_for_durability_and_survives_a_crash` crashes the simulated disk and reads exactly `recovered x` after reopening.

A deliberate acceptance test that did not request the guard fixture failed with `SHELL-13 non-local requests: WS wss://example.com/aelomi-guard-canary`. The test was removed. The local log is `.local-gate/fix1/websocket-negative.log`.

## Limits and tool failures

This is builder self-proof, not the independent verifier or review gate. A physical force click was not synthesized. A CUA screenshot-stream error occurred after the native double-click; the next accessibility read showed Example selected, and app.sh captured the screenshots. The first dev launch ended with its parent tool session, so the retained launch kept its terminal session open. Neither failure is counted as a product pass.

Native programmatic top-level navigation is unverified. In run `~/.local/state/aelomi-verify/20261015-235211-dev-YGuE`, the PID-scoped inspector shortcut produced no inspector window. No navigation was attempted, and this run was cleaned up. The guard's exact-origin decisions have Rust tests. Adding a shipping debug action solely for proof was declined.

The dev CSP configuration does not add a Vite response header. The URL and context-menu self-proof therefore uses the packaged app. The packaged CSP permits no remote origin for scripts, styles, images, frames, or connections.

## Reproduce

`seed.rs.txt` stores the generated workspace and body updates through Store, then flushes. `generate-native.ts.txt` writes the synthetic body with URL marks. `request-counter.py.txt` is the loopback counter. Copy the generator and counter to `.local-gate/fix1/` and the seed to `crates/core/examples/pr86_seed.rs`. Run `node .local-gate/fix1/generate-native.ts`, then `cargo run -p aelomi-core --features test-support --example pr86_seed -- /private/tmp/aelomi-pr86-fix1-native-data .local-gate/fix1/native-body.yupdate`. Start the counter with `python3 .local-gate/fix1/request-counter.py`, and follow `native-actions.js.txt` with the feature recipes in `.claude/skills/verify-aelomi/`.

The refusal recipe creates a fresh fake cloud folder and a sibling symbolic link, sets `AELOMI_CLOUD_ROOTS` to the fake root and `AELOMI_DATA_DIR` to the link, then uses app.sh launch dev, doctor, and screenshots in both appearances. Never use the normal data folder. Preserve the app launch's terminal session until cleanup in the Codex exec environment.

## Published screenshots

[Native self-proof comment](https://github.com/aelomi/aelomi/pull/86#issuecomment-6199227178). Uploaded with GitHub CLI 2.102.0 after verifying its official archive checksum. Every URL below was downloaded without authentication, opened with sips, and compared byte for byte with its local PNG. Raw logs and accessibility trees remain local.

| Screenshot | Bytes | SHA-256 |
| --- | ---: | --- |
| [cloud-dark.png](https://github.com/user-attachments/assets/14799b09-ebaf-436e-8014-b2b84ddbafe5) | 239354 | `84d6311c61f759ec98f9c23dd327497cd780e6b2e5e35b690bf3750a4fabc055` |
| [cloud-light.png](https://github.com/user-attachments/assets/4105a715-f570-4086-857c-325714e33902) | 247747 | `2a8c17c1d2b50d1fffb45a0beb091df0c3ed593b1133c71a182d1a83a86b3ce2` |
| [url-before.png](https://github.com/user-attachments/assets/bb737857-79f1-4e59-8d80-34f6e2563338) | 82816 | `f0e2cbed63da7438c4da603ebe76f186432f7a291bada19be74ab824e7c4859a` |
| [url-after.png](https://github.com/user-attachments/assets/72eb27b5-ba93-446a-9d56-3ef8fe6f8c2b) | 89421 | `360fa3eec3b4d4e025259bbf43e8cf5d8ffc6d4957bd88999bc02d47479cd4a2` |
| [context-after.png](https://github.com/user-attachments/assets/4265118d-27f3-46bf-8c31-9ec04c0f51e4) | 84836 | `cfe69c5209f0d86dd6cc659eb3db43bf5df050031a5b9fea576262b35f98a120` |
| [typed-before-quit.png](https://github.com/user-attachments/assets/bd5df28f-0945-40d1-9c55-31b633745c71) | 94252 | `78793a88f9206d20cdf54643cc58f2ba9801fbcf1ee41f677ba593caa481aaf9` |
| [typed-after-reopen.png](https://github.com/user-attachments/assets/d80fdc54-ebda-4770-819c-731e8517855b) | 88331 | `ce1d9d1c856fea406eee2df5cba85083afd03cb360123c25c8c486889407a7c5` |
