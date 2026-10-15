import { useEffect, useRef } from "react";
import { type StorageStartup, storageCopy } from "../storage-copy";

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
            {startup.recovery &&
              startup.recovery.configured !== startup.recovery.resolved && (
                <div className="storage-refusal-detail">
                  <p>{storageCopy.configuredPath}</p>
                  <p className="storage-refusal-path" dir="ltr">
                    {startup.recovery.configured}
                  </p>
                </div>
              )}
            {startup.recovery !== null && (
              <div className="storage-refusal-detail">
                <p>{storageCopy.cloudDataPath}</p>
                <p className="storage-refusal-path" dir="ltr">
                  {startup.recovery.resolved}
                </p>
              </div>
            )}
            {startup.detail !== null && (
              <div className="storage-refusal-detail">
                <p>{storageCopy.details}</p>
                <p>{startup.detail}</p>
              </div>
            )}
            {startup.recovery !== null && (
              <section
                className="storage-refusal-recovery"
                aria-labelledby="storage-recovery-title"
              >
                <h2 id="storage-recovery-title">{storageCopy.moveTitle}</h2>
                <p>{storageCopy.finder}</p>
                <p>{storageCopy.occupiedDestination}</p>
                <ol>
                  {startup.recovery.steps.map((step) => (
                    <li key={step}>{step}</li>
                  ))}
                </ol>
              </section>
            )}
          </>
        )}
      </section>
    </main>
  );
}
