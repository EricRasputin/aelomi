import { BlockNoteEditor } from "@blocknote/core";
import { blocksToYXmlFragment } from "@blocknote/core/yjs";
import { expect, test } from "vitest";
import * as Y from "yjs";
import { editorSchema } from "../editor/schema";
import { type SeedBlock, writeBody } from "./mock-body";

function layout(node: Y.XmlElement | Y.XmlText | Y.XmlHook): unknown {
  if (node instanceof Y.XmlText) return node.toDelta();
  if (!(node instanceof Y.XmlElement)) return "hook";
  const attributes = Object.entries(node.getAttributes()).sort(([a], [b]) =>
    a < b ? -1 : 1,
  );
  return [node.nodeName, attributes, node.toArray().map(layout)];
}

const seed: SeedBlock[] = [
  {
    id: "h",
    type: "heading",
    level: 2,
    content: ["Café ", { text: "roadmap", styles: ["bold"] }],
  },
  {
    id: "p",
    content: [
      "see ",
      { link: "page-target" },
      { text: " today", styles: ["italic", "code"] },
    ],
    children: [
      { id: "b", type: "bulletListItem", content: ["child"] },
      { id: "n", type: "numberedListItem", content: ["step"] },
      { id: "c", type: "checkListItem", checked: true, content: ["done"] },
    ],
  },
  { id: "q", type: "quote", content: ["naïve"] },
  { id: "k", type: "codeBlock", language: "rust", content: ["fn main() {}"] },
  { id: "d", type: "divider" },
  { id: "e" },
];

type PartialBlock = typeof editorSchema.PartialBlock;

function toBlock(block: SeedBlock): PartialBlock {
  const content = (block.content ?? []).map((inline) =>
    typeof inline === "string"
      ? { type: "text" as const, text: inline, styles: {} }
      : "link" in inline
        ? { type: "pageLink" as const, props: { targetPageId: inline.link } }
        : {
            type: "text" as const,
            text: inline.text,
            styles: Object.fromEntries(inline.styles.map((s) => [s, true])),
          },
  );
  const plain = (block.content ?? [])
    .map((inline) => (typeof inline === "string" ? inline : ""))
    .join("");
  const { id } = block;
  const children = (block.children ?? []).map(toBlock);
  switch (block.type ?? "paragraph") {
    case "heading":
      return {
        id,
        type: "heading",
        props: { level: block.level ?? 1 },
        content,
        children,
      };
    case "checkListItem":
      return {
        id,
        type: "checkListItem",
        props: { checked: block.checked ?? false },
        content,
        children,
      };
    case "codeBlock":
      return {
        id,
        type: "codeBlock",
        props: { language: block.language ?? "text" },
        content: plain,
        children,
      };
    case "divider":
      return { id, type: "divider", children };
    case "bulletListItem":
      return { id, type: "bulletListItem", content, children };
    case "numberedListItem":
      return { id, type: "numberedListItem", content, children };
    case "quote":
      return { id, type: "quote", content, children };
    case "paragraph":
      return { id, type: "paragraph", content, children };
  }
}

test("the mock body writer writes the layout the real BlockNote binding writes", () => {
  const editor = BlockNoteEditor.create({
    schema: editorSchema,
    initialContent: seed.map(toBlock),
  });
  const real = new Y.Doc();
  blocksToYXmlFragment(
    editor,
    editor.document,
    real.getXmlFragment("prosemirror"),
  );
  const mock = new Y.Doc();
  writeBody(mock, seed);
  expect(mock.getXmlFragment("prosemirror").toArray().map(layout)).toEqual(
    real.getXmlFragment("prosemirror").toArray().map(layout),
  );
});
