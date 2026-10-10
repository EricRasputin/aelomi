# Fixtures

`generated/` holds files written by `fixtures/generate.ts` (`pnpm fixtures`).
The generator is seeded (sfc32 seeded by xmur3, integer operations only), so the
same source produces the same bytes on every Node version.
`fixtures/generator.test.ts` regenerates everything in memory and compares the
sha256 of each file with the checked-in copy. Change the generator, run
`pnpm fixtures`, and commit the new files together.

`.gitattributes` marks `generated/` as generated and turns diffs off for it.
Biome skips the folder so the formatter cannot rewrite the files.

## search-10k-pages.jsonl

One page per line: `id`, `title`, `parentId`, `trash` (`null`, or the batch ID
and time), and `body` (plain text, paragraphs separated by `\n`).
Parents come before children, so a reader can build the tree in one pass.
Sibling order is the line order.

## search-10k-expected.json

Content queries and the page IDs they must return, built by planting rare tokens:

| Query | Planted | Expected |
|---|---|---|
| `zorvak` | 1 active page | that page |
| `quillmere` | 7 active pages and 2 trashed pages | the 7 active pages (CMD-10) |
| `kestrel` | `kestrelmoss` in 3 active pages, `kestrelwind` in 1 | those 4 pages, matched as a prefix (CMD-5) |
| `nonesuchword` | nothing | no pages |

The generated vocabulary uses only consonant-vowel syllables, so no other word
starts with a planted token. Expected IDs are sorted.

## page-5000-blocks.json

A JSON array of 5,000 BlockNote blocks (4,362 top level, the rest nested as
`children` of list items). The shape follows the BlockNote `Block` type in
`@blocknote/core` 0.55.0, checked against the published type definitions and
the docs at `blocknotejs.org/docs/foundations/document-structure` and
`blocknotejs.org/docs/features/blocks` on 2026-10-11:
`{ id, type, props, content, children }`, inline content as styled text or
links, and the default props `backgroundColor`, `textColor`, `textAlignment`.
Block types are the EDIT-2 set: `paragraph`, `heading` (levels 1 to 3),
`bulletListItem`, `numberedListItem`, `checkListItem`, `quote`, `codeBlock`,
`divider`, and `image`. `divider` and `image` have no `content` key.
Image URLs use an `asset://` placeholder because the app's image URL scheme is
not decided yet (DATA-3 names image files by content hash).
