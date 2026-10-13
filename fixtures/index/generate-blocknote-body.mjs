import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const [deps, out] = process.argv.slice(2);
if (!deps) {
  console.error("usage: generate-blocknote-body.mjs <deps-dir> [<out-file>]");
  process.exit(2);
}

const versions = {
  "@blocknote/core": "0.55.0",
  "y-prosemirror": "1.3.7",
  yjs: "13.6.33",
};

function packageJson(name) {
  return JSON.parse(
    readFileSync(join(deps, "node_modules", name, "package.json"), "utf8"),
  );
}

async function load(name, subpath = ".") {
  const pkg = packageJson(name);
  let entry = pkg.exports?.[subpath] ?? pkg.exports;
  while (entry && typeof entry === "object") {
    entry = entry.import ?? entry.module ?? entry.default;
  }
  const file = join(deps, "node_modules", name, entry ?? pkg.module);
  return import(pathToFileURL(file).href);
}

for (const [name, version] of Object.entries(versions)) {
  const found = packageJson(name).version;
  if (found !== version) {
    console.error(`${name} is ${found}, expected ${version}`);
    process.exit(1);
  }
}

const core = await load("@blocknote/core");
const { blocksToYXmlFragment } = await load("@blocknote/core", "./yjs");
const Y = await load("yjs");

const pageLink = core.createInlineContentSpec(
  {
    type: "pageLink",
    propSchema: { targetPageId: { default: "" } },
    content: "none",
  },
  { render: () => ({ dom: document.createElement("span") }) },
);
const schema = core.BlockNoteSchema.create({
  inlineContentSpecs: { ...core.defaultInlineContentSpecs, pageLink },
});
const editor = core.BlockNoteEditor.create({ schema });

const link = (targetPageId) => ({ type: "pageLink", props: { targetPageId } });
const text = (value, styles = {}) => ({ type: "text", text: value, styles });

const blocks = [
  {
    id: "block-heading",
    type: "heading",
    props: { level: 2 },
    content: [text("Café "), text("roadmap", { bold: true })],
  },
  {
    id: "block-link",
    type: "paragraph",
    content: [
      text("see "),
      link("page-target"),
      text(" today", { italic: true }),
    ],
  },
  {
    id: "block-twice",
    type: "paragraph",
    content: [
      link("page-target"),
      text(" and "),
      link("page-target"),
      text(" and "),
      link("page-other"),
    ],
  },
  {
    id: "block-parent",
    type: "bulletListItem",
    content: [text("parent item")],
    children: [
      {
        id: "block-child",
        type: "bulletListItem",
        content: [text("child item")],
        children: [
          {
            id: "block-grandchild",
            type: "checkListItem",
            props: { checked: true },
            content: [text("done "), link("page-other")],
          },
        ],
      },
    ],
  },
  {
    id: "block-numbered",
    type: "numberedListItem",
    content: [text("first step")],
  },
  {
    id: "block-code",
    type: "codeBlock",
    props: { language: "rust" },
    content: [text('fn main() {\n    let s = "[[Not a link]]";\n}')],
  },
  {
    id: "block-break",
    type: "paragraph",
    content: [text("line one\nline two")],
  },
  {
    id: "block-href",
    type: "paragraph",
    content: [
      text("read the "),
      {
        type: "link",
        href: "https://example.com/docs",
        content: [text("docs")],
      },
      text(" and [[Literal brackets]]", { code: true }),
    ],
  },
  {
    id: "block-quote",
    type: "quote",
    content: [text("naïve 日本 quote")],
  },
  {
    id: "block-image",
    type: "image",
    props: {
      url: "asset://0123abcd.png",
      caption: "Architecture diagram",
      name: "diagram.png",
    },
  },
  { id: "block-divider", type: "divider" },
  { id: "block-empty", type: "paragraph", content: [] },
];

const ydoc = new Y.Doc();
ydoc.clientID = 1;
blocksToYXmlFragment(editor, blocks, ydoc.getXmlFragment("prosemirror"));
const update = Y.encodeStateAsUpdate(ydoc);

const target =
  out ?? fileURLToPath(new URL("./blocknote-body.yupdate", import.meta.url));
writeFileSync(target, update);
console.log(`${target}\t${update.length} bytes`);
console.log(ydoc.getXmlFragment("prosemirror").toString());
