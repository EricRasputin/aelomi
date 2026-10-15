# Hosted CI measurements for DATA-5

This folder keeps only this README and its scripts, under the [evidence policy](../../native-verification-evidence.md). Every file it names is in the [complete record at 9b44d57](https://github.com/aelomi/aelomi/tree/9b44d575e43efa1170b743cc54b019b5faf7a8cf/docs/evidence/data-5-ci-calibration-0ad6f34).

Both attempts ran unchanged head `0ad6f34dd1b931715e4a086bf3b43e2cde85e582` in [Actions run 38650502888](https://github.com/aelomi/aelomi/actions/runs/38650502888). The product source remains `9d7a9d44acbe4a34d66bcf7f6c8edfb6fe6aa66c`. The second attempt reran the failed jobs. These failures remain failures under their original thresholds.

The jobs used hosted `macos-15` Apple Silicon machines with three CPU cores and Node 24.20.0. Search used the release Rust profile and SQLite 3.53.2. Browser timing used untraced Playwright WebKit over the actual Yjs-bound editor and the 10,000-page palette fixture. The two browser jobs exhausted their 120-second quiet-machine wait and measured anyway. The noise label did not alter any verdict.

| Measurement | Attempt 1 | Attempt 2 | Provisional hosted ceiling | Local gate |
|---|---:|---:|---:|---:|
| Worst fresh-reader query p95 | 27.137709 ms | 30.837625 ms | 35 ms | 5 ms |
| Worst steady query p95 | 5.006167 ms | 8.418250 ms | 35 ms | 5 ms |
| Real 5,000-block typing, pooled app-owned p95 | 33 ms | No series, setup failed | 45 ms | 19 ms, full interval |
| Palette's recorded interval p95 | 29 ms | 36 ms | 40 ms | 16 ms |
| Scroll rendering work p95 | 13 ms | 14 ms | Below 16.7 ms | Below 16.7 ms |
| Longest scroll interval | 24 ms | 35 ms | 50 ms | 50 ms |

Each search job executed all 192 complete-query cases, with 30 fresh-reader and 200 steady queries per case. Opening the reader, statement preparation, tokenizer setup, result selection and selected-text highlights remain inside the first-query clock. The gate consumes the returned output. Both jobs also passed all 331 ordinary Rust tests and both serial 250 ms backlink tests. [search-comparison.json](https://github.com/aelomi/aelomi/blob/d37c4ec4e17139cbbdfe8929d1fe3f0fcb319878/docs/evidence/data-5-ci-calibration-0ad6f34/search-comparison.json) identifies the worst cases and every gate's load context. The query jobs began at one-minute loads of 31.03 and 11.32, and ended at 19.06 and 10.07.

The first real 5,000-block typing series kept 405 samples from three fresh page loads. Its per-load app-owned p95 values were 31, 30 and 41 ms. Full key-to-paint p95 was 36 ms. Every character and sample-count guard passed. The second job failed `page-editor` visibility after five seconds, before collecting this surface. Its captured page state contains the heading and body projection. That state does not prove that editing was ready at the timeout. [editor-setup-error-context.md](https://github.com/aelomi/aelomi/blob/feff6f7571f050a68b2107f3f67a6213a64fddab/docs/evidence/data-5-ci-calibration-0ad6f34/editor-setup-error-context.md) retains the error and excerpts from the captured page state, with the repeated fixture text explicitly omitted.

Both palette jobs measured 55 query keys. Scroll counted 300 and 298 frames. Synthetic typing surfaces remain diagnostic information and do not supply the actual editor's calibration.

The hosted ceilings round the worse observed search and palette p95 values upward. Typing rounds the worst measured load's 41 ms p95 upward by four milliseconds. These are provisional regression ceilings for the measured hosted environment. Two jobs and one completed large-page typing series do not establish a baseline across five independent hosted machines. No hosted CPU or I/O profile identified the cause of individual long tails. [Issue #72](https://github.com/aelomi/aelomi/issues/72) tracks broader calibration and complete presentation timing.

Local search remains 5 ms, palette remains 16 ms and typing remains at the maintainer-authorized 19 ms ceiling with its original 16 ms goal recorded. Storage acknowledgement remains 100 ms and backlinks remain 250 ms. Both scrolling limits and the existing hosted cold-start limits remain unchanged. The maintainer-accepted populated startup median of 618 ms remains in its separate native record.

The typing clock excludes initial layout before WebKit freezes the rAF timestamp, scheduler and IPC delay, frame wait, and presentation. The palette reports its existing key-to-settled-frame recorder interval. These measurements supply no native full-paint or physical durability claim.

Vitest exhausted its 2 GB JavaScript heap in `src/editor/y-binding-oracle.test.ts` in both jobs. The second job also timed out the 1 MB base64 round-trip test after five seconds. The full logs retain both failures. Test fixes are separate from the performance calibration.

## Binding-oracle memory fix

The unchanged oracle also reproduced the OOM locally with this command:

```sh
NODE_OPTIONS=--max-old-space-size=2048 pnpm exec vitest run src/editor/y-binding-oracle.test.ts
```

The raw ProseMirror test view rendered page links through the schema's `toDOM`. That path discarded the renderer's `destroy`, leaving page-tree subscriptions that retained closed editor sessions. The app's editing view uses BlockNote's node view instead. An instrumented private test copy compared the two paths at an 8 GB diagnostic heap. It observed 80 and 200 retained subscriptions for repeated rich-body and real-fixture sessions through `toDOM`, and zero through the node view. [oracle-render-mechanism.log](https://github.com/aelomi/aelomi/blob/1d08e9ae25a8f37c5490c207b07d9a94dc31bc81/docs/evidence/data-5-ci-calibration-0ad6f34/oracle-render-mechanism.log) records that isolated diagnostic. It is not a full-suite pass or an app measurement.

The test now registers a destroyable page-link node view using the same schema render output and removes each closed view's empty mount. All editor-state, Yjs-update, relative-selection and undo assertions remain unchanged. The 5,000-block fixture is unchanged. [oracle-red-2g.log](https://github.com/aelomi/aelomi/blob/feff6f7571f050a68b2107f3f67a6213a64fddab/docs/evidence/data-5-ci-calibration-0ad6f34/oracle-red-2g.log) retains the original failure. Five `oracle-green-2g-*.log` files record five serial passes of all eight tests at the same 2 GB limit. No production renderer changed and the heap limit was not raised. Both the local `check:all` Vitest stage and the hosted workflow now set that limit explicitly.

The separate base64 test uses Node's native byte comparison for its random-array round trip. It still compares every byte and the array length. Production encoding is unchanged. The committed-head full gate validates the complete suite with both test fixes at 2 GB.

The source review also found that production copy, drag or HTML-export rendering may discard page-link cleanup. That production retention has not been measured. [Issue #71](https://github.com/aelomi/aelomi/issues/71) tracks the lifecycle investigation separately.

## Editor fixture readiness

The perf test now waits up to 60 seconds for the surface to become visible and explicitly have `contenteditable="true"`. This deadline applies before focus, settling and timed input. It does not add a sleep or alter the sampling clocks, the 450 sent keys, 405 retained samples or exact-character guards. A page that stays read-only still fails. The deadline matches the palette fixture's setup deadline.

The focused real 5,000-block test passed in local product mode. Its three loads became editable after 2,207, 1,691 and 1,724 ms. Every load's full typing p95 was 18 ms, below the retained 19 ms local ceiling. Pooled app-owned p95 was 17 ms. The synthetic empty surface also passed the readiness guard. `setup-local-5000-green.log` and `setup-local-synthetic-green.log` retain those results, and `setup-red-ci-excerpt.log` retains the original setup failure. The exact reason the hosted editor missed five seconds remains an inference from its load context and later captured page state. No production loading or editor code changed.

The six complete job logs preserve setup, failures and cleanup. Terminal color escapes, byte-order marks and trailing line whitespace were removed; no relevant result was removed. Runner paths refer to disposable public CI machines. Private oracle-log worktree prefixes were replaced with `<worktree>`. `editor-setup-error-context.md` is an explicit excerpt, not the complete 5,000-block Playwright dump. The full private artifact was retained during diagnosis. App data and generated HTML reports are not committed. [files.sha256.json](https://github.com/aelomi/aelomi/blob/d37c4ec4e17139cbbdfe8929d1fe3f0fcb319878/docs/evidence/data-5-ci-calibration-0ad6f34/files.sha256.json) records the published diagnostic files' byte sizes and hashes, excluding this README and the manifest itself. On 2026-10-15 the two `log` fields in `search-comparison.json` were changed from private temporary filenames to the committed `nextest-attempt-1.log` and `nextest-attempt-2.log`, matched by each attempt's gate context, and the manifest was regenerated (issue #76). No measured value changed. Both links point at that version.
