import { Button } from "@base-ui/react/button";
import { useCallback, useEffect, useState } from "react";
import { Sidebar } from "../components/sidebar";
import type { SeedPage } from "../ipc/mock-workspace";
import { PageFrame } from "../pages/page-frame";
import { usePageTreeStore } from "../stores/page-tree";
import { useShellStore } from "../stores/shell";
import { useTreeExpansionStore } from "../stores/tree-expansion";

const titles = [
  "Quarterly planning notes for the platform infrastructure and product design teams",
  "https://example.com/workspaces/acme/projects/q3-launch/docs/9f8e7d6c5b4a?tab=comments",
  "",
  "J",
  "Đặng Thị Ngọc Hân",
  "王秀英",
  "نور الهدى عبد الرحمن",
  "👩🏽‍💻 Project notes",
  "<script>alert(1)</script>",
  "Line one\nLine two",
  "   ",
];
const demo: SeedPage[] = [
  { id: "demo-projects", title: "Projects" },
  { id: "demo-notes", title: "Notes", parentId: "demo-projects" },
];
const worst: SeedPage[] = titles.map((title, index) => ({
  id: `stress-${index}`,
  title,
}));
for (let i = 0; i < 40; i++)
  worst.push({
    id: `deep-${i}`,
    title: `Nested project ${i + 1}`,
    parentId: i ? `deep-${i - 1}` : null,
  });
const datasets = {
  Demo: demo,
  "Worst case": worst,
  Empty: [],
  One: [{ id: "one", title: "" }],
  "10,000 rows": Array.from({ length: 10000 }, (_, i) => ({
    id: `many-${i}`,
    title: `Project notes ${i + 1}`,
  })),
} satisfies Record<string, SeedPage[]>;

export default function SidebarStress() {
  const [pageId, setPageId] = useState<string | null>(null);
  const [selected, setSelected] = useState("Demo");
  const load = useCallback((label: string, pages: SeedPage[]) => {
    window.__aelomiMock?.reset();
    window.__aelomiMock?.workspace.seed(pages);
    useTreeExpansionStore.setState({
      expansion: { kind: "ready", rows: new Set(pages.map((page) => page.id)) },
    });
    void usePageTreeStore
      .getState()
      .refresh()
      .then(() => {
        setPageId(pages[0]?.id ?? null);
      });
    setSelected(label);
  }, []);
  useEffect(() => {
    load("Demo", demo);
  }, [load]);
  useEffect(() => {
    useShellStore.setState({ activePageId: pageId });
  }, [pageId]);
  return (
    <>
      <div className="workspace">
        <Sidebar openPage={setPageId} />
        <div className="editor-pane">
          {pageId ? (
            <PageFrame key={pageId} pageId={pageId} focusBlock={undefined} />
          ) : (
            <section className="empty-state">
              <h1>No page open</h1>
              <p>Start with a new page.</p>
            </section>
          )}
        </div>
      </div>
      <fieldset
        aria-label="Stress data"
        style={{
          position: "fixed",
          bottom: 12,
          left: "50%",
          transform: "translateX(-50%)",
          display: "flex",
          flexWrap: "wrap",
          gap: 4,
          padding: 6,
          borderRadius: 8,
          background: "var(--surface)",
          border: "1px solid var(--border)",
          zIndex: 10,
        }}
      >
        {Object.entries(datasets).map(([label, pages]) => (
          <Button
            className="shell-button"
            key={label}
            aria-pressed={selected === label}
            onClick={() => load(label, pages)}
          >
            {label}
          </Button>
        ))}
      </fieldset>
    </>
  );
}
