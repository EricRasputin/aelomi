# PR 86 round-2 cloud recovery proof

Source commit `dab9ad9a934e6092d3e56a41f77973e10654d312`. Merge base `a4db19d8ab2e1ac072dea8bc3a1cc3747af8fdae` against `origin/main`. Patch-id `004072f8ebf01ef9e2aa772814e49de9fdec6793`.

The proven pathspecs are `crates/core/src/storage/data_path.rs`, `src-tauri/src/storage.rs`, `src/ipc/bindings.ts`, `src/storage-copy.ts`, `src/components/storage-refusal.tsx`, and `src/styles.css`.

## Native refusal

The environment was macOS 27.0.1 on arm64. The synthetic run `~/.local/state/aelomi-verify/20261016-005527-dev-yF17` passed every doctor check. The configured path was `/private/tmp/ae86-r2-s5yv/Support/Aelomi Dev`. `Support` was a symbolic link to the synthetic `cloud` folder. The shell printed:

```text
storage status=CloudPath { configured: "/private/tmp/ae86-r2-s5yv/Support/Aelomi Dev", resolved: "/private/tmp/ae86-r2-s5yv/cloud/Aelomi Dev", link: Some("/private/tmp/ae86-r2-s5yv/Support") } path=/private/tmp/ae86-r2-s5yv/cloud/Aelomi Dev
ping seq=1
```

Both screenshots show the configured and cloud paths, the exact parent to open in Finder, and the `Support` item to trash. The copy says that the cloud target stays intact and that the screen stays open during the move. It gives an occupied-destination remedy. At the default window height, the remaining move and restart steps continue below the fold. The accessibility snapshots include the final `Quit Aelomi and reopen it.` step. No editor is mounted.

The original dark appearance was restored, and the instance was cleaned up. The target still contained exactly one file, `sentinel`, with the unchanged bytes `synthetic cloud data stays intact\n`. Aelomi created no storage or index files there. The [native screenshots comment](https://github.com/aelomi/aelomi/pull/86#issuecomment-6200524269) contains both images. Each published URL was downloaded without authentication, opened, and compared byte for byte with its local PNG.

| Screenshot | Bytes | SHA-256 |
| --- | ---: | --- |
| [cloud-dark.png](https://github.com/user-attachments/assets/8791c286-5ffa-4ef0-8264-eb554f83a1e1) | 252916 | `7febfbf848b1e843c812beb6f40878d26d46c741d35924f15b46ddd4e8bcdc9a` |
| [cloud-light.png](https://github.com/user-attachments/assets/fde5e65e-39d2-4983-a1de-986b9946900a) | 270788 | `8b56816187e65b64c2adf28cf37c5251af3ab3745251406d5b04f77160bc21bd` |

## Recovery cases and design checks

`src/storage-copy.test.ts` asserts the exact recovery mapping for a link at the data folder, a link at a parent, a direct override, and the debug build path. Acceptance 7 asserts the visible parent-link instructions and that quit and reopen is the last step. The shell test `cloud_status_names_a_parent_link_or_a_direct_override_without_changing_data` checks the serialized metadata and exact unchanged sentinel bytes. The core test `reports_the_first_cloud_link_component_and_skips_local_aliases` covers local aliases, a dangling leaf link, a parent link, missing components followed by `..`, and a direct path.

The Emil and Apple checks kept the refusal static, with system typography and the existing light and dark color tokens. The copy names the item before asking the user to remove it.

| Before | After | Why |
| --- | --- | --- |
| Quit before reading the paths | Keep the screen open and restart last | The instructions stay available during recovery |
| Remove an unspecified link | Name its parent, item, and arrow badge | Finder opens the parent rather than following the link |
| Warn against an occupied destination | Move the existing folder aside first | Both sets of data are preserved |
| Show override steps on every refusal | Select steps from nullable link metadata | Each case receives only its applicable instructions |
| Muted recovery steps | Foreground recovery steps | The actions have the strongest reading hierarchy |

Mock run `~/.local/state/aelomi-verify/20261015-192506-mock-9vaI3O` passed 320, 480, 960, and 1440 px in both themes, plus 200% text at 480 px. Long paths with Japanese text, emoji, and unbreakable folder names wrapped without horizontal overflow. The final step remained reachable by scrolling. The local flow's demo toggle was never added to product code. Screenshots, layout results, and raw logs stay in that run directory.

`cloud-proof.sh.txt` reproduces the synthetic native setup. `layout.mjs.txt` contains the mock layout flow. This record covers cloud recovery. URL gesture proof is reported separately on the PR.
