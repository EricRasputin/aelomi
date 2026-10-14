export type Prng = {
  int: (bound: number) => number;
  chance: (percent: number) => boolean;
  pick: <T>(items: readonly T[]) => T;
};

function xmur3(text: string): () => number {
  let h = 1779033703 ^ text.length;
  for (let i = 0; i < text.length; i++) {
    h = Math.imul(h ^ text.charCodeAt(i), 3432918353);
    h = (h << 13) | (h >>> 19);
  }
  return () => {
    h = Math.imul(h ^ (h >>> 16), 2246822507);
    h = Math.imul(h ^ (h >>> 13), 3266489909);
    h ^= h >>> 16;
    return h >>> 0;
  };
}

function sfc32(a: number, b: number, c: number, d: number): () => number {
  return () => {
    a >>>= 0;
    b >>>= 0;
    c >>>= 0;
    d >>>= 0;
    let t = (a + b) | 0;
    a = b ^ (b >>> 9);
    b = (c + (c << 3)) | 0;
    c = (c << 21) | (c >>> 11);
    d = (d + 1) | 0;
    t = (t + d) | 0;
    c = (c + t) | 0;
    return t >>> 0;
  };
}

export function seeded(stream: string): Prng {
  const seed = xmur3(`aelomi-fixtures-v1:${stream}`);
  const u32 = sfc32(seed(), seed(), seed(), seed());
  for (let i = 0; i < 16; i++) u32();
  const int = (bound: number) => u32() % bound;
  return {
    int,
    chance: (percent) => int(100) < percent,
    pick: (items) => {
      const item = items[int(items.length)];
      if (item === undefined) throw new Error("pick from an empty list");
      return item;
    },
  };
}

const consonants = "bdfghjklmnprstvwz";
const vowels = "aeiou";

export function vocabulary(prng: Prng, size: number): string[] {
  const words = new Set<string>();
  while (words.size < size) {
    const syllables = 1 + (prng.chance(20) ? 0 : prng.chance(60) ? 1 : 2);
    let word = "";
    for (let i = 0; i < syllables; i++) {
      word += consonants[prng.int(consonants.length)];
      word += vowels[prng.int(vowels.length)];
    }
    words.add(word);
  }
  return [...words];
}

export function zipf(prng: Prng, words: readonly string[]): () => string {
  const cumulative: number[] = [];
  let total = 0;
  words.forEach((_, rank) => {
    total += Math.floor(1_000_000 / (rank + 1));
    cumulative.push(total);
  });
  return () => {
    const target = prng.int(total);
    let low = 0;
    let high = cumulative.length - 1;
    while (low < high) {
      const mid = (low + high) >> 1;
      if ((cumulative[mid] ?? 0) > target) high = mid;
      else low = mid + 1;
    }
    return words[low] ?? "";
  };
}

export function uuid(prng: Prng): string {
  const hex = () => prng.int(16).toString(16);
  const run = (count: number) => Array.from({ length: count }, hex).join("");
  const variant = (8 + prng.int(4)).toString(16);
  return `${run(8)}-${run(4)}-4${run(3)}-${variant}${run(3)}-${run(12)}`;
}

const capitalize = (word: string) =>
  word.charAt(0).toUpperCase() + word.slice(1);

export function sentence(
  word: () => string,
  prng: Prng,
  min: number,
  max: number,
): string {
  const count = min + prng.int(max - min + 1);
  const words = Array.from({ length: count }, word);
  return `${capitalize(words.join(" "))}.`;
}

export type Page = {
  id: string;
  title: string;
  parentId: string | null;
  trash: { batchId: string; at: number } | null;
  body: string;
};

export type ExpectedQuery = { query: string; expectedIds: string[] };

const PAGE_COUNT = 10_000;
const MAX_DEPTH = 5;
const TRASHED_PAGES = 300;
const EPOCH_MS = 1_700_000_000_000;

export const plantedTokens = {
  zorvak: { active: 1, trashed: 0 },
  quillmere: { active: 7, trashed: 2 },
  kestrelmoss: { active: 3, trashed: 0 },
  kestrelwind: { active: 1, trashed: 0 },
} as const;

export function generatePages(): { pages: Page[]; queries: ExpectedQuery[] } {
  const prng = seeded("search");
  const word = zipf(prng, vocabulary(prng, 4_000));
  const pages: Page[] = [];
  const depth: number[] = [];
  const children: number[][] = [];
  for (let i = 0; i < PAGE_COUNT; i++) {
    let parent = -1;
    if (i > 0 && !prng.chance(2)) {
      for (let attempt = 0; attempt < 8; attempt++) {
        const candidate = prng.int(i);
        if ((depth[candidate] ?? 0) < MAX_DEPTH - 1) {
          parent = candidate;
          break;
        }
      }
    }
    const titleWords = Array.from({ length: 1 + prng.int(4) }, word);
    const paragraphs = Array.from({ length: 2 + prng.int(3) }, () =>
      sentence(word, prng, 6, 18),
    );
    pages.push({
      id: uuid(prng),
      title: titleWords.map(capitalize).join(" "),
      parentId: parent === -1 ? null : (pages[parent]?.id ?? null),
      trash: null,
      body: paragraphs.join("\n"),
    });
    depth.push(parent === -1 ? 0 : (depth[parent] ?? 0) + 1);
    children.push([]);
    if (parent !== -1) children[parent]?.push(i);
  }

  const subtree = (root: number): number[] => {
    const found: number[] = [];
    const stack = [root];
    while (stack.length > 0) {
      const index = stack.pop();
      if (index === undefined) break;
      found.push(index);
      stack.push(...(children[index] ?? []));
    }
    return found;
  };
  let trashed = 0;
  for (let attempt = 0; attempt < 1_000 && trashed < TRASHED_PAGES; attempt++) {
    const root = prng.int(PAGE_COUNT);
    const members = subtree(root);
    if (members.length > 60 || members.some((index) => pages[index]?.trash))
      continue;
    const trash = {
      batchId: uuid(prng),
      at: EPOCH_MS + prng.int(90) * 86_400_000,
    };
    for (const index of members) {
      const page = pages[index];
      if (page) page.trash = trash;
    }
    trashed += members.length;
  }

  const used = new Set<number>();
  const plant = (token: string, trashed: boolean): string => {
    for (;;) {
      const index = prng.int(PAGE_COUNT);
      const page = pages[index];
      if (!page || used.has(index) || (page.trash !== null) !== trashed)
        continue;
      used.add(index);
      const paragraphs = page.body.split("\n");
      const at = prng.int(paragraphs.length);
      const words = (paragraphs[at] ?? "").split(" ");
      words.splice(1 + prng.int(words.length - 1), 0, token);
      paragraphs[at] = words.join(" ");
      page.body = paragraphs.join("\n");
      return page.id;
    }
  };
  const expected = new Map<string, string[]>();
  for (const [token, counts] of Object.entries(plantedTokens)) {
    const ids = Array.from({ length: counts.active }, () =>
      plant(token, false),
    );
    for (let i = 0; i < counts.trashed; i++) plant(token, true);
    expected.set(token, ids);
  }
  const sorted = (ids: readonly string[]) => [...ids].sort();
  const queries: ExpectedQuery[] = [
    { query: "zorvak", expectedIds: sorted(expected.get("zorvak") ?? []) },
    {
      query: "quillmere",
      expectedIds: sorted(expected.get("quillmere") ?? []),
    },
    {
      query: "kestrel",
      expectedIds: sorted([
        ...(expected.get("kestrelmoss") ?? []),
        ...(expected.get("kestrelwind") ?? []),
      ]),
    },
    { query: "nonesuchword", expectedIds: [] },
  ];
  return { pages, queries };
}

export type Styles = Partial<
  Record<"bold" | "italic" | "underline" | "strike" | "code", true>
>;
export type StyledText = { type: "text"; text: string; styles: Styles };
export type Link = { type: "link"; content: StyledText[]; href: string };
export type InlineContent = StyledText | Link;
type DefaultProps = {
  backgroundColor: "default";
  textColor: "default";
  textAlignment: "left";
};
type Nested = { id: string; children: Block[] };
export type Block = Nested &
  (
    | {
        type: "paragraph" | "bulletListItem";
        props: DefaultProps;
        content: InlineContent[];
      }
    | {
        type: "heading";
        props: DefaultProps & { level: 1 | 2 | 3; isToggleable: false };
        content: InlineContent[];
      }
    | {
        type: "numberedListItem";
        props: DefaultProps & { start?: number };
        content: InlineContent[];
      }
    | {
        type: "checkListItem";
        props: DefaultProps & { checked: boolean };
        content: InlineContent[];
      }
    | {
        type: "quote";
        props: { backgroundColor: "default"; textColor: "default" };
        content: InlineContent[];
      }
    | {
        type: "codeBlock";
        props: { language: string };
        content: StyledText[];
      }
    | { type: "divider"; props: Record<never, never> }
    | {
        type: "image";
        props: {
          backgroundColor: "default";
          textAlignment: "left";
          name: string;
          url: string;
          caption: string;
          showPreview: true;
        };
      }
  );

const BLOCK_COUNT = 5_000;
const defaultProps: DefaultProps = {
  backgroundColor: "default",
  textColor: "default",
  textAlignment: "left",
};
const styleNames = ["bold", "italic", "underline", "strike", "code"] as const;
const languages = ["text", "typescript", "rust", "json", "bash"];
type ListType = "bulletListItem" | "numberedListItem" | "checkListItem";
const listTypes: ListType[] = [
  "bulletListItem",
  "numberedListItem",
  "checkListItem",
];

export function generateBlocks(): Block[] {
  const prng = seeded("blocks");
  const word = zipf(prng, vocabulary(prng, 3_000));
  const inline = (min: number, max: number): InlineContent[] => {
    const runs = 1 + (prng.chance(70) ? 0 : prng.chance(60) ? 1 : 2);
    const content: InlineContent[] = [];
    for (let i = 0; i < runs; i++) {
      const text = `${i === 0 ? "" : " "}${sentence(word, prng, min, max)}`;
      const styles: Styles = prng.chance(15)
        ? { [prng.pick(styleNames)]: true }
        : {};
      if (prng.chance(3)) {
        // BlockNote's code mark excludes link, so linked text drops the code
        // style it drew. The draws stay the same, so nothing else moves.
        content.push({
          type: "link",
          content: [{ type: "text", text, styles: styles.code ? {} : styles }],
          href: `https://example.com/${word()}`,
        });
      } else {
        content.push({ type: "text", text, styles });
      }
    }
    return content;
  };
  const listItem = (type: ListType): Block => {
    const id = uuid(prng);
    const content = inline(3, 12);
    if (type === "checkListItem")
      return {
        id,
        type,
        props: { ...defaultProps, checked: prng.chance(40) },
        content,
        children: [],
      };
    if (type === "numberedListItem")
      return { id, type, props: { ...defaultProps }, content, children: [] };
    return { id, type, props: { ...defaultProps }, content, children: [] };
  };
  const leaf = (): Block => {
    const id = uuid(prng);
    const roll = prng.int(100);
    if (roll < 47)
      return {
        id,
        type: "paragraph",
        props: { ...defaultProps },
        content: inline(4, 14),
        children: [],
      };
    if (roll < 55) {
      const level = (1 + prng.int(3)) as 1 | 2 | 3;
      return {
        id,
        type: "heading",
        props: { ...defaultProps, level, isToggleable: false },
        content: inline(2, 6),
        children: [],
      };
    }
    if (roll < 85) return listItem(prng.pick(listTypes));
    if (roll < 90) {
      const lines = Array.from(
        { length: 1 + prng.int(3) },
        () => `let ${word()} = ${prng.int(1000)};`,
      );
      return {
        id,
        type: "codeBlock",
        props: { language: prng.pick(languages) },
        content: [{ type: "text", text: lines.join("\n"), styles: {} }],
        children: [],
      };
    }
    if (roll < 95)
      return {
        id,
        type: "quote",
        props: { backgroundColor: "default", textColor: "default" },
        content: inline(4, 14),
        children: [],
      };
    if (roll < 98) {
      const name = `${uuid(prng).replaceAll("-", "")}.png`;
      const caption = prng.chance(50) ? sentence(word, prng, 1, 5) : "";
      return {
        id,
        type: "image",
        props: {
          backgroundColor: "default",
          textAlignment: "left",
          name,
          url: `asset://${name}`,
          caption,
          showPreview: true,
        },
        children: [],
      };
    }
    return { id, type: "divider", props: {}, children: [] };
  };
  const blocks: Block[] = [];
  let count = 0;
  while (count < BLOCK_COUNT) {
    const block = leaf();
    count += 1;
    if (listTypes.some((type) => type === block.type) && prng.chance(20)) {
      const wanted = Math.min(1 + prng.int(4), BLOCK_COUNT - count);
      for (let i = 0; i < wanted; i++)
        block.children.push(listItem(block.type as ListType));
      count += wanted;
    }
    blocks.push(block);
  }
  return blocks;
}

export type FixtureFiles = Record<string, string>;

export function generateFixtures(): FixtureFiles {
  const { pages, queries } = generatePages();
  const blocks = generateBlocks();
  return {
    "search-10k-pages.jsonl": `${pages.map((page) => JSON.stringify(page)).join("\n")}\n`,
    "search-10k-expected.json": `${JSON.stringify({ queries }, null, 2)}\n`,
    "page-5000-blocks.json": `[\n${blocks.map((block) => JSON.stringify(block)).join(",\n")}\n]\n`,
  };
}
