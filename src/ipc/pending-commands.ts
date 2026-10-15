const pending = new Set<Promise<void>>();
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
  while (pending.size > 0) await Promise.all([...pending]);
}
