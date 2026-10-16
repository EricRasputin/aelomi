import { storageRetry } from "../storage-retry";
import {
  type FailedRecordOp,
  type OperationId,
  type PageId,
  type RecordCommandError,
  type RecordOp,
  type RetainedRecordOp,
  useRecordCommandStore,
} from "../stores/record-commands";
import type { PageError } from "./bindings";
import { retryCommandError } from "./command-retry";
import {
  commandsAccepted,
  forgetCommand,
  retainCommand,
  setCommandAdmission,
  trackCommand,
} from "./pending-commands";

export type RecordCommandResult<T> =
  | { status: "ok"; data: T }
  | { status: "error"; error: RecordCommandError };
type Transport = typeof import("./bindings").commands;
type Commands = Omit<Transport, "pageCreate" | "pageRename" | "pageTrash"> & {
  pageCreate: (
    parentId: PageId | null,
    pageId?: PageId,
  ) => Promise<RecordCommandResult<PageId>>;
  pageRename: (
    pageId: PageId,
    title: string,
  ) => Promise<RecordCommandResult<null>>;
  pageTrash: (pageId: PageId) => Promise<RecordCommandResult<string>>;
};
type JobState<T> = {
  id: OperationId;
  pageId: PageId;
  op: RecordOp;
  attempts: number;
  since: number | null;
  error: RecordCommandError | null;
  accepted: { status: "ok"; data: T } | null;
  status: "queued" | "running" | "retained" | "settled";
  promise: Promise<RecordCommandResult<T>>;
  resolve: (reply: RecordCommandResult<T>) => void;
};
type Job = (
  | (JobState<PageId> & { op: "create"; parentId: PageId | null })
  | (JobState<null> & { op: "rename"; title: string })
  | (JobState<string> & { op: "trash" })
) & { run: () => Promise<boolean> };
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
  const failed = new Map<OperationId, FailedRecordOp>();
  let registered: Job | undefined;
  let running = false;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let backoff = storageRetry.firstMs;
  const sync = () => {
    const retained = new Map<OperationId, RetainedRecordOp>();
    const pendingCreates = new Set<PageId | null>();
    for (const job of queue) {
      if (job.op === "create") pendingCreates.add(job.parentId);
      if (job.since !== null && job.error !== null)
        retained.set(job.id, {
          id: job.id,
          op: job.op,
          pageId: job.pageId,
          error: job.error,
          attempts: job.attempts,
          since: job.since,
        });
    }
    if (registered) forgetCommand(registered);
    registered = queue[0];
    if (registered && registered.since !== null)
      retainCommand(registered, { retry: pump, docs: ["workspace"] });
    useRecordCommandStore.setState({
      retained,
      failed: new Map(failed),
      pendingCreates,
      dismissFailed: (id) => {
        failed.delete(id);
        sync();
      },
    });
  };
  const pump = () => {
    clearTimeout(timer);
    if (running) {
      if (queue[0]?.status === "retained") queueMicrotask(pump);
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
          backoff = storageRetry.firstMs;
          next = queue[0];
        }
        running = false;
      })(),
    );
  };
  const settle = (
    job: Pick<Job, "id" | "op" | "pageId" | "status" | "since">,
    reply: RecordCommandResult<unknown>,
  ) => {
    if (job.status === "settled") return;
    job.status = "settled";
    if (reply.status === "ok") {
      for (const [id, failure] of failed)
        if (failure.pageId === job.pageId) failed.delete(id);
    } else if (job.since !== null && reply.error !== "Abandoned") {
      failed.set(job.id, {
        id: job.id,
        op: job.op,
        pageId: job.pageId,
        error: reply.error,
        at: Date.now(),
      });
    }
    const index = queue.findIndex((item) => item.id === job.id);
    if (index !== -1) queue.splice(index, 1);
    sync();
  };
  const execute = async <T>(
    job: JobState<T>,
    invoke: () => Promise<
      { status: "ok"; data: T } | { status: "error"; error: PageError }
    >,
  ): Promise<boolean> => {
    job.attempts += 1;
    job.status = "running";
    sync();
    const retained = job.since !== null;
    const active = () => job.status !== "settled";
    const retain = (error: RecordCommandError) => {
      if (!active()) return true;
      job.status = "retained";
      job.since ??= Date.now();
      job.error = error;
      sync();
      return false;
    };
    try {
      const reply = job.accepted ?? (await invoke());
      if (!active()) return true;
      if (reply.status === "error" && retryCommandError(reply.error, retained))
        return retain(reply.error);
      if (reply.status === "ok" && job.accepted === null) {
        job.accepted = reply;
        sync();
        publish();
      }
      if (reply.status === "ok" && retained) {
        const flushed = await transport.storageFlush();
        if (!active()) return true;
        if (flushed.status === "error") {
          if (typeof flushed.error === "object") return retain(flushed.error);
          const failure = { status: "error", error: flushed.error } as const;
          settle(job, failure);
          job.resolve(failure);
          return true;
        }
        if (!flushed.data)
          return retain(
            job.error ?? { Io: "Durability confirmation timed out" },
          );
      }
      settle(job, reply);
      job.resolve(reply);
      return true;
    } catch (error: unknown) {
      return retain({
        Transport: error instanceof Error ? error.message : String(error),
      });
    }
  };
  const attempt = <T>(op: RecordOp, pageId: PageId): JobState<T> => {
    let resolve = (_reply: RecordCommandResult<T>) => {};
    const promise = new Promise<RecordCommandResult<T>>((done) => {
      resolve = done;
    });
    return {
      id: crypto.randomUUID(),
      pageId,
      op,
      attempts: 0,
      since: null,
      error: null,
      accepted: null,
      status: "queued",
      promise,
      resolve,
    };
  };
  const enqueue = (job: Job) => {
    queue.push(job);
    sync();
    pump();
  };
  const quitting = { status: "error", error: "Quitting" } as const;
  return {
    ...transport,
    quitAnyway: async () => {
      setCommandAdmission("completing");
      clearTimeout(timer);
      for (const job of [...queue]) {
        const reply = { status: "error", error: "Abandoned" } as const;
        settle(job, reply);
        job.resolve(reply);
      }
      await transport.quitAnyway();
    },
    pageCreate: (parentId, pageId = newPageId()) => {
      if (!commandsAccepted()) return Promise.resolve(quitting);
      const job: Extract<Job, { op: "create" }> = {
        ...attempt<PageId>("create", pageId),
        op: "create",
        parentId,
        run: () => execute(job, () => transport.pageCreate(parentId, pageId)),
      };
      enqueue(job);
      return job.promise;
    },
    pageRename: (pageId, title) => {
      if (!commandsAccepted()) return Promise.resolve(quitting);
      const last = queue.at(-1);
      if (
        last?.op === "rename" &&
        last.status === "queued" &&
        last.pageId === pageId
      ) {
        last.title = title;
        sync();
        return last.promise;
      }
      const job: Extract<Job, { op: "rename" }> = {
        ...attempt<null>("rename", pageId),
        op: "rename",
        title,
        run: () => execute(job, () => transport.pageRename(pageId, job.title)),
      };
      enqueue(job);
      return job.promise;
    },
    pageTrash: (pageId) => {
      if (!commandsAccepted()) return Promise.resolve(quitting);
      const job: Extract<Job, { op: "trash" }> = {
        ...attempt<string>("trash", pageId),
        op: "trash",
        run: () => execute(job, () => transport.pageTrash(pageId)),
      };
      enqueue(job);
      return job.promise;
    },
  };
}
export function onRecordCommand(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

export function newPageId(): PageId {
  return crypto.randomUUID().replaceAll("-", "");
}
