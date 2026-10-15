import { loadavg } from "node:os";
import { readBudgets } from "../../../perf/budgets.ts";
import { keystrokeMode } from "../../../perf/keystroke.ts";
import { machineNoise, withNoise } from "../../../perf/noise.ts";
import { median, percentile95 } from "../../../perf/stats.ts";
import { expect, test } from "../fixtures";

// The tracer snapshots the whole DOM right before each key, which warms the
// page and would be measured as the app's speed.
test.use({ trace: "off" });
const budgets = readBudgets().palette;
const mode = keystrokeMode(process.env);
const budget =
  mode === "ci" ? budgets.hostedAppOwnedP95Ms.value : budgets.p95Ms.value;
const queries = ["zitida", "rehada go", "quillmere", "dapoku govova", "zor"];

type Sample = { down: number; shown: number; query: string };

declare global {
  interface Window {
    __aelomiPalette?: { samples: Sample[] };
  }
}

function installProbe() {
  const probe = { samples: [] as Sample[] };
  window.__aelomiPalette = probe;
  let down = 0;
  const searchInput = () =>
    document.querySelector<HTMLInputElement>(
      'input[aria-label="Search pages"]',
    );
  addEventListener(
    "keydown",
    (event) => {
      down = event.timeStamp;
    },
    true,
  );
  addEventListener("input", (event) => {
    const input = searchInput();
    if (!input || event.target !== input) return;
    const start = down;
    const check = () => {
      const list = document.getElementById("palette-results");
      const ready =
        list?.dataset.settled === "true" && list.dataset.query === input.value;
      if (!ready) {
        requestAnimationFrame(check);
        return;
      }
      const channel = new MessageChannel();
      channel.port1.onmessage = () =>
        probe.samples.push({
          down: start,
          shown: performance.now(),
          query: input.value,
        });
      channel.port2.postMessage(0);
    };
    requestAnimationFrame(check);
  });
}

test("CMD-4 and CMD-5 the palette shows results within one frame of each keystroke on the 10,000-page fixture", async ({
  page,
}, testInfo) => {
  test.setTimeout(10 * 60 * 1000);
  const noise = machineNoise(mode);
  await page.addInitScript(() => {
    window.__aelomiMockSetup = { searchFixture: true };
  });
  await page.addInitScript(installProbe);
  await page.goto("/");
  await expect(page.getByRole("textbox", { name: "Page body" })).toBeVisible({
    timeout: 60_000,
  });
  const samples: Sample[] = [];
  for (const query of queries) {
    await page.keyboard.press("Meta+k");
    const input = page.getByRole("combobox", { name: "Search pages" });
    await expect(input).toBeFocused();
    await page.waitForTimeout(300);
    for (const char of query) {
      await page.keyboard.type(char);
      await page.waitForTimeout(120);
    }
    for (let i = 0; i < 3; i++) {
      await page.keyboard.press("Backspace");
      await page.waitForTimeout(120);
    }
    await expect(page.locator("#palette-results")).toHaveAttribute(
      "data-settled",
      "true",
    );
    await page.keyboard.press("Escape");
    await page.waitForTimeout(200);
  }
  samples.push(
    ...(await page.evaluate(() => window.__aelomiPalette?.samples ?? [])),
  );
  const keys = queries.reduce((n, q) => n + q.length + 3, 0);
  expect(
    samples,
    "did-it-happen guard: one settled frame per key",
  ).toHaveLength(keys);
  const times = samples.map((s) => s.shown - s.down);
  const p50 = median(times);
  const p95 = percentile95(times);
  const slowest = [...samples]
    .sort((a, b) => b.shown - b.down - (a.shown - a.down))
    .slice(0, 5)
    .map((s) => `${JSON.stringify(s.query)} ${s.shown - s.down} ms`)
    .join(", ");
  const summary = withNoise(
    `palette results p50 ${p50} ms p95 ${p95} ms (${mode} p95 budget ${budget} ms${mode === "ci" ? `, p50 budget ${budgets.hostedAppOwnedP50Ms.value} ms` : ""}) over ${times.length} keys, slowest ${slowest}, load average ${loadavg()
      .map((x) => x.toFixed(2))
      .join(" ")}`,
    noise,
  );
  console.log(summary);
  testInfo.annotations.push({ type: "palette", description: summary });
  if (mode === "ci")
    expect(
      p50,
      withNoise("CMD palette results p50 (ci mode)", noise),
    ).toBeLessThanOrEqual(budgets.hostedAppOwnedP50Ms.value);
  expect(
    p95,
    withNoise(`CMD palette results p95 (${mode} mode)`, noise),
  ).toBeLessThanOrEqual(budget);
});
