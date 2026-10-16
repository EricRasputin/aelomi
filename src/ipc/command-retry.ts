import type { PageError, SettingsError, StorageError } from "./bindings";

export type TransportFailure = { Transport: string };
type CommandError =
  | PageError
  | SettingsError
  | StorageError
  | TransportFailure
  | "Abandoned";

export function retryCommandError(
  error: CommandError,
  retained: boolean,
): boolean {
  return (
    (typeof error === "object" && ("Io" in error || "Transport" in error)) ||
    (retained && error === "Quitting")
  );
}
