# Native verification evidence

This policy follows pstack's **principle-prove-it-works**, **show-me-your-work** and `opening-a-pr` guidance:

- **The proof.** It is the real app doing the thing, plus a script a reviewer can rerun.
- **The evidence.** It is a working artifact, kept local by default.
- **The pull request.** It is a short briefing that links the evidence. It is not the lab notebook.

## Where evidence goes

| What | Where |
|---|---|
| Screenshots and videos that prove a visible claim | Attached to a PR comment |
| Proof that the change works | The PR body's `## Verification` section, in one to three bullets. Each bullet names the run path and its outcome. |
| Shell logs, accessibility trees, doctor output, traces, raw exports, decision logs | Local only. `verify-aelomi` keeps each run under `~/.local/state/aelomi-verify/`, which survives a reboot and is never synced by iCloud. |
| A record a later reader must audit | `docs/evidence/<topic>/`, committed, but only for the claims below |

Keep the runs a PR names until it merges, then its owner deletes them. Delete every other run as soon as you no longer need it, because nothing purges `~/.local/state/aelomi-verify/` for you. Review adversaries work in their own worktree at the PR head, where they may build, run tests and write repro scripts, and they change nothing outside it. Their brief carries the PR's `## Verification` bullets, the log lines those bullets rest on, and the output of any check a claim rests on. A finding counts only when its command's output shows the failure.

**Privacy.** Use synthetic data only, in an isolated `AELOMI_DATA_DIR`. Before anything leaves the machine (a quoted log line, a screenshot, a PR comment), remove credentials and personal paths. For example, write `~/` for a home directory.

## When to commit a record

Commit a record only when the trail has to be auditable after the PR merges:

- a change that can destroy, move or migrate user data, whatever layer it lives in, including shell changes to storage, quit, trash or data paths;
- a measured performance claim that a budget or a later decision cites.

Other UI and shell changes need no committed record. Their screenshots and the PR's `## Verification` bullets are enough.

### What a record contains

A committed record holds only these files:

- **`README.md`.** It names the source commit it proves, the paths the record proves (git pathspecs written from the repository root, chosen from the paths `git diff --name-only <merge base> <commit>` lists), the environment, the steps and the results. It quotes inline the few log lines each claim rests on, so the claim can be checked without the full log. It lists each attached screenshot's filename, byte size and SHA-256.
- **Scripts.** These are the scripts that drive or summarize the run, saved as `*.sh.txt`, `*.py.txt`, `*.js.txt` or `*.mjs.txt` so no tool executes them.

Never commit any of these:

- full shell logs, accessibility trees or doctor output;
- trace exports, build logs or hash manifests;
- PNGs, except under the upload fallback below.

Git already hashes every committed file.

Commit the record as its own commit on top of the source commit it proves. A later product-code fix needs a new record for the behavior it touches. Earlier records stay as they are. The record names the commit it tested, the merge base it was tested against (`git merge-base <base> <commit>`, where `<base>` is the PR's base branch, `origin/main` unless the PR is stacked), and the patch-id of its change: the first field of `git diff --full-index --binary <merge base> <commit> | git patch-id --verbatim`. That is the review gate's command, pointed at the tested commit. A later rebase never edits the record. To audit a record after any merge method:

1. Fetch the PR's final head, which GitHub keeps: `git fetch origin refs/pull/<number>/head`.
2. Find the record's commit: `git log --diff-filter=A --format=%H FETCH_HEAD -- docs/evidence/<topic>/README.md`. Call its parent `P`.
3. Pick a recorded merge base, never one computed from today's branch: the record's own if `P` is the commit it names, or, after a rebase, the merge base in the review gate's comment for the PR's final head.
4. Rerun the patch-id command with that merge base and `P`, and compare it with the record's.
5. From the repository root, check that each of the record's pathspecs matches at least one path in `git diff --name-only <merge base> P`, and that `git diff P FETCH_HEAD -- <the record's pathspecs>` is empty. A pathspec that matches nothing fails this step. For a record without pathspecs, use the files `git diff --name-only <merge base> P` lists, leaving out `docs/evidence/`.

If step 4 or 5 fails, the record does not prove the landed change, and the change needs a new record. For a path that several records cover, audit the newest. When you can, write the record after the branch's last planned rebase.

## Attach the screenshots

Use a GitHub CLI release whose `gh pr comment` supports `--attach`. GitHub CLI 2.92.0 lacks it; 2.102.0 has it. If you download the CLI, verify the archive against its published checksums first. Write the comment body to a local Markdown file that references each image by its local path, then post it:

```bash
gh pr comment 123 --repo aelomi/aelomi \
	--body-file /tmp/native-evidence-comment.md \
	--attach "$RUN/before.png" \
	--attach "$RUN/after.png"
```

Read the posted comment back. Download every attachment URL without a token, for example with `curl --disable --fail --location`, then open the file and compare its byte size and SHA-256 with what you recorded. Anything else is a failed publish: a login page, an HTML error, an expired signed URL, a file that isn't the image, or a size or hash mismatch.

**Upload fallback.** If no upload publishes cleanly and the claim needs a committed record, commit only the PNGs that prove it next to the record, with each file's size and SHA-256 and the reason the upload failed. That commit is a new head, so the review gate runs again. For any other claim, the screenshots stay local, and the PR's `## Verification` bullet says so.

## Trimmed records

Records committed before 2026-10-15 kept full logs, accessibility trees and manifests. On that date they were trimmed to their READMEs and scripts:

- **Removed files.** A script turned each README link to a removed file into a full-SHA permalink to the last commit that held it.
- **Prose mentions.** For files a README names without a link, a second step added a note under each title. The note links the complete folder as it stood before the trim.

Nothing was lost; it moved from the tree to history.

Those permalinks point at commits on `main`, except four older links to PR #43's head, which GitHub keeps as `refs/pull/43/head`. Both stay valid. A future record that links to its own branch's commits stays valid while those commits are reachable: from `main` after a merge commit, or from `refs/pull/<number>/head` while the PR's head still contains them.
