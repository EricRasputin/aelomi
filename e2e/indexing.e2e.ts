import { expect } from "e2e";
import * as Y from "yjs";
import { test } from "./fixtures";

declare global {
  interface Window {
    __flashes?: string[];
  }
}

const daily = {
  pages: [{ id: "page-daily", title: "Daily" }],
  bodies: { "page-daily": [{ id: "block-plan", content: ["plan"] }] },
};

test("SHELL-12 acceptance 8: the page stays editable while the index rebuilds, then search finds the edit", async ({
  app,
  browser,
  screen,
}) => {
  await browser.addInitScript((setup) => {
    window.__aelomiMockSetup = { ...setup, indexStatus: "Rebuilding" };
  }, daily);
  await app.open("/");
  await expect(screen.getByRole("heading", "Daily")).toBeVisible();
  const body = screen.getByRole("textbox", "Page body");
  await expect(body).toHaveText("plan");
  await body.click();
  await browser.keyboard.press("Meta+ArrowRight");
  await browser.keyboard.type(" roadmap");
  await expect(body).toHaveText("plan roadmap");
  expect(
    await browser.evaluate(() => ({
      text: window.__aelomiMock?.bodies.text("page-daily") ?? null,
      appended:
        window.__aelomiMock?.calls.some(
          (call) =>
            call.command === "docAppend" && call.args[0] === "page-daily",
        ) ?? null,
    })),
  ).toEqual({ text: "plan roadmap", appended: true });

  await browser.keyboard.press("Meta+k");
  await browser.keyboard.type("a");
  const status = screen.getByRole("status", "Search status");
  await expect(status).toHaveText("Indexing…");
  await expect(screen.getByRole("option", "Daily")).toBeVisible();

  await browser.evaluate(() => {
    window.__aelomiMock?.index.setStatus("Ready");
    return null;
  });
  await expect(status).not.toHaveText("Indexing…");
  await browser.keyboard.press("Backspace");
  await browser.keyboard.type("roadm");
  const hit = screen.getByRole("option", /Daily/);
  await expect(hit).toContainText("plan roadmap");
  await expect(hit.getByText("roadmap", { exact: true })).toBeVisible();

  await browser.keyboard.press("Escape");
  await expect(screen.getByRole("dialog", "Command palette")).not.toBeVisible();
  await browser.keyboard.type("x");
  await expect(body).toHaveText("plan roadmapx");

  await browser.evaluate(() => {
    window.__aelomiMock?.storage.keepForReload();
    return null;
  });
  await browser.reload();
  await expect(screen.getByRole("textbox", "Page body")).toHaveText(
    "plan roadmapx",
  );
  expect(
    await browser.evaluate(() => window.__aelomiMock?.errors.length ?? null),
  ).toBe(0);
});

test("SHELL-12 a rebuilding index never shows an error, and a failed one does until it recovers", async ({
  app,
  browser,
  screen,
}) => {
  await browser.addInitScript((setup) => {
    window.__aelomiMockSetup = { ...setup, indexStatus: "Rebuilding" };
  }, daily);
  await app.open("/page/page-daily");
  await expect(screen.getByRole("textbox", "Page body")).toHaveText("plan");
  await browser.keyboard.press("Meta+k");
  await browser.keyboard.type("plan");
  await expect(screen.getByRole("status", "Search status")).toHaveText(
    "Indexing…",
  );
  await expect(screen.getByRole("alert")).not.toBeVisible();

  await browser.evaluate(() => {
    window.__aelomiMock?.index.setStatus({
      Failed: { reason: "read-only index folder" },
    });
    return null;
  });
  const alert = screen.getByRole("alert");
  await expect(alert).toContainText(
    "Search is unavailable because the index couldn't be rebuilt.",
  );
  await expect(alert).toContainText("read-only index folder");
  await expect(screen.getByRole("status", "Search status")).not.toHaveText(
    "Indexing…",
  );

  await browser.evaluate(() => {
    window.__aelomiMock?.index.setStatus("Ready");
    return null;
  });
  await expect(alert).not.toBeVisible();
  await expect(screen.getByRole("option", /Daily/)).toContainText("plan");
});

test("SHELL-12 search reports partly indexed pages quietly, by title", async ({
  app,
  browser,
  screen,
}) => {
  await browser.addInitScript(() => {
    window.__aelomiMockSetup = {
      pages: [
        { id: "page-daily", title: "Daily" },
        { id: "page-broken", title: "Broken links" },
      ],
      bodies: {
        "page-daily": [{ id: "block-plan", content: ["plan"] }],
        "page-broken": [
          { id: "block-a", content: ["alpha ", { link: "Not a page ID" }] },
        ],
      },
    };
  });
  await app.open("/page/page-daily");
  await expect(screen.getByRole("textbox", "Page body")).toHaveText("plan");
  await browser.keyboard.press("Meta+k");
  await browser.keyboard.type("plan");
  await expect(screen.getByRole("option", /Daily/)).toBeVisible();
  await expect(screen.getByText(/only partly searchable/)).toHaveText(
    "1 page is only partly searchable: Broken links.",
  );
});

test("CMD-4 and CMD-7 the palette opens a title match, and a content match at its block", async ({
  app,
  browser,
  screen,
}) => {
  await browser.addInitScript(() => {
    window.__aelomiMockSetup = {
      pages: [
        { id: "page-daily", title: "Daily" },
        { id: "page-aelomi", title: "Aelomi app" },
        { id: "page-apple", title: "Apple" },
      ],
      bodies: {
        "page-daily": [{ id: "block-plan", content: ["plan"] }],
        "page-aelomi": [
          { id: "block-intro", content: ["intro"] },
          { id: "block-roadmap", content: ["quarterly roadmap"] },
        ],
      },
    };
  });
  await app.open("/page/page-daily");
  await expect(screen.getByRole("textbox", "Page body")).toHaveText("plan");
  await browser.keyboard.press("Meta+k");
  await browser.keyboard.type("aelm");
  await expect(screen.getByRole("option", "Aelomi app")).toBeVisible();
  await expect(screen.getByRole("option", "Apple")).not.toBeVisible();
  await browser.keyboard.press("Enter");
  await expect(screen.getByRole("heading", "Aelomi app")).toBeVisible();

  await browser.keyboard.press("Meta+k");
  await browser.keyboard.type("quarterly");
  await expect(screen.getByRole("option", /Aelomi app/)).toContainText(
    "quarterly roadmap",
  );
  await browser.evaluate(() => {
    const seen: string[] = [];
    window.__flashes = seen;
    new MutationObserver((records) => {
      for (const record of records)
        for (const node of record.addedNodes)
          if (node instanceof HTMLElement && node.className === "block-flash")
            seen.push(node.style.height);
    }).observe(document.body, { childList: true, subtree: true });
    return null;
  });
  await browser.keyboard.press("Enter");
  await expect(screen.getByRole("dialog", "Command palette")).not.toBeVisible();
  expect(await browser.evaluate(() => window.__flashes?.length ?? 0)).toBe(1);
  await browser.keyboard.type("Q2 ");
  expect(
    await browser.evaluate(
      () => window.__aelomiMock?.bodies.text("page-aelomi") ?? null,
    ),
  ).toBe("intro\nQ2 quarterly roadmap");
});

test("DATA-2 a page list another writer stored as a sequence says why no page opens, and nothing is written", async ({
  app,
  browser,
  screen,
}) => {
  const foreign = new Y.Doc();
  foreign.clientID = 9;
  foreign.getArray("pages").insert(0, ["foreign"]);
  const rawWorkspace = Buffer.from(Y.encodeStateAsUpdate(foreign)).toString(
    "base64",
  );
  await browser.addInitScript(
    (setup) => {
      window.__aelomiMockSetup = setup;
    },
    { ...daily, rawWorkspace },
  );
  await app.open("/");
  await expect(
    screen.getByRole("complementary", "Sidebar").getByRole("alert"),
  ).toContainText("Pages couldn't be loaded.");
  const alert = screen.getByRole("main").getByRole("alert");
  await expect(alert).toContainText(
    "Aelomi can't read this workspace's page list, so no page can open. Nothing on disk was changed.",
  );
  await expect(alert).toContainText(
    "workspace doc: the pages root is not a Y.Map, found a sequence",
  );
  expect(
    await browser.evaluate(
      () =>
        window.__aelomiMock?.calls
          .filter((call) => call.command === "docAppend")
          .map((call) => call.args[0]) ?? null,
    ),
  ).toEqual([]);
});
