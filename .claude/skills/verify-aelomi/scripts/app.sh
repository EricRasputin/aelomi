#!/usr/bin/env bash
set -euo pipefail

HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)
REPO=$(cd "$HERE/../../../.." && pwd -P)
AX="$HERE/ax.js"
VERIFY_ROOT=${AELOMI_VERIFY_ROOT:-${XDG_STATE_HOME:-$HOME/.local/state}/aelomi-verify}
BUNDLE="$REPO/target/release/bundle/macos/Aelomi.app"

usage() {
  cat <<'EOF'
Usage: app.sh <command> [args]
  launch dev|app [--timeout S]   start the app, wait for "ping seq=1", print the run directory
  frontmost RUN                  exit 0 only when this run's app PID is frontmost
  doctor RUN                     check that the instance in RUN is worth driving
  tree RUN                       print the web content's accessibility tree
  status RUN                     print the text of the role="status" region
  wait-status RUN TEXT [S]       wait up to S seconds (default 10) until the status reads TEXT
  wait-dialog RUN TITLE [S]      wait up to S seconds for an alert dialog with TITLE
  press RUN NAME                 press the element named NAME (AXPress, no focus change)
  focus RUN NAME                 focus the element named NAME
  keys RUN TEXT [MODS]           bring Aelomi to the front and type TEXT (MODS: command,shift,option,control)
  rename-quit RUN                 rename Getting started and send native quit immediately
  both-quit RUN                   write session settings, rename, and send native quit immediately
  menus RUN                      print the native menu bar titles
  screenshot RUN NAME            save RUN/NAME.png of the window and RUN/NAME.ax.txt with its bounds and tree
  cleanup RUN                    stop every process this run started; keeps RUN and its evidence
EOF
}

die() {
  echo "app.sh: $*" >&2
  exit 1
}

ax() { osascript -l JavaScript "$AX" "$@"; }

proc_cmd() { ps -ww -o command= -p "$1" 2>/dev/null || true; }

descendants() {
  local child
  for child in $(ps -axo pid=,ppid= | awk -v parent="$1" '$2 == parent { print $1 }'); do
    echo "$child"
    descendants "$child"
  done
}

port_owner() { lsof -nP -t -iTCP:"$1" -sTCP:LISTEN 2>/dev/null | head -1 || true; }

# Appends the launch process and its live descendants to RUN/pids as "pid<TAB>command".
record_tree() {
  local run=$1 root=$2 pid cmd
  touch "$run/pids"
  for pid in "$root" $(descendants "$root"); do
    cmd=$(proc_cmd "$pid")
    [ -n "$cmd" ] || continue
    grep -qxF "$pid	$cmd" "$run/pids" || printf '%s\t%s\n' "$pid" "$cmd" >>"$run/pids"
  done
}

# Prints recorded PIDs that still run the recorded command, so a reused PID is never touched.
live_pids() {
  local pid cmd
  [ -f "$1/pids" ] || return 0
  while IFS=$'\t' read -r pid cmd; do
    if [ "$(proc_cmd "$pid")" = "$cmd" ]; then echo "$pid"; fi
  done <"$1/pids" | sort -u
}

load() {
  RUN=${1:-}
  [ -n "$RUN" ] && [ -f "$RUN/run.env" ] || die "no run.env in '${RUN}'. Pass the directory printed by launch."
  source "$RUN/run.env"
}

app_bin() {
  local exe
  exe=$(plutil -extract CFBundleExecutable raw "$BUNDLE/Contents/Info.plist" 2>/dev/null) || return 1
  echo "$BUNDLE/Contents/MacOS/$exe"
}

require_app() {
  [ -n "${APP_PID:-}" ] || die "launch never reached readiness in $RUN, so there is no app PID. Run cleanup."
}

log_action() { printf '%s %s\n' "$(date +%H:%M:%S)" "$*" >>"$RUN/actions.log"; }

launch() {
  local mode=${1:-} timeout=900
  shift || true
  while [ $# -gt 0 ]; do
    case $1 in
      --timeout) timeout=$2; shift 2 ;;
      *) die "unknown option $1" ;;
    esac
  done
  local cmd
  case $mode in
    dev) cmd=(pnpm dev:app) ;;
    app)
      local bin
      bin=$(app_bin) && [ -x "$bin" ] || die "no built app at $BUNDLE. Run: pnpm tauri build --bundles app"
      cmd=("$bin")
      ;;
    *) usage >&2; exit 2 ;;
  esac

  mkdir -p "$VERIFY_ROOT"
  RUN=$(mktemp -d "$VERIFY_ROOT/$(date +%Y%m%d-%H%M%S)-$mode-XXXX")
  (cd "$REPO" && export AELOMI_DATA_DIR="${AELOMI_DATA_DIR:-$RUN/data}" AELOMI_RECOVERY_DIR="${AELOMI_RECOVERY_DIR:-$RUN/recovery}" && exec nohup "${cmd[@]}" >"$RUN/shell.log" 2>&1 </dev/null) &
  local launch_pid=$!
  printf 'MODE=%q\nLAUNCH_PID=%q\nREPO=%q\n' "$mode" "$launch_pid" "$REPO" >"$RUN/run.env"
  echo "app.sh: started ${cmd[*]} as PID $launch_pid, log $RUN/shell.log" >&2

  local deadline=$((SECONDS + timeout))
  until grep -q '^ping seq=1$' "$RUN/shell.log"; do
    record_tree "$RUN" "$launch_pid"
    local reason=""
    kill -0 "$launch_pid" 2>/dev/null || reason="the launch process exited"
    grep -qE 'could not compile|panicked at' "$RUN/shell.log" && reason="the build or the app failed"
    [ "$SECONDS" -lt "$deadline" ] || reason="no 'ping seq=1' within $timeout s"
    if [ -n "$reason" ]; then
      tail -20 "$RUN/shell.log" >&2
      die "launch failed: $reason. Run: $0 cleanup $RUN"
    fi
    sleep 1
  done
  record_tree "$RUN" "$launch_pid"

  local app_pid=$launch_pid pid
  if [ "$mode" = dev ]; then
    local port
    port=$(sed -n 's/^AELOMI_DEV_PORT=\([0-9]*\)$/\1/p' "$RUN/shell.log" | head -1)
    [ -n "$port" ] || die "logged 'ping seq=1' but no AELOMI_DEV_PORT line. Run: $0 cleanup $RUN"
    printf 'DEV_PORT=%q\n' "$port" >>"$RUN/run.env"
    app_pid=""
    for pid in $(descendants "$launch_pid"); do
      case $(proc_cmd "$pid") in */target/debug/aelomi) app_pid=$pid ;; esac
    done
    [ -n "$app_pid" ] || die "logged 'ping seq=1' but found no target/debug/aelomi under PID $launch_pid. Run: $0 cleanup $RUN"
  fi
  printf 'APP_PID=%q\n' "$app_pid" >>"$RUN/run.env"
  # The first accessibility query after launch can find no web content, so poll until one does.
  local tries=0 err=""
  until err=$(ax status "$app_pid" 2>&1 >/dev/null); do
    tries=$((tries + 1))
    if [ "$tries" -ge 30 ]; then
      echo "app.sh: the web content is not in the accessibility tree yet: $err. Run doctor." >&2
      break
    fi
    sleep 0.5
  done
  echo "app.sh: ready, app PID $app_pid" >&2
  echo "$RUN"
}

frontmost() {
  load "$1"
  require_app
  local pid
  pid=$(osascript -e 'with timeout of 60 seconds
    tell application "System Events" to get unix id of first process whose frontmost is true
  end timeout') || return 1
  [ "$pid" = "$APP_PID" ] || die "frontmost PID $pid is not this run's app PID $APP_PID; send no input."
}

doctor() {
  load "$1"
  local failed=0 out="$RUN/doctor.txt"
  : >"$out"
  check() {
    local label=$1
    shift
    if "$@" >/dev/null 2>&1; then
      echo "ok    $label" | tee -a "$out"
    else
      echo "FAIL  $label" | tee -a "$out"
      failed=1
    fi
  }
  is_ours() { [ -n "$(live_pids "$RUN" | grep -x "$1")" ]; }
  check "launch process $LAUNCH_PID runs the command this run started" is_ours "$LAUNCH_PID"
  check "app process ${APP_PID:-none} is running" is_ours "${APP_PID:-none}"
  check "System Events has Accessibility access (System Settings > Privacy & Security > Accessibility)" \
    test "$(osascript -e 'tell application "System Events" to get UI elements enabled' 2>/dev/null)" = true
  check "app window is on screen" ax window "${APP_PID:-0}"
  check "web content has a status region or an alert dialog" ax content "${APP_PID:-0}"
  if [ "$MODE" = dev ]; then
    local owner=""
    [ -z "${DEV_PORT:-}" ] || owner=$(port_owner "$DEV_PORT")
    check "port ${DEV_PORT:-unknown} is served by this run (owner PID ${owner:-none})" is_ours "${owner:-none}"
  else
    local newer
    newer=$(find "$REPO/src" "$REPO/src-tauri/src" "$REPO/src-tauri/tauri.conf.json" "$REPO/src-tauri/Cargo.toml" "$REPO/crates" "$REPO/index.html" -newer "$(app_bin)" -type f -print -quit 2>/dev/null || true)
    check "built app is newer than the sources${newer:+ (stale: $newer is newer)}" test -z "$newer"
  fi
  check "shell log has 'ping seq=1'" grep -q '^ping seq=1$' "$RUN/shell.log"
  return "$failed"
}

screenshot() {
  load "$1"
  require_app
  local name=${2:?screenshot needs a name}
  local info id
  info=$(ax window "$APP_PID")
  id=$(sed -E 's/^id=([0-9]+).*/\1/' <<<"$info")
  screencapture -x -o -l "$id" "$RUN/$name.png"
  { echo "$info"; ax tree "$APP_PID"; } >"$RUN/$name.ax.txt"
  log_action "screenshot $name ($info)"
  echo "$RUN/$name.png"
}

cleanup() {
  load "$1"
  if live_pids "$RUN" | grep -qx "$LAUNCH_PID"; then record_tree "$RUN" "$LAUNCH_PID"; fi
  local pids
  pids=$(live_pids "$RUN")
  if [ -z "$pids" ]; then
    echo "app.sh: nothing from this run is running"
  else
    kill -TERM $pids 2>/dev/null || true
    local i
    for i in $(seq 1 20); do
      [ -z "$(live_pids "$RUN")" ] && break
      sleep 0.5
    done
    pids=$(live_pids "$RUN")
    [ -z "$pids" ] || kill -KILL $pids 2>/dev/null || true
    sleep 0.5
    pids=$(live_pids "$RUN")
    [ -z "$pids" ] || die "still running after SIGKILL: $pids"
    echo "app.sh: stopped every process this run started"
  fi
  if [ "$MODE" = dev ] && [ -n "${DEV_PORT:-}" ]; then
    local owner
    owner=$(port_owner "$DEV_PORT")
    [ -z "$owner" ] || echo "app.sh: port $DEV_PORT is held by PID $owner ($(proc_cmd "$owner")), which this run did not start" >&2
  fi
  echo "app.sh: evidence kept in $RUN:"
  ls -1 "$RUN"
}

command=${1:-}
shift || true
case $command in
  launch) launch "$@" ;;
  frontmost) frontmost "$@" ;;
  doctor) doctor "$@" ;;
  tree | status | menus) load "${1:-}"; require_app; ax "$command" "$APP_PID" ;;
  wait-status) load "${1:-}"; require_app; ax wait-status "$APP_PID" "${2:?wait-status needs TEXT}" "${3:-10}" ;;
  wait-dialog) load "${1:-}"; require_app; ax wait-dialog "$APP_PID" "${2:?wait-dialog needs TITLE}" "${3:-10}" ;;
  press | focus) load "${1:-}"; require_app; log_action "$command \"${2:?$command needs NAME}\""; ax "$command" "$APP_PID" "$2" ;;
  keys) load "${1:-}"; require_app; log_action "keys \"${2:?keys needs TEXT}\" ${3:-}"; ax keys "$APP_PID" "$2" "${3:-}" ;;
  rename-quit | both-quit) load "${1:-}"; require_app; log_action "$command"; ax "$command" "$APP_PID" ;;
  screenshot) screenshot "$@" ;;
  cleanup) cleanup "$@" ;;
  *) usage >&2; exit 2 ;;
esac
