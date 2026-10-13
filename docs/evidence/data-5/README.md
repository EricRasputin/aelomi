# DATA-5 and SHELL-12 native verification

The release app opened with a missing, garbage, and old-schema index. Every run logged `Rebuilding` followed by `Ready`, kept all source files unchanged, and quit with no pending saves. The native window remained usable through a second IPC ping.

The tested source commit is `6e7f10cddedf6169e392b613eb4539e890c7d047`. The separate commit that adds this record publishes evidence only. No product code changed after the source squash.

The runs used macOS 27.0.1, build 26A434, on an eight-core Apple M1, Aelomi 0.1.0, and the ad hoc signed release `.app`. The capture mode was `app`. Verification ran on 2026-10-14 in Asia/Kolkata using [verify-aelomi](../../../.claude/skills/verify-aelomi/SKILL.md), with isolated synthetic data under `/tmp/aelomi-data5-native.KLBOtO`. No daily-use data folder was opened.

## Commands and observations

Build and seed commands ran from the checkout:

```bash
pnpm install --frozen-lockfile
pnpm tauri build --bundles app
cargo run -p aelomi-core --features test-support --example seed_index -- "$DATA" "$CASE"
```

`CASE` was `missing`, `garbage`, or `old-schema`. Each seeder wrote nesting, favorites, a trashed subtree, an erased page body without a workspace record, links, and text. Each data folder started empty. Before launch, a Python SHA-256 manifest recorded every source file, excluding the lock and disposable SQLite files.

Each case ran this sequence in one live shell, with `S=.claude/skills/verify-aelomi/scripts`:

```bash
RUN=$(AELOMI_DATA_DIR="$DATA" "$S/app.sh" launch app)
"$S/app.sh" doctor "$RUN"
"$S/app.sh" wait-status "$RUN" 'IPC ok · core 0.1.0 · seq 1'
"$S/app.sh" screenshot "$RUN" before
"$S/app.sh" press "$RUN" 'Ping again'
"$S/app.sh" wait-status "$RUN" 'IPC ok · core 0.1.0 · seq 2'
"$S/app.sh" screenshot "$RUN" after
"$S/app.sh" keys "$RUN" q command
"$S/app.sh" cleanup "$RUN"
```

The runner waited for the recorded app PID to exit before cleanup. After quit, Python compared the source manifest exactly and opened the cache read-only to check `user_version = 1` and `integrity_check = ok`. The committed `source-before.json` and `source-preservation.json` files record both comparisons and the derived page and favorite rows.

| Case | Native cache action | Visible result and native side effect |
| --- | --- | --- |
| Missing | `cache=created` | Healthy native status advanced from sequence 1 to 2. Source files were unchanged. |
| Garbage | `cache=replaced reason=not-a-database` | Healthy native status advanced from sequence 1 to 2. Source files were unchanged. |
| Old schema | `cache=replaced reason=old-schema (version 0)` | Healthy native status advanced from sequence 1 to 2. Source files were unchanged. |

All three doctor checks passed. All six screenshots were opened and inspected at their captured 1920 by 1280 resolution. Each shows the real Aelomi window and its matching healthy IPC sequence, without an error banner. Each full shell log contains `storage status=Ready`, `index status=Rebuilding`, the cache action, `index status=Ready`, both pings, and quit flushes with `pending=[]`. Every cleanup record confirms that no process from the run remained.

The screenshots prove native window and IPC usability. They do not show the future palette's "Indexing…" text. This PR provides the status command, events, and mock control. Palette display and SPEC section 1 acceptance test 8 remain follow-up work because the palette does not exist.

## Failed verification attempts

The first launch ran in a tool shell that ended before the separate doctor call. Its recorded process was gone, doctor failed, and subsequent capture attempts failed the PID check. The retained `failed-launch` logs contain no panic and show the index reaching Ready, but that attempt supplies no native-window proof. The runner was corrected to keep launch, doctor, captures, and quit in one shell.

The successful missing-index run then hit a runner variable error after native cleanup. Sourcing `run.env` had overwritten the loop's `MODE`. The source comparison was rerun manually against the saved missing-case manifest and passed. Renaming the loop variable to `INDEX_CASE` fixed the runner before the garbage and old-schema runs. No product code changed for either correction.

## Performance checks and limits

`pnpm perf:cold` ran after native cleanup on the same release app. Its ten measured launches had a 422 ms median to first content frame, with a 406 to 437 ms range, below the 500 ms spec gate. The harness used an empty isolated workspace, one discarded warm-up, and the existing paint markers. This does not measure a 10,000-page native disk rebuild.

The release index tests ran five times with:

```bash
cargo nextest run -p aelomi-core --cargo-profile release --run-ignored all \
	-E 'test(measure_content_query_p95_on_the_10k_fixture) | test(a_linking_edit)' --no-capture
```

Each run passed exact result assertions and both LINK-8 tests. The largest observed linking-edit delay was 51.23 ms on a 5,000-block page, below 250 ms. The 200-block page stayed below 40.90 ms. The four fixed 10,000-page fixture queries had p95 values from 147.33 to 151.50 microseconds across 1,000 queries per run. Common-word queries with thousands of matches took 63.47 to 63.81 ms p95 across 200 queries per run, above the 5 ms goal. These are warm queries against an in-memory synthetic Y.Doc source with a real SQLite worker. They are not an app-wide startup or memory measurement.

Follow-ups cover common-word query performance, native disk-backed 10,000-page rebuild duration and memory, physical cache scrubbing for future permanent erase, and private naming improvements from the comment review. Self-links are included by the documented LINK-7 policy.

## Published-file manifest

Personal checkout and home prefixes are replaced with `<checkout>` and `<home>`. The cold-start harness's user-specific temporary prefix is replaced with `<temporary>`. Terminal color escapes were removed. Source-preservation JSON uses the repository formatter without changing its values. The hashes below cover those published text bytes. Databases, app data, `run.env`, and PID files remain local. PNGs retain their original captured bytes and are attached to the PR after publication.

### Screenshots

| Attachment filename | Bytes | SHA-256 |
| --- | ---: | --- |
| `missing/before.png` | 77755 | `dc19870895f46ad150168837ae34c41e924a8a58defa8938b8d41776f55e1061` |
| `missing/after.png` | 78036 | `d8fd45b92f3ccf448449e44427ed3f2e5beb747c4b09ae0f3aef6cf3ae126882` |
| `garbage/before.png` | 77755 | `dc19870895f46ad150168837ae34c41e924a8a58defa8938b8d41776f55e1061` |
| `garbage/after.png` | 78036 | `d8fd45b92f3ccf448449e44427ed3f2e5beb747c4b09ae0f3aef6cf3ae126882` |
| `old-schema/before.png` | 77755 | `dc19870895f46ad150168837ae34c41e924a8a58defa8938b8d41776f55e1061` |
| `old-schema/after.png` | 78036 | `d8fd45b92f3ccf448449e44427ed3f2e5beb747c4b09ae0f3aef6cf3ae126882` |

### Text files

| File | Bytes | SHA-256 |
| --- | ---: | --- |
| [missing/shell.log](missing/shell.log) | 241 | `7ee99c4e1d6aeaf3ba1f0c0814027369e21b4786eed161ada7d8de6dacc9842d` |
| [missing/actions.log](missing/actions.log) | 205 | `c651b70870b1b354f12cb61952d3bb91f54175108175c723502301dc43c7afd8` |
| [missing/doctor.txt](missing/doctor.txt) | 358 | `2cd48b07692cf652e2573bcf29705558cf2ce2803396db7592f06cc4444c5350` |
| [missing/cleanup.txt](missing/cleanup.txt) | 217 | `c1c22443a17b8c729aa02ec102859b9341a926f4c2e09c0ae11cb7aec0fb32da` |
| [missing/before.ax.txt](missing/before.ax.txt) | 253 | `fef7cecbafd30944758ae8f22dbe12a7558cd95277c672bc5c9a83785fe21580` |
| [missing/after.ax.txt](missing/after.ax.txt) | 253 | `84ef7eddf6cf6954cf21515de013f3144bafa769e7fe23c07f9296305818b855` |
| [missing/source-preservation.json](missing/source-preservation.json) | 2098 | `87e637c0bc704c1f01eaa944b666d3b3247301302f367362f2f63df2f6fc1a15` |
| [missing/source-before.json](missing/source-before.json) | 1688 | `21444a16158c4b03485d1d2e8f98d7d3b08781fdc5772951930d37ca3b4773f8` |
| [missing/seed.log](missing/seed.log) | 421 | `08a9ae217474ae9db3bf689ed8ab0fc7bd741aa102df50b9fa00495ecbf4505e` |
| [garbage/shell.log](garbage/shell.log) | 289 | `1bbccb87a5585a55158be104bea55eaf8aa37e429d4b4e857df401354d84153b` |
| [garbage/actions.log](garbage/actions.log) | 205 | `22a24c1344fb3d881f0b2fc988bf30c41132642c1d25a21eb51dfb72ea11691b` |
| [garbage/doctor.txt](garbage/doctor.txt) | 358 | `e3d48c7cb1ff9547ee9e3d38a8768d6cbdf842de86e4985592a381f092934de7` |
| [garbage/cleanup.txt](garbage/cleanup.txt) | 217 | `2db31b7b759b7ad28f565ec38f037caa6c6a2febd5dff86ebd6025d9747afab7` |
| [garbage/before.ax.txt](garbage/before.ax.txt) | 253 | `99c6b00e24424eee9be3475274bc621b910ec3714248bcf50f4eb95ceb64dd22` |
| [garbage/after.ax.txt](garbage/after.ax.txt) | 253 | `9916082800a30da91722ebcfba085cd3f2e0de61d2e1ec1b70e42023926ce26e` |
| [garbage/source-preservation.json](garbage/source-preservation.json) | 2098 | `df8d4dd5fc8f10711351595627c06bbbfc3d33eac7481cd2386a547e55d4c095` |
| [garbage/source-before.json](garbage/source-before.json) | 1688 | `9e31d4f903e58d11ef07bfb40e976098d67f877331aacc7ab636d21cfdde917c` |
| [garbage/seed.log](garbage/seed.log) | 421 | `688a131def38e0773a67f66447c8978214aa6ebd2dedd766641230bc6e871f34` |
| [old-schema/shell.log](old-schema/shell.log) | 275 | `364303d0e7edae3620511ccd49c7139434a19f42c36f410ed0f0e9189c7dadcb` |
| [old-schema/actions.log](old-schema/actions.log) | 205 | `ebce802e6b6b534d0dd831c4c437da74411b7f13d4755ef39a8b18d924b2c234` |
| [old-schema/doctor.txt](old-schema/doctor.txt) | 358 | `ef378ea08e00558d1727132d549ca82e7dc7a42c6409ffba65bbcb9ef5e095fa` |
| [old-schema/cleanup.txt](old-schema/cleanup.txt) | 217 | `4d4137709b59810593a352c9d2163c2069cf47c26bb325bbf15c6ebdbe2beb69` |
| [old-schema/before.ax.txt](old-schema/before.ax.txt) | 253 | `58970394e37750f23476c7eaa7d207033c36916c32b231010b8198e6144c2d29` |
| [old-schema/after.ax.txt](old-schema/after.ax.txt) | 253 | `4e2c242c8ad9e89089b41a25e604da6692ba394b5268f8a7711792c40743bc29` |
| [old-schema/source-preservation.json](old-schema/source-preservation.json) | 2098 | `1263f07ea008e9473f6d3fba1ca04bc1b923001e6354afd76a02646bda533064` |
| [old-schema/source-before.json](old-schema/source-before.json) | 1688 | `91a13ad5fab83c9adf7842a5f70ff96e8e0b9240974af66822836de7563fc00c` |
| [old-schema/seed.log](old-schema/seed.log) | 433 | `d5c28b6ac77f203bf56415eb571ea77c251bec938ffdcdcb6ecc7e33adfe043c` |
| [failed-launch/shell.log](failed-launch/shell.log) | 153 | `05b213fd1f41a441aaf5c64c2c254b1187e84c39b5517949e9d2b2ce69d77dda` |
| [failed-launch/actions.log](failed-launch/actions.log) | 54 | `1d60640d54e1853ee054ee7667865229b8703c4903baad59be7e0a9d3a5647c6` |
| [failed-launch/doctor.txt](failed-launch/doctor.txt) | 358 | `eb5298ff2093a03cec0ca46e7301b11647ca36dd2af721d3b9523e5bcb270971` |
| [failed-launch/cleanup.txt](failed-launch/cleanup.txt) | 169 | `abd454862ca3348bad64a024ab0d29ddef953d57fe6bed958eb498942a6c0517` |
| [cold-start.log](cold-start.log) | 3500 | `b0328df5e25355ecd8a282e0f8d00a47ef35b295420b64e002968a067ff49796` |
| [index-performance.log](index-performance.log) | 10831 | `7fe2601cb18e54f44802ed447cbd968e46893583c6db6eb765a384d82df2eb13` |
