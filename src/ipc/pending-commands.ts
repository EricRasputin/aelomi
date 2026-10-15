const pending = new Set<Promise<string[]>>();
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

export function trackCommand<T>(
  command: Promise<T>,
  failures: (reply: T) => string[] = () => [],
  rejected: string[] = [],
): Promise<T> {
  const settled = command.then(
    (reply) => {
      pending.delete(settled);
      return failures(reply);
    },
    () => {
      pending.delete(settled);
      return rejected;
    },
  );
  pending.add(settled);
  return command;
}

export async function drainCommands(): Promise<string[]> {
  const failed = new Set<string>();
  while (pending.size > 0) {
    for (const docs of await Promise.all([...pending])) {
      for (const doc of docs) failed.add(doc);
    }
  }
  return [...failed];
}
