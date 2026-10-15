import { useRef } from "react";

const filler = Array.from(
  { length: 2_000 },
  (_, i) =>
    `Filler paragraph ${i} wraps across several lines so that a width change makes WebKit break every line again.`,
);

/**
 * A diagnostic control, not product UI. Each input changes the width of a
 * large block of text without reading layout, so WebKit lays it out at the
 * start of its next rendering update, before it stamps the rAF time. A clock
 * that starts at that stamp misses the work; one that starts at the key
 * event does not.
 */
export default function KeystrokeLayoutDelay() {
  const wide = useRef(false);
  const block = useRef<HTMLDivElement>(null);
  return (
    <div className="w-full max-w-2xl">
      <div
        data-testid="keystroke-surface"
        contentEditable
        suppressContentEditableWarning
        spellCheck={false}
        className="min-h-10 whitespace-pre-wrap outline-none"
        onInput={() => {
          wide.current = !wide.current;
          if (block.current)
            block.current.style.width = wide.current ? "601px" : "600px";
        }}
      />
      <div ref={block} style={{ width: "600px" }}>
        {filler.map((text) => (
          <p key={text}>{text}</p>
        ))}
      </div>
    </div>
  );
}
