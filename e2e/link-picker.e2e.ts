import { expect } from "e2e";
import { test } from "./fixtures";

test("TREE-9 the [[ picker lists a page until its parent is trashed, and links an active page", async ({
  app,
  browser,
  screen,
  nonLocalRequests,
}) => {
  await browser.addInitScript(() => {
    window.__aelomiMockSetup = {
      pages: [
        { id: "page-daily", title: "Daily" },
        { id: "page-projects", title: "Projects" },
        { id: "page-aelomi", title: "Aelomi", parentId: "page-projects" },
      ],
      bodies: { "page-daily": [{ id: "block-plan", content: ["plan"] }] },
    };
  });
  await app.open("/page/page-daily");
  const body = screen.getByRole("textbox", "Page body");
  await expect(body).toHaveText("plan");
  await body.click();
  await browser.keyboard.press("Meta+ArrowRight");
  await browser.keyboard.press("Enter");
  await browser.keyboard.type("[[Aelo");
  const picker = screen.getByRole("listbox", "Link to page");
  await expect(picker.getByRole("option", "Aelomi")).toBeVisible();
  await browser.keyboard.press("Escape");
  await expect(picker).not.toBeVisible();
  expect(
    await browser.evaluate(
      () => window.__aelomiMock?.bodies.text("page-daily") ?? null,
    ),
  ).toBe("plan\n[[Aelo");

  await browser.evaluate(() => {
    window.__aelomiMock?.workspace.trash("page-projects", "batch-1");
    return null;
  });
  await browser.keyboard.press("Enter");
  await browser.keyboard.type("[[Aelo");
  await expect(screen.getByText("No matching pages")).toBeVisible();
  await expect(screen.getByRole("option", "Aelomi")).not.toBeVisible();
  await browser.keyboard.press("Escape");
  await browser.keyboard.press("Enter");
  await browser.keyboard.type("[[Proj");
  await expect(screen.getByText("No matching pages")).toBeVisible();
  await expect(screen.getByRole("option", "Projects")).not.toBeVisible();
  await browser.keyboard.press("Escape");

  await browser.keyboard.press("Enter");
  await browser.keyboard.type("[[Dai");
  await expect(picker.getByRole("option", "Daily")).toBeVisible();
  await browser.keyboard.press("Enter");
  await expect(picker).not.toBeVisible();
  expect(
    await browser.evaluate(
      () =>
        window.__aelomiMock?.bodies.blocks("page-daily").blocks.at(-1)
          ?.segments ?? null,
    ),
  ).toEqual([{ kind: "link", pageId: "page-daily" }]);
  await expect(body.getByText("Daily", { exact: true })).toBeVisible();
  expect(nonLocalRequests).toEqual([]);
});

test("LINK-2 [[ is plain text inside a code block and inside inline code", async ({
  app,
  browser,
  screen,
  nonLocalRequests,
}) => {
  await browser.addInitScript(() => {
    window.__aelomiMockSetup = {
      pages: [
        { id: "page-daily", title: "Daily" },
        { id: "page-aelomi", title: "Aelomi" },
      ],
      bodies: {
        "page-daily": [
          { id: "block-text", content: ["prose"] },
          { id: "block-code", type: "codeBlock", content: ["fn main() {}"] },
          {
            id: "block-inline",
            content: [{ text: "inline code span", styles: ["code"] }],
          },
        ],
      },
    };
  });
  await app.open("/page/page-daily");
  const body = screen.getByRole("textbox", "Page body");
  await expect(body).toContainText("prose");
  const picker = screen.getByRole("listbox", "Link to page");

  await screen.getByText("prose").click();
  await browser.keyboard.press("Meta+ArrowRight");
  await browser.keyboard.type("[[");
  await expect(picker).toBeVisible();
  await browser.keyboard.press("Escape");
  await expect(picker).not.toBeVisible();

  await screen.getByText("fn main() {}").click();
  await browser.keyboard.press("Meta+ArrowRight");
  await browser.keyboard.type("[[Ael");
  await expect(screen.getByText("fn main() {}[[Ael")).toBeVisible();
  await expect(picker).not.toBeVisible();

  await screen.getByText("inline code span").click();
  await browser.keyboard.type("[[Ael");
  await expect(body).toContainText("[[Ael", { timeout: 2_000 });
  await expect(picker).not.toBeVisible();
  expect(
    await browser.evaluate(() =>
      (window.__aelomiMock?.bodies.blocks("page-daily").blocks ?? []).map(
        (block) =>
          block.segments
            .map((s) => (s.kind === "text" ? s.text.includes("[[Ael") : "link"))
            .join(","),
      ),
    ),
  ).toEqual(["false", "true", "true"]);
  expect(nonLocalRequests).toEqual([]);
});

test("LINK-2 pages with the same title show their parent paths in the picker", async ({
  app,
  browser,
  screen,
  nonLocalRequests,
}) => {
  await browser.addInitScript(() => {
    window.__aelomiMockSetup = {
      pages: [
        { id: "page-daily", title: "Daily" },
        { id: "page-work", title: "Work" },
        { id: "page-work-notes", title: "Notes", parentId: "page-work" },
        { id: "page-home", title: "Home" },
        { id: "page-home-notes", title: "Notes", parentId: "page-home" },
      ],
      bodies: { "page-daily": [{ id: "block-plan", content: ["plan"] }] },
    };
  });
  await app.open("/page/page-daily");
  const body = screen.getByRole("textbox", "Page body");
  await expect(body).toHaveText("plan");
  await body.click();
  await browser.keyboard.press("Meta+ArrowRight");
  await browser.keyboard.type("[[Notes");
  const picker = screen.getByRole("listbox", "Link to page");
  await expect(picker.getByRole("option", "Notes Work")).toBeVisible();
  await expect(picker.getByRole("option", "Notes Home")).toBeVisible();
  await browser.keyboard.press("ArrowDown");
  await browser.keyboard.press("Enter");
  await expect(picker).not.toBeVisible();
  expect(
    await browser.evaluate(
      () =>
        window.__aelomiMock?.bodies.blocks("page-daily").blocks[0]?.segments ??
        null,
    ),
  ).toEqual([
    { kind: "text", text: "plan" },
    { kind: "link", pageId: "page-home-notes" },
  ]);
  expect(nonLocalRequests).toEqual([]);
});
