import { loadavg } from "node:os";
import { keystrokeSurfaces, readBudgets } from "../../../perf/budgets.ts";
import { median, percentile95, range } from "../../../perf/stats.ts";
import { expect, test } from "../fixtures";

const budgets = readBudgets().keystroke;

type Sample = { rafMc: number; doubleRaf: number; shown: boolean };
type Recorder = { samples: Sample[]; missing: number };

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
    const start = down;
    const target = event.target instanceof HTMLElement ? event.target : null;
    requestAnimationFrame(() => {
      const shown = (target?.lastChild?.textContent ?? "").endsWith(key);
      if (!shown) recorder.missing += 1;
      const sample: Sample = { rafMc: 0, doubleRaf: 0, shown };
      let landed = 0;
      const finish = () => {
        landed += 1;
        if (landed === 2) recorder.samples.push(sample);
      };
      const channel = new MessageChannel();
      channel.port1.onmessage = () => {
        sample.rafMc = performance.now() - start;
        finish();
      };
      channel.port2.postMessage(0);
      requestAnimationFrame(() => {
        sample.doubleRaf = performance.now() - start;
        finish();
      });
    });
  });
}

for (const surface of keystrokeSurfaces) {
  const gated = surface === budgets.gate.value;
  test(`EDIT-14 keystroke-to-paint on the ${surface} surface`, async ({
    page,
  }, testInfo) => {
    await page.addInitScript(installRecorder);
    const loads: { p95: number; kept: Sample[] }[] = [];
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
      const kept = samples.slice(budgets.discardedPerLoad.value);
      loads.push({ p95: percentile95(kept.map((s) => s.rafMc)), kept });
    }
    const kept = loads.flatMap((load) => load.kept);
    const rafMc = kept.map((s) => s.rafMc);
    const p95 = percentile95(rafMc);
    const doubleRafP95 = percentile95(kept.map((s) => s.doubleRaf));
    const verdict = p95 <= budgets.p95Ms.value ? "PASS" : "FAIL";
    const summary = [
      `${surface}: p95 ${p95} ms ${p95 <= budgets.p95Ms.value ? "<=" : ">"} ${budgets.p95Ms.value} ms ${verdict} (${gated ? "gate" : "information"})`,
      `${kept.length} samples from ${loads.length} loads, per-load p95 ${loads.map((load) => load.p95).join(", ")}, p50 ${median(rafMc)} ms, range ${range(rafMc)} ms`,
      `double rAF p95 ${doubleRafP95} ms, load average ${loadavg()
        .map((x) => x.toFixed(2))
        .join(" ")}`,
    ].join("\n  ");
    console.log(summary);
    testInfo.annotations.push({ type: "keystroke", description: summary });
    await testInfo.attach(`keystroke-${surface}.json`, {
      body: JSON.stringify(
        {
          surface,
          gated,
          budgetMs: budgets.p95Ms.value,
          p95,
          doubleRafP95,
          loads,
        },
        null,
        2,
      ),
      contentType: "application/json",
    });
    if (gated)
      expect(
        p95,
        `EDIT-14 ${surface} keystroke-to-paint p95`,
      ).toBeLessThanOrEqual(budgets.p95Ms.value);
  });
}
