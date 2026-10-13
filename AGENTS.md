# Aelomi agent instructions

`CLAUDE.md` is the source of truth for every agent working in this repo, whatever tool runs it. Read it in full before any work.

## Review gate

No pull request merges without a reviewer verdict of APPROVE from Claude Fable for its current head SHA.

- After CI is green, the agent that opened or updated the PR runs a fresh `reviewer` (`.claude/agents/reviewer.md`) on Claude Fable. Builders never use Fable.
- Findings go back to the builder. Each new head gets a fresh review, until the latest verdict is APPROVE for the current head SHA.
- No CI check enforces this. The reviewer's PR comment is the record. Report the verdict line and head SHA to the maintainer before asking for a merge.
- A push or rebase after APPROVE needs a new review.
