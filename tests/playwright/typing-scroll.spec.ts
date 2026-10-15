import { expect, test } from "./fixtures";

declare global {
  interface Window {
    __aelomiScroll?: { scrollTops: number[]; caretGap: number }[];
  }
}

test("EDIT-14 text that wraps at the bottom edge scrolls once, in the frame that shows it, keeping the caret 72 px clear", async ({
  page,
}) => {
  await page.addInitScript(() => {
    window.__aelomiMockSetup = {
      pages: [{ id: "page-long", title: "Long" }],
      bodies: {
        "page-long": Array.from({ length: 80 }, (_, i) => ({
          id: `block-${i}`,
          content: [`line ${i}`],
        })),
      },
    };
  });
  await page.goto("/page/page-long");
  const body = page.getByRole("textbox", { name: "Page body" });
  await expect(body).toContainText("line 79");
  await body.evaluate((el) => {
    el.focus();
    const texts = el.querySelectorAll(".bn-inline-content");
    const last = texts[texts.length - 1];
    if (!last) throw new Error("no paragraph");
    const caret = document.createRange();
    caret.selectNodeContents(last);
    caret.collapse(false);
    getSelection()?.removeAllRanges();
    getSelection()?.addRange(caret);
    last.scrollIntoView({ block: "end" });
    let scroller: Element | null = el;
    while (scroller && getComputedStyle(scroller).overflowY !== "auto")
      scroller = scroller.parentElement;
    if (!scroller) throw new Error("no scroller");
    const target = scroller;
    const keys: { scrollTops: number[]; caretGap: number }[] = [];
    window.__aelomiScroll = keys;
    addEventListener(
      "keydown",
      () => {
        const scrollTops = [target.scrollTop];
        const tick = () => {
          scrollTops.push(target.scrollTop);
          if (scrollTops.length < 5) {
            requestAnimationFrame(tick);
            return;
          }
          const caret = getSelection()?.getRangeAt(0).getBoundingClientRect();
          const caretGap =
            target.getBoundingClientRect().bottom - (caret?.bottom ?? 0);
          keys.push({ scrollTops, caretGap });
        };
        requestAnimationFrame(tick);
      },
      true,
    );
  });
  const text = "lorem ipsum ".repeat(9);
  for (const [i, key] of [...text].entries()) {
    await page.keyboard.type(key);
    await page.waitForFunction(
      (count) => (window.__aelomiScroll?.length ?? 0) > count,
      i,
    );
  }
  const keys = await page.evaluate(() => window.__aelomiScroll ?? []);
  const scrolled = keys.filter((key) => new Set(key.scrollTops).size > 1);
  expect(scrolled.length).toBeGreaterThan(0);
  expect(
    scrolled.filter((key) => new Set(key.scrollTops.slice(1)).size > 1),
  ).toEqual([]);
  for (const key of scrolled) expect(key.caretGap).toBeGreaterThanOrEqual(71);
});
