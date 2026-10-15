# DATA-5 review follow-ups: compaction refuses a doc it can't compare, on 92b47c8

This record proves source commit `92b47c80f5a7f26d37c532fa8ccb6e42d9300678`, tested against merge base `7483d3c21e74bc34ec51004e023c8e071b7a34d1`, patch-id `7ea7a4277200f6f7dfe80046477067d7d8c39548`. It proves these paths:

- `crates/core/src/storage/equality.rs`
- `crates/core/src/storage/compact.rs`

## The change

Compaction deletes a doc's logs only after its new snapshot loads to a doc with the same fingerprint. The fingerprint reads a branch that holds text only as text runs. yrs's text diff leaves out values, binaries and subdocs, and it shows a string embed as text. So a snapshot that changed one of those beside text would still verify. The PR's review raised this. Now such a branch has no fingerprint, and compaction skips the doc and keeps its logs.

## Environment

- **Machine.** Apple M1, macOS 27.0.1, on AC power, Rust 1.98.1.
- **Code.** The run used head `9349db4`. Its non-test code under `crates/core/src` is identical to `92b47c8`: the later commits only move tests and edit `ARCHITECTURE.md`.
- **Data.** The run used a new synthetic folder under `~/.local/state/aelomi-verify/`. That folder stays local.

## Results

**The refusal.** Two new tests cover it:

- `a_branch_whose_text_runs_hide_content_is_not_compared` builds twin docs that match in every operation ID and in their state vectors, and differ only in a value beside text or in a string embed beside text. It asserts that neither twin has a fingerprint.
- `a_doc_whose_snapshot_cannot_be_checked_skips_compaction_and_keeps_its_log` asserts the doc keeps exactly its log, byte for byte.

Both fail when the new check is forced to pass:

```
assertion `left == right` failed: values beside text: a
```
```
  left: ["snapshot-0000000001.ysnap2"]
 right: ["log-0000000000-0000000000.ylog2"]
```

With the check, `cargo nextest run --workspace` passed 357 of 357 tests. That includes the attributed BlockNote page and the every-shape test, which still compare equal and compact.

**A real page still compacts.** [d70c-proof.sh.txt](d70c-proof.sh.txt) repeats the #70 proof from the [d37c4ec record](../data-5-review-followups-d37c4ec/README.md). It seeds the 10,000-page fixture and the 5,000-block BlockNote page through the real Store, and the Store compacts the big page as it closes:

```
seeded ~/.local/state/aelomi-verify/92b47c8-d70c/seed with the search fixture: pages=10001 copies=0 big_page=true updates=10003 fixture_crc32=48c7dc26 source_files=10005 source_bytes=13642100 seconds=2.4
snapshot-0000000001.ysnap2
```

The big page's folder held only the snapshot. The log printed no `skipped compacting` line.
