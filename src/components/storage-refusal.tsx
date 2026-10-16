import { AlertDialog } from "@base-ui/react/alert-dialog";
import { useEffect, useRef, useState } from "react";
import { commands } from "../ipc";
import {
  type StorageStartup,
  storageCopy,
  storageErrorText,
} from "../storage-copy";
import { useIndexStatusStore } from "../stores/index-status";
import { usePageTreeStore } from "../stores/page-tree";
import { useStorageStartupStore } from "../stores/storage-startup";

function PathChip({ path, label }: { path: string; label: string }) {
  return (
    <span className="storage-refusal-chip" data-path={path}>
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
    </span>
  );
}

export function StorageRefusal({
  startup,
}: {
  startup: Exclude<StorageStartup, { kind: "ready" }>;
}) {
  const title = useRef<HTMLHeadingElement>(null);
  useEffect(() => {
    if (startup.kind !== "loading") title.current?.focus();
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
                  <p>
                    {startup.kind === "finish-moving"
                      ? storageCopy.lastSeen
                      : storageCopy.cloudDataPath}
                  </p>
                  <PathChip
                    path={startup.recovery.resolved}
                    label={
                      startup.kind === "finish-moving"
                        ? storageCopy.lastSeen
                        : storageCopy.cloudDataPath
                    }
                  />
                </div>
                <p>{storageCopy.recoveryKeepOpen}</p>
                <ol className="storage-refusal-recovery">
                  {startup.recovery.steps.map((step, index) => (
                    <li key={index}>
                      {step.kind === "text" ? (
                        step.text
                      ) : step.kind === "show" ? (
                        <>
                          <p>{step.text}</p>
                          <ShowRecoveryFolder />
                        </>
                      ) : (
                        <>
                          <p>{step.text}</p>
                          <PathChip path={step.path} label={step.text} />
                          {step.after
                            .split("\n\n")
                            .map((paragraph, paragraphIndex) => (
                              <p
                                className="storage-recovery-paragraph"
                                key={paragraphIndex}
                              >
                                {paragraph}
                              </p>
                            ))}
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
            {startup.kind === "finish-moving" && (
              <NewWorkspaceConfirmation configured={startup.configured} />
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

function ShowRecoveryFolder() {
  const [state, setState] = useState<"idle" | "opening" | "error">("idle");
  async function showFolder() {
    setState("opening");
    try {
      const reply = await commands.showRecoveryFolder();
      setState(reply.status === "ok" ? "idle" : "error");
    } catch {
      setState("error");
    }
  }
  return (
    <div className="mt-2">
      <button
        className="quit-button"
        type="button"
        disabled={state === "opening"}
        onClick={() => void showFolder()}
      >
        {storageCopy.showFolder}
      </button>
      {state === "error" && (
        <p role="alert" className="mt-2 text-[var(--warning-text)]">
          {storageCopy.showFolderError}
        </p>
      )}
    </div>
  );
}

type EmptyChoice =
  | { kind: "closed" }
  | { kind: "confirming" }
  | { kind: "creating" }
  | { kind: "error"; message: string };

function NewWorkspaceConfirmation({ configured }: { configured: string }) {
  const [choice, setChoice] = useState<EmptyChoice>({ kind: "closed" });
  const keepMoving = useRef<HTMLButtonElement>(null);
  const creating = choice.kind === "creating";
  async function createWorkspace() {
    setChoice({ kind: "creating" });
    try {
      const reply = await commands.startEmptyWorkspace();
      if (reply.status === "error") {
        setChoice({ kind: "error", message: storageErrorText(reply.error) });
        return;
      }
      if (reply.data === "Ready") {
        const [, status] = await Promise.all([
          usePageTreeStore.getState().refresh(),
          commands.indexStatus(),
        ]);
        useIndexStatusStore.getState().receiveStatus(status);
      }
      useStorageStartupStore.getState().receive(reply.data);
      setChoice({ kind: "closed" });
    } catch (error: unknown) {
      setChoice({
        kind: "error",
        message: error instanceof Error ? error.message : String(error),
      });
    }
  }
  return (
    <AlertDialog.Root
      open={choice.kind !== "closed"}
      onOpenChange={(open, details) => {
        if (creating) details.cancel();
        else setChoice({ kind: open ? "confirming" : "closed" });
      }}
    >
      <AlertDialog.Trigger className="quit-button storage-start-empty">
        {storageCopy.startEmpty}
      </AlertDialog.Trigger>
      <AlertDialog.Portal>
        <AlertDialog.Backdrop className="quit-overlay" />
        <AlertDialog.Popup
          className="quit-dialog storage-empty-dialog"
          initialFocus={keepMoving}
          aria-busy={creating}
        >
          <AlertDialog.Title className="text-base font-semibold">
            {storageCopy.startEmptyTitle}
          </AlertDialog.Title>
          <div className="storage-empty-body">
            <AlertDialog.Description className="mt-3 text-sm leading-6 text-[var(--muted)]">
              {storageCopy.startEmptyWarning}
            </AlertDialog.Description>
            <PathChip path={configured} label="New workspace location" />
            {choice.kind === "error" && (
              <p
                role="alert"
                className="mt-3 text-sm text-[var(--warning-text)]"
              >
                {choice.message}
              </p>
            )}
          </div>
          <div className="storage-empty-actions mt-6 flex flex-wrap justify-end gap-2">
            <AlertDialog.Close
              ref={keepMoving}
              disabled={creating}
              className="quit-button"
            >
              {storageCopy.cancelEmpty}
            </AlertDialog.Close>
            <button
              type="button"
              disabled={creating}
              className="quit-button"
              onClick={() => void createWorkspace()}
            >
              {storageCopy.confirmEmpty}
            </button>
          </div>
        </AlertDialog.Popup>
      </AlertDialog.Portal>
    </AlertDialog.Root>
  );
}
