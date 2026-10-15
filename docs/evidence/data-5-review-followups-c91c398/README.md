# DATA-5 review follow-ups: #78 retry waits on c91c398

This folder keeps only this README and its scripts, under the pstack-aligned evidence policy. Every file it names is in the [complete record at b76decc](https://github.com/aelomi/aelomi/tree/b76deccdc67efdd777464b19dae7d914caa6ea77/docs/evidence/data-5-review-followups-c91c398).

This record proves source commit `c91c398`, which changes how the index worker waits between repairs (#78). Earlier, every retry wait polled the clock every 10 ms. Now a wait on the system clock sleeps until its deadline or until it is notified, and only a manual test clock polls. The local Fable review of `1854fb7` raised it. The [d37c4ec record](../data-5-review-followups-d37c4ec/README.md) still proves every other issue, because no other product code changed.

It also keeps the failed first `check:all` attempt on `1854fb7`, which that review asked to have committed.

## Environment

- **Machine.** The same Apple M1 MacBook Pro, macOS 27.0.1, on AC power.
- **Build.** The app is the release `.app` that `pnpm tauri build --bundles app` built from `c91c398` at 14:17 IST. Its binary hash is in [d78b-binary.sha256](https://github.com/aelomi/aelomi/blob/b76deccdc67efdd777464b19dae7d914caa6ea77/docs/evidence/data-5-review-followups-c91c398/d78b-binary.sha256).
- **Data.** The run used a new synthetic data folder written by `seed_index valid` ([d78b-seed.log](https://github.com/aelomi/aelomi/blob/b76deccdc67efdd777464b19dae7d914caa6ea77/docs/evidence/data-5-review-followups-c91c398/d78b-seed.log)).
- **Redaction.** Personal path prefixes are redacted as in the earlier records.

## #78, faults that recur after Ready

[d78b-proof.sh.txt](d78b-proof.sh.txt) repeats the d37c4ec record's procedure:

- **Fault.** With ⌘K open on `cafe`, it corrupted the cached title of `reading` with `sqlite3`, eight times. Each fault came after the index logged Ready, and a query hit the corrupt row ([d78b-tamper.log](https://github.com/aelomi/aelomi/blob/b76deccdc67efdd777464b19dae7d914caa6ea77/docs/evidence/data-5-review-followups-c91c398/d78b-tamper.log), [d78b-actions.log](https://github.com/aelomi/aelomi/blob/b76deccdc67efdd777464b19dae7d914caa6ea77/docs/evidence/data-5-review-followups-c91c398/d78b-actions.log)).
- **Window.** It activated the app by bundle ID first, so the window was on screen. The doctor checks all passed ([d78b-doctor.txt](https://github.com/aelomi/aelomi/blob/b76deccdc67efdd777464b19dae7d914caa6ea77/docs/evidence/data-5-review-followups-c91c398/d78b-doctor.txt)).

**Results** ([d78b-shell.log](https://github.com/aelomi/aelomi/blob/b76deccdc67efdd777464b19dae7d914caa6ea77/docs/evidence/data-5-review-followups-c91c398/d78b-shell.log)):

- **First fault.** The cache was repaired at once: `cache=replaced reason=corrupt (field)` at 15,722.6 ms, 6.3 ms after Rebuilding.
- **Later faults.** Each logged `index status=Failed { reason: "the index was repaired repeatedly: corrupt (field)" }`. The repair followed the Failed status by 111, 214, 411, 808, 1,610, 3,210 and 6,413 ms, which matches the 100 ms doubling schedule. On `d37c4ec` the same intervals were 108, 211, 408, 811, 1,611, 3,210 and 6,411 ms. Each wait ended at its deadline, not on a poll.
- **During the last wait.** [d78b-failed-waiting.ax.txt](https://github.com/aelomi/aelomi/blob/b76deccdc67efdd777464b19dae7d914caa6ea77/docs/evidence/data-5-review-followups-c91c398/d78b-failed-waiting.ax.txt) shows the alert text.
- **Recovery.** After tampering stopped, [d78b-recovered.ax.txt](https://github.com/aelomi/aelomi/blob/b76deccdc67efdd777464b19dae7d914caa6ea77/docs/evidence/data-5-review-followups-c91c398/d78b-recovered.ax.txt) shows the results again.
- **Source.** After ⌘Q (`quit flush via=Exit pending=[]`), every source file hashed the same as before launch ([before](https://github.com/aelomi/aelomi/blob/b76deccdc67efdd777464b19dae7d914caa6ea77/docs/evidence/data-5-review-followups-c91c398/d78b-source-before.sha256), [after](https://github.com/aelomi/aelomi/blob/b76deccdc67efdd777464b19dae7d914caa6ea77/docs/evidence/data-5-review-followups-c91c398/d78b-source-after.sha256)).

**What the wait change does.** The unit test `a_retry_wait_on_the_system_clock_sleeps_until_its_deadline` counted 9 clock checks in one 100 ms wait before the change. After it, the count is at most 2, and that held over 20 repeats.

**Screenshots.** The three PNGs from this run are byte-identical to `d78-before.png`, `d78-failed-waiting.png` and `d78-recovered.png` in the d37c4ec record. Those are already attached to the pull request.

| File | Bytes | SHA-256 |
|---|---|---|
| d78b-before.png | 101823 | 9cce0b7bd6425092a7b1c79a96aece8adceb43b970a8fe839d9ad6b4839d573a |
| d78b-failed-waiting.png | 112688 | 335bb781e16c680a293fc9064ee8b8a89ae902ac6a5e14eb4e35960b78856d9c |
| d78b-recovered.png | 102037 | e3cda899c26939131f49f9a74b7664c877156ebba4a2405ff2a5255eea46d50b |

The same states render the same pixels. This run's freshness comes from its own window ID 33501, PID 3629, timestamps and shell log.

## check:all on 1854fb7

**Attempt 1 failed at `keystroke`** ([check-all-1854fb7-attempt1-keystroke.log](https://github.com/aelomi/aelomi/blob/b76deccdc67efdd777464b19dae7d914caa6ea77/docs/evidence/data-5-review-followups-c91c398/check-all-1854fb7-attempt1-keystroke.log)):

- **The failure.** The scroll spec's did-it-happen guard needs more than 200 frames in 5 s and got 150, because WebKit ran rAF at 30 Hz. The idle cadence was 33 ms from the third load of `page-5000-blocks` onward.
- **The budgets.** Every timing budget in that run passed, including scroll work p95 13 ms and the longest interval 34 ms.
- **Afterwards.** The scroll spec alone ran at 60 Hz with 300 frames. No low power mode, thermal warning or display sleep was recorded. The cause of the 30 Hz episode was not found.

**Attempt 2 passed** ([check-all-1854fb7.checks.json](https://github.com/aelomi/aelomi/blob/b76deccdc67efdd777464b19dae7d914caa6ea77/docs/evidence/data-5-review-followups-c91c398/check-all-1854fb7.checks.json)). That run's record gated the local review.

[files.sha256.json](https://github.com/aelomi/aelomi/blob/b76deccdc67efdd777464b19dae7d914caa6ea77/docs/evidence/data-5-review-followups-c91c398/files.sha256.json) records the bytes and SHA-256 of every file here except this README and the manifest.
