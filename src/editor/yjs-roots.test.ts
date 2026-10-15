import { expect, test } from "vitest";
import * as Y from "yjs";
import { type Root, rootContent, rootHasContent } from "./yjs-roots";

function replayed(doc: Y.Doc): Y.Doc {
  const copy = new Y.Doc();
  Y.applyUpdate(copy, Y.encodeStateAsUpdate(doc));
  return copy;
}

function root(doc: Y.Doc, name: string): Root {
  const type = doc.share.get(name);
  if (!type) throw new Error(`no root ${name}`);
  return type;
}

function shape(type: Root) {
  const { sequence, entries } = rootContent(type);
  return {
    values: sequence.flatMap((item) => item.content.getContent()),
    entries,
  };
}

test("an untyped root shows its sequence and its map without being converted", () => {
  const doc = new Y.Doc();
  doc.getText("text").insert(0, "ab");
  doc.getArray("values").insert(0, [1, "two"]);
  doc.getMap("map").set("k", "v");
  const copy = replayed(doc);
  for (const name of ["text", "values", "map"])
    expect(root(copy, name).constructor).toBe(Y.AbstractType);
  expect(shape(root(copy, "text"))).toEqual({
    values: ["a", "b"],
    entries: [],
  });
  expect(shape(root(copy, "values"))).toEqual({
    values: [1, "two"],
    entries: [],
  });
  expect(shape(root(copy, "map"))).toEqual({ values: [], entries: ["k"] });
  expect(root(copy, "map").constructor).toBe(Y.AbstractType);
});

test("deleted content and formatting alone leave a root empty", () => {
  const doc = new Y.Doc();
  const text = doc.getText("text");
  text.insert(0, "bold", { bold: true });
  text.delete(0, 4);
  const map = doc.getMap("map");
  map.set("k", 1);
  map.delete("k");
  doc.getArray("values").insert(0, [1]);
  doc.getArray("values").delete(0, 1);
  const copy = replayed(doc);
  for (const type of [
    root(doc, "text"),
    root(doc, "map"),
    root(copy, "text"),
    root(copy, "map"),
    root(copy, "values"),
  ])
    expect(rootHasContent(type)).toBe(false);
  doc.getText("other").insert(0, "x");
  expect(rootHasContent(root(replayed(doc), "other"))).toBe(true);
});

test("a typed XML root lists its nodes", () => {
  const doc = new Y.Doc();
  const element = new Y.XmlElement("blockGroup");
  doc.getXmlFragment("prosemirror").insert(0, [element]);
  expect(shape(root(doc, "prosemirror"))).toEqual({
    values: [element],
    entries: [],
  });
});
