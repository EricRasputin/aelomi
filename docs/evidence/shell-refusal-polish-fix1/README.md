# Refusal recovery success path, fix round 1

`native-proof.sh.txt` changes the Mac's appearance and must not be rerun as is. Follow the current [verify-aelomi rules](../../../.claude/skills/verify-aelomi/SKILL.md#rules) instead.

Source commit `5fb8fb008f712ac72117b71d6a67ca28d6ccf6bb`. Merge base against `origin/main` is `6d64d683d5bf02bea2f67e8038428caef1ab4d62`. Verbatim patch-id is `954478497f2131627cd3edd8dd64165b3046115f`.

Proven pathspecs are `src/storage-copy.ts` and `src/storage-copy.test.ts`. This historical record covers the successful equal-name recipe at that source commit. Round 2 updates the check copy and proves failure recovery separately.

## Native proof

PASS on macOS 27.0.1 build 26A434, Apple Silicon. The synthetic base was `/private/tmp/ae90-r1-nFJn1Y`. Every app launch used `app.sh launch dev` with explicit synthetic `AELOMI_DATA_DIR` and `AELOMI_CLOUD_ROOTS`.

- leaf run is `~/.local/state/aelomi-verify/20261016-070538-dev-pR7r`.
- recovered run is `~/.local/state/aelomi-verify/20261016-070625-dev-89vZ`.

The successful recovery run read the three displayed path chips from the accessibility tree, asserted each exact value, and put that value on the clipboard, as pr86-fix4 did. No native one-click-copy claim is made.

The recovery started with one owned Finder window. One existing window was preserved. The destination held Archive, Design, OtherApp and Research alongside the equal-name Aelomi link. Finder's ⌘⌫ trashed only that link. View > as Icons left the destination window open. File > New Finder Window opened a second owned window on the fake cloud parent. ⌘C followed by Move Item Here with ⌘⌥V in the step-1 window moved Aelomi into the destination. Equal names produced no rename instruction. Go to Folder on the check chip opened `/private/tmp/ae90-r1-nFJn1Y/Recovery Destination/Aelomi` and showed the original docs and format marker.

The before/after manifest comparison checked every original workspace file's SHA-256 before reopening the app. Every byte matched. Only Finder's `.DS_Store` metadata was permitted as an addition. Both owned Finder windows were closed through their accessibility close buttons.

The successful proof printed these exact lines in `.local-gate/e2-fix1/native.log`:

```text
PASS: only the synthetic link was trashed; the cloud workspace remains
PASS: Finder Move Item Here moved the equal-name folder into the step-1 window
PASS: configured-folder check opened the data folder; every original workspace file is byte-identical
PASS: relaunched app shows exact original page text: Original pages survived the equal-name Finder recovery, round 1.
PASS: all owned Finder windows closed; all app processes cleaned; appearance and clipboard restored
```

The complete native `Page body` value equals the original marker. This record proves the successful equal-name move, configured-folder check and relaunch only. It does not prove the failure remedy, a differing-name rename, cloud-provider removal prompts, native automatic path selection or real drag ergonomics. Finder keyboard equivalents replaced the drags.

Both owned Finder windows were closed. Appearance and clipboard were restored. The real-data folder was listed read-only before launch and after cleanup; the listings were identical.

## Reproduce

(see the warning above)

Copy the text scripts into `.local-gate/e2-fix1/`, removing the final `.txt` suffix except from `seed.rs.txt`. Run `prepare-proof.sh`, then `native-proof.sh` against the recorded source. The scripts use fresh synthetic folders and a fake cloud root. They take the keyboard lock, preserve existing Finder windows, and clean up owned launches. The negative checks now exit explicitly on a match.

## Published screenshots

[Native fix-round proof](https://github.com/aelomi/aelomi/pull/90#issuecomment-6206056209) contains the three success-path screenshots below. Each original was inspected. The cached GitHub CLI 2.102.0 executable matched its checksum-verified archive. Every attachment was downloaded without credentials, opened as a PNG through `sips`, and matched the original byte count and SHA-256. Other Finder shots stay local because their sidebars include the home-folder name.

| Screenshot | Bytes | SHA-256 |
| --- | ---: | --- |
| [link-trashed-icons.png](https://github.com/user-attachments/assets/90727aaf-69a1-4d99-82b9-abb212c5542b) | 565142 | `c7a0b4b92f2231aa37d5ac585e1f0a184eb58889dad37bc8f102ba18d880d129` |
| [configured-folder-check.png](https://github.com/user-attachments/assets/b684b882-b0db-4f42-915a-85e725413a5c) | 546082 | `03404aba1033a505c1eebd10b205e5f6189dd3e64aa442f8da66295c97589394` |
| [original-page-recovered.png](https://github.com/user-attachments/assets/fdc77d11-1501-4252-84e6-c79f1034a1b2) | 89698 | `22e50e20460c1a727a2dc15ffac1394e855605700f0557618b08ace9cf59ed69` |
