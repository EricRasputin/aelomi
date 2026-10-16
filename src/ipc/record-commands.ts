import { storageRetry } from "../storage-retry";
import {
  forgetRecordOp,
  type OperationId,
  type PageId,
  retainRecordOp,
  useRecordCommandStore,
} from "../stores/record-commands";
import type { PageError } from "./bindings";
import {
  commandsAccepted,
  forgetCommand,
  retainCommand,
  trackCommand,
} from "./pending-commands";

export type RecordCommandResult<T> =
  | { status: "ok"; data: T }
  | { status: "error"; error: PageError | string }
  | { status: "retrying"; pageId: PageId; operationId: OperationId };
type Transport = typeof import("./bindings").commands;
type Commands = Omit<Transport, "pageCreate" | "pageRename" | "pageTrash"> & {
  pageCreate: (parentId: PageId | null) => Promise<RecordCommandResult<PageId>>;
  pageRename: (
    pageId: PageId,
    title: string,
  ) => Promise<RecordCommandResult<null>>;
  pageTrash: (pageId: PageId) => Promise<RecordCommandResult<string>>;
};
type Job = {
  id: OperationId;
  pageId: PageId;
  state: "queued" | "running" | "failed";
  run: () => Promise<boolean>;
} & (
  | { kind: "trash" }
  | {
      kind: "create";
      parentId: PageId | null;
      promise: Promise<RecordCommandResult<PageId>>;
    }
  | {
      kind: "rename";
      title: string;
      promise: Promise<RecordCommandResult<null>>;
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

export function withRecordCommands(transport: Transport): Commands {
  const queue: Job[] = [];
  let running = false;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let backoff = storageRetry.firstMs;
  const pump = () => {
    clearTimeout(timer);
    if (running) {
      if (queue[0]?.state === "failed") queueMicrotask(pump);
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
            timer = setTimeout(pump, backoff);
            backoff = Math.min(backoff * 2, storageRetry.maxMs);
            return;
          }
          queue.shift();
          backoff = storageRetry.firstMs;
          next = queue[0];
        }
        running = false;
      })(),
    );
  };
  const attempt = <T>(
    run: () => Promise<
      { status: "ok"; data: T } | { status: "error"; error: PageError }
    >,
  ) => {
    let resolve = (_reply: RecordCommandResult<T>) => {};
    const promise = new Promise<RecordCommandResult<T>>((done) => {
      resolve = done;
    });
    let attempts = 0;
    let accepted: { status: "ok"; data: T } | undefined;
    const execute = async (job: Job) => {
      attempts += 1;
      const prior = useRecordCommandStore.getState().retained.get(job.id);
      if (prior) retainRecordOp({ ...prior, attempts });
      job.state = "running";
      const retain = (error: PageError | string) => {
        job.state = "failed";
        retainCommand(job, { retry: pump, docs: ["workspace"] });
        retainRecordOp({
          id: job.id,
          op: job.kind,
          pageId: job.pageId,
          error,
          attempts,
          since: prior?.since ?? Date.now(),
        });
        resolve({
          status: "retrying",
          pageId: job.pageId,
          operationId: job.id,
        });
        return false;
      };
      const settle = (reply: RecordCommandResult<T>) => {
        forgetCommand(job);
        forgetRecordOp(job.id);
        resolve(reply);
        return true;
      };
      try {
        const reply = accepted ?? (await run());
        if (
          reply.status === "error" &&
          ((typeof reply.error === "object" && "Io" in reply.error) ||
            (prior && reply.error === "Quitting"))
        )
          return retain(reply.error);
        if (reply.status === "ok") {
          if (!accepted) publish();
          if (prior) {
            accepted = reply;
            const flushed = await transport.storageFlush();
            if (flushed.status === "error") {
              if (typeof flushed.error === "object")
                return retain(flushed.error);
              return settle({ status: "error", error: flushed.error });
            }
            if (!flushed.data) return retain(prior.error);
          }
        }
        return settle(reply);
      } catch (error: unknown) {
        return retain(error instanceof Error ? error.message : String(error));
      }
    };
    return { promise, execute };
  };
  const enqueue = (job: Job) => {
    queue.push(job);
    pump();
  };
  const quitting = { status: "error", error: "Quitting" } as const;
  return {
    ...transport,
    pageCreate: (parentId) => {
      if (!commandsAccepted()) return Promise.resolve(quitting);
      const retained = queue.find(
        (job): job is Extract<Job, { kind: "create" }> =>
          job.kind === "create" &&
          job.parentId === parentId &&
          useRecordCommandStore.getState().retained.has(job.id),
      );
      if (retained) {
        pump();
        return retained.promise;
      }
      const pageId = crypto.randomUUID().replaceAll("-", "");
      const { promise, execute } = attempt(() =>
        transport.pageCreate(parentId, pageId),
      );
      const job: Job = {
        kind: "create",
        id: crypto.randomUUID(),
        pageId,
        parentId,
        state: "queued",
        promise,
        run: () => execute(job),
      };
      enqueue(job);
      return promise;
    },
    pageRename: (pageId, title) => {
      if (!commandsAccepted()) return Promise.resolve(quitting);
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
        id: crypto.randomUUID(),
        pageId,
        title,
        promise,
        state: "queued",
        run: () => execute(job),
      };
      enqueue(job);
      return promise;
    },
    pageTrash: (pageId) => {
      if (!commandsAccepted()) return Promise.resolve(quitting);
      const { promise, execute } = attempt(() => transport.pageTrash(pageId));
      const job: Job = {
        kind: "trash",
        id: crypto.randomUUID(),
        pageId,
        state: "queued",
        run: () => execute(job),
      };
      enqueue(job);
      return promise;
    },
  };
}
export function onRecordCommand(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}
