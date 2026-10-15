# DATA-5 review follow-ups: native presentation and check:all on 568168d

This folder keeps only this README and its scripts, under the pstack-aligned evidence policy. Every file it names is in the [complete record at b76decc](https://github.com/aelomi/aelomi/tree/b76deccdc67efdd777464b19dae7d914caa6ea77/docs/evidence/data-5-review-followups-568168d).

This record covers commit `568168d4fc1b51e6b001c4114d1912f24d3bf565`. Its source matches `0ebafea`. The release app built from it has the same product code as `d37c4ec`, because the only later source change is the mock-only layout-delay control, which release builds leave out. The record adds three things:

- **Native presentation for #72.** The [d37c4ec record](../data-5-review-followups-d37c4ec/README.md) listed this as unmeasured, and this trace supersedes that limit.
- **The `check:all` attempts on `568168d`.**
- **The corrected cause of the earlier `cold-start` failures.**

## Environment

- **Machine.** The same Apple M1 MacBook Pro running macOS 27.0.1, with Xcode and Instruments 27.0 (27A266a).
- **Power.** The Mac switched from battery to AC power at 12:50 IST, charging from 16% (`pmset -g log`). Every run in this record used AC power. The d37c4ec and 0ebafea records used battery.
- **Build.** The app under test is the bundle that `check:all`'s `build-app` stage built from `568168d`. Its binary hash is in [d72x-binary.sha256](https://github.com/aelomi/aelomi/blob/1854fb74846a1146fa1bc4a2676cf755a64e89ea/docs/evidence/data-5-review-followups-568168d/d72x-binary.sha256).
- **Redaction.** Personal path prefixes are redacted as in the earlier records. The 731 MB trace bundle stays local because its table of contents names the device. [d72x-trace-info.txt](https://github.com/aelomi/aelomi/blob/1854fb74846a1146fa1bc4a2676cf755a64e89ea/docs/evidence/data-5-review-followups-568168d/d72x-trace-info.txt) keeps the fields that matter.

## check:all on 568168d

**Attempt 1 failed at `cold-start`.** The error was the same `no paint marker in 10 s (window not visible?)` as before ([log](https://github.com/aelomi/aelomi/blob/1854fb74846a1146fa1bc4a2676cf755a64e89ea/docs/evidence/data-5-review-followups-568168d/check-all-568168d-attempt1-cold-start.log)). The frontmost-by-PID watcher from the 0ebafea record ([front-watch.sh.txt](../data-5-review-followups-0ebafea/front-watch.sh.txt)) was running.

**Diagnosis.** [cold-start-space-diagnosis.log](https://github.com/aelomi/aelomi/blob/1854fb74846a1146fa1bc4a2676cf755a64e89ea/docs/evidence/data-5-review-followups-568168d/cold-start-space-diagnosis.log) records the steps and their output:

- **Direct launches.** No directly launched window painted its first frame. That held with T3 Code frontmost, and T3 Code was not full screen.
- **Frontmost by PID.** The frontend started (`ping seq=1`), but System Events listed no window for the process. Making the process frontmost by PID changed nothing, which rules out plain occlusion.
- **Bundle-ID activation.** Activating the app by bundle ID painted the first frame, the report was written, and the app quit.
- **Open question.** A full-screen Dia window held its own Space all session. Which macOS rule keeps a directly spawned app's window off screen until the app is activated was not established.

**Attempt 2 passed.** It ran with [front-watch.sh.txt](front-watch.sh.txt), which activates the app by bundle ID during the `cold-start` stage only. The harness and its budgets are unchanged. Its end-to-end times include however long the watcher took to activate the app. The watcher is a local aid for this Mac and is not part of the repository. The stage output is in [check-all-568168d-perf.log](https://github.com/aelomi/aelomi/blob/1854fb74846a1146fa1bc4a2676cf755a64e89ea/docs/evidence/data-5-review-followups-568168d/check-all-568168d-perf.log), and the gate record is [check-all-568168d.checks.json](https://github.com/aelomi/aelomi/blob/1854fb74846a1146fa1bc4a2676cf755a64e89ea/docs/evidence/data-5-review-followups-568168d/check-all-568168d.checks.json).

| Surface | Result in the passing run |
|---|---|
| 5,000-block editor typing | full p95 17 ms (local ceiling 19 ms), app-owned p95 15 ms |
| Layout-delay control (#72) | Event Timing reported 405 of 405 keys; app-owned p95 11 ms against full p95 52 ms |
| Palette | p95 14 ms |
| Scroll, 5,000 blocks | work p95 12 ms, longest interval 20 ms |
| Cold start, empty workspace, CI mode | median 434.0 ms, nav-to-paint 98.5 ms; load 7.61 at the end, marked NOISY |

The 5,000-block typing p95 of 17 ms comes from a single run on AC power. The d37c4ec record measured 25 or 26 ms on battery with the same harness, and main measured 25 ms. The ceiling is unchanged.

## #72, native presentation

**Method.**

- **Setup.** The release app ran on a copy of the d70 seed (`seed_index search-10k --big-page`) with its index cache removed. The app rebuilt the cache and logged Ready after 5.1 s ([d72x-shell.log](https://github.com/aelomi/aelomi/blob/1854fb74846a1146fa1bc4a2676cf755a64e89ea/docs/evidence/data-5-review-followups-568168d/d72x-shell.log)).
- **Page.** ⌘K opened Kestrel field notes, the 5,000-block page, and the body took focus ([d72x-actions.log](https://github.com/aelomi/aelomi/blob/1854fb74846a1146fa1bc4a2676cf755a64e89ea/docs/evidence/data-5-review-followups-568168d/d72x-actions.log)).
- **Recording.** [d72x-record.sh.txt](d72x-record.sh.txt) activated the app by bundle ID and attached `xctrace record --template 'Animation Hitches' --time-limit 40s`.
- **Keys.** Five seconds after recording started, the script typed 40 `x` characters at least 450 ms apart, then pressed Page Down 25 times at least 500 ms apart.
- **Delivery checks.** The key script throws if the app loses the front, and neither call threw ([d72x-keys.log](https://github.com/aelomi/aelomi/blob/1854fb74846a1146fa1bc4a2676cf755a64e89ea/docs/evidence/data-5-review-followups-568168d/d72x-keys.log)). The page's source gained a 2,440-byte log segment written at 13:16:07 IST, during the recording ([d72x-source-log.txt](https://github.com/aelomi/aelomi/blob/1854fb74846a1146fa1bc4a2676cf755a64e89ea/docs/evidence/data-5-review-followups-568168d/d72x-source-log.txt)).

**Recording.** The recording ended at its time limit and saved without run issues (`record exit 0`, [d72x-xctrace-record.log](https://github.com/aelomi/aelomi/blob/1854fb74846a1146fa1bc4a2676cf755a64e89ea/docs/evidence/data-5-review-followups-568168d/d72x-xctrace-record.log)). [d72x-summarize.py.txt](d72x-summarize.py.txt) reads the six exported tables. It takes the app's frames from `hitches-updates` by process ID and joins the other tables to them by swap ID ([d72x-summary.txt](https://github.com/aelomi/aelomi/blob/1854fb74846a1146fa1bc4a2676cf755a64e89ea/docs/evidence/data-5-review-followups-568168d/d72x-summary.txt)).

| The app's frames | n | p50 | p95 | max |
|---|---|---|---|---|
| Commit (`hitches-updates`) | 350 | 0.12 ms | 0.74 ms | 0.90 ms |
| Render (`hitches-renders`) | 352 | 2.87 ms | 7.06 ms | 41.79 ms |
| GPU (`hitches-gpu`) | 350 | 0.42 ms | 1.80 ms | 2.12 ms |
| Frame wait (`hitches-framewait`) | 350 | 12.74 ms | 14.14 ms | 30.26 ms |
| Frame lifetime (`hitches-frame-lifetimes`) | 350 | 40.09 ms | 48.85 ms | 73.29 ms |

**How Instruments defines these.** The Hitches modeler in Xcode 27.0 (`Hitches.instrdst/.../modeling-rules/rules-0000.clp`) works from WindowServer's records:

- **Frame lifetime** is a frame's presentation time minus the earliest time WindowServer records for that frame.
- **A hitch** is a frame whose lifetime exceeds the display's buffer count times the refresh interval, plus half a refresh. Its duration is how far presentation fell behind that expected time.

**Hitches.** 35 of the app's 350 frames were hitches: 26 one refresh late (16.67 ms) and 9 two refreshes late (33.33 ms), 733 ms in total. The trace recorded no hitches for any other process. Counting per second of the recording:

- **Seconds 0 to 27.** The caret blink, plus typing from about second 5, produced 98 frames, 25 of them late.
- **Seconds 28 to 41.** Scrolling by Page Down, which began about then, produced 252 frames, 10 of them late.

**What this shows and what it does not.**

- **Measured.** The native leg, from the app's committed frames to their presentation on the display, is now measured on the release app. The d37c4ec record's limit on it no longer holds.
- **Not a key-to-glass time.** The trace does not give a time per key. System Events sent the keys, and they leave no timestamps in the trace's clock. Event Timing covers key to WebKit paint, and this trace covers frames to the display. The two legs come from different runs and clocks, so they are not added together.
- **No verdict.** No budget covers native presentation, and main was not recorded under the same conditions, so this is an observation, not a pass or a fail. The load average was 3.05 before the keys and 4.22 after.

## Screenshot

Attached to the pull request, not committed: d72x-big-page.png, 1920 by 1280, 254172 bytes, SHA-256 `e914bda4a30018e22320827301f920514ad75a7f1875805a3393bfdebe92fe24`. It shows Kestrel field notes before the recording; the `x` lines on top came from the earlier attempts on the same seed.

[files.sha256.json](https://github.com/aelomi/aelomi/blob/1854fb74846a1146fa1bc4a2676cf755a64e89ea/docs/evidence/data-5-review-followups-568168d/files.sha256.json) records the bytes and SHA-256 of every file here except this README and the manifest.
