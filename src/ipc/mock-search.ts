import type { TextPart } from "./bindings";

type Token = { term: string; start: number; end: number };

const word = /[\p{L}\p{N}\p{M}]+/gu;
const marks = /\p{M}/gu;

function fold(text: string): string {
  return text.normalize("NFD").replace(marks, "").toLowerCase();
}

/** Close to FTS5 `unicode61 remove_diacritics 2`: letters and digits, folded. */
export function tokenize(text: string): Token[] {
  const tokens: Token[] = [];
  for (const match of text.matchAll(word)) {
    const term = fold(match[0]);
    if (term === "") continue;
    tokens.push({
      term,
      start: match.index,
      end: match.index + match[0].length,
    });
  }
  return tokens;
}

/** Each whitespace word is a phrase; the last token of the last one is a prefix. */
export type Query = { phrases: string[][] };

export function parseQuery(text: string): Query | null {
  const phrases = text
    .split(/\s+/)
    .map((part) => tokenize(part).map((token) => token.term))
    .filter((phrase) => phrase.length > 0);
  return phrases.length > 0 ? { phrases } : null;
}

export type Prepared = {
  text: string;
  tokens: readonly Token[];
  terms: string;
};

export function prepare(text: string): Prepared {
  const tokens = tokenize(text);
  return { text, tokens, terms: ` ${tokens.map((t) => t.term).join(" ")}` };
}

/** Highlighted parts when every phrase matches `text`, else null. */
export function matchText(
  query: Query,
  { text, tokens, terms }: Prepared,
): TextPart[] | null {
  for (const phrase of query.phrases) {
    if (!terms.includes(` ${phrase[0]}`)) return null;
  }
  const spans: [number, number][] = [];
  const last = query.phrases.length - 1;
  for (const [p, phrase] of query.phrases.entries()) {
    let found = false;
    for (let i = 0; i + phrase.length <= tokens.length; i++) {
      const fits = phrase.every((term, k) => {
        const token = tokens[i + k];
        if (!token) return false;
        return p === last && k === phrase.length - 1
          ? token.term.startsWith(term)
          : token.term === term;
      });
      if (!fits) continue;
      found = true;
      const first = tokens[i];
      const end = tokens[i + phrase.length - 1];
      if (first && end) spans.push([first.start, end.end]);
    }
    if (!found) return null;
  }
  spans.sort((a, b) => a[0] - b[0]);
  const parts: TextPart[] = [];
  let at = 0;
  for (const [start, end] of spans) {
    if (end <= at) continue;
    const from = Math.max(start, at);
    if (from > at)
      parts.push({ text: text.slice(at, from), highlighted: false });
    parts.push({ text: text.slice(from, end), highlighted: true });
    at = end;
  }
  if (at < text.length)
    parts.push({ text: text.slice(at), highlighted: false });
  return parts;
}

export type SearchablePage = {
  id: string;
  title: Prepared;
  createdAt: number;
  blocks: readonly { id: string; text: Prepared }[];
};

export type MockHit = {
  page_id: string;
  title: TextPart[];
  block: { block_id: string; text: TextPart[] } | null;
};

export type Scope =
  | { kind: "all" }
  | { kind: "content"; exclude: ReadonlySet<string> };

const plain = (text: string): TextPart[] =>
  text === "" ? [] : [{ text, highlighted: false }];

/** The contract's page order: newest created first, then page ID. */
export function newestFirst(a: SearchablePage, b: SearchablePage): number {
  return b.createdAt - a.createdAt || (a.id < b.id ? -1 : a.id > b.id ? 1 : 0);
}

/**
 * Pages in `newestFirst` order with a term index over their bodies, so a
 * query visits only pages that hold every word, as FTS would.
 */
export class PageIndex {
  readonly ordered: readonly SearchablePage[];
  private readonly terms: string[];
  private readonly postings = new Map<string, number[]>();

  constructor(pages: readonly SearchablePage[]) {
    this.ordered = [...pages].sort(newestFirst);
    for (const [position, page] of this.ordered.entries()) {
      for (const block of page.blocks) {
        for (const token of block.text.tokens) {
          const list = this.postings.get(token.term);
          if (!list) this.postings.set(token.term, [position]);
          else if (list.at(-1) !== position) list.push(position);
        }
      }
    }
    this.terms = [...this.postings.keys()].sort();
  }

  private withPrefix(prefix: string): string[] {
    let low = 0;
    let high = this.terms.length;
    while (low < high) {
      const mid = (low + high) >> 1;
      if ((this.terms[mid] ?? "") < prefix) low = mid + 1;
      else high = mid;
    }
    const found: string[] = [];
    for (let i = low; i < this.terms.length; i++) {
      const term = this.terms[i] ?? "";
      if (!term.startsWith(prefix)) break;
      found.push(term);
    }
    return found;
  }

  /** Marks each page whose body holds every query word somewhere. */
  candidates(query: Query): Uint8Array {
    const groups = query.phrases.flatMap((phrase, p) =>
      phrase.map((term, k) =>
        p === query.phrases.length - 1 && k === phrase.length - 1
          ? this.withPrefix(term)
          : [term],
      ),
    );
    const counts = new Uint8Array(this.ordered.length);
    const seen = new Uint8Array(this.ordered.length);
    for (const [g, group] of groups.entries()) {
      seen.fill(0);
      for (const term of group) {
        for (const position of this.postings.get(term) ?? []) {
          if (seen[position] || counts[position] !== g) continue;
          seen[position] = 1;
          counts[position] = g + 1;
        }
      }
    }
    return counts.map((count) => (count === groups.length ? 1 : 0));
  }
}

/**
 * All scope lists title matches before body matches. Content scope skips the
 * excluded pages and always returns a block.
 */
export function searchPages(
  index: PageIndex,
  text: string,
  limit: number,
  scope: Scope,
): MockHit[] {
  const query = parseQuery(text);
  if (!query) return [];
  const clamped = Math.min(Math.max(limit, 1), 200);
  const hits: MockHit[] = [];
  const titled = new Set<string>();
  if (scope.kind === "all") {
    for (const page of index.ordered) {
      if (hits.length >= clamped) return hits;
      const title = matchText(query, page.title);
      if (!title) continue;
      titled.add(page.id);
      hits.push({ page_id: page.id, title, block: null });
    }
  }
  const candidates = index.candidates(query);
  for (const [position, page] of index.ordered.entries()) {
    if (hits.length >= clamped) break;
    if (!candidates[position] || titled.has(page.id)) continue;
    if (scope.kind === "content" && scope.exclude.has(page.id)) continue;
    const block = firstBlock(query, page);
    if (!block) continue;
    hits.push({
      page_id: page.id,
      title: matchText(query, page.title) ?? plain(page.title.text),
      block,
    });
  }
  return hits;
}

function firstBlock(query: Query, page: SearchablePage): MockHit["block"] {
  for (const block of page.blocks) {
    const parts = matchText(query, block.text);
    if (parts) return { block_id: block.id, text: parts };
  }
  return null;
}
