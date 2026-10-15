import type { TextPart } from "../ipc";

export type TitleTier = "exact" | "prefix" | "wordStart" | "subsequence";

export type TitleCandidate = { id: string; title: string };

export type PreparedTitles = readonly {
  id: string;
  title: string;
  lower: string;
  wordStarts: readonly number[];
  position: number;
}[];

export type TitleMatch = {
  id: string;
  tier: TitleTier;
  parts: TextPart[];
};

export function prepareTitles(
  candidates: readonly TitleCandidate[],
): PreparedTitles {
  return candidates.map(({ id, title }, position) => {
    const lower = title.toLowerCase();
    const wordStarts: number[] = [];
    for (let i = 0; i < lower.length; i++) {
      const char = lower[i] ?? "";
      const previous = i === 0 ? " " : (lower[i - 1] ?? " ");
      if (/[\p{L}\p{N}]/u.test(char) && !/[\p{L}\p{N}]/u.test(previous))
        wordStarts.push(i);
    }
    return { id, title, lower, wordStarts, position };
  });
}

function parts(title: string, marked: readonly number[]): TextPart[] {
  const result: TextPart[] = [];
  const set = new Set(marked);
  for (let i = 0; i < title.length; i++) {
    const highlighted = set.has(i);
    const last = result.at(-1);
    if (last && last.highlighted === highlighted) last.text += title[i];
    else result.push({ text: title[i] ?? "", highlighted });
  }
  return result;
}

function range(start: number, length: number): number[] {
  return Array.from({ length }, (_, i) => start + i);
}

/**
 * CMD-4: the query matches when its characters appear in the title in order,
 * ignoring case. Exact matches rank first, then title prefixes, then word
 * starts, then any other match. Ties go to the most recently opened page,
 * then tree order.
 */
export function rankTitles(
  titles: PreparedTitles,
  rawQuery: string,
  openedAt: ReadonlyMap<string, number>,
  limit: number,
): TitleMatch[] {
  const query = rawQuery.trim().toLowerCase();
  if (query === "") return [];
  const chars = [...query];
  const tiers: number[][] = [[], [], [], []];
  for (let i = 0; i < titles.length; i++) {
    const lower = titles[i]?.lower ?? "";
    if (lower === query) tiers[0]?.push(i);
    else if (lower.startsWith(query)) tiers[1]?.push(i);
    else if (titles[i]?.wordStarts.some((at) => lower.startsWith(query, at)))
      tiers[2]?.push(i);
    else {
      let at = 0;
      let matched = true;
      for (const char of chars) {
        const next = lower.indexOf(char, at);
        if (next < 0) {
          matched = false;
          break;
        }
        at = next + char.length;
      }
      if (matched) tiers[3]?.push(i);
    }
  }
  const picked: { index: number; tier: TitleTier }[] = [];
  for (const [rank, members] of tiers.entries()) {
    if (picked.length >= limit) break;
    const opened = members
      .filter((i) => openedAt.has(titles[i]?.id ?? ""))
      .sort(
        (a, b) =>
          (openedAt.get(titles[b]?.id ?? "") ?? 0) -
          (openedAt.get(titles[a]?.id ?? "") ?? 0),
      );
    const rest =
      opened.length === 0
        ? members
        : members.filter((i) => !opened.includes(i));
    const tier = tierOrder[rank] ?? "subsequence";
    for (const index of [...opened, ...rest]) {
      if (picked.length >= limit) break;
      picked.push({ index, tier });
    }
  }
  return picked.map(({ index, tier }) => {
    const { id, title, lower, wordStarts } = titles[index] ?? {
      id: "",
      title: "",
      lower: "",
      wordStarts: [],
    };
    return {
      id,
      tier,
      parts: parts(title, marks(tier, lower, query, wordStarts)),
    };
  });
}

const tierOrder: readonly TitleTier[] = [
  "exact",
  "prefix",
  "wordStart",
  "subsequence",
];

function marks(
  tier: TitleTier,
  lower: string,
  query: string,
  wordStarts: readonly number[],
): number[] {
  switch (tier) {
    case "exact":
    case "prefix":
      return range(0, query.length);
    case "wordStart":
      return range(
        wordStarts.find((at) => lower.startsWith(query, at)) ?? 0,
        query.length,
      );
    case "subsequence": {
      const marked: number[] = [];
      let at = 0;
      for (const char of query) {
        const next = lower.indexOf(char, at);
        marked.push(next);
        at = next + char.length;
      }
      return marked;
    }
  }
}
