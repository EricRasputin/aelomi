const pending = new Set<Promise<void>>();
const retained = new Map<object, () => void>();
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

export function retainRecordCommand(operation: object, retry: () => void) {
  retained.set(operation, retry);
}

export function forgetRecordCommand(operation: object) {
  retained.delete(operation);
}

export function commandFailures(): string[] {
  return retained.size > 0 ? ["workspace"] : [];
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

export async function drainCommands(): Promise<string[]> {
  for (const retry of [...retained.values()]) retry();
  while (pending.size > 0) {
    await Promise.all([...pending]);
  }
  return commandFailures();
}
