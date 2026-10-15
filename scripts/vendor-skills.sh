#!/usr/bin/env bash
# Vendors third-party agent skills into .claude/ at pinned commits.
# To update: change a *_REF below, rerun, review the diff, commit.
set -euo pipefail

PSTACK_URL=https://github.com/backnotprop/pstack
PSTACK_REF=1f0f9c3bac03c4297f6fc8f7644fa412eda57085
# Comment Sicko (.claude/agents/comment-sicko.md) invokes these two; every other pstack skill keeps pstack's flag.
PSTACK_UNHIDE="how why"

EMIL_URL=https://github.com/emilkowalski/skill
EMIL_REF=2d228630ba1446365f614a108cb8252f1feda202
# Not used in Aelomi.
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

# pstack hides every skill from the model. Unhide PSTACK_UNHIDE by dropping the
# flag from the frontmatter only (between the first two --- lines).
# install copies fresh files, so the line is always there.
UNHIDE_LINE='disable-model-invocation: true'
for skill in $PSTACK_UNHIDE; do
  file="$SKILLS/$skill/SKILL.md"
  if ! awk -v line="$UNHIDE_LINE" '
    $0 == "---" && fences < 2 { fences++ }
    fences == 1 && $0 == line { found = 1; next }
    { print }
    END { exit !found }
  ' "$file" >"$file.tmp"; then
    rm -f "$file.tmp"
    echo "error: $file frontmatter lacks '$UNHIDE_LINE'; pstack changed upstream, recheck the unhide step" >&2
    exit 1
  fi
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
