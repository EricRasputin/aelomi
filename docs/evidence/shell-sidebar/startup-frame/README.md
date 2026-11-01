# Visible empty-state startup frame

This follow-up proves source commit `53c0c104440fc0c39811b3c9351e654e8574a423` against base `53b8763c1ae1b4a46d193113fe67767bbde72578` from `origin/feat/shell-page-records`. The patch-id is `6e019aa6c8ef62a5b3c225926204354458a20b66`. Its proven pathspec is `src/router.tsx`.

The new empty state initially renders while the source tree loads. Its startup mark now waits until the tree leaves Loading, so the measured index frame contains the empty state and its New page button. This changes the startup clock, not the create, rename, trash, or disclosure behavior proven in the parent record.

The release app was rebuilt on the same Apple M1 and macOS 27.0.1 environment. A fresh synthetic data folder copied `perf/fixtures/empty-workspace`, so the first launch had no active pages and did not create Getting started. The app launched through verify-aelomi with AELOMI_DATA_DIR set to that folder. Doctor passed. The keyboard lock covered inspection, capture, and the normal quit.

The native run is `~/.local/state/aelomi-verify/20261016-041936-app-VkKU`. Its exact accessibility assertions reported `PASS stored empty workspace opens the empty state`. The capture shows No page open and New page. Quit reported these lines.

```text
quit draining via=ExitRequested revision=1
quit flush via=CompleteQuit pending=None
quit confirmed via=drain
quit allowed via=ExitRequested confirmed=true
quit flush via=Exit pending=None
```

The read-only real Aelomi folder listing remained identical after this last native run. The lock was released and the owned app process was cleaned up.

[The screenshot comment](https://github.com/aelomi/aelomi/pull/89#issuecomment-6203910663) attaches `empty-first-launch.png`. It is 88,779 bytes with SHA-256 `2ac89f65a70d82441ce18bdbb8a88811891d86fe9df9b3062d7971d1ff680123`. The public download was checked without a token and matched the inspected local PNG byte-for-byte.

`run.sh.txt` repeats the synthetic setup, launch, lock, doctor, capture, and quit. Copy the parent record's keyboard-lock helper to `/tmp/aelomi-d1-lock.py` before running it. The final committed-head check:all run also exercises the native cold-start variants with this clock.
