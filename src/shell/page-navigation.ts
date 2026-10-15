import type { TreePage } from "../ipc";

export type VisiblePage = {
  page: TreePage;
  depth: number;
  hasChildren: boolean;
};

export function flattenPages(
  pages: readonly TreePage[],
  expanded: ReadonlySet<string>,
): VisiblePage[] {
  const children = new Map<string | null, TreePage[]>();
  for (const page of pages) {
    const siblings = children.get(page.parent_id) ?? [];
    siblings.push(page);
    children.set(page.parent_id, siblings);
  }
  const stack = (children.get(null) ?? [])
    .map((page) => ({ page, depth: 0 }))
    .reverse();
  const rows: VisiblePage[] = [];
  while (stack.length) {
    const row = stack.pop();
    if (!row) break;
    const nested = children.get(row.page.id) ?? [];
    rows.push({ ...row, hasChildren: nested.length > 0 });
    if (expanded.has(row.page.id)) {
      for (let i = nested.length - 1; i >= 0; i--) {
        const page = nested[i];
        if (page) stack.push({ page, depth: row.depth + 1 });
      }
    }
  }
  return rows;
}

export function pageAfterTrash({
  pageId,
  before,
  active,
  openedAt,
}: {
  pageId: string;
  before: readonly TreePage[];
  active: readonly TreePage[];
  openedAt: ReadonlyMap<string, number>;
}): string | null {
  const byId = new Map(before.map((page) => [page.id, page]));
  const activeIds = new Set(active.map((page) => page.id));
  const page = byId.get(pageId);
  if (page) {
    const next = before
      .slice(before.indexOf(page) + 1)
      .find(
        (candidate) =>
          candidate.parent_id === page.parent_id && activeIds.has(candidate.id),
      );
    if (next) return next.id;
    let ancestor = page.parent_id;
    const seen = new Set<string>();
    while (ancestor && !seen.has(ancestor)) {
      if (activeIds.has(ancestor)) return ancestor;
      seen.add(ancestor);
      ancestor = byId.get(ancestor)?.parent_id ?? null;
    }
  }
  let recent: string | null = null;
  let latest = -Infinity;
  for (const [id, time] of openedAt) {
    if (activeIds.has(id) && time > latest) {
      recent = id;
      latest = time;
    }
  }
  return recent;
}
