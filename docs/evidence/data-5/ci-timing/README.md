# LINK-8 CI timing correction

[The first nextest job](https://github.com/aelomi/aelomi/actions/runs/38468768976/job/115542119254) failed on `a2f8b821298ff211e7ccac62ce38c7250f15c4be`. The 5,000-block linking-edit test ran in the concurrent debug suite and measured a 310.059 ms maximum against the 250 ms LINK-8 target. Its five measurements were 217.743291, 221.084125, 138.137125, 149.358791, and 310.059 ms. The failure excerpt retains its output and the job's cancellation summary.

The same tests passed in a local isolated debug run on Apple M1 at 121.017667 ms for 5,000 blocks and 47.020625 ms for 200 blocks. This did not reproduce the hosted timing miss on local hardware. It confirms the debug and machine-load costs already flagged by Fable in [#66](https://github.com/aelomi/aelomi/issues/66). The release product measurement remains the relevant LINK-8 gate.

The correction changes tests and their execution, not product code. Both timing tests keep their exact 250 ms assertions and run as ignored tests in a mandatory isolated release step of the existing nextest CI job. `pnpm check:all` runs the same step. The ordinary debug suite retains the functional index tests. A continuous-edit test now keeps its producer running until the query returns exactly the expected `daily` page, then sends an explicit stop signal. It proves progress before editing stops without imposing a release performance deadline on the debug runner.

```bash
cargo nextest run --workspace --locked
cargo nextest run -p aelomi-core --cargo-profile release --locked --run-ignored only --test-threads 1 index::tests::latency::
```

The focused release run passed both timing assertions at 51.229833 ms for 5,000 blocks and 40.83 ms for 200 blocks. The focused continuous-edit test passed. The CI/local-command guard's two Vitest tests passed. The full local gate and fresh reviews follow on the committed correction.

The diagnostic runs added `--no-capture`, which itself serializes nextest. The mandatory CI command above uses `--test-threads 1` without that flag. Both select only the two LINK-8 tests in release. The source of the native app remains `6e7f10cddedf6169e392b613eb4539e890c7d047`; [its native proof](../README.md) remains valid because these changes affect no release app code. No history was rewritten.

Checkout prefixes are replaced with `<checkout>` and terminal color escapes are removed. Hashes cover the published bytes.

| File | Bytes | SHA-256 |
| --- | ---: | --- |
| [local-debug.log](local-debug.log) | 1596 | `2aded8863d41758c9ef07b6c11399f8b445113a10897bea05e96cb40a59f28c5` |
| [local-release.log](local-release.log) | 1647 | `a9b756a8a1c2835ef8be3263b0d39a73493d3cbe1b37d3f586a3713e95198969` |
| [continuous-edits.log](continuous-edits.log) | 830 | `44f3d02b33cb4ae05d651da85d793074d7a1b0c64464a2a2e0f1a5c8b7c65fcf` |
| [hosted-debug-failure.log](hosted-debug-failure.log) | 2453 | `d6aaf489a6d41b741a460603b1b560cf53239f950b1daa91c6b5781bfef62558` |
