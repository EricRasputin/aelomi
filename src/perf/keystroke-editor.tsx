import { BlockNoteEditor } from "@blocknote/core";
import { blocksToYXmlFragment } from "@blocknote/core/yjs";
import { useEffect, useState } from "react";
import * as Y from "yjs";
import blocksJson from "../../fixtures/generated/page-5000-blocks.json?raw";
import { editorSchema } from "../editor/schema";
import { toBase64 } from "../ipc/base64";
import { PageFrame } from "../pages/page-frame";
import { usePageTreeStore } from "../stores/page-tree";

type Surface = { pageId: string; title: string; body: () => string | null };

/** The fixture page as the real BlockNote binding writes it. */
function fixtureBody(): string {
  const editor = BlockNoteEditor.create({ schema: editorSchema });
  const doc = new Y.Doc();
  blocksToYXmlFragment(
    editor,
    JSON.parse(blocksJson),
    doc.getXmlFragment("prosemirror"),
  );
  return toBase64(Y.encodeStateAsUpdate(doc));
}

/**
 * The product page view, seeded into the IPC mock: the real BlockNote editor
 * bound to the page's Y.Doc, sending every update through `docAppend`.
 */
function EditorSurface({ pageId, title, body }: Surface) {
  const [ready, setReady] = useState(false);
  useEffect(() => {
    const mock = window.__aelomiMock;
    if (!mock) throw new Error("Missing IPC mock");
    mock.workspace.seed([{ id: pageId, title }]);
    const raw = body();
    if (raw) mock.bodies.raw(pageId, raw);
    void usePageTreeStore
      .getState()
      .refresh()
      .then(() => setReady(true));
  }, [pageId, title, body]);
  return ready ? <PageFrame pageId={pageId} focusBlock={undefined} /> : null;
}

export function KeystrokeEditorEmpty() {
  return <EditorSurface pageId="perf-empty" title="Empty" body={() => null} />;
}

export function KeystrokeEditorPage5000() {
  return (
    <EditorSurface
      pageId="perf-page-5000"
      title="5,000 blocks"
      body={fixtureBody}
    />
  );
}
