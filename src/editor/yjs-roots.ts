import type * as Y from "yjs";

/** A root as `Y.Doc.share` holds it, typed or not. */
export type Root = Y.Doc["share"] extends Map<string, infer T> ? T : never;

/**
 * What a root holds, read without asking Yjs for a type. Yjs 13.6 leaves a
 * root that arrived in an update untyped, and asking for a type converts it,
 * so only the item list on `AbstractType` shows both its sequence and its map.
 * This module is the one reader of `_start` and `_map`, and
 * `yjs-roots.test.ts` covers every root shape it must classify, so a Yjs
 * upgrade that changes them fails there first.
 */
export type RootContent = {
  /** Live items of the root's sequence, in order. */
  sequence: readonly Y.Item[];
  /** Keys whose current value is live. */
  entries: readonly string[];
};

export function rootContent(root: Root): RootContent {
  const sequence: Y.Item[] = [];
  for (let item = root._start; item !== null; item = item.right)
    if (!item.deleted && item.countable) sequence.push(item);
  const entries: string[] = [];
  for (const [key, item] of root._map) if (!item.deleted) entries.push(key);
  return { sequence, entries };
}

export function rootHasContent(root: Root): boolean {
  const { sequence, entries } = rootContent(root);
  return sequence.length > 0 || entries.length > 0;
}
