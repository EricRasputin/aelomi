import { expect, test } from "vitest";
import { prepareTitles, rankTitles } from "./title-match";

const titles = prepareTitles(
  [
    "Apple",
    "Aelomi app",
    "Aelomi",
    "Reading list",
    "Map of apps",
    "aelomi notes",
    "Plan",
  ].map((title, i) => ({ id: `p${i}`, title })),
);

const rank = (query: string, opened: [string, number][] = []) =>
  rankTitles(titles, query, new Map(opened), 200).map(
    (match) => `${match.tier}:${match.id}`,
  );

test("CMD-4 ranks exact, then prefix, then word start, then any in-order match", () => {
  expect(rank("aelomi")).toEqual(["exact:p2", "prefix:p1", "prefix:p5"]);
  expect(rank("app")).toEqual(["prefix:p0", "wordStart:p1", "wordStart:p4"]);
  expect(rank("aelm")).toEqual([
    "subsequence:p1",
    "subsequence:p2",
    "subsequence:p5",
  ]);
  expect(rank("APP")).toEqual(rank("app"));
  expect(rank("zz")).toEqual([]);
  expect(rank("   ")).toEqual([]);
});

test("CMD-4 ties go to the most recently opened page, then tree order", () => {
  expect(
    rank("aelomi", [
      ["p5", 2],
      ["p1", 1],
    ]),
  ).toEqual(["exact:p2", "prefix:p5", "prefix:p1"]);
  expect(rank("a", [["p6", 9]])).toEqual([
    "prefix:p0",
    "prefix:p1",
    "prefix:p2",
    "prefix:p5",
    "wordStart:p4",
    "subsequence:p6",
    "subsequence:p3",
  ]);
});

test("highlights the matched characters only", () => {
  const [match] = rankTitles(titles, "aelm", new Map(), 1);
  expect(match?.parts).toEqual([
    { text: "Ael", highlighted: true },
    { text: "o", highlighted: false },
    { text: "m", highlighted: true },
    { text: "i app", highlighted: false },
  ]);
  const [word] = rankTitles(titles, "list", new Map(), 1);
  expect(word?.parts).toEqual([
    { text: "Reading ", highlighted: false },
    { text: "list", highlighted: true },
  ]);
});

test("caps the group at the limit", () => {
  const many = prepareTitles(
    Array.from({ length: 250 }, (_, i) => ({
      id: `n${i}`,
      title: `Note ${i}`,
    })),
  );
  const ranked = rankTitles(many, "note", new Map(), 200);
  expect(ranked).toHaveLength(200);
  expect(ranked[199]?.id).toBe("n199");
});
