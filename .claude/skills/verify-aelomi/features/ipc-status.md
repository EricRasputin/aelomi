# IPC status line

The status landmark shows whether the UI can reach the Rust core. On launch it reads `Connecting to the core…`, then `IPC ok · core <version> · seq 1`. Each press of **Ping again** sends one more `ping` command, and the sequence number goes up by one. Hovering the button shows the tooltip `Check the connection to the Rust core`.

## Sub-features

- `ipc-initial` shows `IPC ok · core 0.1.0 · seq 1` after launch.
- `ipc-ping-again` increases the sequence number by one per press of **Ping again**, and the core logs `ping seq=N` for each call.
- `ipc-tooltip` shows `Check the connection to the Rust core` while the pointer is over **Ping again**.
- `ipc-error` shows `IPC failed · <message>` when the command fails. No user path triggers it today.

## How to get to it (user POV)

- In a dev or mock build, open `/dev/ipc`. The diagnostic shows the status line above **Ping again**.
- Press the **Ping again** button below the status line.
- Hover the pointer over **Ping again**.

## Driving it with app.sh and mock-drive

Preconditions:

- `RUN=$($S/app.sh launch dev)` printed a run directory, and `$S/app.sh doctor "$RUN"` reports every line as `ok`.
- The run is on `/dev/ipc` and has not pressed **Ping again** yet.

- **Initial status.** Run `$S/app.sh wait-status "$RUN" "IPC ok · core 0.1.0 · seq 1"`. It prints the same text. `grep '^ping seq=' "$RUN/shell.log"` prints only `ping seq=1`.
- **Before state.** Run `$S/app.sh screenshot "$RUN" before`. `before.png` shows `IPC ok · core 0.1.0 · seq 1` above the **Ping again** button.
- **Ping again.** Press the button. Run `$S/app.sh press "$RUN" "Ping again"`. It prints `pressed "Ping again"`.
- **After state.** Run `$S/app.sh wait-status "$RUN" "IPC ok · core 0.1.0 · seq 2"` and `$S/app.sh screenshot "$RUN" after`. `after.png` shows `seq 2`.
- **Core side effect.** Run `grep '^ping seq=' "$RUN/shell.log"`. The output is `ping seq=1` and then `ping seq=2`.
- **Tooltip, mock browser.** Run `$S/mock-drive.mjs --flow $S/flows/tooltip.mjs`. It prints `PASS`, and `tooltip.aria.yml` contains `tooltip "Check the connection to the Rust core"`.
- **Round trip, mock browser.** Run `$S/mock-drive.mjs`. It prints `PASS`. `summary.json` lists two `ping` calls with `seq` 1 and 2, and `nonLocalRequests` is empty. This proves the UI against the mock only.
- **Proof.** Keep `before.png`, `after.png`, their `.ax.txt` files, `actions.log`, and `shell.log` from the real-app run.

## Gotchas

- **Ping again** is disabled while a ping is pending. Press it only after the status reads `IPC ok`.
- The reply arrives after `press` returns. Wait with `wait-status`, never `sleep`.
- The mock build reads `core 0.1.0-mock`. The real app reads `core 0.1.0`, the workspace version in `Cargo.toml`.
- The sequence starts at 1 on every launch. To prove `seq 2`, start from a fresh launch or read the current value with `status` first.
- The tooltip appears on hover. `AXPress` does not hover, so prove the tooltip on the mock browser.
- `ipc-error` needs a failing command. Do not fake it by editing the mock during a verification run. Report it as unreachable.
