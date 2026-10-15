import {
  commandsAccepted,
  forgetRecordCommand,
  retainRecordCommand,
  trackCommand,
} from "./pending-commands";

type Transport = typeof import("./bindings").commands;
type Create = (parentId: string | null) => ReturnType<Transport["pageCreate"]>;
type Commands = Omit<Transport, "pageCreate"> & { pageCreate: Create };
type RecordReply = Awaited<
  ReturnType<Commands["pageCreate" | "pageRename" | "pageTrash"]>
>;
type RenameReply = Awaited<ReturnType<Commands["pageRename"]>>;
type Job = {
  state: "queued" | "running" | "failed";
  run: () => Promise<boolean>;
} & (
  | { kind: "command" }
  | {
      kind: "rename";
      pageId: string;
      title: string;
      promise: Promise<RenameReply>;
    }
);

const listeners = new Set<() => void>();

function publish() {
  for (const listener of listeners) {
    try {
      listener();
    } catch (error: unknown) {
      console.error("Record command listener failed", error);
    }
  }
}

function refused(reply: RecordReply): boolean {
  return (
    reply.status === "error" &&
    typeof reply.error === "string" &&
    [
      "Quitting",
      "InvalidPageId",
      "NotActive",
      "MissingBatch",
      "Cycle",
      "OrderExhausted",
      "InvalidTime",
    ].includes(reply.error)
  );
}

export function withRecordCommands(transport: Transport): Commands {
  const queue: Job[] = [];
  let running = false;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let backoff = 100;
  const retry = () => {
    clearTimeout(timer);
    if (running) {
      if (queue[0]?.state === "failed") queueMicrotask(retry);
      return;
    }
    if (queue.length === 0) return;
    running = true;
    void trackCommand(
      (async () => {
        let next = queue[0];
        while (next) {
          if (!(await next.run())) {
            running = false;
            timer = setTimeout(retry, backoff);
            backoff = Math.min(backoff * 2, 5000);
            return;
          }
          queue.shift();
          backoff = 100;
          next = queue[0];
        }
        running = false;
      })(),
    );
  };
  const attempt = <T extends RecordReply>(run: () => Promise<T>) => {
    let resolve = (_reply: T) => {};
    let reject = (_error: unknown) => {};
    const promise = new Promise<T>((done, fail) => {
      resolve = done;
      reject = fail;
    });
    const execute = async (job: Job) => {
      const wasFailed = job.state === "failed";
      job.state = "running";
      try {
        const reply = await run();
        const complete =
          reply.status === "ok" || (!wasFailed && refused(reply));
        if (complete) {
          forgetRecordCommand(job);
          if (reply.status === "ok") publish();
        } else {
          job.state = "failed";
          retainRecordCommand(job, retry);
        }
        resolve(reply);
        return complete;
      } catch (error: unknown) {
        job.state = "failed";
        retainRecordCommand(job, retry);
        reject(error);
        return false;
      }
    };
    return { promise, execute };
  };
  const submit = <T extends RecordReply>(run: () => Promise<T>) => {
    const { promise, execute } = attempt(run);
    const job: Job = {
      kind: "command",
      state: "queued",
      run: () => execute(job),
    };
    queue.push(job);
    retry();
    return promise;
  };
  return {
    ...transport,
    pageCreate: (parentId) => {
      if (!commandsAccepted())
        return Promise.resolve({ status: "error", error: "Quitting" });
      const pageId = crypto.randomUUID().replaceAll("-", "");
      return submit(() => transport.pageCreate(parentId, pageId));
    },
    pageRename: (pageId, title) => {
      if (!commandsAccepted())
        return Promise.resolve({ status: "error", error: "Quitting" });
      const last = queue.at(-1);
      if (
        last?.kind === "rename" &&
        last.state === "queued" &&
        last.pageId === pageId
      ) {
        last.title = title;
        return last.promise;
      }
      const { promise, execute } = attempt(() =>
        transport.pageRename(pageId, job.title),
      );
      const job: Extract<Job, { kind: "rename" }> = {
        kind: "rename",
        state: "queued",
        pageId,
        title,
        promise,
        run: () => execute(job),
      };
      queue.push(job);
      retry();
      return promise;
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
