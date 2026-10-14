import { readFileSync } from "node:fs";
import { BlockNoteEditor } from "@blocknote/core";
import { blocksToYXmlFragment } from "@blocknote/core/yjs";
import { expect, test } from "vitest";
import * as Y from "yjs";
import { writeBody } from "../ipc/mock-body";
import { checkBody } from "./body-guard";
import { editorSchema } from "./schema";

const fixture = readFileSync(
  new URL("../../fixtures/index/blocknote-body.yupdate", import.meta.url),
);
const editor = BlockNoteEditor.create({ schema: editorSchema });
const schema = editor.pmSchema;

function canonical(edit?: (paragraph: Y.XmlElement, doc: Y.Doc) => void) {
  const doc = new Y.Doc();
  writeBody(doc, [
    { id: "block-a", content: ["see ", { link: "page-target" }, " today"] },
    { id: "block-b", type: "heading", level: 2, content: ["Plan"] },
  ]);
  const group = doc.getXmlFragment("prosemirror").get(0) as Y.XmlElement;
  const paragraph = (group.get(0) as Y.XmlElement).get(0) as Y.XmlElement;
  doc.transact(() => edit?.(paragraph, doc));
  return checkBody(doc, schema);
}

test("the real BlockNote fixture, a canonical body, and an empty doc are editable", () => {
  const doc = new Y.Doc();
  Y.applyUpdate(doc, fixture);
  expect(checkBody(doc, schema)).toEqual({ kind: "editable", empty: false });
  expect(canonical()).toEqual({ kind: "editable", empty: false });
  expect(checkBody(new Y.Doc(), schema)).toEqual({
    kind: "editable",
    empty: true,
  });
});

test("anything y-prosemirror would delete or rewrite opens read-only", () => {
  const cases: [string, Parameters<typeof canonical>[0]][] = [
    ["unknownNode", (p) => p.insert(0, [new Y.XmlElement("mention")])],
    ["unknownAttribute", (p) => p.setAttribute("fontFamily", "Serif")],
    [
      "changedAttribute",
      (p) => p.setAttribute("textAlignment", undefined as unknown as string),
    ],
    [
      "changedAttribute",
      (p) => p.setAttribute("textColor", null as unknown as string),
    ],
    [
      "unknownMark",
      (p) => (p.get(0) as Y.XmlText).format(0, 3, { highlight: {} }),
    ],
    [
      "changedMark",
      (p) => (p.get(0) as Y.XmlText).format(0, 3, { bold: { weight: 900 } }),
    ],
    [
      "changedMark",
      (p) => (p.get(0) as Y.XmlText).format(0, 3, { "bold--AbCd1234": {} }),
    ],
    [
      "changedMark",
      (p) =>
        (p.get(0) as Y.XmlText).format(0, 3, {
          code: {},
          link: { href: "https://example.com" },
        }),
    ],
    [
      "splitText",
      (p) => {
        const text = new Y.XmlText();
        text.insert(0, "more");
        p.insert(1, [text]);
      },
    ],
    ["splitText", (p) => p.insert(1, [new Y.XmlText()])],
    ["embed", (p) => (p.get(0) as Y.XmlText).insertEmbed(0, { image: "x" })],
    ["missingBlockId", (p) => (p.parent as Y.XmlElement).removeAttribute("id")],
    [
      "duplicateBlockId",
      (p) =>
        ((p.parent as Y.XmlElement).parent as Y.XmlElement)
          .get(1)
          .setAttribute("id", "block-a"),
    ],
    [
      "invalidPageLink",
      (p) => (p.get(1) as Y.XmlElement).setAttribute("targetPageId", "Bad ID"),
    ],
    [
      "invalidContent",
      (p) =>
        (p.parent as Y.XmlElement).insert(0, [new Y.XmlElement("divider")]),
    ],
    ["foreignRoot", (_p, doc) => doc.getMap("meta").set("version", 1)],
  ];
  for (const [kind, edit] of cases)
    expect(canonical(edit), kind).toMatchObject({
      kind: "readOnly",
      problem: { kind },
    });
});

function firstText(parent: Y.XmlFragment | Y.XmlElement): Y.XmlText {
  const child = parent.get(0);
  if (child instanceof Y.XmlText) return child;
  if (child instanceof Y.XmlElement) return firstText(child);
  throw new Error("no text in the body");
}

test("a mark with an unknown attribute opens read-only even when the value is null", () => {
  const unknown: [string, Record<string, unknown>][] = [
    ["bold", { futureField: null }],
    ["link", { href: "https://example.com", futureField: null }],
    ["textColor", { stringValue: "red", futureField: null }],
  ];
  for (const [mark, attrs] of unknown)
    expect(
      canonical((p) => firstText(p).format(0, 3, { [mark]: attrs })),
      mark,
    ).toEqual({ kind: "readOnly", problem: { kind: "changedMark", mark } });

  const doc = new Y.Doc();
  writeBody(doc, [{ id: "block-a", content: ["see today"] }]);
  const body = doc.getXmlFragment("prosemirror");
  firstText(body).format(0, 3, { bold: { futureField: null } });
  const before = Y.encodeStateAsUpdate(doc);
  checkBody(doc, schema);
  expect(Y.encodeStateAsUpdate(doc)).toEqual(before);
});

test("known mark attributes stay editable, including a null one", () => {
  const known: Record<string, unknown>[] = [
    { link: { href: null } },
    { textColor: { stringValue: "red" } },
  ];
  for (const attributes of known)
    expect(
      canonical((p) => firstText(p).format(0, 3, attributes)),
      Object.keys(attributes)[0],
    ).toEqual({ kind: "editable", empty: false });
});

test("a foreign root that is empty or fully deleted does not block editing", () => {
  expect(
    canonical((_p, doc) => {
      doc.getMap("meta").set("version", 1);
      doc.getMap("meta").delete("version");
    }),
  ).toEqual({ kind: "editable", empty: false });
});

test("checking a body never writes to it", () => {
  const doc = new Y.Doc();
  Y.applyUpdate(doc, fixture);
  const before = Y.encodeStateAsUpdate(doc);
  checkBody(doc, schema);
  expect(Y.encodeStateAsUpdate(doc)).toEqual(before);
});

test("no mark in the schema overlaps itself, so no canonical key is hashed", () => {
  expect(
    Object.values(schema.marks)
      .filter((type) => !type.excludes(type))
      .map((type) => type.name),
  ).toEqual([]);
});

test("BlockNote's own undefined `start` on a numbered item stays editable", () => {
  const doc = new Y.Doc();
  blocksToYXmlFragment(
    editor,
    [
      { id: "n-1", type: "numberedListItem", content: "first" },
    ] as unknown as typeof editor.document,
    doc.getXmlFragment("prosemirror"),
  );
  const item = (
    (doc.getXmlFragment("prosemirror").get(0) as Y.XmlElement).get(
      0,
    ) as Y.XmlElement
  ).get(0) as Y.XmlElement;
  expect(item.nodeName).toBe("numberedListItem");
  const stored = item.getAttributes() as Record<string, unknown>;
  expect(Object.hasOwn(stored, "start")).toBe(true);
  expect(stored.start).toBeUndefined();
  expect(checkBody(doc, schema)).toEqual({ kind: "editable", empty: false });
});
