import type { BlockNoteEditor } from "@blocknote/core";
import * as Y from "yjs";
import { bodyRoot, isPageId, rootHasContent } from "./body-reader";

type Schema = BlockNoteEditor["pmSchema"];
type PMNode = ReturnType<Schema["text"]>;

export type GuardProblem =
  | { kind: "foreignRoot"; root: string }
  | { kind: "notAFragment" }
  | { kind: "unknownNode"; node: string }
  | { kind: "unknownAttribute"; node: string; attribute: string }
  | { kind: "changedAttribute"; node: string; attribute: string }
  | { kind: "unknownMark"; mark: string }
  | { kind: "changedMark"; mark: string }
  | { kind: "splitText"; node: string }
  | { kind: "embed"; node: string }
  | { kind: "invalidContent"; node: string }
  | { kind: "missingBlockId" }
  | { kind: "duplicateBlockId"; blockId: string }
  | { kind: "invalidPageLink" };

export type BodyCheck =
  | { kind: "editable"; empty: boolean }
  | { kind: "readOnly"; problem: GuardProblem };

const hashedMarkName = /(.*)(--[a-zA-Z0-9+/=]{8})$/;

/**
 * y-prosemirror 1.3.7 stores a mark that may overlap itself as
 * `name--<hash>` and every other mark under its plain name. No mark in this
 * schema overlaps itself, so a hashed key came from another writer, and the
 * binding rewrites it to the plain name at the first edit of that text.
 */
function markName(attribute: string): string {
  return hashedMarkName.exec(attribute)?.[1] ?? attribute;
}

function validate(node: PMNode) {
  if (!node.type.validContent(node.content))
    throw new Unsupported({ kind: "invalidContent", node: node.type.name });
  node.forEach(validate);
}

class Unsupported extends Error {
  constructor(readonly problem: GuardProblem) {
    super(problem.kind);
  }
}

function attributesMatch(
  stored: Record<string, unknown>,
  computed: Record<string, unknown>,
): boolean {
  const storedKeys = Object.keys(stored).filter((key) => stored[key] !== null);
  const computedKeys = Object.keys(computed).filter(
    (key) => computed[key] !== null,
  );
  return (
    storedKeys.length === computedKeys.length &&
    storedKeys.every((key) => stored[key] === computed[key])
  );
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/**
 * Checks a page body before y-prosemirror binds it. y-prosemirror deletes Y
 * nodes and text it cannot build, rewrites attributes that differ from the
 * ProseMirror node, and merges split text runs. Each of those would change
 * source the user never touched, so such a body opens read-only instead.
 */
export function checkBody(doc: Y.Doc, schema: Schema): BodyCheck {
  for (const [name, type] of doc.share) {
    if (name !== bodyRoot && rootHasContent(type))
      return { kind: "readOnly", problem: { kind: "foreignRoot", root: name } };
  }
  let fragment: Y.XmlFragment;
  try {
    fragment = doc.getXmlFragment(bodyRoot);
  } catch {
    return { kind: "readOnly", problem: { kind: "notAFragment" } };
  }
  if (fragment.length === 0) return { kind: "editable", empty: true };
  const blockIds = new Set<string>();

  const element = (el: Y.XmlElement): PMNode => {
    const type = schema.nodes[el.nodeName];
    if (!type)
      throw new Unsupported({ kind: "unknownNode", node: el.nodeName });
    const stored = el.getAttributes() as Record<string, unknown>;
    for (const [attribute, value] of Object.entries(stored)) {
      if (!Object.hasOwn(type.spec.attrs ?? {}, attribute))
        throw new Unsupported({
          kind: "unknownAttribute",
          node: el.nodeName,
          attribute,
        });
      // The binding deletes a stored null at the element's first edit, and
      // ProseMirror then reads the attribute's default.
      if (value === null)
        throw new Unsupported({
          kind: "changedAttribute",
          node: el.nodeName,
          attribute,
        });
    }
    if (el.nodeName === "blockContainer") {
      const id = stored.id;
      if (typeof id !== "string" || id === "")
        throw new Unsupported({ kind: "missingBlockId" });
      if (blockIds.has(id))
        throw new Unsupported({ kind: "duplicateBlockId", blockId: id });
      blockIds.add(id);
    }
    if (el.nodeName === "pageLink" && !isPageId(stored.targetPageId))
      throw new Unsupported({ kind: "invalidPageLink" });
    const content = children(el);
    let node: PMNode;
    try {
      node = type.create(stored, content);
    } catch {
      throw new Unsupported({ kind: "invalidContent", node: el.nodeName });
    }
    for (const [attribute, value] of Object.entries(stored)) {
      if (node.attrs[attribute] !== value)
        throw new Unsupported({
          kind: "changedAttribute",
          node: el.nodeName,
          attribute,
        });
    }
    return node;
  };

  const text = (yText: Y.XmlText, parent: string): PMNode[] => {
    const nodes: PMNode[] = [];
    for (const op of yText.toDelta() as {
      insert: unknown;
      attributes?: Record<string, unknown>;
    }[]) {
      if (typeof op.insert !== "string")
        throw new Unsupported({ kind: "embed", node: parent });
      const marks = Object.entries(op.attributes ?? {}).map(([key, attrs]) => {
        const name = markName(key);
        const markType = schema.marks[name];
        if (!markType)
          throw new Unsupported({ kind: "unknownMark", mark: name });
        if (key !== name)
          throw new Unsupported({ kind: "changedMark", mark: name });
        if (!isRecord(attrs))
          throw new Unsupported({ kind: "changedMark", mark: name });
        // ProseMirror drops an unknown attribute, so the binding rewrites the
        // stored value without it, even a null, at any edit in that Y text.
        for (const attribute of Object.keys(attrs))
          if (!Object.hasOwn(markType.spec.attrs ?? {}, attribute))
            throw new Unsupported({ kind: "changedMark", mark: name });
        const mark = markType.create(attrs);
        if (!attributesMatch(attrs, mark.attrs))
          throw new Unsupported({ kind: "changedMark", mark: name });
        return mark;
      });
      // y-prosemirror renders a run whose marks exclude each other, such as
      // a link on inline code, but ProseMirror reparses the run when the user
      // types in or next to it and drops the excluded mark.
      for (const mark of marks)
        for (const other of marks)
          if (mark !== other && mark.type.excludes(other.type))
            throw new Unsupported({
              kind: "changedMark",
              mark: other.type.name,
            });
      nodes.push(schema.text(op.insert, marks));
    }
    return nodes;
  };

  const children = (parent: Y.XmlElement | Y.XmlFragment) => {
    const name = parent instanceof Y.XmlElement ? parent.nodeName : bodyRoot;
    const nodes: PMNode[] = [];
    let previousText = false;
    for (const child of parent.toArray()) {
      if (child instanceof Y.XmlText) {
        if (previousText || child.length === 0)
          throw new Unsupported({ kind: "splitText", node: name });
        previousText = true;
        nodes.push(...text(child, name));
      } else if (child instanceof Y.XmlElement) {
        previousText = false;
        nodes.push(element(child));
      } else {
        throw new Unsupported({ kind: "unknownNode", node: "hook" });
      }
    }
    return nodes;
  };

  try {
    validate(schema.topNodeType.create(null, children(fragment)));
  } catch (error: unknown) {
    if (error instanceof Unsupported)
      return { kind: "readOnly", problem: error.problem };
    throw error;
  }
  return { kind: "editable", empty: false };
}
