import { shortcuts } from "../shortcuts";
import { prepareTitles, rankTitles } from "./title-match";

const entries = shortcuts.filter(
  (entry) => entry.command === "newPage" || entry.command === "trashPage",
);
export type PageCommand = (typeof entries)[number];
const titles = prepareTitles(
  entries.map((entry) => ({ id: entry.command, title: entry.label })),
);

export function matchingPageCommands(
  query: string,
  active: boolean,
): PageCommand[] {
  if (!active) return [];
  if (query.trim() === "") return entries;
  const matches = rankTitles(titles, query, new Map(), entries.length);
  return matches.flatMap((match) => {
    const entry = entries.find((entry) => entry.command === match.id);
    return entry ? [entry] : [];
  });
}
