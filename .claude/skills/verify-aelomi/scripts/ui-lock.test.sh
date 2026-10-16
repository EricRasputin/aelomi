#!/usr/bin/env bash
set -euo pipefail

S=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)
TEST_ROOT=$(mktemp -d "${TMPDIR:-/tmp}/aelomi-ui-test.XXXXXX")
trap 'jobs -p | xargs kill 2>/dev/null || true; rm -rf "$TEST_ROOT"' EXIT
export AELOMI_UI_LOCK="$TEST_ROOT/lock"
export AELOMI_UI_LOCK_WAIT_SECONDS=0

assert_equal() {
  [ "$1" = "$2" ] || { printf 'expected <%s>, got <%s>\n' "$2" "$1" >&2; exit 1; }
}

assert_equal "$("$S/ui-lock.sh" status)" free
assert_equal "$("$S/ui-lock.sh" acquire 'first run')" 'acquired: first run'
assert_equal "$(cat "$AELOMI_UI_LOCK/owner")" 'first run'
if "$S/ui-lock.sh" acquire second >"$TEST_ROOT/timeout" 2>&1; then exit 1; else assert_equal "$?" 1; fi
assert_equal "$(cat "$AELOMI_UI_LOCK/owner")" 'first run'
grep -q 'timed out' "$TEST_ROOT/timeout"
if "$S/ui-lock.sh" release second; then exit 1; else assert_equal "$?" 1; fi
assert_equal "$("$S/ui-lock.sh" status)" 'held: first run'
"$S/ui-lock.sh" release 'first run'
"$S/ui-lock.sh" release 'first run'
assert_equal "$("$S/ui-lock.sh" status)" free
echo 'PASS: acquire, timeout without owner overwrite, matching release, and status'

"$S/ui-lock.sh" acquire first >/dev/null
touch -t 200001010000 "$AELOMI_UI_LOCK"
if "$S/ui-lock.sh" acquire early >/dev/null 2>&1; then exit 1; else assert_equal "$?" 1; fi
assert_equal "$(cat "$AELOMI_UI_LOCK/owner")" first
AELOMI_UI_LOCK_WAIT_SECONDS=5 "$S/ui-lock.sh" acquire second >"$TEST_ROOT/waiter" &
waiter=$!
assert_equal "$(cat "$AELOMI_UI_LOCK/owner")" first
"$S/ui-lock.sh" release first
wait "$waiter"
assert_equal "$(cat "$AELOMI_UI_LOCK/owner")" second
"$S/ui-lock.sh" release second
echo 'PASS: a waiting contender acquires after release'

"$S/ui-lock.sh" acquire stale >/dev/null
touch -t 200001010000 "$AELOMI_UI_LOCK/owner"
for n in 1 2 3 4 5 6 7 8; do
  (if "$S/ui-lock.sh" acquire "candidate-$n" >"$TEST_ROOT/candidate-$n.log" 2>&1; then
    echo "$n" >"$TEST_ROOT/winner-$n"
  fi) &
done
wait
winners=("$TEST_ROOT"/winner-*)
assert_equal "${#winners[@]}" 1
winner=$(cat "${winners[0]}")
assert_equal "$(cat "$AELOMI_UI_LOCK/owner")" "candidate-$winner"
"$S/ui-lock.sh" release "candidate-$winner"
echo 'PASS: concurrent stale-lock contenders leave exactly one owner'

mkdir "$AELOMI_UI_LOCK"
if "$S/ui-lock.sh" acquire early >/dev/null 2>&1; then exit 1; else assert_equal "$?" 1; fi
touch -t 200001010000 "$AELOMI_UI_LOCK"
"$S/ui-lock.sh" acquire recovered >/dev/null
assert_equal "$(cat "$AELOMI_UI_LOCK/owner")" recovered
"$S/ui-lock.sh" release recovered
echo 'PASS: an ownerless lock is reclaimed only after it becomes stale'

if "$S/ui-lock.sh" acquire '' >/dev/null 2>&1; then exit 1; else assert_equal "$?" 2; fi
assert_equal "$("$S/ui-lock.sh" status)" free
echo 'PASS: an invalid label creates no lock'

python3 - "$AELOMI_UI_LOCK.guard" "$TEST_ROOT/guard-ready" <<'PYTEST' &
import fcntl
from pathlib import Path
import sys
import time
with open(sys.argv[1], "a") as guard:
    fcntl.flock(guard, fcntl.LOCK_EX)
    Path(sys.argv[2]).touch()
    time.sleep(10)
PYTEST
guard_pid=$!
for n in $(seq 1 100); do
  [ ! -f "$TEST_ROOT/guard-ready" ] || break
  sleep 0.05
done
[ -f "$TEST_ROOT/guard-ready" ]
if "$S/ui-lock.sh" acquire blocked >"$TEST_ROOT/guard-timeout" 2>&1; then exit 1; else assert_equal "$?" 1; fi
grep -q 'timed out' "$TEST_ROOT/guard-timeout"
kill "$guard_pid"
wait "$guard_pid" 2>/dev/null || true
assert_equal "$("$S/ui-lock.sh" status)" free
echo 'PASS: a busy filesystem guard respects the acquisition deadline'

mkdir "$TEST_ROOT/bin" "$TEST_ROOT/run"
printf 'APP_PID=12345\n' >"$TEST_ROOT/run/run.env"
cat >"$TEST_ROOT/bin/osascript" <<'MOCK'
#!/usr/bin/env bash
printf '%s\n' "$@" >"$AELOMI_TEST_SCRIPT_ARGS"
if [ "${AELOMI_TEST_APPLE_EVENT_ERROR:-0}" = 1 ]; then exit 1; fi
printf '%s\n' "$AELOMI_TEST_FRONTMOST"
MOCK
chmod +x "$TEST_ROOT/bin/osascript"
export PATH="$TEST_ROOT/bin:$PATH"
export AELOMI_TEST_SCRIPT_ARGS="$TEST_ROOT/osascript-args"
export AELOMI_TEST_FRONTMOST=12345
"$S/app.sh" frontmost "$TEST_ROOT/run"
grep -q 'unix id of first process whose frontmost is true' "$AELOMI_TEST_SCRIPT_ARGS"
grep -q 'with timeout of 60 seconds' "$AELOMI_TEST_SCRIPT_ARGS"
export AELOMI_TEST_FRONTMOST=54321
if "$S/app.sh" frontmost "$TEST_ROOT/run"; then exit 1; else assert_equal "$?" 1; fi
export AELOMI_TEST_APPLE_EVENT_ERROR=1
if "$S/app.sh" frontmost "$TEST_ROOT/run"; then exit 1; else assert_equal "$?" 1; fi
echo 'PASS: frontmost accepts only the run PID and refuses Apple event errors'
