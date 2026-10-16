#!/usr/bin/env bash
set -euo pipefail

exec python3 - "$@" <<'PY'
import fcntl
import os
from pathlib import Path
import sys
import time


def fail(message, code=1):
    print(f"ui-lock.sh: {message}", file=sys.stderr)
    sys.exit(code)


args = sys.argv[1:]
if not (args == ["status"] or (
    len(args) == 2 and args[0] in ("acquire", "release") and args[1].strip()
    and "\n" not in args[1] and "\r" not in args[1]
)):
    fail("usage: ui-lock.sh acquire <label> | release <label> | status", 2)

lock = Path(os.environ.get("AELOMI_UI_LOCK", "/tmp/aelomi-ui.lock"))
owner = lock / "owner"
command = args[0]
label = args[1] if len(args) == 2 else None
wait_seconds = 45 * 60
if "AELOMI_UI_LOCK" in os.environ:
    try:
        wait_seconds = int(os.environ.get("AELOMI_UI_LOCK_WAIT_SECONDS", wait_seconds))
    except ValueError:
        fail("AELOMI_UI_LOCK_WAIT_SECONDS must be an integer", 2)
    if not 0 <= wait_seconds <= 45 * 60:
        fail("AELOMI_UI_LOCK_WAIT_SECONDS must be between 0 and 2700", 2)
deadline = time.monotonic() + wait_seconds


def read_owner():
    try:
        return owner.read_text().rstrip("\n")
    except FileNotFoundError:
        return None


def remove_lock():
    owner.unlink(missing_ok=True)
    lock.rmdir()


def attempt():
    if command == "status":
        print(f"held: {read_owner() or '(owner not recorded)'}" if lock.exists() else "free")
        return True
    if command == "release":
        if not lock.exists():
            return True
        if read_owner() != label:
            fail("release refused: owner label does not match")
        remove_lock()
        return True
    if lock.exists():
        stamp = owner.stat() if owner.exists() else lock.stat()
        if time.time() - stamp.st_mtime <= 30 * 60:
            return False
        remove_lock()
    try:
        lock.mkdir()
    except FileExistsError:
        return False
    owner.write_text(label + "\n")
    print(f"acquired: {label}")
    return True


try:
    # Keep this inode: unlinking a flock file lets waiters lock different files.
    with open(f"{lock}.guard", "a") as guard:
        while True:
            try:
                fcntl.flock(guard, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except BlockingIOError:
                pass
            else:
                try:
                    if attempt():
                        break
                finally:
                    fcntl.flock(guard, fcntl.LOCK_UN)
            if time.monotonic() >= deadline:
                fail("timed out waiting for the UI lock")
            time.sleep(min(1, max(0, deadline - time.monotonic())))
except OSError as error:
    fail(str(error))
PY
