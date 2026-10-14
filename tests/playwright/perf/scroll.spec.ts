import { loadavg } from "node:os";
import { readBudgets } from "../../../perf/budgets.ts";
import { keystrokeMode } from "../../../perf/keystroke.ts";
import { machineNoise, withNoise } from "../../../perf/noise.ts";
import { expect, test } from "../fixtures";
import { scrollFrames, scrollSummary } from "./scroll-frames";

/**
 * SPEC section 3 (EDIT-14): on the 5,000-block page in the product editor,
 * scrolling for 5 seconds keeps the p95 frame time under 16.7 ms with no
 * frame over 50 ms. The page's scroller moves 48 px per frame, down then up.
 * The budgets and their reasons live in `perf/budgets.json` (`scroll`).
 *
 * A frame's time is its rendering work: from the rAF timestamp to the
 * MessageChannel task posted from that frame's callback, which WebKit runs
 * after the frame's rendering update has sent its layer commit. That is a
 * lower bound on the whole frame. It leaves out layout that ran before the
 * timestamp, the scheduler and IPC that started the update, presentation,
 * and tasks between frames; only the longest raw interval, held to 50 ms,
 * covers those. The idle cadence and raw intervals are reported, not gated:
 * WebKit stamps rAF when the update starts, so even a page that does no work
 * reads raw intervals of 14 to 19 ms at 60 Hz.
 */
const mode = keystrokeMode(process.env);

// Tracing captures screencast frames while the page scrolls, which would be
// measured as the app's frame time.
test.use({ trace: "off" });
const budgets = readBudgets().scroll;
const p95Budget = budgets.p95FrameMs.value;
const maxBudget = budgets.maxFrameMs.value;

test("EDIT-14 the 5,000-block editor page scrolls smoothly for 5 seconds", async ({
  page,
}, testInfo) => {
  const noise = machineNoise(mode);
  await page.goto("/perf/keystroke/editor-page-5000-blocks");
  const editor = page.getByTestId("page-editor");
  await expect(editor).toBeVisible({ timeout: 30_000 });
  await page.waitForTimeout(1_500);
  const run = scrollSummary(await editor.evaluate(scrollFrames));
  const summary = withNoise(
    [
      `scroll frames ${run.frames}, rendering work p50 ${run.workP50} ms p95 ${run.workP95} ms (budget < ${p95Budget}) max ${run.workMax} ms, longest interval ${run.rawMax} ms (budget ${maxBudget})`,
      `raw intervals p50 ${run.rawP50} ms p95 ${run.rawP95} ms and idle cadence ${run.idleCadence} ms (information)`,
      `load average ${loadavg()
        .map((x) => x.toFixed(2))
        .join(" ")}`,
    ].join("\n  "),
    noise,
  );
  console.log(summary);
  testInfo.annotations.push({ type: "scroll", description: summary });
  expect(
    run.frames,
    "did-it-happen guard: about 5 s of frames",
  ).toBeGreaterThan(200);
  expect(
    run.moved,
    "did-it-happen guard: the scroller moved on every frame",
  ).toBe(run.frames);
  expect(
    run.landed,
    "did-it-happen guard: every frame's end stamp landed",
  ).toBe(run.frames);
  expect(
    run.rawMax,
    withNoise("EDIT-14 no scroll frame over 50 ms", noise),
  ).toBeLessThanOrEqual(maxBudget);
  if (mode === "local")
    expect(
      run.workP95,
      withNoise("EDIT-14 scroll frame p95", noise),
    ).toBeLessThan(p95Budget);
});
