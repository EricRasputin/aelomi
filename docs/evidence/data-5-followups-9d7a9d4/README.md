# DATA-5 follow-up native verification on 9d7a9d4

This record proves the listed native behavior on source commit `9d7a9d44acbe4a34d66bcf7f6c8edfb6fe6aa66c`. It also records a failed populated-workspace cold-start gate. This is an intermediate record, not a claim that the expanded PR is ready. Later product fixes need their own affected native verification.

The app was Aelomi 0.1.0, built with `pnpm tauri build --bundles app`, on an Apple M1 with eight CPUs and macOS 27.0.1, build 26A434. Every driven run used the release Tauri app, real IPC and an isolated synthetic `AELOMI_DATA_DIR`. No durability interposer was loaded. `verify-aelomi` supplied PID-bound Accessibility actions, doctor, window screenshots and cleanup. Every required screenshot was inspected at its captured resolution of 1920 by 1280. Personal path prefixes in the text were replaced with `<worktree>` or `<user>`.

## Missing, garbage and old-schema index recovery

The release `seed_index` example seeded the same nested, favorite, trashed, erased, linked and text-bearing workspace in three new folders. Its modes were `missing`, `garbage` and `old-schema`. Each folder was opened with `app.sh launch app`. After doctor passed every check, the page was captured, then Command-K and the literal query `Aelomi` were sent through System Events. The palette was captured after the query.

| Case | Native log | Visible page and palette |
|---|---|---|
| Missing | `Rebuilding`, `cache=created`, `Ready`. Worker startup 37.964 ms. | Projects showed its text and caption. Aelomi appeared in Pages; Daily and Roadmap appeared in Content with the matching link labels highlighted. |
| Garbage | `Rebuilding`, `cache=replaced reason=not-a-database`, `Ready`. Worker startup 13.756 ms. | The same page and expected title/content matches, without an index error. |
| Old schema | `Rebuilding`, `cache=replaced reason=schema-version (found 0, expected 2)`, `Ready`. Worker startup 25.425 ms. | The same page and expected title/content matches, without an index error. |

The before/after source manifests compare exactly in each case. A read-only SQLite check after cleanup asserted schema version 2, `integrity_check == [("ok",)]`, and exact equality of every row in `pages`, `title_text`, `blocks`, `block_text` and `links` across all three native cases. [native-cache-equality.json](native-cache-equality.json) records those synthetic rows. This supplements the core API rebuild/search/backlink assertions; it does not replace them.

Missing and old-schema runs exited through normal Command-Q. Their complete logs show the revisioned frontend drain, `CompleteQuit pending=[]`, confirmation and final `Exit pending=[]`. The garbage run sent Command-Q but called cleanup before that event was processed. It proves recovery and source preservation, not normal quitting.

The first missing-index attempt lost its launch process between `launch` and doctor. Doctor failed process/window checks. It was cleaned up without being driven. Its full log and doctor result remain under `discarded-launch-*`. Subsequent launches kept their owning terminal session open and passed doctor. The discarded attempt supplies no native verification claim.

The small rebuilds finished before the palette opened. These screenshots show ready results, not a visible `Indexing…` interval.

## Unsupported null-valued mark attribute

A private Rust helper appended a synthetic raw page update containing a bold mark with `futureField: null` through the real Store. The page was titled Future formatting. After doctor passed, the real app showed the read-only notice and the literal paragraphs `plain bold tail` and `other block`. Its Accessibility tree had no editable Page body.

Typing `MUTATIONSHOULDNOTAPPEAR` through the native keyboard path changed neither visible content nor any source-file hash. The before and after PNGs are byte-identical. Normal Command-Q drained and exited. On relaunch, doctor passed again and [unknown-mark-reopen-reopen.ax.txt](unknown-mark-reopen-reopen.ax.txt) showed the same notice and paragraphs. The post-relaunch source manifest still equals the original exactly. This verifies that the guard preserves unsupported formatting before binding.

## Native 10,001-page workspace

The example seeded `search-10k --big-page`, which contains the checked-in 10,000-page search fixture plus the checked-in 5,000-block page. It wrote 10,003 updates and 12,564,561 source bytes in 10,005 files. The fixed fixture CRC-32 was `48c7dc26`. The seed created no index.

The real app's log shows the source-backed `page_tree` and first `doc_load` before `index status=Ready`. The tree reply contained 9,701 active pages and 1,756,818 JSON bytes. Its measured IPC time was 80.926 ms. The first body load was 0.439 ms for 512 base64 bytes. The worker rebuilt in 3,996.709 ms. Its log separates schema/open, source loading, decoding, projection and SQL work.

`main_peak_rss_bytes=297189376` is the macOS kernel's main-process resident-memory peak from process launch through rebuild completion, about 283 MiB. The independent 100 ms sampler took 200 samples after attachment and observed 288,368 KiB. Both measurements exclude WebKit helper processes. The sampler is not a replacement for the kernel peak.

After doctor passed, `roadmap` correctly had no content matches in this generated fixture. Replacing the query with `kestrel` showed Kestrel field notes in Pages and matching generated content in Content, with prefix highlights. Both screenshots were inspected. Normal Command-Q drained and exited before the cold-start harness ran.

Seeding and native loading logged `compacting big-page failed: the snapshot read back differs from the doc`. The Store retained the source history. This is a reproduced compaction verification failure, not a reproduced data-loss result. The complete seed and shell logs retain it; the separate compaction follow-up must describe the observed failure without claiming a proven cause.

## Populated cold-start gate failed

The unchanged native harness ran against that disk folder with `--expected-variant page`. It required the first editable page content frame, discarded one warm-up, then timed ten new app processes. All ten exited with code 0 and reported the page variant. No browser suite, build or other Aelomi run overlapped it.

The median was **618 ms**, with a range of **612 to 639 ms**. Navigation-to-paint was 257 ms median, with a range of 253 to 272 ms. The local 500 ms budget failed. The hosted budgets passed as information only. Load was 2.92 before and 3.90 after on eight CPUs, below the noise threshold. [cold-10k.log](cold-10k.log), [cold-results.json](cold-results.json) and every raw per-run report preserve the failure.

This result is not covered by the maintainer's small typing tolerance. Startup remains a 500 ms requirement. A focused startup fix and fresh native verification are pending. This record makes no 100 ms physical durability, native IME, visible indexing-state or final-head approval claim.

## Publication and manifests

[text-manifest.json](text-manifest.json) records the byte sizes and SHA-256 values of every companion text artifact. It excludes itself and this README to avoid recursive hashes. [screenshots.json](screenshots.json) records all ten required PNGs. They remain retained locally for attachment to PR #68 after the final local gates. Their attachment comment must name these manifest filenames. Anonymous downloads, byte/hash checks and independent image inspection remain required before PR review. No required screenshot may be discarded while publication is pending.

Raw Y.Docs, SQLite files, `run.env`, PID files and other app data remain local. Original published DATA-5 and SHELL-10 evidence stays unchanged.
