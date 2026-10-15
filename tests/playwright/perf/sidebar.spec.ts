import { loadavg } from "node:os";
import { readBudgets } from "../../../perf/budgets.ts";
import { keystrokeMode } from "../../../perf/keystroke.ts";
import { expect, test } from "../fixtures";
import { scrollFrames, scrollSummary } from "./scroll-frames";

const mode = keystrokeMode(process.env);
const budgets = readBudgets().scroll;
test.use({ trace: "off" });

test("TREE-6 10,000 sidebar pages scroll for five seconds with bounded rendered rows", async ({
  page,
}, testInfo) => {
  await page.addInitScript(() => {
    window.__aelomiMockSetup = {
      pages: Array.from({ length: 10000 }, (_, i) => ({
        id: `sidebar-${i}`,
        title: `Project notes ${i + 1}`,
      })),
    };
  });
  await page.goto("/");
  const sidebar = page.getByRole("navigation", { name: "Pages" });
  await expect(
    sidebar.getByRole("button", { name: "Project notes 1", exact: true }),
  ).toBeVisible();
  const run = scrollSummary(await sidebar.evaluate(scrollFrames));
  const rendered = await sidebar.locator(".sidebar-row").count();
  const sourcePages = await page.evaluate(
    () => window.__aelomiMock?.workspace.tree().pages.length,
  );
  const summary = `sidebar source_pages=${sourcePages} rendered_rows=${rendered} frames=${run.frames} moved=${run.moved} rendering_p95_ms=${run.workP95} raw_max_ms=${run.rawMax} idle_cadence_ms=${run.idleCadence} load=${loadavg().join(",")}`;
  console.log(summary);
  testInfo.annotations.push({ type: "sidebar-scroll", description: summary });
  expect(sourcePages).toBe(10000);
  expect(rendered).toBeLessThan(50);
  expect(run.frames).toBeGreaterThan(200);
  expect(run.moved).toBe(run.frames);
  expect(run.landed).toBe(run.frames);
  expect(run.rawMax).toBeLessThanOrEqual(budgets.maxFrameMs.value);
  if (mode === "local")
    expect(run.workP95).toBeLessThan(budgets.p95FrameMs.value);
});
