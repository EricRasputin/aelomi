const pending = new Set<Promise<void>>();

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
