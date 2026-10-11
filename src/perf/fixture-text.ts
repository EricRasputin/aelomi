export type FixtureParagraph = { id: string; text: string };

function inlineText(inline: unknown): string {
  if (typeof inline !== "object" || inline === null)
    throw new Error("inline content is not an object");
  const { type, text, content } = inline as {
    type?: unknown;
    text?: unknown;
    content?: unknown;
  };
  if (type === "text" && typeof text === "string") return text;
  if (type === "link" && Array.isArray(content))
    return content.map(inlineText).join("");
  throw new Error(`unknown inline content ${String(type)}`);
}

export function fixtureParagraphs(raw: unknown): FixtureParagraph[] {
  if (!Array.isArray(raw)) throw new Error("fixture is not a block array");
  const paragraphs: FixtureParagraph[] = [];
  const visit = (block: unknown) => {
    if (typeof block !== "object" || block === null)
      throw new Error("block is not an object");
    const { id, content, children } = block as {
      id?: unknown;
      content?: unknown;
      children?: unknown;
    };
    if (typeof id !== "string") throw new Error("block has no id");
    if (Array.isArray(content))
      paragraphs.push({ id, text: content.map(inlineText).join("") });
    if (Array.isArray(children)) children.forEach(visit);
  };
  raw.forEach(visit);
  return paragraphs;
}
