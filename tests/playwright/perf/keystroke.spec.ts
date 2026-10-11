import { loadavg } from "node:os";
import { keystrokeSurfaces, readBudgets } from "../../../perf/budgets.ts";
import {
  keystrokeMode,
  type Split,
  type Stamps,
  splitResidual,
  splitSample,
} from "../../../perf/keystroke.ts";
import { median, percentile95, range } from "../../../perf/stats.ts";
import { expect, test } from "../fixtures";

const budgets = readBudgets().keystroke;
const mode = keystrokeMode(process.env);
const gates = {
  local: { metric: "full", budget: budgets.p95Ms.value },
  ci: { metric: "appOwned", budget: budgets.hostedAppOwnedP95Ms.value },
} as const satisfies Record<string, { metric: keyof Split; budget: number }>;
const gate = gates[mode];

type Recorder = { samples: Stamps[]; missing: number };

declare global {
  interface Window {
    __aelomiKeystroke?: Recorder;
  }
}

function installRecorder() {
  const recorder: Recorder = { samples: [], missing: 0 };
  window.__aelomiKeystroke = recorder;
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
      stamps.shown = (target?.lastChild?.textContent ?? "").endsWith(key);
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

for (const surface of keystrokeSurfaces) {
  const gated = surface === budgets.gate.value;
  test(`EDIT-14 keystroke-to-paint on the ${surface} surface`, async ({
    page,
  }, testInfo) => {
    await page.addInitScript(installRecorder);
    const loads: { p95: number; kept: Split[] }[] = [];
    const cadences: number[] = [];
    for (let load = 1; load <= budgets.pageLoads.value; load++) {
      await page.goto(`/perf/keystroke/${surface}`);
      const surfaceEl = page.getByTestId("keystroke-surface");
      await expect(surfaceEl).toBeVisible();
      const before = await surfaceEl.evaluate((el) => {
        el.focus();
        const caret = document.createRange();
        caret.selectNodeContents(el);
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
      const kept = samples
        .slice(budgets.discardedPerLoad.value)
        .map(splitSample);
      loads.push({ p95: percentile95(kept.map((s) => s[gate.metric])), kept });
    }
    const kept = loads.flatMap((load) => load.kept);
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
    const summary = [
      `${surface}: full key-to-paint p95 ${verdict(fullP95, budgets.p95Ms.value)} (spec, ${role("full")})`,
      `app-owned p95 ${verdict(appOwnedP95, budgets.hostedAppOwnedP95Ms.value)} (hosted, ${role("appOwned")})`,
      `tick wait ${p50p95(column("tickWait"))}, key-to-input ${p50p95(column("keyToInput"))}, render ${p50p95(column("render"))}, frame start to the recorder's rAF ${p50p95(column("frameToRaf"))}`,
      `full key-to-paint p50 ${median(column("full"))} ms, range ${range(column("full"))} ms, per-load ${gate.metric} p95 ${loads.map((load) => load.p95).join(", ")}, double rAF p95 ${percentile95(column("doubleRaf"))} ms`,
      `split check: max |app-owned + tick wait - full| ${residual.toFixed(3)} ms over ${kept.length} samples from ${loads.length} loads`,
      `idle rAF cadence per load ${cadences.map((c) => c.toFixed(1)).join(", ")} ms, load average ${loadavg()
        .map((x) => x.toFixed(2))
        .join(" ")}`,
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
    if (gated)
      expect(
        percentile95(column(gate.metric)),
        `EDIT-14 ${surface} ${gate.metric} p95 (${mode} mode)`,
      ).toBeLessThanOrEqual(gate.budget);
  });
}
