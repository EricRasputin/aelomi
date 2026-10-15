# Settings review fix round 2

This is builder self-proof for PR #83. Independent review and verification remain with the orchestrator.

## Source and environment

- Source commit `041cc5da954547e8a2d53795cea6ab3c2606993d`.
- Merge base `a4db19d8ab2e1ac072dea8bc3a1cc3747af8fdae`, computed against `origin/main`.
- Patch-id `215301a7512f88cbd6023f317cccd45fbcb96e5e`.
- Proven pathspecs are `crates/core/src/`, `src-tauri/src/`, `src/ipc/pending-commands.ts`, `src/ipc/index.ts`, `src/ipc/mock.ts`, `src/editor/quit-drain.ts`, `src/stores/quit.ts`, and `src/components/storage-notices.tsx`.
- The evidence commit also refreshes one generated QuitPhase doc comment in bindings.ts. Commands, types, and runtime behavior are unchanged by that documentation refresh. That generated comment is outside the proven pathspecs.
- macOS 27.0.1, build 26A434, Apple Silicon arm64. Native debug Tauri app with real WKWebView IPC.
- Every folder contains synthetic configuration only. No user data folder was opened.

## Data shape and execution

Settings use one cached session snapshot, an ordered recent-page list capped at 50, and a sorted expanded-page set.
`Pending { docs: Vec<DocId>, settings: bool }` keeps settings distinct from documents through flush, close, health, and quit.
A `DurableFile` token proves that the replacement source completed its sync before publication.

Model the Domain drove the pending-edit shape. Type System Discipline drove the durable-source token.
Test Behavior, Not Implementation drove the real frontend command-boundary regression and exact reopened snapshots.
Sequence Work into Verifiable Units drove the failing regressions before the fixes. Prove It Works drove the native run and attachment checks.
One builder owned the shared APIs. Retry, writer, pending-state, and IPC edits were checked in order without concurrent file writers.

## Act on findings and proving tests

1. The failing attempt carries its captured sequence. A newer patch stays immediately eligible and never receives the older retry's error. `crates/core/src/storage/settings_tests.rs`, `shell_10_an_older_retry_cannot_fail_or_defer_a_newer_settings_patch`, failed on the previous implementation and passes with the fix.
2. The frontend boundary tracks settings invocations through a generic promise tracker. Quit awaits all outstanding commands, including those added during a drain. `src/ipc/quit-invocations.test.ts`, `SHELL-9 the real settings command boundary drains invokes held before native admission`, failed because complete_quit ran before the held settings dispatch, then passed after the fix. This exercises the generated command and invoke boundary; the native disk path is proven separately below.
3. The writer collects, syncs, and acknowledges documents before settings in the same pass. Each side has its own unwind boundary. `shell_9_a_document_is_acknowledged_before_a_held_settings_write_in_the_same_pass` failed before the fix. `shell_10_a_settings_panic_does_not_fail_documents_and_a_document_panic_does_not_fail_settings` checks both panic directions and exact reopened values. Both live in `crates/core/src/storage/settings_tests.rs`. Replacement consumes the token from create_durable and does not sync the source again.
4. Store flush and close return both pending kinds. Close returns the store while settings remain pending. `shell_10_close_returns_the_store_while_settings_are_pending` failed before the fix. IPC health and quit expose pending_settings, and QuitGate accepts DocId slices. `src-tauri/src/lib.rs` covers registered and unregistered quit, a mixed failed frontend drain, and full recovery. `src/stores/storage-events.test.ts` covers the typed frontend decisions.
5. `crates/core/src/storage/fs.rs`, `data_4_os_publication_requires_source_sync_and_preserves_the_old_file_on_sync_failure`, observes the production creation and OsFs replacement path. A failed source sync leaves the old destination unchanged. A successful write orders source sync, publication, and directory sync. Temporarily removing the source full_sync made this test fail because publication incorrectly returned success. The sync was restored before the source commit.

## Consider decisions

- Keep raw list fields when any element is unparsable. Readers still project valid IDs. This preserves a newer build's shapes through unrelated writes. `data_4_each_unparsable_field_survives_an_unrelated_patch` now includes mixed future-shaped lists and rejected IDs. It failed before the fix. Explicit patches replace the retained raw field.
- Queue under the Workspace lock, then release it before waiting. Reads remain available during disk I/O, and quick patches coalesce. `src-tauri/src/settings/tests.rs`, `data_4_reads_do_not_wait_for_disk_and_quick_patches_coalesce`, reads the cumulative cache during a held write and proves three patches use two writes.
- Merge mock frontend delivery failures with retained settings and document failures. `src/ipc/mock-settings.test.ts`, `SHELL-10 a failed frontend drain retains both refused documents and settings`, failed before the fix. It now asserts sorted, deduplicated document IDs and pending_settings true.
- Preserve typed write failures over IPC as SettingsError::Write(FailReason). The owner tests distinguish write failures from unreadable settings. The native failure screenshot shows Write PermissionDenied, and the existing mock settings test asserts the Write error variant.

## SPEC coverage

- DATA-4. Real-folder and simulated session round trips, every atomic crash step, unknown values, raw-list preservation, concurrent patches, and coalescing. Cases are in `crates/core/src/storage/settings_tests.rs` and `src-tauri/src/settings/tests.rs`.
- SHELL-9 for settings edits. Queued writes complete durably within the controllable-clock budget. Held writes and pre-admission frontend invocations delay quit. A document acknowledgement precedes a held settings write. Native immediate quit and relaunch preserve the exact session.
- SHELL-10 for settings edits. Retained settings retry, clear health, keep close from discarding the snapshot, and appear separately in quit confirmation. Mixed mock failures match the native aggregation.

SHELL-3 restoration UI, SHELL-5 theme controls, TREE-6 tree UI, and CMD-2 recent-page UI remain in later PRs. This PR provides their persisted fields.

## Native procedure and observations

[replay.sh.txt](replay.sh.txt) records the commands and the immediate-quit accessibility action.
The skill helper doctor passed for each successful launch. Screenshots were opened and inspected.

Local runs are retained under `~/.local/state/aelomi-verify/`:

- `20261015-233227-dev-Ei3m`. Defaults, full session write, permission failure banner, settings-specific confirmation, Keep app open, automatic recovery without another patch, and normal quit.
- `20261015-233457-dev-v8AE`. Fresh defaults, then one accessibility action pressed Write session settings and sent Command-Q without waiting for the reply. Exit completed and settings.json equaled the exact expected full snapshot.
- `20261015-233553-dev-1V2h`. Relaunched on the second run's data. The displayed settings_get result equaled the full disk snapshot without another write.

The first run's shell log contains:

```text
quit draining via=ExitRequested revision=1
quit flush via=CompleteQuit pending=Some(Pending { docs: [], settings: true })
quit draining via=ExitRequested revision=4
quit flush via=CompleteQuit pending=None
quit confirmed via=drain
quit allowed via=ExitRequested confirmed=true
quit flush via=Exit pending=None
```

The immediate-quit run contains the full successful updated snapshot followed by:

```text
quit draining via=ExitRequested revision=1
quit flush via=CompleteQuit pending=None
quit confirmed via=drain
quit allowed via=ExitRequested confirmed=true
quit flush via=Exit pending=None
```

The immediate-exit and reopened disk checks require exactly:

```json
{"theme":"Dark","openPage":{"pageId":"probe-page","topVisibleBlockId":"probe-block"},"recentPages":["probe-page","older-page"],"expandedRows":["parent-page","probe-page"]}
```

The dev probe keeps its last command error after automatic recovery. The failure banner clears correctly. The native run does not claim to hold the disk operation; deterministic held-write and pre-admission tests prove those interleavings.

## Screenshots

[Native self-proof comment](https://github.com/aelomi/aelomi/pull/83#issuecomment-6198935261) holds all seven images. Each was downloaded without authentication and matched the inspected original byte for byte, including its size and SHA-256.

- `20261015-233227-dev-Ei3m/settings-before.png`, 91749 bytes, SHA-256 `f6f9e34203ae03587767a25f7ca0960b6f529c808722617857c2c3811aaa44e0`.
- `20261015-233227-dev-Ei3m/settings-written.png`, 114185 bytes, SHA-256 `529e44a3594b8a30e1416bc062d9d8f14c79f9768f0c1c807451f94b842220b7`.
- `20261015-233227-dev-Ei3m/settings-failed.png`, 111083 bytes, SHA-256 `2f0d8b7e9edd207b9a9bcc3693ed671cd453d030aec139d4ab1f17db8d390ab9`.
- `20261015-233227-dev-Ei3m/settings-confirmation.png`, 193000 bytes, SHA-256 `47ff20ade66d9ddb047a99be039ffe283fb1ea3b29adde0d02e6693142fd6ef1`.
- `20261015-233227-dev-Ei3m/settings-recovered.png`, 82771 bytes, SHA-256 `5af53f424ec25ae3f3c96c045a066c424bfb834f9fdaba491475edfe90be7fa3`.
- `20261015-233457-dev-v8AE/immediate-before.png`, 96580 bytes, SHA-256 `3a815d923571c0b70d9be72da515d3d93dad21438d31e1e96a77e1a3f3242397`.
- `20261015-233553-dev-1V2h/settings-reopened.png`, 114185 bytes, SHA-256 `529e44a3594b8a30e1416bc062d9d8f14c79f9768f0c1c807451f94b842220b7`.
