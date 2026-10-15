import * as Y from "yjs";

export type SeedStyle = "bold" | "italic" | "underline" | "strike" | "code";

export type SeedInline =
  | string
  | { text: string; styles: readonly SeedStyle[] }
  | { link: string };

export type SeedBlock = {
  id: string;
  type?:
    | "paragraph"
    | "heading"
    | "bulletListItem"
    | "numberedListItem"
    | "checkListItem"
    | "quote"
    | "codeBlock"
    | "divider";
  content?: readonly SeedInline[];
  level?: 1 | 2 | 3;
  checked?: boolean;
  language?: string;
  children?: readonly SeedBlock[];
};

const colors = { backgroundColor: "default", textColor: "default" };
const aligned = { ...colors, textAlignment: "left" };

/** An attribute value as BlockNote's Yjs binding stores it. */
export type StoredAttribute = string | number | boolean | null | undefined;

function attributes(block: SeedBlock): Record<string, StoredAttribute> {
  switch (block.type ?? "paragraph") {
    case "numberedListItem":
      return { ...aligned, start: undefined };
    case "heading":
      return { ...aligned, level: block.level ?? 1, isToggleable: false };
    case "checkListItem":
      return { ...aligned, checked: block.checked ?? false };
    case "quote":
      return colors;
    case "codeBlock":
      return { language: block.language ?? "text" };
    case "divider":
      return {};
    default:
      return aligned;
  }
}

/**
 * Stores one attribute as BlockNote 0.55 does. Yjs types attribute values
 * without `undefined`, yet BlockNote writes `start: undefined` on every
 * numbered item, so that value alone needs the cast.
 */
export function storeAttribute(
  element: Y.XmlElement,
  key: string,
  value: StoredAttribute,
) {
  const stored: Y.XmlElement<{
    [key: string]: string | number | boolean | null;
  }> = element;
  stored.setAttribute(key, value === undefined ? (value as never) : value);
}

function inlineNodes(
  content: readonly SeedInline[],
): (Y.XmlText | Y.XmlElement)[] {
  const nodes: (Y.XmlText | Y.XmlElement)[] = [];
  let run: { insert: string; attributes?: Record<string, object> }[] = [];
  const flush = () => {
    if (run.length === 0) return;
    const text = new Y.XmlText();
    text.applyDelta(run);
    nodes.push(text);
    run = [];
  };
  for (const inline of content) {
    if (typeof inline === "string") {
      if (inline !== "") run.push({ insert: inline });
    } else if ("link" in inline) {
      flush();
      const link = new Y.XmlElement("pageLink");
      link.setAttribute("targetPageId", inline.link);
      nodes.push(link);
    } else if (inline.text !== "") {
      run.push({
        insert: inline.text,
        ...(inline.styles.length > 0 && {
          attributes: Object.fromEntries(
            inline.styles.map((style) => [style, {}]),
          ),
        }),
      });
    }
  }
  flush();
  return nodes;
}

function container(block: SeedBlock): Y.XmlElement {
  const outer = new Y.XmlElement("blockContainer");
  outer.setAttribute("id", block.id);
  const content = new Y.XmlElement(block.type ?? "paragraph");
  for (const [key, value] of Object.entries(attributes(block)))
    storeAttribute(content, key, value);
  content.insert(0, inlineNodes(block.content ?? []));
  const parts: Y.XmlElement[] = [content];
  if (block.children && block.children.length > 0) {
    const group = new Y.XmlElement("blockGroup");
    group.insert(0, block.children.map(container));
    parts.push(group);
  }
  outer.insert(0, parts);
  return outer;
}

/**
 * Writes a page body in the layout BlockNote's Yjs binding writes, as
 * `crates/core/src/workspace/fixture.rs` does, with BlockNote's default
 * attributes so the editor opens it without writing anything.
 */
export function writeBody(doc: Y.Doc, blocks: readonly SeedBlock[]) {
  const fragment = doc.getXmlFragment("prosemirror");
  doc.transact(() => {
    const group = new Y.XmlElement("blockGroup");
    group.insert(0, blocks.map(container));
    fragment.insert(0, [group]);
  });
}
