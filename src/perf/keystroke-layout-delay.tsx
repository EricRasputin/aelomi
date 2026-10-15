import { useLayoutEffect, useRef, useState } from "react";

const targetLayoutMs = 30;
const maxParagraphs = 64_000;

const filler = (count: number) =>
  Array.from(
    { length: count },
    (_, i) =>
      `Filler paragraph ${i} wraps across several lines so that a width change makes WebKit break every line again.`,
  );

/**
 * A diagnostic control, not product UI. Each input changes the width of a
 * large block of text without reading layout, so WebKit lays it out at the
 * start of its next rendering update, before it stamps the rAF time. A clock
 * that starts at that stamp misses the work; one that starts at the key
 * event does not. The block grows until one such layout takes 30 ms, so the
 * work stays well above Event Timing's 16 ms reporting threshold on any
 * machine, and the surface becomes editable only then.
 */
export default function KeystrokeLayoutDelay() {
  const [paragraphs, setParagraphs] = useState(2_000);
  const [calibrated, setCalibrated] = useState(false);
  const wide = useRef(false);
  const block = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    const element = block.current;
    if (!element || calibrated) return;
    element.style.width = "601px";
    const start = performance.now();
    void element.offsetHeight;
    const layoutMs = performance.now() - start;
    element.style.width = "600px";
    void element.offsetHeight;
    if (layoutMs >= targetLayoutMs || paragraphs >= maxParagraphs)
      setCalibrated(true);
    else setParagraphs(paragraphs * 2);
  }, [paragraphs, calibrated]);
  return (
    <div className="w-full max-w-2xl">
      <div
        data-testid="keystroke-surface"
        data-paragraphs={paragraphs}
        contentEditable={calibrated}
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
        {filler(paragraphs).map((text) => (
          <p key={text}>{text}</p>
        ))}
      </div>
    </div>
  );
}
