// @vitest-environment happy-dom
import { readFileSync } from "node:fs";
import { BlockNoteEditor } from "@blocknote/core";
import { blocksToYXmlFragment } from "@blocknote/core/yjs";
import type { Node as PMNode } from "prosemirror-model";
import {
  AllSelection,
  EditorState,
  NodeSelection,
  Plugin,
  type Selection,
  TextSelection,
  type Transaction,
} from "prosemirror-state";
import { EditorView, type NodeView } from "prosemirror-view";
import { afterAll, beforeAll, expect, test } from "vitest";
import * as Y from "yjs";
import { editorSchema } from "./schema";
import {
  type BindingModule,
  loadBindings,
  repoFile,
  sha256,
  syncPluginSources,
  upstreamSyncPluginSha256,
} from "./y-binding-variants";

const editor = BlockNoteEditor.create({ schema: editorSchema });
const schema = editor.pmSchema;
let bindings: Awaited<ReturnType<typeof loadBindings>>;

beforeAll(async () => {
  bindings = await loadBindings();
});
afterAll(() => bindings.dispose());

function bodyBytes(blocks: unknown[]): Uint8Array {
  const doc = new Y.Doc();
  blocksToYXmlFragment(
    editor,
    blocks as typeof editor.document,
    doc.getXmlFragment("prosemirror"),
  );
  return Y.encodeStateAsUpdate(doc);
}

const text = (value: string, styles: Record<string, boolean> = {}) => ({
  type: "text",
  text: value,
  styles,
});
const pageLink = (targetPageId: string) => ({
  type: "pageLink",
  props: { targetPageId },
});

const rich = bodyBytes([
  { id: "r-intro", type: "paragraph", content: "Intro line with words" },
  { id: "r-empty1", type: "paragraph", content: "" },
  { id: "r-empty2", type: "paragraph", content: "" },
  {
    id: "r-h",
    type: "heading",
    props: { level: 2 },
    content: [text("Head "), text("bold", { bold: true })],
  },
  {
    id: "r-img",
    type: "image",
    props: { url: "data:image/png;base64,iVBORw0KGgo=", caption: "cap" },
  },
  { id: "r-div", type: "divider" },
  { id: "r-num1", type: "numberedListItem", content: "first" },
  { id: "r-num2", type: "numberedListItem", content: "second" },
  {
    id: "r-bul",
    type: "bulletListItem",
    content: "parent",
    children: [
      {
        id: "r-child",
        type: "paragraph",
        content: "child text",
        children: [
          {
            id: "r-grand",
            type: "checkListItem",
            props: { checked: true },
            content: "grand",
          },
        ],
      },
    ],
  },
  {
    id: "r-code",
    type: "codeBlock",
    props: { language: "rust" },
    content: "fn main() {}\nlet x = 1;",
  },
  {
    id: "r-quote",
    type: "quote",
    content: [
      text("see "),
      { type: "link", href: "https://example.com", content: "example" },
      text(" now", { italic: true }),
    ],
  },
  {
    id: "r-link",
    type: "paragraph",
    content: [
      text("go "),
      pageLink("page-target"),
      text(" and  "),
      pageLink("page-other"),
    ],
  },
  { id: "r-dup1", type: "paragraph", content: "same text" },
  { id: "r-dup2", type: "paragraph", content: "same text" },
  { id: "r-uni", type: "paragraph", content: "émoji 😀 עברית 漢字 é" },
]);

const realBody = new Uint8Array(
  readFileSync(repoFile("fixtures/index/blocknote-body.yupdate")),
);

type Session = {
  ydoc: Y.Doc;
  view: EditorView;
  sync: Plugin;
  undoPlugin: Plugin;
  binding: BindingModule;
  updates: string[];
};

const hex = (bytes: Uint8Array) => Buffer.from(bytes).toString("hex");

function blockIds(): Plugin {
  let next = 0;
  return new Plugin({
    appendTransaction: (trs, _old, state) => {
      if (!trs.some((tr) => tr.docChanged)) return null;
      const seen = new Set<string>();
      let tr: Transaction | null = null;
      state.doc.descendants((node, pos) => {
        if (node.type.name !== "blockContainer") return true;
        const id = node.attrs.id as string | null;
        if (id && !seen.has(id)) {
          seen.add(id);
          return true;
        }
        next += 1;
        tr ??= state.tr;
        tr.setNodeMarkup(pos, undefined, { ...node.attrs, id: `gen-${next}` });
        seen.add(`gen-${next}`);
        return true;
      });
      return tr;
    },
  });
}

// ProseMirror's default schema rendering ignores the destroy that toDOM
// returns to release the page tree store subscription, so it would retain
// every closed view. The app renders page links through BlockNote's node
// view, which calls it.
function pageLinkView(node: PMNode): NodeView {
  const rendered = nodeType("pageLink").spec.toDOM?.(node);
  if (
    typeof rendered !== "object" ||
    !("dom" in rendered) ||
    !("destroy" in rendered) ||
    typeof rendered.destroy !== "function"
  )
    throw new Error("pageLink no longer renders a destroyable node view");
  return rendered as NodeView;
}

function open(binding: BindingModule, bytes: Uint8Array): Session {
  const ydoc = new Y.Doc();
  Y.applyUpdate(ydoc, bytes);
  ydoc.clientID = 4242;
  const type = ydoc.getXmlFragment("prosemirror");
  const sync = binding.ySyncPlugin(type);
  const undoPlugin = binding.yUndoPlugin();
  const mount = document.createElement("div");
  document.body.append(mount);
  const view = new EditorView(mount, {
    state: EditorState.create({
      schema,
      plugins: [sync, undoPlugin, blockIds()],
    }),
    nodeViews: { pageLink: pageLinkView },
  });
  const updates: string[] = [];
  ydoc.on("update", (update: Uint8Array) => updates.push(hex(update)));
  return { ydoc, view, sync, undoPlugin, binding, updates };
}

function close(session: Session) {
  const mount = session.view.dom.parentElement;
  session.view.destroy();
  mount?.remove();
  session.ydoc.destroy();
}

function undoManager(session: Session): Y.UndoManager {
  return (
    session.undoPlugin.getState(session.view.state) as {
      undoManager: Y.UndoManager;
    }
  ).undoManager;
}

type Rel = {
  anchor: Y.RelativePosition | null;
  head: Y.RelativePosition | null;
};

const encodeRel = (rel: Rel | null | undefined) =>
  rel
    ? [rel.anchor, rel.head].map((p) =>
        p ? hex(Y.encodeRelativePosition(p)) : null,
      )
    : null;

function observe(session: Session) {
  const { state } = session.view;
  const syncBinding = (session.sync.getState(state) as { binding: unknown })
    .binding;
  const um = undoManager(session);
  const stored = (item: { meta: Map<unknown, unknown> }) =>
    encodeRel(item.meta.get(syncBinding) as Rel | undefined);
  return {
    doc: state.doc.toJSON(),
    selection: state.selection.toJSON(),
    updates: session.updates.splice(0),
    relative: encodeRel(
      session.binding.getRelativeSelection(syncBinding, state) as Rel,
    ),
    undo: um.undoStack.map(stored),
    redo: um.redoStack.map(stored),
  };
}

type Block = { pos: number; node: PMNode; start: number; end: number };

function block(doc: PMNode, id: string): Block {
  let found: Block | null = null;
  doc.descendants((node, pos) => {
    if (found) return false;
    if (node.type.name === "blockContainer" && node.attrs.id === id) {
      const content = node.firstChild;
      const start = pos + 2;
      found = {
        pos,
        node,
        start,
        end: start + (content?.content.size ?? 0),
      };
      return false;
    }
    return true;
  });
  if (!found) throw new Error(`no block ${id}`);
  return found;
}

function textBlocks(doc: PMNode): string[] {
  const ids: string[] = [];
  doc.descendants((node) => {
    if (
      node.type.name === "blockContainer" &&
      node.firstChild?.isTextblock &&
      typeof node.attrs.id === "string"
    )
      ids.push(node.attrs.id);
    return true;
  });
  return ids;
}

function atomBlocks(doc: PMNode): string[] {
  const ids: string[] = [];
  doc.descendants((node) => {
    if (
      node.type.name === "blockContainer" &&
      node.firstChild &&
      !node.firstChild.isTextblock &&
      typeof node.attrs.id === "string"
    )
      ids.push(node.attrs.id);
    return true;
  });
  return ids;
}

type Op = (state: EditorState) => Transaction | null;

function nodeType(name: string) {
  const type = schema.nodes[name];
  if (!type) throw new Error(`no node type ${name}`);
  return type;
}

function markType(name: string) {
  const type = schema.marks[name];
  if (!type) throw new Error(`no mark type ${name}`);
  return type;
}

const caret =
  (id: string, where: "start" | "mid" | "end"): Op =>
  (state) => {
    const b = block(state.doc, id);
    const at =
      where === "start"
        ? b.start
        : where === "end"
          ? b.end
          : b.start + ((b.end - b.start) >> 1);
    return state.tr.setSelection(TextSelection.create(state.doc, at));
  };

const siblingAfter = (state: EditorState, id: string) => {
  const b = block(state.doc, id);
  const $pos = state.doc.resolve(b.pos);
  const index = $pos.index();
  const parent = $pos.parent;
  return index + 1 < parent.childCount
    ? (parent.child(index + 1).attrs.id as string)
    : null;
};
const siblingBefore = (state: EditorState, id: string) => {
  const b = block(state.doc, id);
  const $pos = state.doc.resolve(b.pos);
  const index = $pos.index();
  return index > 0 ? ($pos.parent.child(index - 1).attrs.id as string) : null;
};

function opsFor(id: string): [string, Op[]][] {
  const typeText: Op = (state) => state.tr.insertText("ab");
  const split: Op = (state) => {
    const $from = state.selection.$from;
    if (!$from.parent.isTextblock) return null;
    return state.tr.split($from.pos, 2);
  };
  const mark =
    (name: string, attrs: Record<string, unknown> = {}, part = false): Op =>
    (state) => {
      const b = block(state.doc, id);
      const to = part ? b.start + ((b.end - b.start) >> 1) : b.end;
      if (to <= b.start) return null;
      return state.tr.addMark(b.start, to, markType(name).create(attrs));
    };
  const retype =
    (name: string, attrs: Record<string, unknown> = {}): Op =>
    (state) => {
      const b = block(state.doc, id);
      const content = b.node.firstChild;
      if (!content) return null;
      const type = nodeType(name);
      if (!type.validContent(content.content)) return null;
      return state.tr.setNodeMarkup(b.pos + 1, type, {
        ...content.attrs,
        ...attrs,
      });
    };
  const move =
    (direction: -1 | 1): Op =>
    (state) => {
      const other =
        direction < 0 ? siblingBefore(state, id) : siblingAfter(state, id);
      if (!other) return null;
      const b = block(state.doc, id);
      const o = block(state.doc, other);
      const tr = state.tr.delete(b.pos, b.pos + b.node.nodeSize);
      const target =
        direction < 0
          ? tr.mapping.map(o.pos)
          : tr.mapping.map(o.pos + o.node.nodeSize);
      return tr.insert(target, b.node);
    };
  const across =
    (insert: string | null): Op =>
    (state) => {
      const next = siblingAfter(state, id);
      if (!next) return null;
      const b = block(state.doc, id);
      const n = block(state.doc, next);
      const from = b.start + ((b.end - b.start) >> 1);
      const to = n.start + ((n.end - n.start) >> 1);
      const tr = state.tr.setSelection(
        TextSelection.create(state.doc, from, to),
      );
      return insert === null ? tr.deleteSelection() : tr.insertText(insert);
    };
  const joinBackward: Op = (state) => {
    const before = siblingBefore(state, id);
    if (!before) return null;
    const p = block(state.doc, before);
    if (!p.node.firstChild?.isTextblock || p.node.childCount > 1) return null;
    const b = block(state.doc, id);
    return state.tr.delete(p.end, b.start);
  };
  const deleteForward: Op = (state) => {
    const b = block(state.doc, id);
    if (b.end <= b.start) return null;
    return state.tr.delete(b.end - 1, b.end);
  };
  const duplicate: Op = (state) => {
    const b = block(state.doc, id);
    return state.tr.insert(b.pos + b.node.nodeSize, b.node);
  };
  const identical: Op = (state) => {
    const b = block(state.doc, id);
    return state.tr.replaceWith(
      b.pos,
      b.pos + b.node.nodeSize,
      b.node.type.create(b.node.attrs, b.node.content, b.node.marks),
    );
  };
  const remove: Op = (state) => {
    const b = block(state.doc, id);
    return state.tr.delete(b.pos, b.pos + b.node.nodeSize);
  };
  const indent: Op = (state) => {
    const before = siblingBefore(state, id);
    if (!before) return null;
    const p = block(state.doc, before);
    const b = block(state.doc, id);
    const group = p.node.childCount > 1 ? p.node.child(1) : null;
    const tr = state.tr.delete(b.pos, b.pos + b.node.nodeSize);
    if (group) return tr.insert(p.pos + p.node.nodeSize - 2, b.node);
    return tr.insert(
      p.pos + p.node.nodeSize - 1,
      nodeType("blockGroup").create(null, b.node),
    );
  };
  const insertLink: Op = (state) =>
    state.tr.replaceSelectionWith(
      nodeType("pageLink").create({ targetPageId: "page-target" }),
    );
  const selectAll =
    (insert: string | null): Op =>
    (state) => {
      const tr = state.tr.setSelection(new AllSelection(state.doc));
      return insert === null ? tr.deleteSelection() : tr.insertText(insert);
    };
  return [
    ["type at start", [caret(id, "start"), typeText]],
    ["type in the middle", [caret(id, "mid"), typeText]],
    ["type at end", [caret(id, "end"), typeText]],
    ["enter at start", [caret(id, "start"), split]],
    ["enter in the middle", [caret(id, "mid"), split]],
    ["enter at end", [caret(id, "end"), split]],
    ["enter then type", [caret(id, "end"), split, typeText]],
    ["backspace at start", [caret(id, "start"), joinBackward]],
    ["delete at end", [caret(id, "end"), deleteForward]],
    ["indent", [caret(id, "mid"), indent]],
    ["move up", [caret(id, "mid"), move(-1)]],
    ["move down", [caret(id, "mid"), move(1)]],
    ["bold all", [caret(id, "mid"), mark("bold")]],
    ["bold part", [caret(id, "mid"), mark("bold", {}, true)]],
    [
      "link part",
      [caret(id, "mid"), mark("link", { href: "https://x.org" }, true)],
    ],
    ["to heading", [caret(id, "mid"), retype("heading", { level: 1 })]],
    ["to bullet", [caret(id, "mid"), retype("bulletListItem")]],
    [
      "text color",
      [
        caret(id, "mid"),
        (state) => {
          const b = block(state.doc, id);
          return state.tr.setNodeMarkup(b.pos + 1, undefined, {
            ...b.node.firstChild?.attrs,
            textColor: "red",
          });
        },
      ],
    ],
    ["delete across blocks", [across(null)]],
    ["type across blocks", [across("Z")]],
    ["insert page link", [caret(id, "mid"), insertLink]],
    ["duplicate", [caret(id, "mid"), duplicate]],
    ["identical replace", [caret(id, "mid"), identical]],
    ["remove", [caret(id, "mid"), remove]],
    ["select all then type", [caret(id, "mid"), selectAll("all")]],
    ["select all then delete", [caret(id, "mid"), selectAll(null)]],
  ];
}

function atomOpsFor(id: string): [string, Op[]][] {
  const select: Op = (state) =>
    state.tr.setSelection(
      NodeSelection.create(state.doc, block(state.doc, id).pos + 1),
    );
  const remove: Op = (state) => {
    const b = block(state.doc, id);
    return state.tr.delete(b.pos, b.pos + b.node.nodeSize);
  };
  return [
    [
      "node select then delete",
      [select, (state) => state.tr.deleteSelection()],
    ],
    ["node select then type", [select, (state) => state.tr.insertText("t")]],
    ["remove", [select, remove]],
  ];
}

type Step = "op" | "undo" | "redo" | "type";
const afterOp: Step[] = [
  "undo",
  "redo",
  "undo",
  "redo",
  "type",
  "undo",
  "undo",
];

function apply(session: Session, step: Step, ops: Op[]) {
  const { view } = session;
  if (step === "undo") session.binding.undo(view.state);
  else if (step === "redo") session.binding.redo(view.state);
  else if (step === "type") {
    view.dispatch(view.state.tr.insertText("k"));
    view.dispatch(view.state.tr.insertText("m"));
  } else
    for (const op of ops) {
      const tr = op(view.state);
      if (tr) view.dispatch(tr);
    }
  undoManager(session).stopCapturing();
}

function attempt(session: Session, step: Step, ops: Op[]): string | null {
  try {
    apply(session, step, ops);
    return null;
  } catch (error: unknown) {
    return error instanceof Error ? error.message : String(error);
  }
}

function compare(bytes: Uint8Array, name: string, ops: Op[], steps = afterOp) {
  const up = open(bindings.upstream, bytes);
  const pa = open(bindings.patched, bytes);
  try {
    expect(observe(pa), `${name}: after open`).toEqual(observe(up));
    for (const step of ["op", ...steps] as Step[]) {
      const upError = attempt(up, step, ops);
      const paError = attempt(pa, step, ops);
      expect(paError, `${name}: ${step} error`).toBe(upError);
      if (upError !== null) break;
      expect(observe(pa), `${name}: after ${step}`).toEqual(observe(up));
    }
    return { up: up.view.state, pa: pa.view.state };
  } finally {
    close(up);
    close(pa);
  }
}

function visible(
  binding: BindingModule,
  bytes: Uint8Array,
  ops: Op[],
  steps: Step[],
) {
  const session = open(binding, bytes);
  try {
    return (["op", ...steps] as Step[]).map((step) => {
      apply(session, step, ops);
      const { state } = session.view;
      return {
        step,
        texts: blockTexts(state.doc),
        selection: state.selection.toJSON(),
      };
    });
  } finally {
    close(session);
  }
}

const blockTexts = (doc: PMNode) => {
  const texts: string[] = [];
  doc.descendants((node) => {
    if (node.isTextblock) texts.push(node.textContent);
    return !node.isTextblock;
  });
  return texts;
};

test("the rebuilt upstream source is the pinned tarball's file", () => {
  expect(sha256(syncPluginSources().upstream)).toBe(upstreamSyncPluginSha256);
  expect(syncPluginSources().patched).not.toBe(syncPluginSources().upstream);
});

test("audit repro: type over a cross-block range, undo, redo, type keeps upstream's caret", () => {
  const ops: Op[] = [
    (state) =>
      state.tr.setSelection(
        TextSelection.create(
          state.doc,
          block(state.doc, "r-empty2").start,
          block(state.doc, "r-h").start + "Head ".length,
        ),
      ),
    (state) => state.tr.insertText("q"),
  ];
  const steps: Step[] = ["undo", "redo", "type"];
  const seen = visible(bindings.patched, rich, ops, steps);
  expect(seen).toEqual(visible(bindings.upstream, rich, ops, steps));
  expect(seen.at(-1)?.texts).toContain("qboldkm");
  compare(rich, "redo range", ops, steps);
});

test("audit repro: undoing a deleted image selects the image again, not the divider", () => {
  const ops: Op[] = [
    (state) =>
      state.tr.setSelection(
        NodeSelection.create(state.doc, block(state.doc, "r-img").pos + 1),
      ),
    (state) => state.tr.deleteSelection(),
  ];
  const steps: Step[] = ["undo", "redo", "undo"];
  const seen = visible(bindings.patched, rich, ops, steps);
  expect(seen).toEqual(visible(bindings.upstream, rich, ops, steps));
  const final = compare(rich, "node undo", ops, steps);
  const selection = final.pa.selection as NodeSelection;
  expect(selection.node.type.name).toBe("image");
});

test("every edit kind with undo and redo matches upstream on a rich body", () => {
  const doc = open(bindings.upstream, rich);
  const texts = textBlocks(doc.view.state.doc);
  const atoms = atomBlocks(doc.view.state.doc);
  close(doc);
  let scenarios = 0;
  for (const id of texts)
    for (const [name, ops] of opsFor(id)) {
      compare(rich, `${id} ${name}`, ops);
      scenarios += 1;
    }
  for (const id of atoms)
    for (const [name, ops] of atomOpsFor(id)) {
      compare(rich, `${id} ${name}`, ops);
      scenarios += 1;
    }
  expect(scenarios).toBeGreaterThan(300);
}, 120_000);

test("every edit kind with undo and redo matches upstream on the real BlockNote body", () => {
  const doc = open(bindings.upstream, realBody);
  const texts = textBlocks(doc.view.state.doc);
  close(doc);
  let scenarios = 0;
  for (const id of texts)
    for (const [name, ops] of opsFor(id)) {
      compare(realBody, `${id} ${name}`, ops);
      scenarios += 1;
    }
  expect(scenarios).toBeGreaterThan(50);
}, 120_000);

test("seeded random sessions match upstream step by step", () => {
  for (let seed = 1; seed <= 12; seed++) {
    let state = seed * 2654435761;
    const random = () => {
      state = (state * 1103515245 + 12345) % 2147483648;
      return state / 2147483648;
    };
    const up = open(bindings.upstream, rich);
    const pa = open(bindings.patched, rich);
    try {
      for (let step = 0; step < 40; step++) {
        const roll = random();
        const ids = textBlocks(up.view.state.doc);
        const id = ids[Math.floor(random() * ids.length)] ?? "r-intro";
        let kind: Step = "op";
        let ops: Op[] = [];
        if (roll < 0.15) kind = "undo";
        else if (roll < 0.25) kind = "redo";
        else if (roll < 0.35) kind = "type";
        else {
          const choices = opsFor(id);
          ops = choices[Math.floor(random() * choices.length)]?.[1] ?? [];
        }
        const upError = attempt(up, kind, ops);
        const paError = attempt(pa, kind, ops);
        expect(paError, `seed ${seed} step ${step} ${kind} error`).toBe(
          upError,
        );
        if (upError !== null) break;
        expect(observe(pa), `seed ${seed} step ${step} ${kind}`).toEqual(
          observe(up),
        );
      }
    } finally {
      close(up);
      close(pa);
    }
  }
}, 120_000);

test("the 5,000-block page matches upstream at its first, nested, middle, and last blocks", () => {
  const blocks = JSON.parse(
    readFileSync(repoFile("fixtures/generated/page-5000-blocks.json"), "utf8"),
  );
  const page = bodyBytes(blocks);
  const doc = open(bindings.upstream, page);
  const texts = textBlocks(doc.view.state.doc);
  close(doc);
  const nested = texts.find((id) => {
    const probe = open(bindings.upstream, page);
    try {
      return (
        probe.view.state.doc.resolve(block(probe.view.state.doc, id).pos)
          .depth > 1
      );
    } finally {
      close(probe);
    }
  });
  const sample = [texts[0], nested, texts[texts.length >> 1], texts.at(-1)];
  const kinds = new Set([
    "type at end",
    "enter in the middle",
    "backspace at start",
    "move up",
    "bold part",
    "delete across blocks",
  ]);
  for (const id of sample)
    for (const [name, ops] of opsFor(id ?? ""))
      if (kinds.has(name)) compare(page, `${id} ${name}`, ops);
}, 300_000);

test("on synced states every position encodes as upstream encodes it", () => {
  for (const bytes of [rich, realBody]) {
    const up = open(bindings.upstream, bytes);
    const pa = open(bindings.patched, bytes);
    try {
      const size = up.view.state.doc.content.size;
      for (let pos = 0; pos <= size; pos++) {
        for (const session of [up, pa]) {
          const { state } = session.view;
          let selection: Selection;
          try {
            selection = TextSelection.create(state.doc, pos);
          } catch {
            continue;
          }
          session.view.dispatch(state.tr.setSelection(selection));
        }
        expect(observe(pa).relative, `position ${pos}`).toEqual(
          observe(up).relative,
        );
      }
    } finally {
      close(up);
      close(pa);
    }
  }
}, 120_000);
