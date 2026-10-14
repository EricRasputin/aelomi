import * as Y from "yjs";

export const bodyRoot = "prosemirror";

export type Segment =
  | { kind: "text"; text: string }
  | { kind: "link"; pageId: string };

export type BodyBlock = { id: string; segments: Segment[] };

/** Listed in precedence order: a body reports only the first that applies. */
export type BodyProblem =
  | { kind: "Undecodable" }
  | { kind: "UnrecognizedRoot"; root: string }
  | { kind: "MissingBlockId" }
  | { kind: "InvalidPageLink" };

export type Body = { blocks: BodyBlock[]; problem: BodyProblem | null };

const problemRank: Record<BodyProblem["kind"], number> = {
  Undecodable: 0,
  UnrecognizedRoot: 1,
  MissingBlockId: 2,
  InvalidPageLink: 3,
};

const pageIdPattern = /^[0-9a-z-]{1,64}$/;

export function isPageId(value: unknown): value is string {
  return (
    typeof value === "string" &&
    value !== "workspace" &&
    pageIdPattern.test(value)
  );
}

type RootType = {
  _start: { deleted: boolean; right: unknown } | null;
  _map: Map<string, { deleted: boolean }>;
};

/** Whether a root holds anything not deleted, whatever type it was made as. */
export function rootHasContent(type: unknown): boolean {
  const root = type as RootType;
  let item = root._start;
  while (item) {
    if (!item.deleted) return true;
    item = item.right as RootType["_start"];
  }
  for (const entry of root._map.values()) if (!entry.deleted) return true;
  return false;
}

/**
 * Mirrors `crates/core/src/workspace/body.rs`: every block in document order,
 * parents before children. A block with no ID is skipped but its children are
 * read. An invalid page link reads as one space, so its neighbours never join
 * into a word that isn't there.
 */
export function readBody(
  doc: Y.Doc,
  { keepUnidentified = false }: { keepUnidentified?: boolean } = {},
): Body {
  let problem: BodyProblem | null = null;
  const report = (next: BodyProblem) => {
    if (!problem || problemRank[next.kind] < problemRank[problem.kind])
      problem = next;
  };
  let fragment: Y.XmlFragment;
  try {
    fragment = doc.getXmlFragment(bodyRoot);
  } catch {
    return { blocks: [], problem: { kind: "Undecodable" } };
  }
  if (fragment.length === 0) {
    for (const [name, type] of doc.share) {
      if (name !== bodyRoot && rootHasContent(type))
        return {
          blocks: [],
          problem: { kind: "UnrecognizedRoot", root: name },
        };
    }
    return { blocks: [], problem: null };
  }
  const blocks: BodyBlock[] = [];
  const stack: (Y.XmlElement | Y.XmlText | Y.XmlHook)[] = fragment
    .toArray()
    .reverse();
  while (stack.length > 0) {
    const node = stack.pop();
    if (!(node instanceof Y.XmlElement)) continue;
    if (node.nodeName !== "blockContainer") {
      stack.push(...node.toArray().reverse());
      continue;
    }
    const id = node.getAttribute("id") as unknown;
    const segments: Segment[] = [];
    const nested: (Y.XmlElement | Y.XmlText | Y.XmlHook)[] = [];
    for (const child of node.toArray()) {
      if (child instanceof Y.XmlElement && child.nodeName === "blockGroup")
        nested.push(...child.toArray());
      else readContent(child, segments, report);
    }
    if (typeof id === "string" && id !== "") blocks.push({ id, segments });
    else {
      report({ kind: "MissingBlockId" });
      if (keepUnidentified) blocks.push({ id: "", segments });
    }
    stack.push(...nested.reverse());
  }
  return { blocks, problem };
}

function pushText(segments: Segment[], text: string) {
  if (text === "") return;
  const last = segments.at(-1);
  if (last?.kind === "text") last.text += text;
  else segments.push({ kind: "text", text });
}

function readContent(
  content: Y.XmlElement | Y.XmlText | Y.XmlHook,
  segments: Segment[],
  report: (problem: BodyProblem) => void,
) {
  const stack = [content];
  while (stack.length > 0) {
    const node = stack.pop();
    if (node instanceof Y.XmlText) {
      for (const op of node.toDelta() as { insert: unknown }[]) {
        if (typeof op.insert === "string") pushText(segments, op.insert);
      }
    } else if (node instanceof Y.XmlElement) {
      if (node.nodeName === "hardBreak") pushText(segments, "\n");
      else if (node.nodeName === "pageLink") {
        const target = node.getAttribute("targetPageId") as unknown;
        if (isPageId(target)) segments.push({ kind: "link", pageId: target });
        else {
          report({ kind: "InvalidPageLink" });
          pushText(segments, " ");
        }
      } else {
        const caption = node.getAttribute("caption") as unknown;
        if (node.nodeName === "image" && typeof caption === "string")
          pushText(segments, caption);
        stack.push(...node.toArray().reverse());
      }
    }
  }
}

export function renderSegments(
  segments: readonly Segment[],
  label: (pageId: string) => string,
): string {
  return segments
    .map((segment) =>
      segment.kind === "text" ? segment.text : label(segment.pageId),
    )
    .join("");
}
