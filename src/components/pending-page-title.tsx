import { useLayoutEffect, useRef, useState } from "react";
import { pageTitleHistory } from "../editor/field-history";
import { onQuitDrain } from "../editor/quit-drain";
import { usePageTitlesStore } from "../stores/page-titles";
import { useQuitStore } from "../stores/quit";
import { useShellStore } from "../stores/shell";

export function PendingPageTitle() {
  const intent = useShellStore((state) => state.createIntent);
  const field = useRef<HTMLInputElement>(null);
  const [frozen, setFrozen] = useState(
    () => useQuitStore.getState().decision.kind !== "open",
  );
  const decision = useQuitStore((state) => state.decision.kind);
  useLayoutEffect(() => {
    if (decision === "open") setFrozen(false);
  }, [decision]);
  useLayoutEffect(() => {
    if (intent?.source !== "button") field.current?.focus();
  }, [intent?.source]);
  useLayoutEffect(
    () =>
      onQuitDrain({
        finish: () => field.current?.blur(),
        freeze: () => {
          if (field.current) field.current.readOnly = true;
          setFrozen(true);
        },
      }),
    [],
  );
  if (!intent) return null;
  return (
    <div className="pending-page-title">
      <label htmlFor="new-page-title">New page title</label>
      <input
        id="new-page-title"
        ref={field}
        placeholder="Untitled"
        value={intent.title}
        readOnly={frozen || (decision !== "open" && decision !== "draining")}
        onChange={(event) => {
          const title = event.target.value;
          useShellStore.setState((state) => ({
            createIntent: state.createIntent && {
              ...state.createIntent,
              title,
            },
          }));
          if (intent.stage.kind === "opening") {
            pageTitleHistory(intent.stage.pageId, "").record(title);
            void usePageTitlesStore
              .getState()
              .rename(intent.stage.pageId, title);
          }
        }}
      />
      <span role="status">
        {intent.stage.kind === "failed"
          ? "Title kept for another attempt."
          : "Creating page…"}
      </span>
    </div>
  );
}
