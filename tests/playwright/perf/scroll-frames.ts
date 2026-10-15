import { median, percentile95 } from "../../../perf/stats.ts";

/**
 * One scroll frame: its rAF timestamp, the interval since the previous
 * frame's timestamp, the time the MessageChannel task posted from its
 * callback ran, and whether the scroller moved.
 */
export type ScrollFrame = {
  stamp: number;
  interval: number;
  end: number;
  moved: boolean;
};

export type ScrollRun = { idle: number[]; frames: ScrollFrame[] };

/**
 * Runs in the page. Finds the editor's scrolling ancestor, idles 120 frames,
 * then scrolls it 48 px per frame, down then up, for 5 s. Each frame's
 * callback posts one message on a single channel; WebKit runs it after that
 * frame's rendering update. Resolves once every frame's message has landed.
 */
export function scrollFrames(editor: Element): Promise<ScrollRun> {
  return new Promise((done) => {
    let scroller: Element | null = editor;
    while (scroller && getComputedStyle(scroller).overflowY !== "auto")
      scroller = scroller.parentElement;
    if (!(scroller instanceof HTMLElement))
      throw new Error("the editor has no scrolling ancestor");
    const target = scroller;
    const idle: number[] = [];
    const frames: ScrollFrame[] = [];
    const waiting: ScrollFrame[] = [];
    const channel = new MessageChannel();
    let finished = false;
    channel.port1.onmessage = () => {
      const frame = waiting.shift();
      if (frame) frame.end = performance.now();
      if (finished && waiting.length === 0) done({ idle, frames });
    };
    let last = 0;
    let started = 0;
    let direction = 1;
    const scrollStep = (now: number) => {
      const bottom = target.scrollHeight - target.clientHeight;
      if (target.scrollTop >= bottom - 48) direction = -1;
      if (target.scrollTop <= 0) direction = 1;
      const before = target.scrollTop;
      target.scrollTop += 48 * direction;
      const frame = {
        stamp: now,
        interval: now - last,
        end: 0,
        moved: target.scrollTop !== before,
      };
      last = now;
      frames.push(frame);
      waiting.push(frame);
      channel.port2.postMessage(0);
      if (now - started < 5_000) requestAnimationFrame(scrollStep);
      else finished = true;
    };
    const idleStep = (now: number) => {
      if (last !== 0) idle.push(now - last);
      last = now;
      if (idle.length < 120) requestAnimationFrame(idleStep);
      else {
        started = now;
        requestAnimationFrame(scrollStep);
      }
    };
    requestAnimationFrame(idleStep);
  });
}

export function scrollSummary({ idle, frames }: ScrollRun) {
  const work = frames.map((frame) => frame.end - frame.stamp);
  const intervals = frames.map((frame) => frame.interval);
  return {
    frames: frames.length,
    moved: frames.filter((frame) => frame.moved).length,
    landed: frames.filter((frame) => frame.end > 0).length,
    workP50: median(work),
    workP95: percentile95(work),
    workMax: Math.max(...work),
    rawP50: median(intervals),
    rawP95: percentile95(intervals),
    rawMax: Math.max(...intervals),
    idleCadence: median(idle),
  };
}
