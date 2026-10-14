import { commands } from "../ipc";

declare global {
  interface Window {
    __AELOMI_PERF__?: true;
  }
}

const epochMs = () => performance.timeOrigin + performance.now();
const moduleEval = epochMs();

type Variant = "index" | "page";
type Marks = {
  variant: Variant;
  commit: number;
  raf1: number;
  mcAfterRaf1: number;
  raf2: number;
  mcAfterRaf2: number;
};

let entry: Variant | null = null;
let sent = false;
const measured: Partial<Record<Variant, Marks>> = {};

function afterTask(fn: () => void) {
  const channel = new MessageChannel();
  channel.port1.onmessage = () => fn();
  channel.port2.postMessage(0);
}

function send() {
  if (sent || entry === null) return;
  const marks = measured[entry];
  if (!marks || marks.mcAfterRaf1 === 0 || marks.mcAfterRaf2 === 0) return;
  sent = true;
  const fcp = performance.getEntriesByName("first-contentful-paint")[0];
  void commands.perfReport({
    variant: marks.variant,
    time_origin: performance.timeOrigin,
    module_eval: moduleEval,
    commit: marks.commit,
    raf1: marks.raf1,
    mc_after_raf1: marks.mcAfterRaf1,
    raf2: marks.raf2,
    mc_after_raf2: marks.mcAfterRaf2,
    fcp: fcp ? performance.timeOrigin + fcp.startTime : null,
  });
}

function measure(variant: Variant) {
  if (window.__AELOMI_PERF__ !== true || measured[variant]) return;
  const marks: Marks = {
    variant,
    commit: epochMs(),
    raf1: 0,
    mcAfterRaf1: 0,
    raf2: 0,
    mcAfterRaf2: 0,
  };
  measured[variant] = marks;
  requestAnimationFrame(() => {
    marks.raf1 = epochMs();
    afterTask(() => {
      marks.mcAfterRaf1 = epochMs();
      send();
    });
    requestAnimationFrame(() => {
      marks.raf2 = epochMs();
      afterTask(() => {
        marks.mcAfterRaf2 = epochMs();
        send();
      });
    });
  });
}

export function markIndexCommit() {
  measure("index");
}

/** The first commit that shows an editable page's content. */
export function markPageContent() {
  measure("page");
}

export function resolveEntry(variant: Variant) {
  if (entry !== null) return;
  entry = variant;
  send();
}
