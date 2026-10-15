#!/bin/sh
# Links CLAUDE.md and AGENTS.md in this checkout to the maintainer's private agent instructions.
# Safe to rerun. T3 Code's worktree action and a Claude Code SessionStart hook run it; run it by
# hand in a checkout that predates them. Messages go to stdout, which the SessionStart hook adds to
# the session. A regular file that matches the private file is replaced by the link; one that
# differs is reported, never replaced, so no local edit is lost.
src="$HOME/.agents/aelomi.md"
top=$(git rev-parse --show-toplevel 2>/dev/null) && cd "$top" || exit 0
if [ ! -f "$src" ]; then
  echo "No agent instructions on this machine: $src is missing, so CLAUDE.md and AGENTS.md are absent."
  exit 0
fi
for f in CLAUDE.md AGENTS.md; do
  if git ls-files --error-unmatch "$f" >/dev/null 2>&1; then
    git check-ignore -q --no-index "$f" &&
      echo "$f is tracked by git, but agent instructions must stay out of the repo. Run git rm --cached $f and read $src instead."
    continue
  fi
  if [ -L "$f" ] && [ "$(readlink "$f")" = "$src" ]; then
    continue
  elif [ -L "$f" ] || [ ! -e "$f" ] || cmp -s "$f" "$src"; then
    ln -sfn "$src" "$f"
    echo "Linked $f to $src. Read $f in full before any work."
  else
    echo "$f is a regular file that differs from $src. Read $src: it is the current instructions. Merge any edits from $f into it, delete $f, and rerun scripts/link-agent-instructions.sh."
  fi
done
