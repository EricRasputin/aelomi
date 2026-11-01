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
shopt -s nullglob
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

"$S/ui-lock.sh" acquire holder >/dev/null
touch -t 200001010000 "$AELOMI_UI_LOCK/owner"
renewed_after=$(date +%s)
"$S/ui-lock.sh" renew holder
[ "$(stat -f %m "$AELOMI_UI_LOCK/owner")" -ge "$renewed_after" ]
if "$S/ui-lock.sh" acquire intruder >/dev/null 2>&1; then exit 1; else assert_equal "$?" 1; fi
assert_equal "$(cat "$AELOMI_UI_LOCK/owner")" holder
echo 'PASS: renew by the owner refreshes the heartbeat, so a waiter cannot reclaim the lock'

touch -t 200001010000 "$AELOMI_UI_LOCK/owner"
if "$S/ui-lock.sh" renew intruder 2>"$TEST_ROOT/renew-refused"; then exit 1; else assert_equal "$?" 1; fi
assert_equal "$(cat "$TEST_ROOT/renew-refused")" 'ui-lock.sh: renew refused: the lock is held by holder'
assert_equal "$(cat "$AELOMI_UI_LOCK/owner")" holder
assert_equal "$(stat -f %Sm -t %Y%m%d%H%M "$AELOMI_UI_LOCK/owner")" 200001010000
"$S/ui-lock.sh" release holder
if "$S/ui-lock.sh" renew holder 2>"$TEST_ROOT/renew-free"; then exit 1; else assert_equal "$?" 1; fi
assert_equal "$(cat "$TEST_ROOT/renew-free")" 'ui-lock.sh: renew refused: the lock is not held'
assert_equal "$("$S/ui-lock.sh" status)" free
echo 'PASS: renew by a non-owner or on a free lock fails and changes nothing'

"$S/ui-lock.sh" acquire guarded >/dev/null
python3 - "$AELOMI_UI_LOCK.guard" "$TEST_ROOT/guard-ready" <<'PYTEST' &
import fcntl
from pathlib import Path
import sys
import time
with open(sys.argv[1], "a") as guard:
    fcntl.flock(guard, fcntl.LOCK_EX)
    Path(sys.argv[2]).touch()
    time.sleep(1)
PYTEST
guard_pid=$!
for n in $(seq 1 100); do
  [ ! -f "$TEST_ROOT/guard-ready" ] || break
  sleep 0.05
done
[ -f "$TEST_ROOT/guard-ready" ]
"$S/ui-lock.sh" release guarded
wait "$guard_pid"
assert_equal "$("$S/ui-lock.sh" status)" free
echo 'PASS: release waits out a briefly held guard instead of timing out'

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

unset AELOMI_TEST_APPLE_EVENT_ERROR
cat >"$TEST_ROOT/bin/pnpm" <<'MOCK'
#!/usr/bin/env bash
sleep 120 &
echo 'error: could not compile `aelomi`'
wait
MOCK
chmod +x "$TEST_ROOT/bin/pnpm"
export AELOMI_VERIFY_ROOT="$TEST_ROOT/verify"
"$S/ui-lock.sh" acquire holder >/dev/null
rm -f "$AELOMI_TEST_SCRIPT_ARGS"
if env -u AELOMI_UI_LOCK_LABEL "$S/app.sh" keys "$TEST_ROOT/run" x 2>"$TEST_ROOT/no-label"; then exit 1; else assert_equal "$?" 1; fi
grep -q 'set AELOMI_UI_LOCK_LABEL' "$TEST_ROOT/no-label"
for input in "press $TEST_ROOT/run Ping" "focus $TEST_ROOT/run Ping" "keys $TEST_ROOT/run x" "rename-quit $TEST_ROOT/run" "both-quit $TEST_ROOT/run" "launch dev"; do
  read -r -a argv <<<"$input"
  if AELOMI_UI_LOCK_LABEL=intruder "$S/app.sh" "${argv[@]}" 2>"$TEST_ROOT/refused"; then exit 1; else assert_equal "$?" 1; fi
  grep -q "the keyboard lock is not held by 'intruder'" "$TEST_ROOT/refused"
done
[ ! -e "$AELOMI_TEST_SCRIPT_ARGS" ]
[ ! -e "$AELOMI_VERIFY_ROOT" ]
[ ! -e "$TEST_ROOT/run/actions.log" ]
assert_equal "$(cat "$AELOMI_UI_LOCK/owner")" holder
touch -t 200001010000 "$AELOMI_UI_LOCK/owner"
renewed_after=$(date +%s)
AELOMI_UI_LOCK_LABEL=holder "$S/app.sh" keys "$TEST_ROOT/run" x >/dev/null
grep -qx keys "$AELOMI_TEST_SCRIPT_ARGS"
[ "$(stat -f %m "$AELOMI_UI_LOCK/owner")" -ge "$renewed_after" ]
echo 'PASS: input commands and launch refuse before osascript unless the caller holds the lock, and renew it when it does'

if AELOMI_UI_LOCK_LABEL=holder "$S/app.sh" launch dev >"$TEST_ROOT/launch.out" 2>"$TEST_ROOT/launch.err"; then exit 1; else assert_equal "$?" 1; fi
assert_equal "$(cat "$TEST_ROOT/launch.out")" ''
grep -q 'launch failed: the build or the app failed' "$TEST_ROOT/launch.err"
grep -q 'stopped every process this run started' "$TEST_ROOT/launch.err"
launch_runs=("$AELOMI_VERIFY_ROOT"/*)
assert_equal "${#launch_runs[@]}" 1
grep -q $'\tsleep 120$' "${launch_runs[0]}/pids"
while IFS=$'\t' read -r pid cmd; do
  if [ "$(ps -ww -o command= -p "$pid" 2>/dev/null || true)" = "$cmd" ]; then exit 1; fi
done <"${launch_runs[0]}/pids"
"$S/ui-lock.sh" release holder
echo 'PASS: a failed launch stops every process it started'
