import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { expect, test } from "vitest";
import {
  type Block,
  generateBlocks,
  generateFixtures,
  generatePages,
  type Page,
  plantedTokens,
} from "./generator.ts";

const sha256 = (bytes: Buffer) =>
  createHash("sha256").update(bytes).digest("hex");
const generated = generateFixtures();

for (const [name, content] of Object.entries(generated)) {
  test(`${name} regenerates byte for byte (run pnpm fixtures after changing the generator)`, () => {
    const committed = readFileSync(
      new URL(`./generated/${name}`, import.meta.url),
    );
    expect(sha256(Buffer.from(content, "utf8"))).toBe(sha256(committed));
  });
}

test("search pages form a tree of 10,000 pages with parents before children", () => {
  const { pages } = generatePages();
  expect(pages).toHaveLength(10_000);
  const depth = new Map<string, number>();
  for (const page of pages) {
    const parentDepth = page.parentId === null ? -1 : depth.get(page.parentId);
    expect(parentDepth, `parent of ${page.id} comes first`).not.toBeUndefined();
    depth.set(page.id, (parentDepth ?? -1) + 1);
  }
  expect(Math.max(...depth.values())).toBeLessThanOrEqual(4);
  const roots = pages.filter((page) => page.parentId === null).length;
  expect(roots).toBeGreaterThan(100);
  const trashed = pages.filter((page) => page.trash !== null).length;
  expect(trashed).toBeGreaterThan(100);
  expect(trashed).toBeLessThan(2_000);
});

test("expected query results are active pages that contain the token", () => {
  const { pages, queries } = generatePages();
  const byId = new Map<string, Page>(pages.map((page) => [page.id, page]));
  const contains = (query: string) =>
    pages.filter((page) =>
      page.body.split(/[ \n.]/).some((word) => word.startsWith(query)),
    );
  for (const { query, expectedIds } of queries) {
    for (const id of expectedIds) {
      const page = byId.get(id);
      expect(page?.trash, `${id} for ${query} is active`).toBeNull();
      expect(page?.body, `${id} contains ${query}`).toContain(query);
    }
    const active = contains(query).filter((page) => page.trash === null);
    expect(active.map((page) => page.id).sort()).toEqual(expectedIds);
  }
  expect(
    contains("quillmere").filter((page) => page.trash !== null),
  ).toHaveLength(plantedTokens.quillmere.trashed);
});

test("the editor page has exactly 5,000 blocks including children", () => {
  const blocks = generateBlocks();
  const count = (list: Block[]): number =>
    list.reduce((total, block) => total + 1 + count(block.children), 0);
  expect(count(blocks)).toBe(5_000);
  const ids = new Set<string>();
  const visit = (list: Block[]) => {
    for (const block of list) {
      ids.add(block.id);
      visit(block.children);
    }
  };
  visit(blocks);
  expect(ids.size).toBe(5_000);
  const types = new Set(blocks.map((block) => block.type));
  expect([...types].sort()).toEqual([
    "bulletListItem",
    "checkListItem",
    "codeBlock",
    "divider",
    "heading",
    "image",
    "numberedListItem",
    "paragraph",
    "quote",
  ]);
});

test("every run holds marks BlockNote can store together, and each mark stays covered", () => {
  // In BlockNote 0.55 the code mark excludes every other mark, link included,
  // and code blocks hold plain text. A run breaking either rule opens read-only.
  const supported = ["bold", "code", "italic", "strike", "underline"];
  const problems: string[] = [];
  const plain = new Set<string>();
  const linked = new Set<string>();
  let links = 0;
  const visit = (list: Block[]) => {
    for (const block of list) {
      for (const inline of "content" in block ? block.content : []) {
        const isLink = inline.type === "link";
        if (isLink) links += 1;
        for (const run of isLink ? inline.content : [inline]) {
          const styles = Object.keys(run.styles);
          for (const style of styles) {
            if (!supported.includes(style))
              problems.push(`${block.id}: ${style}`);
            (isLink ? linked : plain).add(style);
          }
          if (styles.length > 1 && styles.includes("code"))
            problems.push(`${block.id}: code with ${styles}`);
          if (isLink && styles.includes("code"))
            problems.push(`${block.id}: code inside a link`);
          if (block.type === "codeBlock" && styles.length > 0)
            problems.push(`${block.id}: ${styles} in a code block`);
        }
      }
      visit(block.children);
    }
  };
  visit(generateBlocks());
  expect(problems).toEqual([]);
  expect([...plain].sort()).toEqual(supported);
  expect(links).toBeGreaterThan(0);
  expect(linked.size).toBeGreaterThan(0);
});
