# Final native checks for DATA-5 and SHELL-12

This folder keeps only this README and its scripts, under the [evidence policy](../../native-verification-evidence.md). Every file it names is in the [complete record at 9b44d57](https://github.com/aelomi/aelomi/tree/9b44d575e43efa1170b743cc54b019b5faf7a8cf/docs/evidence/data-5-final-native-9d7a9d4).

These real-app runs prove product source `9d7a9d44acbe4a34d66bcf7f6c8edfb6fe6aa66c`. The separate evidence commits add text records and document the maintainer's accepted startup result. They change no runtime code. The app was version 0.1.0, built for Apple Silicon and run on macOS 27.0.1, build 26A434, on the local Apple M1. Every run used an isolated synthetic data folder and `AELOMI_TRACE_IPC=1`.

The [earlier native record](../data-5-followups-9d7a9d4/README.md) proves missing, garbage and old-schema recovery, unchanged source files, unsupported-mark preservation, the 10,001-page rebuild, memory and cold startup. Its measured 618 ms median stays a failed result against the 500 ms goal. On 2026-10-15 the maintainer accepted 618 ms and requested later optimization. That decision supersedes the earlier record's pending startup work. Hosted budgets remain unchanged.

## Visible indexing and automatic results

The earlier 10k capture reached Ready before its screenshot. To capture the pending state, the same synthetic generator seeded an auxiliary 30,000-page workspace. This is a state-transition fixture, not a startup-budget measurement. After the preceding owned app exited, only the four named disposable index files were removed.

The run `/tmp/aelomi-verify/20261015-030839-app-Yp0i` passed every doctor check. Its page loaded from the source while the index rebuilt. Command-K and the query `q` showed visible `Indexing…`. The first screenshot and accessibility tree show that state with the page behind the palette. The second shows highlighted content results for the unchanged query after Ready. No further query input caused the refresh.

The shell log records an Indexing search reply, Ready and then 20 results. The worker reported 13,275.362 ms for this larger rebuild. The source-backed tree returned 29,100 active records before Ready. These numbers describe the auxiliary run only. Command-Q drained, flushed and exited before cleanup.

The first auxiliary attempt missed the visible indexing interval. Its screenshot already showed results. The `indexing-missed-capture-*` files retain that attempt's full relevant log, doctor result, actions and accessibility tree. It establishes no visible Indexing claim.

## Immediate edits survive every quit route

The release app opened the rebuild fixture's Projects page. An initial native edit added `Z`. The app quit normally and reopened. Its body was exactly `project indexZ\n\nProject diagram`.

Each following run passed doctor, captured its initial body, focused Page body and moved the caret to the first paragraph's end. A PID-addressed System Events helper typed one character and immediately requested the selected quit route. There was no screenshot, inspection or intentional wait between the edit and quit request. Apple events followed the key helper as the next shell command. Automation overhead is present; these checks do not measure the physical durability interval.

| Action | Route | Exact body on the next launch |
|---|---|---|
| Add `Q` | Command-Q | `project indexZQ\n\nProject diagram` |
| Add `W` | Command-W | `project indexZQW\n\nProject diagram` |
| Add `A` | PID-addressed quit Apple event | `project indexZQWA\n\nProject diagram` |
| Add `L` | PID-addressed quit Apple event with logout reason | `project indexZQWAL\n\nProject diagram` |

Every screenshot was inspected. The literal accessibility assertions in [exact-results.json](https://github.com/aelomi/aelomi/blob/925139382f88e5c0037b037871197f6db179ea61/docs/evidence/data-5-final-native-9d7a9d4/exact-results.json) passed. The shell logs show source appends, a frontend drain, `CompleteQuit pending=[]` and `Exit pending=[]`. The keyboard routes logged ExitRequested and CloseRequested. Both Apple-event routes logged Terminate. Their initial replies were cancelled with error -128 while the frontend drain completed, then the app exited normally. The logout-reason event asks only this app to quit and does not log the Mac out.

The final reopened app quit normally too. Every recorded PID was gone before cleanup. All owned launch sessions closed before any build, browser suite or report generation. Cleanup preserved the local runs.

## Commands and measurement limits

The native helpers came from [verify-aelomi](../../../.claude/skills/verify-aelomi/SKILL.md). The working directory was the feature worktree. Launch used `AELOMI_DATA_DIR=<synthetic folder> AELOMI_TRACE_IPC=1 .claude/skills/verify-aelomi/scripts/app.sh launch app --timeout 60`. A held PTY kept each directly launched process alive until its normal quit.

Every launch was followed by `app.sh doctor <run>`. The visible proof used `app.sh screenshot`, `app.sh focus <run> 'Page body'` and native keyboard input. [native-key-once.js.txt](native-key-once.js.txt) records the exact keyboard helper. [native-caret-end.js.txt](native-caret-end.js.txt) records caret placement. [native-quit-event.swift.txt](https://github.com/aelomi/aelomi/blob/925139382f88e5c0037b037871197f6db179ea61/docs/evidence/data-5-final-native-9d7a9d4/native-quit-event.swift.txt) is the PID-addressed sender already used for SHELL-10. The key records use `mach_continuous_time`, with its timebase in [mach-timebase.json](https://github.com/aelomi/aelomi/blob/2ed0b50ac8857f9ac4b6bc7135176638c381fff5/docs/evidence/data-5-final-native-9d7a9d4/mach-timebase.json). Their start precedes the System Events call and includes automation overhead.

A private attempt to interpose `F_FULLFSYNC` did not load into the hardened release app. There was no probe output file. The app's signing flags remain recorded in [context.json](https://github.com/aelomi/aelomi/blob/2ed0b50ac8857f9ac4b6bc7135176638c381fff5/docs/evidence/data-5-final-native-9d7a9d4/context.json). The `probe-not-loaded-*` record retains the real edit, normal quit and full shell log, but provides no native 100 ms physical-flush proof. No production signing, entitlements or storage code changed. The core's controlled-clock and crash-disk tests remain the deadline and durability checks. This record claims normal edit delivery and quit survival, not power-loss timing or native IME coverage.

## Published files

Personal path prefixes are replaced with `<user>` and `<worktree>`. Terminal control escapes are removed. JSON whitespace follows the repository formatter; every parsed value was compared before and after formatting and remained equal. The original captured files remain local. Captured action logs retain their literal trailing spaces. Relevant shell logs include startup, failures and normal exits. Synthetic source files, databases, `run.env`, PID files and the private interposer remain local.

[screenshots.json](https://github.com/aelomi/aelomi/blob/925139382f88e5c0037b037871197f6db179ea61/docs/evidence/data-5-final-native-9d7a9d4/screenshots.json) records each of the eight required PNG filenames, byte sizes and SHA-256 hashes. The captures remain at 1920 by 1280. The images will attach to PR #68 after its local gate and fresh local Fable approval. [manifest.json](https://github.com/aelomi/aelomi/blob/2ed0b50ac8857f9ac4b6bc7135176638c381fff5/docs/evidence/data-5-final-native-9d7a9d4/manifest.json) hashes every companion text file and this README. It excludes itself.

| Screenshot | Inspected visible result |
|---|---|
| `indexing-visible.png` | The open page remains behind the palette, which says Indexing… for `q`. |
| `indexing-ready-after.png` | The same query automatically shows highlighted content results after Ready. |
| `probe-edit-Z.png` | Projects shows the native edit `project indexZ`. |
| `quit-recovered-Z.png` | The first relaunch retains `project indexZ`. This is the before state for Command-Q. |
| `quit-recovered-ZQ.png` | The immediate Command-Q edit survives. This is the before state for Command-W. |
| `quit-recovered-ZQW.png` | The immediate Command-W edit survives. This is the before state for the quit Apple event. |
| `quit-recovered-ZQWA.png` | The immediate quit Apple-event edit survives. This is the before state for the logout-reason event. |
| `quit-recovered-ZQWAL.png` | The immediate logout-reason edit survives. |

After publication, every attachment must be downloaded anonymously, hash-checked and inspected again. Every committed text file must also match its remote bytes. The final PR review follows green CI on the final head. This record alone is no reviewer approval.
