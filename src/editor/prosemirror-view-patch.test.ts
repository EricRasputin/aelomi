import { readFileSync } from "node:fs";
import { BlockNoteEditor } from "@blocknote/core";
import { blocksToYXmlFragment } from "@blocknote/core/yjs";
import type { Node as PMNode } from "prosemirror-model";
import { EditorState } from "prosemirror-state";
import { Decoration, DecorationSet } from "prosemirror-view";
import { beforeAll, expect, test } from "vitest";
import { yXmlFragmentToProseMirrorRootNode } from "y-prosemirror";
import * as Y from "yjs";
import { editorSchema } from "./schema";
import { repoFile } from "./y-binding-variants";

type Internal = DecorationSet & {
  children: (number | DecorationSet)[];
  local: { from: number; to: number; inline: boolean }[];
};

const editor = BlockNoteEditor.create({ schema: editorSchema });
let page: PMNode;

beforeAll(() => {
  const doc = new Y.Doc();
  blocksToYXmlFragment(
    editor,
    JSON.parse(
      readFileSync(
        repoFile("fixtures/generated/page-5000-blocks.json"),
        "utf8",
      ),
    ),
    doc.getXmlFragment("prosemirror"),
  );
  page = yXmlFragmentToProseMirrorRootNode(
    doc.getXmlFragment("prosemirror"),
    editor.pmSchema,
  );
});

function scannedChild(set: Internal, offset: number) {
  for (let i = 0; i < set.children.length; i += 3)
    if ((set.children[i] as number) >= offset)
      return set.children[i] === offset ? set.children[i + 2] : undefined;
  return undefined;
}

function decorate(doc: PMNode, every: number) {
  const decorations: Decoration[] = [];
  let index = 0;
  doc.descendants((node, pos) => {
    index += 1;
    if (node.type.name === "numberedListItem" || index % every === 0)
      decorations.push(
        Decoration.node(pos, pos + node.nodeSize, { "data-index": `${index}` }),
      );
    if (node.isText && index % (every * 3) === 0)
      decorations.push(Decoration.inline(pos, pos + 1, { class: "x" }));
    return true;
  });
  return DecorationSet.create(doc, decorations);
}

function checkEveryLookup(set: DecorationSet, doc: PMNode): number {
  let checked = 0;
  const walk = (current: Internal, node: PMNode) => {
    if (current === DecorationSet.empty) return;
    node.forEach((child, offset) => {
      const got = current.forChild(offset, child) as Internal;
      checked += 1;
      if (child.isLeaf) {
        expect(got).toBe(DecorationSet.empty);
        return;
      }
      const scanned = scannedChild(current, offset);
      const local = current.local.some(
        (d) =>
          d.inline &&
          d.from < offset + 1 + child.content.size &&
          d.to > offset + 1,
      );
      if (!local) expect(got).toBe(scanned ?? DecorationSet.empty);
      else {
        const below = scanned ? (scanned as DecorationSet).find() : [];
        const found = got.find().map((d) => [d.from, d.to]);
        for (const d of below) expect(found).toContainEqual([d.from, d.to]);
      }
      if (scanned) walk(scanned as Internal, child);
    });
  };
  walk(set as Internal, doc);
  return checked;
}

test("child decoration lookups match the scan on the 5,000-block page", () => {
  for (const every of [1, 7, 50]) {
    const set = decorate(page, every);
    expect(checkEveryLookup(set, page)).toBeGreaterThan(5000);
  }
});

test("child decoration lookups match the scan after edits map the set", () => {
  let state = EditorState.create({ doc: page });
  let set = decorate(page, 5);
  const edits = [
    (s: typeof state) => s.tr.insertText("abc", 40),
    (s: typeof state) => s.tr.delete(200, 900),
    (s: typeof state) => s.tr.split(3000, 2),
    (s: typeof state) =>
      s.tr.delete(s.doc.content.size - 400, s.doc.content.size - 300),
    (s: typeof state) => s.tr.insertText("x", s.doc.content.size - 10),
  ];
  for (const edit of edits) {
    const tr = edit(state);
    set = set.map(tr.mapping, tr.doc);
    state = state.apply(tr);
    expect(checkEveryLookup(set, state.doc)).toBeGreaterThan(4000);
  }
});
