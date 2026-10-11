#!/usr/bin/env bash
# Vendors third-party agent skills into .claude/ at pinned commits.
# To update: change a *_REF below, rerun, review the diff, commit.
set -euo pipefail

PSTACK_URL=https://github.com/backnotprop/pstack
PSTACK_REF=3a604672c46cd8187d2b19980eae0a34f9f91138
# Skills whose disable-model-invocation line is removed, so agents can call them: every
# skill a pstack prompt sends an agent to. Still hidden: poteto-mode, which agents read by
# path, and bro, correct, make-bot-ui, blast-radius, teach, recall, and poteto-help, which
# only poteto-help's recipes for the user name.
PSTACK_UNHIDE="
  how why architect arena swarm interrogate reflect figure-it-out show-me-your-work
  no-comments unslop technical-writing tdd benchmark-checklist typescript-best-practices
  automate-me create-verification-skill maintain-verification-skill
  principle-attack-the-premise principle-boundary-discipline principle-build-the-lever
  principle-encode-lessons-in-structure principle-exhaust-the-design-space
  principle-experience-first principle-explain-the-number principle-fix-root-causes
  principle-foundational-thinking principle-guard-the-context-window
  principle-laziness-protocol principle-make-operations-idempotent
  principle-migrate-callers-then-delete-legacy-apis principle-minimize-reader-load
  principle-model-the-domain principle-never-block-on-the-human
  principle-outcome-oriented-execution principle-prove-it-works
  principle-redesign-from-first-principles principle-separate-before-serializing-shared-state
  principle-sequence-verifiable-units principle-subtract-before-you-add
  principle-test-behavior-not-implementation principle-type-system-discipline
"

EMIL_URL=https://github.com/emilkowalski/skill
EMIL_REF=e8a175de22ae1e49370fc144c1f3bb9aeedf988d
# Not used in Aelomi (see CLAUDE.md > Workflow).
EMIL_EXCLUDE="animate-expo mobile-native write-swift"

# pen.dev's skill ships as SKILL.md inside its CLI package, under a license
# that forbids redistribution. It is fetched here but gitignored, never committed.
PEN_PKG=@pen.dev/cli
PEN_VERSION=0.3.11

ROOT=$(cd "$(dirname "$0")/.." && pwd)
SKILLS="$ROOT/.claude/skills"
AGENTS="$ROOT/.claude/agents"
VENDOR="$ROOT/.claude/vendor"
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

fetch() { # name url ref
  git init -q "$TMP/$1"
  git -C "$TMP/$1" fetch -q --depth 1 "$2" "$3"
  git -C "$TMP/$1" checkout -q FETCH_HEAD
}

# install name src_skills_dir excluded_names...
# Removes the skills this source installed last time, then copies the current set.
install() {
  local name=$1 src=$2
  shift 2
  local manifest="$VENDOR/$name/skills.txt"
  if [ -f "$manifest" ]; then
    while read -r skill; do rm -rf "${SKILLS:?}/$skill"; done <"$manifest"
  fi
  mkdir -p "$SKILLS" "$VENDOR/$name"
  : >"$manifest"
  for dir in "$src"/*/; do
    local skill
    skill=$(basename "$dir")
    case " $* " in *" $skill "*) continue ;; esac
    rsync -a --delete "$dir" "$SKILLS/$skill/"
    echo "$skill" >>"$manifest"
  done
}

fetch pstack "$PSTACK_URL" "$PSTACK_REF"
fetch emil "$EMIL_URL" "$EMIL_REF"

install pstack "$TMP/pstack/skills"
install emil "$TMP/emil/skills" $EMIL_EXCLUDE

# pstack hides every skill from the model, but its own prompts send agents to many of them.
# Unhide those. install copies fresh files, so the line is always there.
UNHIDE_LINE='disable-model-invocation: true'
for skill in $PSTACK_UNHIDE; do
  file="$SKILLS/$skill/SKILL.md"
  if ! grep -qxF "$UNHIDE_LINE" "$file"; then
    echo "error: $file lacks '$UNHIDE_LINE'; pstack changed upstream, recheck the unhide step" >&2
    exit 1
  fi
  grep -vxF "$UNHIDE_LINE" "$file" >"$file.tmp"
  mv "$file.tmp" "$file"
done

mkdir -p "$AGENTS"
cp "$TMP/pstack/agents/poteto-agent.md" "$AGENTS/poteto-agent.md"
cp "$TMP/pstack/agents/comment-sicko.md" "$AGENTS/comment-sicko.md"

mkdir -p "$TMP/pen" "$SKILLS/pen-design"
npm pack -q --pack-destination "$TMP/pen" "$PEN_PKG@$PEN_VERSION" >/dev/null
tar -xzf "$TMP/pen"/*.tgz -C "$TMP/pen"
cp "$TMP/pen/package/SKILL.md" "$SKILLS/pen-design/SKILL.md"

cp "$TMP/pstack/LICENSE" "$VENDOR/pstack/LICENSE"
cp "$TMP/emil/LICENSE" "$VENDOR/emil/LICENSE"
printf '%s\n%s\n' "$PSTACK_URL" "$PSTACK_REF" >"$VENDOR/pstack/SOURCE"
printf '%s\n%s\n' "$EMIL_URL" "$EMIL_REF" >"$VENDOR/emil/SOURCE"
for skill in $PSTACK_UNHIDE; do
  echo "skills/$skill/SKILL.md: removed '$UNHIDE_LINE' (scripts/vendor-skills.sh)"
done >"$VENDOR/pstack/PATCHES"

echo "Vendored pstack@${PSTACK_REF:0:7}, emil@${EMIL_REF:0:7} and pen-design@$PEN_VERSION into .claude/"
