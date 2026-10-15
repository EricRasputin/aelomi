import { loadavg } from "node:os";
import { readBudgets } from "../../../perf/budgets.ts";
import {
  keystrokeMode,
  type Split,
  type Stamps,
  splitResidual,
  splitSample,
} from "../../../perf/keystroke.ts";
import { machineNoise, withNoise } from "../../../perf/noise.ts";
import { median, percentile95, range } from "../../../perf/stats.ts";
import { expect, test } from "../fixtures";

// The tracer snapshots the whole DOM right before each key, which warms the
// page and would be measured as the app's speed.
test.use({ trace: "off" });
const budgets = readBudgets().keystroke;
const mode = keystrokeMode(process.env);
const gates = {
  local: { metric: "full", budget: budgets.p95Ms.value },
  ci: { metric: "appOwned", budget: budgets.hostedAppOwnedP95Ms.value },
} as const satisfies Record<string, { metric: keyof Split; budget: number }>;
const gate = gates[mode];

const surfaces = [
  { name: "editor-empty", testId: "page-editor", gated: true, control: false },
  {
    name: "editor-page-5000-blocks",
    testId: "page-editor",
    gated: true,
    control: false,
  },
  { name: "empty", testId: "keystroke-surface", gated: false, control: false },
  {
    name: "page-5000-blocks",
    testId: "keystroke-surface",
    gated: false,
    control: false,
  },
  {
    name: "layout-delay",
    testId: "keystroke-surface",
    gated: false,
    control: true,
  },
] as const;

/**
 * WebKit's Event Timing entry for a key event: from the event's timestamp to
 * the paint after its handlers ran, rounded to 8 ms, reported only at 16 ms or
 * more. It is the engine's clock, independent of the recorder's rAF stamps.
 */
type EngineEvent = { name: string; startTime: number; duration: number };

type Recorder = {
  samples: Stamps[];
  missing: number;
  engine: EngineEvent[];
};

declare global {
  interface Window {
    __aelomiKeystroke?: Recorder;
  }
}

function installRecorder() {
  const recorder: Recorder = { samples: [], missing: 0, engine: [] };
  window.__aelomiKeystroke = recorder;
  // The DOM typings predate Event Timing's durationThreshold, which WebKit honors.
  const events: PerformanceObserverInit & { durationThreshold: number } = {
    type: "event",
    durationThreshold: 16,
    buffered: true,
  };
  new PerformanceObserver((list) => {
    for (const entry of list.getEntries())
      recorder.engine.push({
        name: entry.name,
        startTime: entry.startTime,
        duration: entry.duration,
      });
  }).observe(events);
  let down = 0;
  let key = "";
  addEventListener(
    "keydown",
    (event) => {
      down = event.timeStamp;
      key = event.key;
    },
    true,
  );
  addEventListener("input", (event) => {
    const stamps: Stamps = {
      down,
      input: performance.now(),
      frame: 0,
      raf: 0,
      rafMc: 0,
      doubleRaf: 0,
      shown: false,
    };
    const target = event.target instanceof HTMLElement ? event.target : null;
    requestAnimationFrame((frame) => {
      stamps.frame = frame;
      stamps.raf = performance.now();
      const caret = getSelection();
      const node = caret?.anchorNode ?? null;
      const offset = caret?.anchorOffset ?? 0;
      const beforeCaret =
        node === null
          ? ""
          : node.nodeType === Node.TEXT_NODE
            ? (node.textContent ?? "").slice(0, offset)
            : (node.childNodes[offset - 1]?.textContent ?? "");
      stamps.shown =
        target !== null &&
        node !== null &&
        target.contains(node) &&
        beforeCaret.endsWith(key);
      if (!stamps.shown) recorder.missing += 1;
      let landed = 0;
      const finish = () => {
        landed += 1;
        if (landed === 2) recorder.samples.push(stamps);
      };
      const channel = new MessageChannel();
      channel.port1.onmessage = () => {
        stamps.rafMc = performance.now();
        finish();
      };
      channel.port2.postMessage(0);
      requestAnimationFrame(() => {
        stamps.doubleRaf = performance.now();
        finish();
      });
    });
  });
}

const p50p95 = (values: number[]) =>
  `p50 ${median(values)} ms p95 ${percentile95(values)} ms`;

for (const { name: surface, testId, gated, control } of surfaces) {
  test(`EDIT-14 keystroke-to-paint on the ${surface} surface`, async ({
    page,
  }, testInfo) => {
    const startNoise = machineNoise(mode);
    if (startNoise) console.log(`${surface}: ${startNoise}`);
    await page.addInitScript(installRecorder);
    const loads: { p95: number; kept: Split[]; engine: (number | null)[] }[] =
      [];
    const cadences: number[] = [];
    for (let load = 1; load <= budgets.pageLoads.value; load++) {
      const setupStart = Date.now();
      await page.goto(`/perf/keystroke/${surface}`);
      const surfaceEl = page.getByTestId(testId);
      await expect(surfaceEl).toBeVisible({ timeout: 60_000 });
      await expect(surfaceEl).toHaveAttribute("contenteditable", "true", {
        timeout: 60_000,
      });
      console.log(
        `${surface}: load ${load} editable after ${Date.now() - setupStart} ms`,
      );
      const before = await surfaceEl.evaluate((el) => {
        el.focus();
        const texts = el.querySelectorAll(".bn-inline-content");
        const caret = document.createRange();
        caret.selectNodeContents(texts[texts.length - 1] ?? el);
        caret.collapse(false);
        const selection = getSelection();
        selection?.removeAllRanges();
        selection?.addRange(caret);
        (el.lastElementChild ?? el).scrollIntoView({ block: "end" });
        return el.textContent?.length ?? 0;
      });
      await page.waitForTimeout(2_000);
      const cadence = await page.evaluate(
        () =>
          new Promise<number[]>((done) => {
            const stamps: number[] = [];
            const tick = (now: number) => {
              stamps.push(now);
              if (stamps.length <= 60) requestAnimationFrame(tick);
              else done(stamps.slice(1).map((t, i) => t - (stamps[i] ?? t)));
            };
            requestAnimationFrame(tick);
          }),
      );
      cadences.push(median(cadence));
      await page.waitForTimeout(500);
      for (let i = 0; i < budgets.samplesPerLoad.value; i++) {
        await page.keyboard.press("x");
        await page.waitForFunction(
          (count) => (window.__aelomiKeystroke?.samples.length ?? 0) > count,
          i,
          { polling: 50 },
        );
        await page.waitForTimeout(budgets.gapMs.value);
      }
      const after = await surfaceEl.evaluate((el) => ({
        recorder: window.__aelomiKeystroke,
        length: el.textContent?.length ?? 0,
      }));
      const samples = after.recorder?.samples ?? [];
      expect(
        samples.length,
        `load ${load}: did-it-happen guard, one sample per key`,
      ).toBe(budgets.samplesPerLoad.value);
      expect(
        after.length - before,
        `load ${load}: did-it-happen guard, the surface grew by one character per key`,
      ).toBe(budgets.samplesPerLoad.value);
      expect(
        after.recorder?.missing,
        `load ${load}: typed character missing from the DOM at the first frame`,
      ).toBe(0);
      const keptStamps = samples.slice(budgets.discardedPerLoad.value);
      const kept = keptStamps.map(splitSample);
      const engineEvents = after.recorder?.engine ?? [];
      const engine = keptStamps.map((stamps) => {
        const durations = engineEvents
          .filter(
            (event) =>
              event.startTime >= stamps.down && event.startTime <= stamps.input,
          )
          .map((event) => event.duration);
        return durations.length === 0 ? null : Math.max(...durations);
      });
      loads.push({
        p95: percentile95(kept.map((s) => s[gate.metric])),
        kept,
        engine,
      });
    }
    const kept = loads.flatMap((load) => load.kept);
    const engine = loads.flatMap((load) => load.engine);
    const engineReported = engine.filter((d): d is number => d !== null);
    const column = (metric: keyof Split) => kept.map((s) => s[metric]);
    const fullP95 = percentile95(column("full"));
    const appOwnedP95 = percentile95(column("appOwned"));
    const residual = Math.max(...kept.map(splitResidual));
    const unordered = kept.filter(
      (s) => s.tickWait < 0 || s.frameToRaf < 0,
    ).length;
    const verdict = (value: number, budget: number) =>
      `${value} ms ${value <= budget ? "<=" : ">"} ${budget} ms ${value <= budget ? "PASS" : "FAIL"}`;
    const role = (metric: keyof Split) =>
      gated && metric === gate.metric ? "gate" : "information";
    const endNoise = machineNoise(mode);
    const noise = startNoise ?? endNoise;
    const summary = [
      withNoise(
        `${surface}: full key-to-paint p95 ${verdict(fullP95, budgets.p95Ms.value)} (local budget, ${role("full")})`,
        noise,
      ),
      `app-owned p95 ${verdict(appOwnedP95, budgets.hostedAppOwnedP95Ms.value)} (hosted budget, ${role("appOwned")})`,
      `tick wait ${p50p95(column("tickWait"))}, key-to-input ${p50p95(column("keyToInput"))}, render ${p50p95(column("render"))}, frame start to the recorder's rAF ${p50p95(column("frameToRaf"))}`,
      `full key-to-paint p50 ${median(column("full"))} ms, range ${range(column("full"))} ms, per-load ${gate.metric} p95 ${loads.map((load) => load.p95).join(", ")}, double rAF p95 ${percentile95(column("doubleRaf"))} ms`,
      `split check: max |app-owned + tick wait - full| ${residual.toFixed(3)} ms over ${kept.length} samples from ${loads.length} loads`,
      `engine Event Timing (key event to next paint, 8 ms rounding, reported at 16 ms or more): ${engineReported.length} of ${engine.length} keys reported${engineReported.length > 0 ? `, max ${Math.max(...engineReported)} ms, median of reported ${median(engineReported)} ms` : ""}`,
      `idle rAF cadence per load ${cadences.map((c) => c.toFixed(1)).join(", ")} ms, load average ${loadavg()
        .map((x) => x.toFixed(2))
        .join(" ")}`,
      ...(endNoise ? [`${surface}: ${endNoise}`] : []),
    ].join("\n  ");
    console.log(summary);
    testInfo.annotations.push({ type: "keystroke", description: summary });
    await testInfo.attach(`keystroke-${surface}.json`, {
      body: JSON.stringify(
        {
          surface,
          mode,
          gated,
          gate,
          fullP95,
          appOwnedP95,
          residual,
          engineReported: engineReported.length,
          loads,
        },
        null,
        2,
      ),
      contentType: "application/json",
    });
    expect(
      residual,
      `${surface}: split check, app-owned plus tick wait must equal full key-to-paint`,
    ).toBeLessThanOrEqual(1);
    expect(
      unordered,
      `${surface}: the frame timestamp must sit between the input handler's end and the recorder's rAF callback`,
    ).toBe(0);
    if (control) {
      expect(
        engineReported.length,
        `${surface}: the engine clock reports the layout that runs before the rAF stamp`,
      ).toBeGreaterThanOrEqual(kept.length / 2);
      expect(
        median(column("appOwned")),
        `${surface}: the app-owned split leaves that layout out`,
      ).toBeLessThan(median(column("tickWait")));
    }
    if (gated)
      expect(
        percentile95(column(gate.metric)),
        withNoise(
          `EDIT-14 ${surface} ${gate.metric} p95 (${mode} mode)`,
          noise,
        ),
      ).toBeLessThanOrEqual(gate.budget);
  });
}
