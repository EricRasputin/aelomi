import * as Y from "yjs";
import { rootContent, rootHasContent } from "./yjs-roots";

export const bodyRoot = "prosemirror";

export type Segment =
  | { kind: "text"; text: string }
  | { kind: "link"; pageId: string };

export type BodyBlock = { id: string; segments: Segment[] };

/** A block the read-only view shows, including one with no ID. */
export type ShownBlock = { id: string | null; segments: Segment[] };

/** Listed in precedence order: a body reports only the first that applies. */
export type BodyProblem =
  | { kind: "Undecodable" }
  | { kind: "UnrecognizedRoot"; root: string }
  | { kind: "NotAFragment" }
  | { kind: "MissingBlockId" }
  | { kind: "InvalidPageLink" };

export type Body<Block = BodyBlock> = {
  blocks: Block[];
  problem: BodyProblem | null;
};

const problemRank: Record<BodyProblem["kind"], number> = {
  Undecodable: 0,
  UnrecognizedRoot: 1,
  NotAFragment: 2,
  MissingBlockId: 3,
  InvalidPageLink: 4,
};

const pageIdPattern = /^[0-9a-z-]{1,64}$/;

export function isPageId(value: unknown): value is string {
  return (
    typeof value === "string" &&
    value !== "workspace" &&
    pageIdPattern.test(value)
  );
}

type XmlNode = Y.XmlElement | Y.XmlText;

/**
 * Mirrors `crates/core/src/workspace/body.rs`: every block in document order,
 * parents before children. A block with no ID is skipped but its children are
 * read. An invalid page link reads as one space, so its neighbours never join
 * into a word that isn't there. Content that is not XML is skipped and named.
 */
export function readBody(doc: Y.Doc): Body {
  const { blocks, problem } = readShownBody(doc);
  return {
    blocks: blocks.flatMap(({ id, segments }) =>
      id === null ? [] : [{ id, segments }],
    ),
    problem,
  };
}

/** `readBody`, keeping blocks with no ID for the read-only view. */
export function readShownBody(doc: Y.Doc): Body<ShownBlock> {
  let problem: BodyProblem | null = null;
  const report = (next: BodyProblem) => {
    if (!problem || problemRank[next.kind] < problemRank[problem.kind])
      problem = next;
  };
  const nodes = (values: readonly unknown[]): XmlNode[] =>
    values.flatMap((value) => {
      if (value instanceof Y.XmlElement || value instanceof Y.XmlText)
        return [value];
      report({ kind: "NotAFragment" });
      return [];
    });
  const root = doc.share.get(bodyRoot);
  const content = root ? rootContent(root) : { sequence: [], entries: [] };
  if (content.sequence.length === 0 && content.entries.length === 0) {
    for (const [name, type] of doc.share) {
      if (name !== bodyRoot && rootHasContent(type))
        return {
          blocks: [],
          problem: { kind: "UnrecognizedRoot", root: name },
        };
    }
    return { blocks: [], problem: null };
  }
  if (content.entries.length > 0) report({ kind: "NotAFragment" });
  const blocks: ShownBlock[] = [];
  const stack = nodes(
    content.sequence.flatMap((item) => item.content.getContent()),
  ).reverse();
  while (stack.length > 0) {
    const node = stack.pop();
    if (!(node instanceof Y.XmlElement)) continue;
    if (node.nodeName !== "blockContainer") {
      stack.push(...nodes(node.toArray()).reverse());
      continue;
    }
    const id: unknown = node.getAttribute("id");
    const segments: Segment[] = [];
    const nested: XmlNode[] = [];
    for (const child of nodes(node.toArray())) {
      if (child instanceof Y.XmlElement && child.nodeName === "blockGroup")
        nested.push(...nodes(child.toArray()));
      else readContent(child, segments, report, nodes);
    }
    if (typeof id === "string" && id !== "") blocks.push({ id, segments });
    else {
      report({ kind: "MissingBlockId" });
      blocks.push({ id: null, segments });
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
  content: XmlNode,
  segments: Segment[],
  report: (problem: BodyProblem) => void,
  nodes: (values: readonly unknown[]) => XmlNode[],
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
        const target: unknown = node.getAttribute("targetPageId");
        if (isPageId(target)) segments.push({ kind: "link", pageId: target });
        else {
          report({ kind: "InvalidPageLink" });
          pushText(segments, " ");
        }
      } else {
        const caption: unknown = node.getAttribute("caption");
        if (node.nodeName === "image" && typeof caption === "string")
          pushText(segments, caption);
        stack.push(...nodes(node.toArray()).reverse());
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
