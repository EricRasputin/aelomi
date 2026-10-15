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

/**
 * The report waits for both the entry route, which names the variant that
 * counts, and that variant's complete marks. Marks of a variant that finishes
 * first wait in `awaitingEntry`.
 */
type Report =
  | { phase: "awaitingEntry"; measured: Map<Variant, Marks> }
  | { phase: "awaitingMarks"; entry: Variant }
  | { phase: "sent" };

let report: Report = { phase: "awaitingEntry", measured: new Map() };
const started = new Set<Variant>();

function afterTask(): Promise<number> {
  return new Promise((done) => {
    const channel = new MessageChannel();
    channel.port1.onmessage = () => done(epochMs());
    channel.port2.postMessage(0);
  });
}

function send(marks: Marks) {
  report = { phase: "sent" };
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

function measured(marks: Marks) {
  switch (report.phase) {
    case "awaitingEntry":
      report.measured.set(marks.variant, marks);
      return;
    case "awaitingMarks":
      if (report.entry === marks.variant) send(marks);
      return;
    case "sent":
      return;
  }
}

function measure(variant: Variant) {
  if (window.__AELOMI_PERF__ !== true || started.has(variant)) return;
  started.add(variant);
  const commit = epochMs();
  requestAnimationFrame(() => {
    const raf1 = epochMs();
    const mcAfterRaf1 = afterTask();
    requestAnimationFrame(() => {
      const raf2 = epochMs();
      void Promise.all([mcAfterRaf1, afterTask()]).then(([first, second]) =>
        measured({
          variant,
          commit,
          raf1,
          mcAfterRaf1: first,
          raf2,
          mcAfterRaf2: second,
        }),
      );
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
  if (report.phase !== "awaitingEntry") return;
  const marks = report.measured.get(variant);
  if (marks) send(marks);
  else report = { phase: "awaitingMarks", entry: variant };
}
