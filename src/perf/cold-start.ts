import { commands } from "../ipc";

declare global {
  interface Window {
    __AELOMI_PERF__?: true;
  }
}

const epochMs = () => performance.timeOrigin + performance.now();
const moduleEval = epochMs();
let armed = false;

function afterTask(fn: () => void) {
  const channel = new MessageChannel();
  channel.port1.onmessage = () => fn();
  channel.port2.postMessage(0);
}

export function markIndexCommit() {
  if (armed || window.__AELOMI_PERF__ !== true) return;
  armed = true;
  const commit = epochMs();
  const marks = { raf1: 0, mcAfterRaf1: 0, raf2: 0, mcAfterRaf2: 0 };
  const sendWhenComplete = () => {
    if (marks.mcAfterRaf1 === 0 || marks.mcAfterRaf2 === 0) return;
    const fcp = performance.getEntriesByName("first-contentful-paint")[0];
    void commands.perfReport({
      variant: "index",
      time_origin: performance.timeOrigin,
      module_eval: moduleEval,
      commit,
      raf1: marks.raf1,
      mc_after_raf1: marks.mcAfterRaf1,
      raf2: marks.raf2,
      mc_after_raf2: marks.mcAfterRaf2,
      fcp: fcp ? performance.timeOrigin + fcp.startTime : null,
    });
  };
  requestAnimationFrame(() => {
    marks.raf1 = epochMs();
    afterTask(() => {
      marks.mcAfterRaf1 = epochMs();
      sendWhenComplete();
    });
    requestAnimationFrame(() => {
      marks.raf2 = epochMs();
      afterTask(() => {
        marks.mcAfterRaf2 = epochMs();
        sendWhenComplete();
      });
    });
  });
}
