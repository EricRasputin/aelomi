import type * as Y from "yjs";
import { writeRecords } from "./mock-workspace";

type FixturePage = { id: string; title: string; lines: string[] };

export async function loadSearchFixture(
  doc: Y.Doc,
  fixtureBodies: Map<string, string[]>,
) {
  const { default: jsonl } = await import(
    "../../fixtures/generated/search-10k-pages.jsonl?raw"
  );
  const pages: FixturePage[] = jsonl
    .split("\n")
    .filter((line) => line.trim() !== "")
    .map((line) => {
      const page = JSON.parse(line) as {
        id: string;
        title: string;
        parentId: string | null;
        trash: { batchId: string; trashedAt: number } | null;
        body: string;
      };
      fixtureBodies.set(page.id, page.body.split("\n"));
      return { id: page.id, title: page.title, lines: [] };
    });
  const parents = new Map<string, string | null>();
  const trash = new Map<
    string,
    { batchId: string; trashedAt: number } | null
  >();
  for (const line of jsonl.split("\n")) {
    if (line.trim() === "") continue;
    const page = JSON.parse(line) as {
      id: string;
      parentId: string | null;
      trash: { batchId: string; trashedAt: number } | null;
    };
    parents.set(page.id, page.parentId);
    trash.set(page.id, page.trash);
  }
  writeRecords(
    doc,
    pages.map((page, line) => {
      const trashed = trash.get(page.id) ?? null;
      return {
        id: page.id,
        title: page.title,
        parentId: parents.get(page.id) ?? null,
        order: line,
        createdAt: 1_700_000_000_000 + line * 1000,
        updatedAt: 1_700_000_000_000 + line * 1000,
        trash: trashed && {
          originalParentId: parents.get(page.id) ?? null,
          originalOrder: line,
          batchId: trashed.batchId,
          trashedAt: trashed.trashedAt,
        },
      };
    }),
  );
}
