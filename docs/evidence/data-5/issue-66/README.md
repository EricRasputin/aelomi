# Issue 66 verification

[Issue 66](https://github.com/aelomi/aelomi/issues/66) is addressed in PR 68. The release LINK-8 gate keeps both 250 ms assertions. The functional test now proves multiple appends complete while the index worker is held, then checks that search returns exactly the `daily` page before the producer stops. The producer returns its accepted edit count, which must exceed one. On channel failure, the test releases the worker, stops the producer, and resumes the producer's original panic if one occurred.

Two temporary test mutations exercised the failure paths. An injected panic before the producer's first append failed with `issue66 injected producer panic`, without a bare `RecvError`. A producer that exited after one accepted edit failed with `producer accepted 1 edits while the index was held`. Both mutations were removed and the complete test source was verified against its saved bytes before the final focused test passed. They are diagnostic probes, not shipped code.

The previous correction at `e2dc56de7844ee4885da5ceff4ef8bb3c570e86a` also contained an invalid YAML scalar ending in an unquoted colon. [GitHub rejected the workflow before creating jobs](https://github.com/aelomi/aelomi/actions/runs/38472117888). The release filter now ends in `latency` rather than `latency::`, selecting the same two tests. A new Vitest guard parses the actual workflow with the system Ruby YAML parser. It rejects the previously pushed workflow and accepts the corrected one. The two existing guards still check that CI and local job names and commands match. No dependency was added.

Focused commands:

```bash
cargo nextest run -p aelomi-core --locked -E 'test(continuous_edits)' --no-capture
pnpm exec vitest run scripts/check-all.test.ts
cargo nextest run -p aelomi-core --cargo-profile release --locked --run-ignored only --test-threads 1 index::tests::latency --no-capture
```

The final producer test passed. All three workflow guards passed. The focused release tests measured at most 46.099875 ms on 5,000 blocks and 33.498041 ms on 200 blocks, below 250 ms. `--no-capture` serializes the diagnostic run; CI uses `--test-threads 1` without it. The full local gate and fresh Fable reviews follow on the committed head.

These changes affect tests, CI checks, and documentation. Runtime modules, dependencies, IPC types, and UI code are unchanged. The native source commit remains `6e7f10cddedf6169e392b613eb4539e890c7d047`, so [its existing native proof](../README.md) still applies. Previous evidence and SHAs remain in history.

Personal checkout prefixes are replaced with `<checkout>`. Terminal color escapes were removed. The hashes cover those published bytes. `producer-panic.log`, `single-edit.log`, `yaml-before.log`, and `yaml-guard-before.log` record expected diagnostic failures.

| File | Bytes | SHA-256 |
| --- | ---: | --- |
| [producer-progress.log](producer-progress.log) | 830 | `156565892613f45cf697c52dfbc3704dd4d33d9b273e2c948b71a12d4e986d9f` |
| [producer-panic.log](producer-panic.log) | 1315 | `2888c7f76ed426d7726615f72d48b4a9069c20e7a21af254ff597eebed48bd1b` |
| [single-edit.log](single-edit.log) | 1441 | `e5465618c7280360c8a038cafd7e616a9f4d30042d6614fcb3f71d9a1908b75d` |
| [yaml-before.log](yaml-before.log) | 440 | `b4cc636dcab2bfd17d932205b6c2cc4ab362454eb743ba36e4a9aa278587e6fb` |
| [yaml-after.log](yaml-after.log) | 21 | `55f5d0210da4c1f8d08bc76a4133a810979ddd34e94bb30c324ad52926614399` |
| [yaml-guard-before.log](yaml-guard-before.log) | 1410 | `18524b153c23a515b5b5959b2efd736fd5a301abe8865150adeff163f56f765a` |
| [yaml-guard-after.log](yaml-guard-after.log) | 170 | `5cad3af76d9de1e710be75e36c4b7f6853e6148af743de1a7a772a035122adcf` |
| [release-linking.log](release-linking.log) | 1585 | `c203e0bfeda6f52318277c119288739b2c1651c4ed4b6706c0e052d35180146a` |
