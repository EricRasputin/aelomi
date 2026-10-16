import { expect } from "e2e";
import { test } from "./fixtures";

test("SHELL-1 SHELL-6 Toggle sidebar", async ({ app, browser, screen }) => {
  await browser.addInitScript(() => {
    window.__aelomiMockSetup = { pages: [{ id: "a", title: "A" }] };
  });
  await app.open("/");
  const sidebar = screen.getByRole("complementary", "Sidebar");
  await expect(sidebar).toBeVisible();
  await browser.keyboard.press("Meta+Backslash");
  await expect(sidebar).not.toBeVisible();
  await browser.keyboard.press("Meta+Backslash");
  await expect(sidebar).toBeVisible();
});

test("SHELL-4 TREE-1 Empty state creates an untitled page with its title focused", async ({
  app,
  browser,
  screen,
}) => {
  await browser.addInitScript(() => {
    window.__aelomiMockSetup = { pages: [{ id: "a", title: "A" }] };
  });
  await app.open("/");
  await expect(screen.getByRole("textbox", "Page title")).toBeVisible();
  await browser.keyboard.press("Meta+k");
  await browser.keyboard.type("Move to trash");
  await screen.getByRole("option", "Move to trash").click();
  await expect(screen.getByRole("region", "No page open")).toBeVisible();
  await screen
    .getByRole("region", "No page open")
    .getByRole("button", "New page")
    .click();
  await expect(screen.getByRole("textbox", "Page title")).toBeVisible();
  await browser.keyboard.type("Created from empty state");
  await expect(
    screen
      .getByRole("navigation", "Pages")
      .getByRole("button", "Created from empty state"),
  ).toBeVisible();
});

test("TREE-1 TREE-7 EDIT-1 Rename updates the sidebar and Return moves to the body", async ({
  app,
  browser,
  screen,
}) => {
  await app.open("/");
  await expect(screen.getByRole("region", "No page open")).toBeVisible();
  await browser.keyboard.press("Meta+n");
  await expect(screen.getByRole("textbox", "Page title")).toBeVisible();
  await browser.keyboard.type("Projects");
  await expect(
    screen.getByRole("navigation", "Pages").getByRole("button", "Projects"),
  ).toBeVisible();
  await browser.keyboard.press("Enter");
  await browser.keyboard.type("body token");
  await expect(screen.getByRole("textbox", "Page body")).toHaveText(
    "body token",
  );
});

test("TREE-9 Trash the open page chooses its sibling then the empty state", async ({
  app,
  browser,
  screen,
}) => {
  await browser.addInitScript(() => {
    window.__aelomiMockSetup = {
      pages: [
        { id: "projects", title: "Projects" },
        { id: "a", title: "A", parentId: "projects" },
        { id: "b", title: "B", parentId: "projects" },
      ],
      settings: { expanded_rows: ["projects"] },
    };
  });
  await app.open("/page/a");
  await expect(screen.getByRole("textbox", "Page body")).toBeVisible();
  await browser.keyboard.press("Meta+k");
  await browser.keyboard.type("Move to trash");
  await screen.getByRole("option", "Move to trash").click();
  await expect(screen.getByRole("heading", "B")).toBeVisible();
  await screen
    .getByRole("navigation", "Pages")
    .getByRole("button", "Projects", { exact: true })
    .click();
  await browser.keyboard.press("Meta+k");
  await browser.keyboard.type("Move to trash");
  await screen.getByRole("option", "Move to trash").click();
  await expect(screen.getByRole("region", "No page open")).toBeVisible();
});

test("CMD-2 CMD-3 CMD-6 New page is reachable by query and shortcut from the empty state", async ({
  app,
  browser,
  screen,
}) => {
  await app.open("/");
  await expect(screen.getByRole("region", "No page open")).toBeVisible();
  await browser.keyboard.press("Meta+k");
  await expect(screen.getByRole("option", "New page")).not.toBeVisible();
  await browser.keyboard.type("new");
  await expect(screen.getByRole("option", "New page")).toBeVisible();
  await expect(screen.getByRole("option", "Move to trash")).not.toBeVisible();
  await screen.getByRole("option", "New page").click();
  await expect(screen.getByRole("textbox", "Page title")).toBeVisible();
  await browser.keyboard.type("Created by palette");
  await expect(
    screen
      .getByRole("navigation", "Pages")
      .getByRole("button", "Created by palette"),
  ).toBeVisible();
  await browser.keyboard.press("Meta+k");
  await browser.keyboard.type("Move to trash");
  await screen.getByRole("option", "Move to trash").click();
  await expect(screen.getByRole("region", "No page open")).toBeVisible();
  await browser.keyboard.press("Meta+n");
  await expect(screen.getByRole("textbox", "Page title")).toBeVisible();
  await browser.keyboard.type("Created by shortcut");
  await expect(
    screen
      .getByRole("navigation", "Pages")
      .getByRole("button", "Created by shortcut"),
  ).toBeVisible();
});
