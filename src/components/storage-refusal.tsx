import { useEffect, useRef } from "react";
import { type StorageStartup, storageCopy } from "../storage-copy";

function PathChip({ path, label }: { path: string; label: string }) {
  return (
    <textarea
      className="storage-refusal-path"
      dir="ltr"
      aria-label={label}
      readOnly
      rows={1}
      value={path}
      onPointerDown={(event) => event.preventDefault()}
      onClick={(event) => {
        event.preventDefault();
        event.currentTarget.focus();
        event.currentTarget.select();
      }}
      onFocus={(event) => event.currentTarget.select()}
    />
  );
}

export function StorageRefusal({
  startup,
}: {
  startup: Exclude<StorageStartup, { kind: "ready" }>;
}) {
  const title = useRef<HTMLHeadingElement>(null);
  useEffect(() => {
    if (startup.kind === "refused") title.current?.focus();
  }, [startup]);
  return (
    <main className="storage-refusal" aria-busy={startup.kind === "loading"}>
      <section
        className="storage-refusal-content"
        aria-labelledby="storage-refusal-title"
      >
        {startup.kind === "loading" ? (
          <p id="storage-refusal-title">{storageCopy.opening}</p>
        ) : (
          <>
            <h1 id="storage-refusal-title" ref={title} tabIndex={-1}>
              {startup.title}
            </h1>
            <p className="storage-refusal-message">{startup.message}</p>
            {startup.recovery !== null && (
              <>
                <div className="storage-refusal-location">
                  <p>{storageCopy.cloudDataPath}</p>
                  <PathChip
                    path={startup.recovery.resolved}
                    label={storageCopy.cloudDataPath}
                  />
                </div>
                <ol className="storage-refusal-recovery">
                  {startup.recovery.steps.map((step) => (
                    <li key={step.text}>
                      {step.kind === "text" ? (
                        step.text
                      ) : (
                        <>
                          <p>{step.text}</p>
                          <PathChip path={step.path} label={step.text} />
                          <p>{step.after}</p>
                        </>
                      )}
                    </li>
                  ))}
                </ol>
                <p className="storage-refusal-message">
                  {storageCopy.unchanged}
                </p>
              </>
            )}
            {startup.detail !== null && (
              <div className="storage-refusal-detail">
                <p>{storageCopy.details}</p>
                <p>{startup.detail}</p>
              </div>
            )}
          </>
        )}
      </section>
    </main>
  );
}
