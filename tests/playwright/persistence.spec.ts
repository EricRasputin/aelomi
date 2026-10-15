import { expect, test } from "./fixtures";

declare global {
  interface Window {
    __aelomiProbe?: { keyAt: number; seen: { ms: number; text: string }[] };
  }
}

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    window.__aelomiMockSetup = {
      pages: [
        { id: "page-daily", title: "Daily" },
        { id: "page-other", title: "Other" },
      ],
      bodies: {
        "page-daily": [{ id: "block-plan", content: ["plan"] }],
        "page-other": [{ id: "block-other", content: ["other"] }],
      },
    };
  });
});

async function openDaily(page: import("@playwright/test").Page) {
  await page.goto("/page/page-daily");
  const body = page.getByRole("textbox", { name: "Page body" });
  await expect(body).toHaveText("plan");
  await body.click();
  await page.keyboard.press("Meta+ArrowRight");
  return body;
}

test("SHELL-9 typing x reaches the persist command within 100 ms", async ({
  page,
}) => {
  await openDaily(page);
  await page.evaluate(() => {
    const probe = { keyAt: 0, seen: [] as { ms: number; text: string }[] };
    window.__aelomiProbe = probe;
    addEventListener(
      "keydown",
      (event) => (probe.keyAt = event.timeStamp),
      true,
    );
    addEventListener("input", () =>
      probe.seen.push({
        ms: performance.now() - probe.keyAt,
        text: window.__aelomiMock?.bodies.text("page-daily") ?? "",
      }),
    );
  });
  await page.keyboard.press("x");
  const seen = await page.evaluate(() => window.__aelomiProbe?.seen ?? []);
  expect(seen).toHaveLength(1);
  expect(seen[0]?.text).toBe("planx");
  expect(seen[0]?.ms).toBeLessThan(100);
});

test("SHELL-9 opening another page right after typing x stores x", async ({
  page,
}) => {
  await openDaily(page);
  await page.evaluate(() => {
    addEventListener(
      "input",
      () => {
        history.pushState(null, "", "/page/page-other");
        dispatchEvent(new PopStateEvent("popstate"));
      },
      { once: true },
    );
  });
  await page.keyboard.press("x");
  await expect(page.getByRole("textbox", { name: "Page body" })).toHaveText(
    "other",
  );
  expect(
    await page.evaluate(() => window.__aelomiMock?.bodies.text("page-daily")),
  ).toBe("planx");
});

test("SHELL-9 quitting at once after typing x delivers x before the quit completes", async ({
  page,
}) => {
  await openDaily(page);
  await page.evaluate(() => {
    addEventListener("input", () => window.__aelomiMock?.quit.request(), {
      once: true,
    });
  });
  await page.keyboard.press("x");
  await expect
    .poll(() => page.evaluate(() => window.__aelomiMock?.quit.exited ?? null))
    .toBe(true);
  const order = await page.evaluate(() =>
    (window.__aelomiMock?.calls ?? [])
      .map((call) => call.command)
      .filter((command) =>
        ["docAppend", "frontendReady", "completeQuit"].includes(command),
      ),
  );
  expect(order).toEqual(["frontendReady", "docAppend", "completeQuit"]);
  expect(
    await page.evaluate(() => window.__aelomiMock?.bodies.text("page-daily")),
  ).toBe("planx");
  await expect(page.getByTestId("page-editor")).toHaveAttribute(
    "contenteditable",
    "false",
  );
});

test("SHELL-10 a drain whose delivery fails asks before quitting and keeps the edit", async ({
  page,
}) => {
  await openDaily(page);
  await page.evaluate(() => window.__aelomiMock?.storage.setStatus("Locked"));
  await page.keyboard.type("y");
  await expect(page.getByRole("alert")).toContainText(
    "Changes aren't being saved.",
  );
  await page.evaluate(() => window.__aelomiMock?.quit.request());
  const dialog = page.getByRole("alertdialog");
  await expect(dialog).toContainText("Quit with unsaved changes?");
  expect(
    await page.evaluate(() =>
      (window.__aelomiMock?.calls ?? [])
        .filter((call) => call.command === "quitDrainFailed")
        .map((call) => call.args[1]),
    ),
  ).toEqual([["page-daily"]]);
  await expect(page.getByTestId("page-editor")).toHaveAttribute(
    "contenteditable",
    "false",
  );
  await page.evaluate(() => window.__aelomiMock?.storage.setStatus("Ready"));
  await page.getByRole("button", { name: "Keep app open" }).click();
  await expect(dialog).not.toBeVisible();
  await expect(page.getByTestId("page-editor")).toHaveAttribute(
    "contenteditable",
    "true",
  );
  await page.evaluate(() => window.__aelomiMock?.quit.request());
  await expect
    .poll(() => page.evaluate(() => window.__aelomiMock?.quit.exited ?? null))
    .toBe(true);
  expect(
    await page.evaluate(() => window.__aelomiMock?.bodies.text("page-daily")),
  ).toBe("plany");
});
