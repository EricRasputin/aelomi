import { commandsAccepted, trackCommand } from "./pending-commands";

type Commands = typeof import("./bindings").commands;
type RecordReply = Awaited<
  ReturnType<Commands["pageCreate" | "pageRename" | "pageTrash"]>
>;

const listeners = new Set<() => void>();

function track<T extends RecordReply>(command: Promise<T>): Promise<T> {
  return trackCommand(
    command,
    (reply) => {
      if (reply.status === "ok") {
        for (const listener of listeners) {
          try {
            listener();
          } catch (error: unknown) {
            console.error("Record command listener failed", error);
          }
        }
      }
      const succeeded =
        reply.status === "ok" ||
        (typeof reply.error === "string" &&
          [
            "Quitting",
            "InvalidPageId",
            "NotActive",
            "MissingBatch",
            "Cycle",
            "OrderExhausted",
            "InvalidTime",
          ].includes(reply.error));
      return succeeded ? [] : ["workspace"];
    },
    ["workspace"],
  );
}

type RenameReply = Awaited<ReturnType<Commands["pageRename"]>>;
type Job =
  | { kind: "command"; run: () => Promise<void> }
  | {
      kind: "rename";
      pageId: string;
      title: string;
      promise: Promise<RenameReply>;
      run: () => Promise<void>;
    };

function queued<T extends RecordReply>(run: () => Promise<T>) {
  let execute = async () => {};
  const promise = new Promise<T>((resolve, reject) => {
    execute = async () => {
      try {
        resolve(await run());
      } catch (error: unknown) {
        reject(error);
      }
    };
  });
  return { promise: track(promise), run: execute };
}

export function withRecordCommands(transport: Commands): Commands {
  const queue: Job[] = [];
  let running = false;
  const enqueue = (job: Job) => {
    queue.push(job);
    if (running) return;
    running = true;
    void (async () => {
      let next = queue.shift();
      while (next) {
        await next.run();
        next = queue.shift();
      }
      running = false;
    })();
  };
  const submit = <T extends RecordReply>(run: () => Promise<T>) => {
    const job = queued(run);
    enqueue({ kind: "command", run: job.run });
    return job.promise;
  };
  return {
    ...transport,
    pageCreate: (parentId) =>
      commandsAccepted()
        ? submit(() => transport.pageCreate(parentId))
        : Promise.resolve({ status: "error", error: "Quitting" }),
    pageRename: (pageId, title) => {
      if (!commandsAccepted())
        return Promise.resolve({ status: "error", error: "Quitting" });
      const last = queue.at(-1);
      if (last?.kind === "rename" && last.pageId === pageId) {
        last.title = title;
        return last.promise;
      }
      const job: Extract<Job, { kind: "rename" }> = {
        kind: "rename",
        pageId,
        title,
        ...queued(() => transport.pageRename(pageId, job.title)),
      };
      enqueue(job);
      return job.promise;
    },
    pageTrash: (pageId) =>
      commandsAccepted()
        ? submit(() => transport.pageTrash(pageId))
        : Promise.resolve({ status: "error", error: "Quitting" }),
  };
}

export function onRecordCommand(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}
