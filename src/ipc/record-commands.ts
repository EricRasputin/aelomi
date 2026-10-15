type Commands = typeof import("./bindings").commands;
type RecordReply = Awaited<
  ReturnType<Commands["pageCreate" | "pageRename" | "pageTrash"]>
>;

const pending = new Set<Promise<boolean>>();
const listeners = new Set<() => void>();

function track<T extends RecordReply>(command: Promise<T>): Promise<T> {
  const completion = command.then(
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
      return (
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
          ].includes(reply.error))
      );
    },
    () => false,
  );
  pending.add(completion);
  void completion.then(() => pending.delete(completion));
  return command;
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
    pageCreate: (parentId) => submit(() => transport.pageCreate(parentId)),
    pageRename: (pageId, title) => {
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
    pageTrash: (pageId) => submit(() => transport.pageTrash(pageId)),
  };
}

export function onRecordCommand(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

export async function drainRecordCommands(): Promise<string[]> {
  let succeeded = true;
  while (pending.size !== 0) {
    const results = await Promise.all([...pending]);
    succeeded = results.every(Boolean) && succeeded;
  }
  return succeeded ? [] : ["workspace"];
}
