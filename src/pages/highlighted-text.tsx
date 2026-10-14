import type { TextPart } from "../ipc";

/** Renders highlight parts as React text, so no markup from a page is ever parsed. */
export function HighlightedText({ parts }: { parts: readonly TextPart[] }) {
  return parts.map((part, index) =>
    part.highlighted ? (
      // biome-ignore lint/suspicious/noArrayIndexKey: parts have no identity but their order
      <mark key={index}>{part.text}</mark>
    ) : (
      // biome-ignore lint/suspicious/noArrayIndexKey: parts have no identity but their order
      <span key={index}>{part.text}</span>
    ),
  );
}
