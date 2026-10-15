import { readFileSync } from "node:fs";
import { expect, test } from "vitest";
import * as Y from "yjs";
import { writeBody } from "../ipc/mock-body";
import { readBody, renderSegments } from "./body-reader";

const t = (text: string) => ({ kind: "text" as const, text });
const l = (pageId: string) => ({ kind: "link" as const, pageId });

test("reads the real BlockNote fixture as the Rust reader does", () => {
  const doc = new Y.Doc();
  Y.applyUpdate(
    doc,
    readFileSync(
      new URL("../../fixtures/index/blocknote-body.yupdate", import.meta.url),
    ),
  );
  expect(readBody(doc)).toEqual({
    problem: null,
    blocks: [
      { id: "block-heading", segments: [t("Café roadmap")] },
      {
        id: "block-link",
        segments: [t("see "), l("page-target"), t(" today")],
      },
      {
        id: "block-twice",
        segments: [
          l("page-target"),
          t(" and "),
          l("page-target"),
          t(" and "),
          l("page-other"),
        ],
      },
      { id: "block-parent", segments: [t("parent item")] },
      { id: "block-child", segments: [t("child item")] },
      { id: "block-grandchild", segments: [t("done "), l("page-other")] },
      { id: "block-numbered", segments: [t("first step")] },
      {
        id: "block-code",
        segments: [t('fn main() {\n    let s = "[[Not a link]]";\n}')],
      },
      { id: "block-break", segments: [t("line one\nline two")] },
      {
        id: "block-href",
        segments: [t("read the docs and [[Literal brackets]]")],
      },
      { id: "block-quote", segments: [t("naïve 日本 quote")] },
      { id: "block-image", segments: [t("Architecture diagram")] },
      { id: "block-divider", segments: [] },
      { id: "block-empty", segments: [] },
    ],
  });
});

test("salvages readable blocks and reports the most serious problem", () => {
  const doc = new Y.Doc();
  writeBody(doc, [
    {
      id: "parent",
      content: ["alpha", { link: "Bad ID" }, "beta"],
      children: [{ id: "child", content: ["kept"] }],
    },
  ]);
  const group = doc.getXmlFragment("prosemirror").get(0) as Y.XmlElement;
  (group.get(0) as Y.XmlElement).removeAttribute("id");
  const body = readBody(doc);
  expect(body).toEqual({
    problem: { kind: "MissingBlockId" },
    blocks: [{ id: "child", segments: [t("kept")] }],
  });
  expect(readBody(doc, { keepUnidentified: true }).blocks).toEqual([
    { id: "", segments: [t("alpha beta")] },
    { id: "child", segments: [t("kept")] },
  ]);
});

test("a body under another root is unrecognized, and an empty doc has no blocks", () => {
  const old = new Y.Doc();
  old.getText("body").insert(0, "an old root");
  expect(readBody(old)).toEqual({
    blocks: [],
    problem: { kind: "UnrecognizedRoot", root: "body" },
  });
  expect(readBody(new Y.Doc())).toEqual({ blocks: [], problem: null });
});

test("renders links with their current labels", () => {
  expect(
    renderSegments([t("see "), l("page-a"), t(" now")], (id) =>
      id === "page-a" ? "Aelomi" : "Deleted page",
    ),
  ).toBe("see Aelomi now");
});
