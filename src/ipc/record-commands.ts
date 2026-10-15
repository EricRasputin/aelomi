type Commands = typeof import("./bindings").commands;

const pending = new Set<Promise<boolean>>();
const listeners = new Set<() => void>();

function track<T extends { status: "ok" | "error" }>(
  command: Promise<T>,
): Promise<T> {
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
      return reply.status === "ok";
    },
    () => false,
  );
  pending.add(completion);
  void completion.then(() => pending.delete(completion));
  return command;
}

export function withRecordCommands(transport: Commands): Commands {
  return {
    ...transport,
    pageCreate: (parentId) => track(transport.pageCreate(parentId)),
    pageRename: (pageId, title) => track(transport.pageRename(pageId, title)),
    pageTrash: (pageId) => track(transport.pageTrash(pageId)),
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
