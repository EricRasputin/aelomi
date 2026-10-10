#!/usr/bin/env bash
# Requires every CI job on pull requests into main. Rerun after adding or renaming a job in
# .github/workflows/ci.yml. strict stays off so stacked PRs don't need a rebase per layer, and
# enforce_admins stays off so an admin can land a PR that predates a required check.
set -euo pipefail

REPO=${REPO:-aelomi/aelomi}
CHECKS=(biome tsc vitest nextest cargo-deny pnpm-audit build-app e2e playwright cold-start keystroke)

contexts=$(printf '"%s",' "${CHECKS[@]}")
gh api -X PUT "repos/$REPO/branches/main/protection" --input - <<EOF
{
  "required_status_checks": { "strict": false, "contexts": [${contexts%,}] },
  "enforce_admins": false,
  "required_pull_request_reviews": null,
  "restrictions": null
}
EOF
gh api "repos/$REPO/branches/main/protection/required_status_checks" --jq '.contexts | join(" ")'
