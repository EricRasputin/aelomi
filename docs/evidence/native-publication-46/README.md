# Native evidence publication proof

This record exercises the [future evidence policy](../../native-verification-evidence.md) for issue #46. It proves publication and retrieval, with no app behavior change. The PNGs are GitHub attachments; the five text files live beside this README.

## Capture

- App source commit `5603b6813af94d904e4bed697c787a987977798d`. The follow-up changes documentation only.
- Real Tauri dev window, Aelomi 0.1.0, macOS 27.0.1, build 26A434. Claude Opus 5.5 captured and inspected both screenshots.
- Run `20261013-091400-dev-4yev`, with isolated data under `/private/tmp/aelomi-verify/20261013-091400-dev-4yev/data`. The run directory is provenance, not a review link.
- Existing [IPC recipe at the app source commit](https://github.com/aelomi/aelomi/blob/5603b6813af94d904e4bed697c787a987977798d/.claude/skills/verify-aelomi/features/ipc-status.md), behaviors `ipc-initial` and `ipc-ping-again`.

The commands used `.claude/skills/verify-aelomi/scripts/app.sh` with the run directory. `doctor` passed all checks. `wait-status` confirmed `IPC ok · core 0.1.0 · seq 1`, then `screenshot before` captured it. `press "Ping again"` sent AXPress. `wait-status` confirmed `IPC ok · core 0.1.0 · seq 2`, then `screenshot after` captured it. `keys q command` requested a normal quit. `cleanup` reported that nothing from this run remained running, and port 53523 was free. Theme and privacy grants stayed unchanged.

The [shell log](shell.log) records the native side effects:

```text
storage status=Ready path=/private/tmp/aelomi-verify/20261013-091400-dev-4yev/data
ping seq=1
ping seq=2
quit flush via=ExitRequested pending=[]
quit flush via=Exit pending=[]
```

[Actions](actions.log), [doctor](doctor.txt), [before accessibility tree](before.ax.txt), and [after accessibility tree](after.ax.txt) provide the matching text record. The published shell log strips ANSI color escapes and replaces the personal checkout path with `<checkout>`. All other content remains. Hashes below cover the published bytes. Raw app data, environment, and PID files were not published.

## Publication and retrieval

GitHub CLI 2.92.0 lacked `--attach`. An official GitHub CLI 2.102.0 archive was downloaded from `cli/cli` and checked against its published SHA-256 checksums. Its supported `gh pr comment 49 --body-file … --attach … --attach …` command uploaded both PNGs and replaced their local Markdown paths in the [posted evidence comment](https://github.com/aelomi/aelomi/pull/49#issuecomment-6137785205).

The T3 browser's default profile was signed out, so browser upload was not exercised. T3 exposes thread attachment tools, but those do not provide the public PR proof links this workflow requires. The GitHub integration returned `403 Resource not accessible by integration` on PR creation; `gh pr create` created this ready PR.

Both attachment URLs were downloaded with `curl --disable --fail --location`, without a token or authorization header, into a fresh directory. Each download was a PNG, matched the original byte size and SHA-256, and matched the original bytes. The coordinator opened both downloaded images. Each shows the Aelomi dark native window with its traffic lights, **Ping again**, and the expected sequence number. The screenshots are 1920 by 1280 pixels.

| File | Public attachment | Bytes | SHA-256 |
|---|---|---:|---|
| `before.png` | [Native window with seq 1](https://github.com/user-attachments/assets/996e831b-4927-4cc8-92a5-2f662e99ca45) | 82722 | `62852fdcbfbbc3381fd8844d3107a73cda6db5dfaffd063980e4e9b9d1591608` |
| `after.png` | [Native window with seq 2](https://github.com/user-attachments/assets/e37b5d66-0983-403c-a102-49062cecc241) | 83003 | `ff43481a7a756930788f1e736c9eca47546b320262527085b79424289543f635` |

The text record is committed and linked from the PR with its full evidence commit. Download each text file through that commit's raw URL or GitHub Contents API and compare with this manifest. The README itself is excluded from its own hash list.

| File | Bytes | SHA-256 |
|---|---:|---|
| `actions.log` | 205 | `e088512970e2f1462cff1520da6cac8619ccbc8cafc006abeefdc75c6f14690b` |
| `before.ax.txt` | 315 | `8f3176a28112994e95595a4e2586b38055ea4badf9add3240e816f46b7a21778` |
| `after.ax.txt` | 315 | `4e97d623bbc19f1fcf2a66f3f96ed45c961fef9130426095634d693ed5d02e61` |
| `doctor.txt` | 373 | `1ade4ccaea5af3d744f2f50340315e53e4b25212cf936126e1f8d785fd85f3db` |
| `shell.log` | 2036 | `c35555c228bf4143cc7361ac63447ab529b5562c39fa7fe8176a54fe26c2dd86` |

The committed fallback was retrieval-tested against the unchanged [SHELL-10 screenshot](https://github.com/aelomi/aelomi/blob/5603b6813af94d904e4bed697c787a987977798d/docs/evidence/shell-10/healthy-dark.png) and [quit shell log](https://github.com/aelomi/aelomi/blob/5603b6813af94d904e4bed697c787a987977798d/docs/evidence/shell-10/quit-shell.log). The raw-file host stalled, so those two downloads were stopped and retried anonymously through GitHub's documented Contents API, with the full source commit and `Accept: application/vnd.github.raw+json`. Both files matched the original committed bytes. This checks access to already published fallback files; it does not claim a new committed-image upload.

This proof does not re-test SHELL-10 or the tooltip. Their existing proof stays unchanged. Actions artifacts were not used as an archive, and no release was published.
