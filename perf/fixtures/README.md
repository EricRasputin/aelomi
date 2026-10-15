# Empty workspace for cold-start timing

`empty-workspace/` contains one format-2 log record holding the empty Yjs v1 update, `[0, 0]`. It has stored history and no pages. The shell seed leaves it empty. The cold-start harness copies it into a temporary folder when `--expected-variant index` has no supplied data folder.

Generate it through the production Store with `cargo run -p aelomi-core --features test-support --example seed_index -- <new folder> empty-workspace`. Copy only `format.json` and `docs/workspace/*.ylog2`. The storage seed test checks the exact decoded record and the empty projected tree after the explicit shell seed step.
