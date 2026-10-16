# PR 86 final copy and proof continuity

This record's appearance-capture recipe changes the Mac's appearance and must not be rerun as is. Follow the current [verify-aelomi rules](../../../.claude/skills/verify-aelomi/SKILL.md#rules) instead.

Source commit `e5a94111548e817fc6f7a36f0cf9e4b540b168c5`. Merge base `a4db19d8ab2e1ac072dea8bc3a1cc3747af8fdae` against `origin/main`. Patch-id `71adac6729fae82c18a2d3b339217ba0866a7937`. The proven pathspecs are `src/` and `src-tauri/`.

This record supplements the [recovery and link self-proof](https://github.com/aelomi/aelomi/blob/96cdc94ccb48be742d5bdc907de1f609f4a747fe/docs/evidence/pr86-fix1-native/README.md). Comparing its source commit `06079e2917eee86acd7cc40dae939a9825252d24` with this source under both pathspecs yields exactly `src/storage-copy.ts`. All other app source is byte-identical. The prior packaged URL, context-menu, exact quit-survival, and recovery tests therefore prove the same app code. The original record keeps their scripts, run paths, log excerpts, screenshot sizes, and hashes.

## Final refusal proof

The removal instruction now applies to a symbolic link into cloud storage at the configured path or a parent. It preserves the target folder. Harmless local links such as macOS's `/tmp` redirect are outside that instruction.

Fresh synthetic run `~/.local/state/aelomi-verify/20261015-235949-dev-rnpA` passed doctor. `AELOMI_DATA_DIR` was `/private/tmp/aelomi-pr86-fix1-cloud/configured-data`, a link to `/private/tmp/aelomi-pr86-fix1-cloud/cloud/data`. `AELOMI_CLOUD_ROOTS` was `/private/tmp/aelomi-pr86-fix1-cloud/cloud`. The startup line reports both configured and resolved paths. The screenshots show the complete paths, Finder's Go to Folder, the cloud-specific link instruction, the overwrite precaution, and reopening. Both appearances remain readable. The original dark appearance was restored and the instance was cleaned up.

Reproduce with the earlier record's synthetic folder recipe and app.sh commands. No real data folder is used. Raw logs and accessibility trees remain local.

[Final refusal screenshots](https://github.com/aelomi/aelomi/pull/86#issuecomment-6199403774) were uploaded with a checksum-verified GitHub CLI 2.102.0. Both URLs were downloaded without authentication, opened with sips, and compared byte for byte with the local PNGs.

| Screenshot | Bytes | SHA-256 |
| --- | ---: | --- |
| [cloud-dark.png](https://github.com/user-attachments/assets/d6b10fca-6fb7-4e2a-a1ca-2528ac3071e1) | 240094 | `b565e983bda8a183c3eb7b6fa9683ddf3a4eaf077ccbcc1529ca6721ae77b280` |
| [cloud-light.png](https://github.com/user-attachments/assets/2b79fdc3-4877-4bbe-9178-3e15d3ae7341) | 248912 | `d89b2210ef43c5e8d405a64a0607b8b124ced814bb33b2f0114a5e43233b756a` |

## Verification isolation and limits

The first full gate passed the main and release Rust checks and all 18 acceptance tests, then stopped because another worktree owned Playwright's port 4173. `AELOMI_TEST_PORT` now overrides the port for local isolation while preserving 4173 by default. All three save-failure WebKit cases passed on port 54886. The final committed-head gate runs with `AELOMI_TEST_PORT=54886` and retains every CI stage and budget.

Native programmatic top-level navigation remains unverified for the inspector precondition described in the earlier record. A physical force click was not synthesized. The packaged context-menu route was proven with an empty request counter and a successful browser positive control. No review gate or independent verifier was run by this builder.
