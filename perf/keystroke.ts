export type Stamps = {
  down: number;
  input: number;
  frame: number;
  raf: number;
  rafMc: number;
  doubleRaf: number;
  shown: boolean;
};

export type Split = {
  keyToInput: number;
  tickWait: number;
  frameToRaf: number;
  render: number;
  appOwned: number;
  full: number;
  doubleRaf: number;
};

export type Mode = "local" | "ci";

export function splitSample(stamps: Stamps): Split {
  const keyToInput = stamps.input - stamps.down;
  const tickWait = stamps.frame - stamps.input;
  const render = stamps.rafMc - stamps.frame;
  return {
    keyToInput,
    tickWait,
    frameToRaf: stamps.raf - stamps.frame,
    render,
    appOwned: keyToInput + render,
    full: stamps.rafMc - stamps.down,
    doubleRaf: stamps.doubleRaf - stamps.down,
  };
}

export function splitResidual(split: Split): number {
  return Math.abs(split.appOwned + split.tickWait - split.full);
}

export function keystrokeMode(env: NodeJS.ProcessEnv): Mode {
  return env.AELOMI_PERF_CI === "1" ? "ci" : "local";
}
