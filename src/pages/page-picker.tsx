import { createPortal } from "react-dom";
import { usePickerStore } from "../stores/picker";
import { HighlightedText } from "./highlighted-text";

const width = 320;
const maxHeight = 280;

export function PagePicker() {
  const open = usePickerStore((state) => state.open);
  const fresh = usePickerStore((state) => state.fresh);
  const anchor = usePickerStore((state) => state.anchor);
  const options = usePickerStore((state) => state.options);
  const selected = usePickerStore((state) => state.selected);
  const choose = usePickerStore((state) => state.choose);
  if (!open || !anchor) return null;
  const below = anchor.bottom + 6 + maxHeight < window.innerHeight;
  const style = {
    left: Math.max(8, Math.min(anchor.left, window.innerWidth - width - 8)),
    ...(below
      ? { top: anchor.bottom + 6 }
      : { bottom: window.innerHeight - anchor.top + 6 }),
  };
  return createPortal(
    <div className="picker" style={style} data-testid="page-picker">
      {fresh && options.length === 0 ? (
        <p className="picker-empty">No matching pages</p>
      ) : (
        <div role="listbox" aria-label="Link to page" className="picker-list">
          {options.map((option, index) => (
            <div
              key={option.id}
              id={`picker-${option.id}`}
              role="option"
              aria-selected={index === selected}
              tabIndex={-1}
              data-page-id={option.id}
              className="picker-option"
              onMouseDown={(event) => event.preventDefault()}
              onClick={() => choose(index)}
              onKeyDown={() => {}}
              ref={(node) => {
                if (index === selected)
                  node?.scrollIntoView({ block: "nearest" });
              }}
            >
              <span className="picker-title">
                <HighlightedText parts={option.parts} />
              </span>
              {option.detail && (
                <span className="picker-detail">{option.detail}</span>
              )}
            </div>
          ))}
        </div>
      )}
    </div>,
    document.body,
  );
}
