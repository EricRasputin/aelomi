import type { commands as generatedCommands } from "./bindings";

type Commands = typeof generatedCommands;
type MockCall = {
  [Name in keyof Commands]: {
    command: Name;
    args: Parameters<Commands[Name]>;
  };
}[keyof Commands];

const calls: MockCall[] = [];

export const commands = {
  ping: async (request) => {
    calls.push({ command: "ping", args: [structuredClone(request)] });
    return { seq: request.seq, core_version: "0.1.0-mock" };
  },
  perfReport: async (marks) => {
    calls.push({ command: "perfReport", args: [structuredClone(marks)] });
  },
} satisfies Commands;

const control = {
  get calls(): readonly MockCall[] {
    return calls;
  },
  reset() {
    calls.length = 0;
  },
};

declare global {
  interface Window {
    __aelomiMock?: typeof control;
  }
}

export function installMock(): Commands {
  window.__aelomiMock = control;
  return commands;
}
