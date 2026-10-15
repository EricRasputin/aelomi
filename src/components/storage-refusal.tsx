import { type StorageStartup, storageCopy } from "../storage-copy";

export function StorageRefusal({
  startup,
}: {
  startup: Exclude<StorageStartup, { kind: "ready" }>;
}) {
  return (
    <main className="storage-refusal">
      <section
        className="storage-refusal-content"
        aria-labelledby="storage-refusal-title"
      >
        <p className="storage-refusal-app">Aelomi</p>
        {startup.kind === "loading" ? (
          <p>{storageCopy.opening}</p>
        ) : (
          <>
            <h1 id="storage-refusal-title">{startup.title}</h1>
            <p className="storage-refusal-message">{startup.message}</p>
            {startup.detail !== null && (
              <div className="storage-refusal-detail">
                {startup.recovery === "moveFolder" && (
                  <p>{storageCopy.resolvedPath}</p>
                )}
                <p className="storage-refusal-path" dir="auto">
                  {startup.detail}
                </p>
              </div>
            )}
            {startup.recovery === "moveFolder" && (
              <section
                className="storage-refusal-recovery"
                aria-labelledby="storage-recovery-title"
              >
                <h2 id="storage-recovery-title">{storageCopy.moveTitle}</h2>
                <ol>
                  {storageCopy.moveSteps.map((step) => (
                    <li key={step}>{step}</li>
                  ))}
                </ol>
                <p className="storage-refusal-path">{storageCopy.localPath}</p>
              </section>
            )}
          </>
        )}
      </section>
    </main>
  );
}
