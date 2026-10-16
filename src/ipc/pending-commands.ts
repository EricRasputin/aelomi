const pending = new Set<Promise<void>>();
const retained = new Map<
  object,
  { retry: () => void; docs: readonly string[] }
>();
let admission: "open" | "completing" = "open";

export function setCommandAdmission(next: typeof admission) {
  admission = next;
}

export function commandsAccepted(): boolean {
  return admission === "open";
}

export function hasPendingCommands(): boolean {
  return pending.size > 0;
}

export function retainCommand(
  operation: object,
  entry: { retry: () => void; docs: readonly string[] },
) {
  retained.set(operation, entry);
}

export function forgetCommand(operation: object) {
  retained.delete(operation);
}

export function commandFailures(): string[] {
  return [
    ...new Set([...retained.values()].flatMap(({ docs }) => docs)),
  ].sort();
}

export function trackCommand<T>(command: Promise<T>): Promise<T> {
  const settled = command.then(
    () => {
      pending.delete(settled);
    },
    () => {
      pending.delete(settled);
    },
  );
  pending.add(settled);
  return command;
}

export async function drainCommands(): Promise<void> {
  for (const { retry } of [...retained.values()]) retry();
  while (pending.size > 0) {
    await Promise.all([...pending]);
  }
}
