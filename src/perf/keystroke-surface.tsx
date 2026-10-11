import type { FixtureParagraph } from "./fixture-text";

export function KeystrokeSurface({
  paragraphs,
}: {
  paragraphs: readonly FixtureParagraph[];
}) {
  return (
    <div
      data-testid="keystroke-surface"
      contentEditable
      suppressContentEditableWarning
      spellCheck={false}
      className="min-h-40 w-full max-w-2xl whitespace-pre-wrap outline-none"
    >
      {paragraphs.map((paragraph) => (
        <p key={paragraph.id}>{paragraph.text}</p>
      ))}
    </div>
  );
}
