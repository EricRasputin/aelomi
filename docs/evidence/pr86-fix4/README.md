# PR 86 round 4 Finder recovery proof

`finder-proof.sh.txt` changes the Mac's appearance and must not be rerun as is. Follow the current [verify-aelomi rules](../../../.claude/skills/verify-aelomi/SKILL.md#rules) instead.

Source commit `acd375ec26d319eabc912f3a7da29550f2a3b8c0`. Merge base against `origin/main` is `a39625b427c8675d4ca2a5ffa62bc8c1de2aa82e`. Verbatim patch-id is `967b11e1a2f9c1f93824ac6b1a9899e04aa56fe7`.

Proven pathspecs, from the repository root, are `src/storage-copy.ts`, `src/components/storage-refusal.tsx`, and `src/styles.css`. The app performs no folder movement. This record proves the manual recovery recipe that moves a user's workspace.

## Shape and scope

The existing `CloudRecovery` enum still chooses the recipe in Rust.
The frontend keeps the resolved path and ordered `RecoveryStep` union, with text steps or a Finder prompt, path, and action.
The unused configured-path field is removed from the frontend recovery model because no view reads it.

Step 2 explicitly opens a second Finder window before Go to Folder and names the destination as the window from step 1. Step 1, step 3, and the conditional rename are unchanged. All three variants say `Keep this window open until the last step.` This prevents an early restart from creating a destination that collides with the move. The duplicate URL-link CSS rule is merged into the shared link rule.

The PR body now says that paths through a shared parent link receive cloud-settings guidance, including `AELOMI_DATA_DIR` paths. Direct cloud overrides with no link receive environment guidance. No recovery selection or override behavior changes in this round. The deferred round-4 findings remain assigned to #87.

Model the Domain kept the existing enum and step union. Laziness Protocol removed the unused field and duplicate CSS. Test Behavior, Not Implementation kept literal recovery mappings and the acceptance screen assertions. Prove It Works requires the complete Finder move and an exact page-text assertion after relaunch. No Comments was applied directly within the round-4 scope because the builder cannot spawn subagents. It found no comments or suppressions to delete, no reshape flags, and no unenforced constraints.

## Automated proof

- `src/storage-copy.test.ts`, `maps safe recovery steps for $name`, covers the leaf link, differing names, shared parent, override, and debug name. The updated expectations failed on the old implementation. All 11 tests in the file pass afterward.
- `e2e/cloud-folder.e2e.ts`, `SHELL-11 acceptance 7: a cloud folder shows its resolved path and how to move it back`, passes with the new Finder prompt, drop target, keep-open reminder, exact chips, complete selection, and absent editor.
- `tests/playwright/storage-refusal.spec.ts`, `SHELL-11 light recovery chips wrap and select without automatic field sizing` and its dark counterpart, pass at 320 pixels and 200% text size.
- Mock run `~/.local/state/aelomi-verify/20261015-220118-mock-kFKW7T` passes all three variants with demo and worst-case data, light and dark, widths 320, 480, 960, and 1440, long names, Japanese text, emoji, and 200% text. Paths wrap without clipping or horizontal overflow. The proof-only fixture toggle is absent from production.

## Reproduce the native proof

(see the warning above)

Use macOS 27.0.1 on Apple Silicon. Run from the repository root. Do not launch a default Aelomi instance.

```bash
mkdir -p .local-gate/fix4
cp docs/evidence/pr86-fix4/generate-body.mjs.txt .local-gate/fix4/generate-body.mjs
cp docs/evidence/pr86-fix4/finder-proof.sh.txt .local-gate/fix4/finder-proof.sh
cp docs/evidence/pr86-fix4/seed.rs.txt crates/core/examples/pr86_fix4_seed.rs
node .local-gate/fix4/generate-body.mjs
BASE=$(mktemp -d /private/tmp/ae86-r4-XXXX)
printf '%s\n' "$BASE" > .local-gate/fix4/base.txt
mkdir -p "$BASE/local" "$BASE/cloud"
cargo run -p aelomi-core --features test-support --example pr86_fix4_seed -- "$BASE/cloud/Aelomi Notes" .local-gate/fix4/original.yupdate
rm crates/core/examples/pr86_fix4_seed.rs
ls -la "$HOME/Library/Application Support/Aelomi" > .local-gate/fix4/real-data-before.txt
bash .local-gate/fix4/finder-proof.sh > .local-gate/fix4/finder-proof.log 2>&1
```

The driver launches only through verify-aelomi, with the synthetic `AELOMI_DATA_DIR` and debug cloud root. Builds and launches precede the keyboard lock. Every key, Finder operation, and screenshot holds the lock. Traps restore appearance and clipboard, release the owned lock, clean up the owned app, and compare the final read-only real-data folder listing.

The driver reads each parent path verbatim from its native `AXTextArea` value before pasting it into Finder. It does not claim native click-to-select or focus-to-select proof. An earlier focus-and-copy attempt did not produce the expected path. A direct Finder AppleScript close command also stalled and was stopped. The final driver uses System Events keyboard commands instead.

## Native result

The successful refusal run is `~/.local/state/aelomi-verify/20261016-034555-dev-goFl`. The recovered run is `~/.local/state/aelomi-verify/20261016-034724-dev-Sk6i`. Both passed doctor. The synthetic root is `/private/tmp/ae86-r4-12pH`.

1. With every Finder window closed, Go to Folder opened the configured-parent chip. The `Aelomi` link was selected and moved to the Trash with Command-Delete. The cloud workspace remained intact.
2. Command-N opened the second Finder window. Go to Folder opened the cloud-parent chip. Both windows remained open. Finder Move Item Here, Command-Option-V, moved `Aelomi Notes` into the window from step 1. Finder renamed the folder `Aelomi`.
3. Every original workspace file matched its pre-move SHA-256. No original file was missing or changed. Finder added only `.DS_Store` metadata. The destination was a regular folder.
4. Command-Q quit the refused app. Relaunch through verify-aelomi used the same `AELOMI_DATA_DIR` and cloud root. The shell reported Ready, and an exact accessibility-text assertion found the original page body in the editor. A second Command-Q also completed the normal quit drain.

The local proof log ends with these assertions.

```text
PASS: Finder move and rename preserved every original workspace file byte for byte; only Finder .DS_Store metadata may be added
PASS: relaunched app shows the exact original page text: Original pages survived Finder recovery, round 4.
PASS: Finder recovery used no shell equivalent for any recovery step
```

The recovered shell logged `storage status=Ready path=/private/tmp/ae86-r4-12pH/local/Aelomi`. Both quits logged `quit flush via=CompleteQuit pending=None` and `quit confirmed via=drain`. The before-and-after `ls -la` outputs for the real data folder are identical. The keyboard lock is released and every owned app process is cleaned up.

Every per-step screenshot stays in the refusal run, with the recovered page screenshot in the recovered run. The initial Go to Folder screenshot uses Finder's column view and has its contents partly offscreen. Selecting the link scrolls it into view in `02-link-selected.png`. The later screenshots show the removed link, cloud source, moved folder, renamed folder, and exact original page text. All screenshots were inspected locally. No performance improvement is claimed.

A prior complete move stopped at an assertion that rejected Finder's added `.DS_Store` file. Its original files were all byte-identical. The successful proof used a fresh synthetic fixture and explicitly permits only that Finder metadata addition. Earlier automation attempts were stopped and cleaned up. None launched the default data folder.

## Rebase

Main advanced during self-proof when #85 landed. The branch was rebased onto `a39625b427c8675d4ca2a5ffa62bc8c1de2aa82e`. Conflict resolutions preserve the shared shortcut menu, its typed event and mock, and the storage startup gate. `pnpm bindings` regenerated an identical binding file, and typecheck passed. The final native proof was repeated on the combined source commit above.

An intermediate conflict-resolution script removed too much text. The first rebased commit was reconstructed from the original three-way sources before the rebase continued. Its delta is the original 524 insertions and 5 deletions. Comparing the final source to the previous head shows only #85's affected files. The refusal component, copy, CSS, storage selection, and recovery tests are byte-identical across that rebase.


## Published screenshots

[Post-rebase native self-proof](https://github.com/aelomi/aelomi/pull/86#issuecomment-6203440074). GitHub CLI 2.102.0 matched the cached published archive checksum. Every public attachment was downloaded without credentials and matched the original PNG bytes, size, and SHA-256. Raw logs and accessibility trees remain local.

| Screenshot | Bytes | SHA-256 |
| --- | ---: | --- |
| [leaf-dark.png](https://github.com/user-attachments/assets/98d9b634-909f-4978-a70b-9511027d8275) | 216427 | `76f86a5320cc0f7f9e1aedaac9c99eb34b4884038c42d8123c549b54540b23b9` |
| [leaf-light.png](https://github.com/user-attachments/assets/26cc3e77-be30-4259-9f5b-85b796a84dd6) | 229377 | `0a34eca483201d1060baf3486a7986f9de669560ca1261e7539aa5de3f35fbf3` |
| [02-link-selected.png](https://github.com/user-attachments/assets/1a50145c-49d3-426d-903a-ad094bbabe16) | 588337 | `c731cd19b81bb2e7d24acf56e8cb11a272044232f995ea1331d35b1e26f2162a` |
| [03-link-trashed.png](https://github.com/user-attachments/assets/6a8997ad-e88a-4a85-b302-ce70c7211c60) | 565702 | `610a133b91dfc9af21036bd9f39e70efb22daa7036fc860810632f2c36537dfa` |
| [04-second-window-cloud-parent.png](https://github.com/user-attachments/assets/5b8feecb-ee8f-453f-8f8c-6e0162fe55d1) | 523447 | `ea24484ac7e697dcf082c9ac3e67f682a0e39e2db8134f855ae1db9aecd73df2` |
| [05-folder-moved.png](https://github.com/user-attachments/assets/4254df67-182a-4fa8-b855-5772708ca31c) | 571173 | `d7f04dc207c8d15a9fe3881817f638781b12d16e8ac9c8c3db58dfc8895059bf` |
| [06-folder-renamed.png](https://github.com/user-attachments/assets/514dcdab-0ee6-42b4-8267-c5428c045d27) | 587191 | `af68da940791f8d4e6e42df6bfad756bfbf208debdc303d22cfef1e876c2f7f0` |
| [07-original-page-recovered.png](https://github.com/user-attachments/assets/e5e9642d-5714-4626-a17e-18e062768a90) | 87930 | `c38cf9634495df54373aa1993e007b238dad735d2b60bdd6043a83f1460027f4` |
