# Index fixtures

## blocknote-body.yupdate

One page body written by the real BlockNote Yjs binding, so the Rust body reader
(`crates/core/src/workspace/body.rs`) is checked against the package rather than
against our own reading of it. `the_real_blocknote_fixture_reads_as_its_blocks`
in `crates/core/src/workspace/tests/body.rs` asserts every block it holds.

It is one Yjs v1 update of a Y.Doc whose only root is the Y.XmlFragment
`prosemirror`, written by `blocksToYXmlFragment` from `@blocknote/core/yjs`. The
blocks cover headings and marks, a custom `pageLink` inline node that stores only
`targetPageId`, a repeated link, three levels of nesting, a checked to-do, a
numbered item, a code block with raw newlines and literal `[[...]]`, a hard break,
a URL link, a quote with non-ASCII text, an image caption, a divider, and an empty
paragraph. `generate-blocknote-body.mjs` lists them.

The repo does not depend on BlockNote, so the generator imports it from a
throwaway folder. To regenerate:

```sh
mkdir /tmp/bn-deps && cd /tmp/bn-deps
echo '{"private":true}' > package.json
npm install --ignore-scripts --no-audit --no-fund --save-exact \
  @blocknote/core@0.55.0 y-prosemirror@1.3.7 yjs@13.6.33 \
  y-protocols prosemirror-model prosemirror-state prosemirror-view
cd -
node fixtures/index/generate-blocknote-body.mjs /tmp/bn-deps
```

The generator refuses other versions of the three packages it names. The client
ID and every block ID are fixed, so the output is the same bytes on every run.

Generated on 2026-10-14 with Node 24.13.1 from these packages:

| Package | Version | License | npm integrity |
|---|---|---|---|
| `@blocknote/core` | 0.55.0 | MPL-2.0 | `sha512-QRRnOuVDrt0pPwaCXQ/dc4gxQ3KEGkBC8Hh4hyl4lxOU+5TMLNKKX9gZgeGkq7WKVK2EZbnPea6pcx25aS5SGA==` |
| `y-prosemirror` | 1.3.7 | MIT | `sha512-NpM99WSdD4Fx4if5xOMDpPtU3oAmTSjlzh5U4353ABbRHl1HtAFUx6HlebLZfyFxXN9jzKMDkVbcRjqOZVkYQg==` |
| `yjs` | 13.6.33 | MIT | `sha512-q/UYvr1gOk4XII+Cu1CW2dINjYqJJFifTzogEaIARznrvX7uOlHsdTEXcnOBMuIsrCGEEKdNxi74kyMLZlKVfw==` |
| `prosemirror-model` | 1.25.12 | MIT | `sha512-Ue2gTmXMa7EhpLNhC7J+h4+ykD8ha12K6rrZFFKKJHBForfIStw5gJ6Zrf1mqqAa9NmpmEB4wp9bKmA4eUVjWg==` |
| `@tiptap/core` | 3.32.0 | MIT | `sha512-Ghh2h02GKSqzytQGcLV4espYOG4EHxjj0MsrUB2Vnr47xmlZYRf6r7k2J09H+JpBdaGrWwrTkyBEi5i3c3yy2Q==` |

`blocknote-body.yupdate` is 2,712 bytes with SHA-256
`d35701bca23242b4d329c03a35bfb497cacd821a246eb77749cad9987497a891`.
