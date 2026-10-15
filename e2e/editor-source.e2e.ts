import { readFileSync } from "node:fs";
import { expect } from "e2e";
import * as Y from "yjs";
import { test } from "./fixtures";

const fixture = readFileSync(
  new URL("../fixtures/index/blocknote-body.yupdate", import.meta.url),
);

type Raw = Record<string, string>;

function encode(build: (doc: Y.Doc) => void): string {
  const doc = new Y.Doc();
  doc.clientID = 7;
  build(doc);
  return Buffer.from(Y.encodeStateAsUpdate(doc)).toString("base64");
}

function paragraph(id: string, text: string, attrs?: Record<string, string>) {
  const container = new Y.XmlElement("blockContainer");
  container.setAttribute("id", id);
  const content = new Y.XmlElement("paragraph");
  for (const [key, value] of Object.entries({
    backgroundColor: "default",
    textColor: "default",
    textAlignment: "left",
    ...attrs,
  }))
    content.setAttribute(key, value);
  const run = new Y.XmlText();
  run.insert(0, text);
  content.insert(0, [run]);
  container.insert(0, [content]);
  return container;
}

function body(doc: Y.Doc, ...blocks: Y.XmlElement[]) {
  const group = new Y.XmlElement("blockGroup");
  group.insert(0, blocks);
  doc.getXmlFragment("prosemirror").insert(0, [group]);
}

const unsupported: Raw = {
  "page-node": encode((doc) => {
    const table = new Y.XmlElement("blockContainer");
    table.setAttribute("id", "block-table");
    table.insert(0, [new Y.XmlElement("table")]);
    body(doc, paragraph("block-a", "unknown node"), table);
  }),
  "page-mark": encode((doc) => {
    const block = paragraph("block-a", "unknown mark");
    body(doc, block);
    const run = (block.get(0) as Y.XmlElement).get(0) as Y.XmlText;
    run.format(0, 7, { highlight: { color: "yellow" } });
  }),
  "page-code-link": encode((doc) => {
    const block = paragraph("block-a", "linked code");
    body(doc, block);
    const run = (block.get(0) as Y.XmlElement).get(0) as Y.XmlText;
    run.format(0, 6, { code: {}, link: { href: "https://example.com" } });
  }),
  "page-attribute": encode((doc) =>
    body(
      doc,
      paragraph("block-a", "unknown attribute", { fontFamily: "Serif" }),
    ),
  ),
  "page-root": encode((doc) => doc.getText("body").insert(0, "an old root")),
  "page-id": encode((doc) => {
    const block = paragraph("block-a", "no block id");
    block.removeAttribute("id");
    body(doc, block);
  }),
};

test("DATA-1 a body the editor can't hold opens read-only and is never written", async ({
  app,
  browser,
  screen,
  nonLocalRequests,
}) => {
  await browser.addInitScript((rawBodies) => {
    window.__aelomiMockSetup = {
      pages: Object.keys(rawBodies).map((id) => ({ id, title: id })),
      rawBodies,
    };
  }, unsupported);
  const seededState = (pageId: string) => {
    const seeded = new Y.Doc();
    Y.applyUpdate(seeded, Buffer.from(unsupported[pageId] ?? "", "base64"));
    return Buffer.from(Y.encodeStateAsUpdate(seeded)).toString("base64");
  };
  const stored = (pageId: string) =>
    browser.evaluate((page) => {
      const mock = window.__aelomiMock;
      return {
        state: mock?.bodies.state(page) ?? null,
        appends:
          mock?.calls.filter((call) => call.command === "docAppend").length ??
          null,
      };
    }, pageId);
  const cases: [string, string][] = [
    ["page-node", "unknown node"],
    ["page-mark", "unknown mark"],
    ["page-code-link", "linked code"],
    ["page-attribute", "unknown attribute"],
    ["page-id", "no block id"],
    ["page-root", "an old root"],
  ];
  for (const [pageId, text] of cases) {
    await app.open(`/page/${pageId}`);
    await expect(screen.getByText(/^Read-only\./)).toBeVisible();
    await expect(screen.getByRole("textbox", "Page body")).not.toBeVisible();
    if (pageId !== "page-root") {
      await expect(screen.getByText(text)).toBeVisible();
      await screen.getByText(text).click();
    }
    await browser.keyboard.type("typed");
    expect(await stored(pageId)).toEqual({
      state: seededState(pageId),
      appends: 0,
    });
  }
  expect(nonLocalRequests).toEqual([]);
});

function deletedOriginals(original: Y.Doc, edited: Y.Doc): string[] {
  const deletes = Y.createDeleteSetFromStructStore(edited.store);
  const deleted: string[] = [];
  for (const [client, structs] of original.store.clients)
    for (const struct of structs) {
      if (!(struct instanceof Y.Item) || struct.deleted) continue;
      let hit = false;
      for (let i = 0; i < struct.length; i++)
        hit ||= Y.isDeleted(deletes, Y.createID(client, struct.id.clock + i));
      if (!hit) continue;
      const owner =
        struct.parent instanceof Y.XmlElement ? struct.parent.nodeName : "text";
      deleted.push(
        struct.parentSub === null
          ? `${owner} content`
          : `${owner}.${struct.parentSub}`,
      );
    }
  return deleted;
}

function layout(node: Y.XmlElement | Y.XmlText | Y.XmlHook): unknown {
  if (node instanceof Y.XmlText) return node.toDelta();
  if (!(node instanceof Y.XmlElement)) return "hook";
  return [node.nodeName, node.getAttributes(), node.toArray().map(layout)];
}

test("DATA-1 the real BlockNote body opens without a write and keeps every block, link, mark, and attribute after editing", async ({
  app,
  browser,
  screen,
  nonLocalRequests,
}) => {
  const raw = fixture.toString("base64");
  await browser.addInitScript((update) => {
    window.__aelomiMockSetup = {
      pages: [
        { id: "page-fixture", title: "Fixture" },
        { id: "page-target", title: "Target" },
        { id: "page-other", title: "Other" },
      ],
      rawBodies: { "page-fixture": update },
    };
  }, raw);
  await app.open("/page/page-fixture");
  const editor = screen.getByRole("textbox", "Page body");
  await expect(editor).toContainText("Café roadmap");
  await expect(editor).toContainText("see Target today");
  await browser.evaluate(
    () => new Promise<null>((done) => setTimeout(() => done(null), 500)),
  );
  expect(
    await browser.evaluate(
      () =>
        window.__aelomiMock?.calls.filter(
          (call) => call.command === "docAppend",
        ).length ?? null,
    ),
  ).toBe(0);

  await screen.getByText("first step").click();
  await browser.keyboard.press("Meta+ArrowRight");
  await browser.keyboard.type(" and more");
  await expect(editor).toContainText("first step and more");

  const state = await browser.evaluate(
    () => window.__aelomiMock?.bodies.state("page-fixture") ?? "",
  );
  const edited = new Y.Doc();
  Y.applyUpdate(edited, Buffer.from(state, "base64"));
  const original = new Y.Doc();
  Y.applyUpdate(original, fixture);
  const before = original.getXmlFragment("prosemirror").toArray().map(layout);
  const after = edited.getXmlFragment("prosemirror").toArray().map(layout);
  const expected = JSON.parse(
    JSON.stringify(before).replace(
      '[{"insert":"first step"}]',
      '[{"insert":"first step and more"}]',
    ),
  );
  expect(JSON.parse(JSON.stringify(after))).toEqual(expected);
  // The layout above cannot see an attribute stored as undefined or which Y
  // item holds a value. Typing deletes exactly one loaded item: the edited
  // numbered item's stored `start: undefined`, which upstream y-prosemirror
  // deletes the same way at the first edit inside that block.
  expect(deletedOriginals(original, edited)).toEqual([
    "numberedListItem.start",
  ]);
  expect([...edited.share.keys()]).toEqual(["prosemirror"]);
  expect(nonLocalRequests).toEqual([]);
});
