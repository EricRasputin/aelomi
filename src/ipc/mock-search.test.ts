import { expect, test } from "vitest";
import {
  matchText,
  PageIndex,
  parseQuery,
  prepare,
  searchPages,
} from "./mock-search";

const page = (
  id: string,
  createdAt: number,
  title: string,
  ...blocks: string[]
) => ({
  id,
  title: prepare(title),
  createdAt,
  blocks: blocks.map((text, i) => ({ id: `${id}-${i}`, text: prepare(text) })),
});

const pages = new PageIndex([
  page("p-old", 1, "Roadmap", "quarterly plan"),
  page("p-mid", 2, "Daily", "nothing here", "quarterly roadmap review"),
  page("p-twin", 3, "Notes 2", "roadmapping"),
  page("p-new", 3, "Notes", "Café roadmap"),
]);

test("content scope orders newest created first, then page ID, and returns each page's first matching block", () => {
  expect(
    searchPages(pages, "roadm", 20, {
      kind: "content",
      exclude: new Set(),
    }).map((hit) => [hit.page_id, hit.block?.block_id]),
  ).toEqual([
    ["p-new", "p-new-0"],
    ["p-twin", "p-twin-0"],
    ["p-mid", "p-mid-1"],
  ]);
});

test("content scope skips excluded pages before the limit", () => {
  expect(
    searchPages(pages, "roadm", 1, {
      kind: "content",
      exclude: new Set(["p-new"]),
    }).map((hit) => hit.page_id),
  ).toEqual(["p-twin"]);
});

test("all scope lists title matches first, with no block, then body matches", () => {
  expect(
    searchPages(pages, "roadmap", 20, { kind: "all" }).map((hit) => [
      hit.page_id,
      hit.block === null,
    ]),
  ).toEqual([
    ["p-old", true],
    ["p-new", false],
    ["p-twin", false],
    ["p-mid", false],
  ]);
});

test("only the last word matches as a prefix, diacritics fold, and matched words are highlighted", () => {
  const query = parseQuery("cafe road");
  expect(query).toEqual({ phrases: [["cafe"], ["road"]] });
  expect(matchText(query ?? { phrases: [] }, prepare("Café roadmap"))).toEqual([
    { text: "Café", highlighted: true },
    { text: " ", highlighted: false },
    { text: "roadmap", highlighted: true },
  ]);
  expect(
    matchText(
      parseQuery("caf road") ?? { phrases: [] },
      prepare("Café roadmap"),
    ),
  ).toBe(null);
  expect(parseQuery("  ,. ")).toBe(null);
});

test("a word must be in the same page body for the page to match", () => {
  expect(
    searchPages(pages, "quarterly roadm", 20, {
      kind: "content",
      exclude: new Set(),
    }).map((hit) => hit.page_id),
  ).toEqual(["p-mid"]);
  expect(
    searchPages(pages, "nothing roadm", 20, {
      kind: "content",
      exclude: new Set(),
    }).map((hit) => hit.page_id),
  ).toEqual([]);
});
