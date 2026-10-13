# Publish native verification evidence

For future native runs, attach the required PNG screenshots to the pull request and commit the small text record under `docs/evidence/<behavior-or-issue>/`. Commit the text record before the branch goes public; attach the screenshots once the PR opens (see **Private until ready** in `CLAUDE.md`). Keep the screenshots at their captured resolution. Include the shell log, accessibility trees, actions, doctor result, and a README that connects each action to its visible result and native side effect.

Use [verify-aelomi](../.claude/skills/verify-aelomi/SKILL.md) to capture the proof. Publish and check every required file before deleting the local run. A `/tmp` path, a T3 thread attachment, or an expiring Actions artifact alone does not complete verification.

The existing [SHELL-10 proof](evidence/shell-10/README.md) stays in Git. Keep its fourteen PNGs, text files, and the immutable links in PR #43 unchanged. This policy applies to future evidence.

## Prepare the record

1. Record the source commit before the native run with `git rev-parse HEAD`. Record the macOS version, app version, run mode, behavior IDs, commands, and observed results in the evidence README. Distinguish the tested source commit from later commits that only publish its evidence.
2. Run the applicable verification recipe with isolated `AELOMI_DATA_DIR`. Capture each required before and after state and inspect every screenshot. Keep the matching `shell.log` lines and `.ax.txt` files. A mock screenshot does not prove native IPC.
3. Quit or clean up that run before any build, browser suite, or report generator runs in the same checkout. Restore any theme or permissions the recipe changed. Stop only that run's recorded PIDs.
4. Copy the relevant text files into the evidence directory. Inspect them for credentials and unrelated personal data before publication. Use synthetic test data. Keep the full relevant shell log, including failures and cleanup, rather than only a success excerpt. Strip terminal color escapes and redact personal path prefixes. Mark any redaction in the README and hash the published bytes.
5. Record each screenshot's filename, byte size, and SHA-256 in the README. Run `shasum -a 256 "$RUN/before.png" "$RUN/after.png"` for a two-state recipe. Record hashes of the other published text files as well. Leave app data, databases, `run.env`, and PID files local.

## Attach the screenshots

Check `gh pr comment --help` for `--attach`. An older CLI may lack the flag. Use an official GitHub CLI release that supports it, or the GitHub browser's attachment control. The [CLI attachment guide](https://docs.github.com/en/github-cli/github-cli/attaching-files-with-github-cli) requires push access and supports images and videos, so keep shell logs in the committed text record.

Write a comment body to a local Markdown file. Describe each screenshot and reference its local path. For a two-state recipe, use this command after the PR exists. Replace `123`, the paths, and `$RUN` with the actual PR and run.

```bash
gh pr comment 123 --repo aelomi/aelomi \
	--body-file /tmp/native-evidence-comment.md \
	--attach "$RUN/before.png#Native state before the action" \
	--attach "$RUN/after.png#Native state after the action"
```

GitHub replaces matching local image paths in the comment with uploaded URLs. Read the posted comment back with `gh api --paginate repos/aelomi/aelomi/issues/123/comments`. Put the exact screenshot URLs and comment permalink in the evidence README. Embed the images and link the text record from the PR body.

In an authenticated GitHub browser, use the comment box's attachment control for PNGs. Wait for the upload to produce Markdown URLs, then post the comment and read it back. GitHub documents a 10 MB image limit in [Attaching files](https://docs.github.com/en/get-started/writing-on-github/working-with-advanced-formatting/attaching-files). Split a large proof into the required states; preserve legibility rather than shrinking text to fit.

## Verify publication before review

1. Download every screenshot URL to a fresh directory without a GitHub token. For example, `curl --disable --fail --location --connect-timeout 10 --max-time 60 "$SCREENSHOT_URL" --output /tmp/downloaded-before.png`. Public-repository attachments should be readable without authentication. Reject a login page, HTML error, or expired signed URL.
2. Compare each download's SHA-256 and byte size with the manifest. Open each downloaded image and confirm it shows the intended native window and state.
3. Commit and push the text record on the PR branch. Link it with `https://github.com/aelomi/aelomi/blob/<full-evidence-commit>/docs/evidence/<directory>/README.md`. Use that evidence commit in every text-file link too. Download the remote text files and compare their hashes with the manifest.
4. Read the PR body back. Require every proof link to resolve to the expected bytes independently of the original run directory. Only then remove local copies if needed.
5. After any product-code change, repeat the affected native proof and append a new evidence record. Keep previous published records and state which source commit each proves. Run the repository's CI and fresh-review gate on the final PR head.

Attachments avoid adding PNG blobs to Git history, but GitHub does not promise permanent archival storage. Keep the posted evidence comment and its URLs. If a required attachment becomes inaccessible, restore it from a retained copy or rerun the proof and append a replacement record. Mark the gap until the replacement passes the download checks.

## Use the committed fallback when uploads fail

If `--attach` is missing, permission fails, the browser is signed out, or attachment automation fails, commit the required PNGs alongside the text record. Keep only the states needed to prove the behavior, with all required native paths still covered. Omit duplicate captures, build reports, and raw app data. Explain the upload failure in the README and PR body.

Use full-commit `blob` links for the README and logs, and full-commit `raw` links for PNGs. Download and hash-check those remote files as above. Do not leave required proof only in `/tmp` while waiting for an upload preference. A successful later attachment upload does not justify rewriting history or removing an already approved record.

If the raw-file host is unreachable, use GitHub's documented [Contents API](https://docs.github.com/en/rest/repos/contents#get-repository-content). Request `/repos/aelomi/aelomi/contents/<path>?ref=<full-evidence-commit>` with `Accept: application/vnd.github.raw+json`. Public files need no authorization header. Compare the returned bytes with the same manifest.

Actions artifacts are useful additional diagnostics. This repository's CI uploads currently retain artifacts for seven days. GitHub allows at most 90 days for public-repository artifact retention, as documented in [Actions retention settings](https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/enabling-features-for-your-repository/managing-github-actions-settings-for-a-repository). Publish required screenshots and logs through the attachment or committed workflow before those artifacts expire. Evidence publication needs no product release or app network change.

The [native publication proof](evidence/native-publication-46/README.md) records the attachment upload, anonymous screenshot retrieval, and committed-file fallback check for issue #46.
