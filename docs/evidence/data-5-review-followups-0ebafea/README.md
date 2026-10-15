# DATA-5 review follow-ups: calibrated layout-delay control

This folder keeps only this README and its scripts, under the pstack-aligned evidence policy. Every file it names is in the [complete record at b76decc](https://github.com/aelomi/aelomi/tree/b76deccdc67efdd777464b19dae7d914caa6ea77/docs/evidence/data-5-review-followups-0ebafea).

This record covers source commit `0ebafea617d80051c7871c643f8a32e388a258fa`, which changes only the mock-only layout-delay control from #72. The [d37c4ec record](../data-5-review-followups-d37c4ec/README.md) still holds every native proof. Mock builds alone register the control's route (`import.meta.env.MODE === "mock"`), so the release app and its native behavior are unchanged.

## Why the control changed

`pnpm check:all` ran three times on `25c204d` (source `d37c4ec` plus its record). Each attempt's own record is kept here:

1. **Attempts 1 and 2 failed at `cold-start`** with `no paint marker in 10 s (window not visible?)` ([attempt 1](https://github.com/aelomi/aelomi/blob/568168d4fc1b51e6b001c4114d1912f24d3bf565/docs/evidence/data-5-review-followups-0ebafea/check-all-25c204d-attempt1-tail.log), [attempt 2](https://github.com/aelomi/aelomi/blob/568168d4fc1b51e6b001c4114d1912f24d3bf565/docs/evidence/data-5-review-followups-0ebafea/check-all-25c204d-attempt2-tail.log)). The display was on, the lid open and the session unlocked. The harness launches the app binary directly, and on this Mac that window never came on screen, so WebKit never painted it. An earlier version of this record blamed occlusion by the maximized T3 Code window and added [front-watch.sh.txt](front-watch.sh.txt), which makes each harness-launched process frontmost by PID. That was wrong: with that watcher, the first `check:all` attempt on `568168d` failed the same way. The [`568168d` record](../data-5-review-followups-568168d/README.md) has the diagnosis and the watcher that worked, which activates the app by bundle ID during the `cold-start` stage only.
2. **Attempt 3 failed earlier, at `keystroke`.** The control's 2,000-paragraph block laid out in about 10 ms on this run (tick wait p50 10 ms), so only 173 of 405 keys reached Event Timing's 16 ms reporting threshold. The assertion needs half ([excerpt](https://github.com/aelomi/aelomi/blob/568168d4fc1b51e6b001c4114d1912f24d3bf565/docs/evidence/data-5-review-followups-0ebafea/check-all-25c204d-attempt3-keystroke-excerpt.log)). The control had failed because the machine was fast, not because a clock gap was missing.

The control now doubles its block until one forced layout takes 30 ms, and only then becomes editable.

## Result on 0ebafea

Two CI-mode runs on battery, at load averages of 5.36 and 4.23 ([layout-delay-calibrated.log](https://github.com/aelomi/aelomi/blob/568168d4fc1b51e6b001c4114d1912f24d3bf565/docs/evidence/data-5-review-followups-0ebafea/layout-delay-calibrated.log)):

| Run | Event Timing keys at 16 ms or more | App-owned p95 | Tick wait p50 |
|---|---|---|---|
| 1 | 383 of 405 | 11 ms | 36 ms |
| 2 | 405 of 405 | 12 ms | 49 ms |

Both passed. Three earlier runs of the same source, before it was committed, gave 383, 405 and 405 keys.

[files.sha256.json](https://github.com/aelomi/aelomi/blob/568168d4fc1b51e6b001c4114d1912f24d3bf565/docs/evidence/data-5-review-followups-0ebafea/files.sha256.json) records the bytes and SHA-256 of every file here except this README and the manifest.
