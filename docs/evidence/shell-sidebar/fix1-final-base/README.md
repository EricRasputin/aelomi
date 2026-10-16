# Sidebar fix round 1: final base

Source commit: `1d9ab6c9046c3d921df513967a93a649b035e442`. Base branch: `origin/feat/shell-page-records`.
Recorded merge base and #84 head: `42c47c0357b4dea9c75200350dc4681360a12b55`.
Verbatim patch ID: `06d65cf5ea085c6005f714ab99b761fbda3087b8`.

Pathspecs proved, relative to the repository root:

```text
src/components/sidebar.tsx
src/pages/page-view.tsx
src/pages/page-route.tsx
src/stores/page-titles.ts
src/stores/page-tree.ts
src/stores/tree-expansion.ts
src/stores/shell.ts
src/shell/use-page-commands.ts
src/palette/page-commands.ts
src/router.tsx
src/styles.css
crates/core/tests/rename_latency.rs
```

## Final API and retained proof

D1 uses `useRecordCommandStore` from `src/stores/record-commands.ts` and the wrappers' ok/error/retrying results. D1's earlier observer addition to #84's `src/ipc/record-commands.ts` is removed. That file and its tests match the recorded #84 head byte for byte. Retrying titles keep a draft with no per-title error; retrying creates navigate to their returned ID without mounting an editor before the record exists; retrying trash waits for source recovery. Final action errors show failures.

The rebase changed only three #84 evidence files relative to `4b326064c65c23f413b644e084ae35e3c8ed0801`; all product and test source is identical. The [record API proof](../fix1-record-api/README.md) and [original round-one proof and SPEC-to-case map](../fix1/README.md#proving-cases) therefore still prove the selected source paths. The former replaces the latter's historical observer test with five title-store cases and three retained-create/action-result WebKit cases. All eight Act items and six Consider decisions are mapped there. Earlier records remain unchanged.

## Fresh native rerun

Environment: Apple M1, eight logical CPUs, arm64, macOS 27.0.1 (26A434), native Tauri 2 WKWebView. Free space before the build was 91 GiB. Run: `~/.local/state/aelomi-verify/20261016-065955-dev-WZZb`, with a fresh synthetic folder created by app.sh. Its doctor checks all passed. D1 waited for the prior live keyboard-lock owner, held its own lock only during interaction, and released it before the disk reader. The app exited normally and cleanup found no owned process remaining.

Command: `bash docs/evidence/shell-sidebar/fix1-record-api/run.sh.txt`. That archived script temporarily installs its probe, restores index.html, removes the probe, restores appearance, and cleans up on exit. It drives retained creation, recovery and automatic title focus; types a title and body; holds renames through I/O failure, then recovers; holds trash through I/O failure, then recovers to Getting started; creates another page; and arms composition immediately before pressing the target PID's native Quit menu item. The fault controls change transport outcomes; storage writes use real Rust storage. The probe is absent from shipped source.

Acknowledgement and quit lines:

```text
DRAFT=none;RETAINED=0;ERROR=none
quit draining via=ExitRequested revision=1
quit flush via=CompleteQuit pending=None
quit confirmed via=drain
quit allowed via=ExitRequested confirmed=true
quit flush via=Exit pending=None
```

After process exit, the reader asserts exactly three records, one trashed record, and exactly one body block containing only `Text("body token")` on the trashed record. Exact readback:

```text
active_titles=["Getting started", "日本語の題名"] trashed_titles=["Pending native title"]
```

The required read-only real-data directory listings before the first launch and after this run match exactly. Earlier light/dark layout, unknown-title recovery, selection versus hover, palette over one-line/wrapped titles, collapse/relaunch, final refusals and stress proof remain in the linked records; their product bytes are unchanged. This fresh rerun captured dark mode. Only the U.S. input source is enabled: event-level composition and exact Japanese persistence are proved; hardware Japanese conversion Return remains unverified.

## Screenshots and limits

All three new frames were opened and inspected, posted in [the final-base PR comment](https://github.com/aelomi/aelomi/pull/89#issuecomment-6205999527), read back, downloaded without authentication, opened as PNGs and matched byte-for-byte. The same readback verification passed for all 43 published images. Logs and accessibility trees remain local. The earlier records retain their own image sizes and hashes.

| Filename | Bytes | SHA-256 |
| --- | ---: | --- |
| `final-base-create-retrying.png` | 151608 | `eae7086086e43d9d5c8ac5fde6805351f721520f8a11df80611f036c6ac06141` |
| `final-base-title-retrying.png` | 161046 | `be02b749483222c7b68ee9db00ab3a42993d7187cdf6d330f3d2ba1b210a29ca` |
| `final-base-trash-retrying.png` | 160693 | `fd2d88e6d871fb1417751b27d53658dd16de385bfe86be044300c7cd6473506e` |

The counted release measurement in the API record applies to the identical runtime: 100 measured successes, zero failures, with exact readbacks. It establishes no typing-budget pass. At 10,000 pages the deadline is missed by the code's structure for keystrokes closer together than one edit; [#91](https://github.com/aelomi/aelomi/issues/91) tracks incremental edits and projection. Narrow/deep tree polish remains [#87](https://github.com/aelomi/aelomi/issues/87).

The final committed-head `pnpm check:all` JSON is separate from this native record and is named in the PR body. Its node shim takes the same keyboard lock for the repository's isolated cold-start harness. The pre-rebase gate was interrupted during performance work and is not a passing gate record.
