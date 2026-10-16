const pending = new Set<Promise<void>>();
const retained = new Map<
  object,
  { retry: () => void; docs: readonly string[]; settings?: boolean }
>();
const admissionOpened = new Set<() => void>();
let admission: "open" | "completing" = "open";

export function setCommandAdmission(next: typeof admission) {
  if (admission === next) return;
  admission = next;
  if (next === "open") for (const retry of [...admissionOpened]) retry();
}

export function onCommandAdmissionOpen(retry: () => void): () => void {
  admissionOpened.add(retry);
  return () => {
    admissionOpened.delete(retry);
  };
}

export function commandsAccepted(): boolean {
  return admission === "open";
}

export function hasPendingCommands(): boolean {
  return pending.size > 0;
}

export function retainCommand(
  operation: object,
  entry: { retry: () => void; docs: readonly string[]; settings?: boolean },
) {
  retained.set(operation, entry);
}

export function forgetCommand(operation: object) {
  retained.delete(operation);
}

export function commandFailures(): { docs: string[]; settings: boolean } {
  const entries = [...retained.values()];
  return {
    docs: [...new Set(entries.flatMap(({ docs }) => docs))].sort(),
    settings: entries.some((entry) => entry.settings === true),
  };
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
