# Aelomi

Run the macOS app with `pnpm install --frozen-lockfile` and `pnpm tauri dev`.
Build the ad-hoc signed app with `pnpm tauri build --bundles app`.

After changing a Rust command, run `pnpm bindings` and `pnpm typecheck`.
`cargo nextest run --workspace` checks the committed bindings against the command builder.

Run `pnpm lint`, `pnpm typecheck`, `pnpm test`, `cargo nextest run --workspace`,
`cargo deny check`, `pnpm audit`, and `pnpm run licenses` before committing.
Use `pnpm run licenses` because pnpm reserves the bare `licenses` command.

The license policy allows Unicode-3.0 for the ICU and unicode-ident dependencies,
Zlib for foldhash and zlib-rs, and Unlicense for isbot.
`deny.toml` names two unmaintained transitive crates with no patched releases.
