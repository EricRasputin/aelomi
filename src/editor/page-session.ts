import { BlockNoteEditor, createExtension } from "@blocknote/core";
import { withCollaboration } from "@blocknote/core/yjs";
import * as Y from "yjs";
import { commands, type StorageError } from "../ipc";
import { fromBase64 } from "../ipc/base64";
import { checkBody, type GuardProblem } from "./body-guard";
import { type Body, readShownBody, type ShownBlock } from "./body-reader";
import { pendingUpdates, trackLoad } from "./delivery";
import { localImageUrl } from "./image-url";
import { type PickerKey, pagePicker } from "./page-picker";
import { loadOrigin, syncPage } from "./page-sync";
import { onQuitDrain } from "./quit-drain";
import { editorSchema } from "./schema";

export type PageEditor = BlockNoteEditor<
  typeof editorSchema.blockSchema,
  typeof editorSchema.inlineContentSchema,
  typeof editorSchema.styleSchema
>;

export type ReadOnlyReason =
  | GuardProblem
  | { kind: "undecodable" }
  | { kind: "tripwire" };

export type PageSession =
  | {
      kind: "editable";
      editor: PageEditor;
      /** Mounts the editor in `host`. The returned unmount may run any number of times. */
      mount: (host: HTMLElement) => () => void;
      /** The opener is done with the page. */
      close: () => void;
    }
  | { kind: "readOnly"; reason: ReadOnlyReason; body: Body<ShownBlock> }
  | { kind: "error"; error: StorageError };

const userInputEvents = [
  "keydown",
  "beforeinput",
  "paste",
  "drop",
  "cut",
  "pointerdown",
] as const;

function readOnlyBody(raw: Uint8Array): Body<ShownBlock> {
  const doc = new Y.Doc();
  try {
    Y.applyUpdate(doc, raw, loadOrigin);
    return readShownBody(doc);
  } catch {
    return { blocks: [], problem: { kind: "Undecodable" } };
  } finally {
    doc.destroy();
  }
}

type OpenOptions = {
  pageId: string;
  onPickerKey: (key: PickerKey) => boolean;
  /** Called when the mount tripwire turns an open editor read-only. */
  onTrip: (session: PageSession) => void;
};

/**
 * Loads a page body and binds BlockNote to the raw Y.Doc. The guard runs
 * before the binding exists, so a body the editor would rewrite never mounts.
 * Every update except the load itself goes to the store at once. Until the
 * user's first input, an update that deletes anything is held back and the
 * page reopens read-only from the bytes it loaded.
 */
export async function openPage({
  pageId,
  onPickerKey,
  onTrip,
}: OpenOptions): Promise<PageSession> {
  const pending = pendingUpdates(pageId);
  const loaded = await trackLoad(commands.docLoad(pageId));
  if (loaded.status === "error") return { kind: "error", error: loaded.error };
  let raw = fromBase64(loaded.data);
  const ydoc = new Y.Doc();
  try {
    const recovered = [...pending, ...pendingUpdates(pageId)];
    if (recovered.length > 0) raw = Y.mergeUpdates([raw, ...recovered]);
    Y.applyUpdate(ydoc, raw, loadOrigin);
  } catch {
    ydoc.destroy();
    return {
      kind: "readOnly",
      reason: { kind: "undecodable" },
      body: { blocks: [], problem: { kind: "Undecodable" } },
    };
  }
  let userInput = false;
  const inputWatch = createExtension(() => ({
    key: "aelomiInputWatch",
    mount: ({ dom, signal }) => {
      const seen = () => {
        userInput = true;
      };
      for (const type of userInputEvents)
        dom.addEventListener(type, seen, { capture: true, signal });
    },
  }));
  const editor: PageEditor = BlockNoteEditor.create(
    withCollaboration({
      schema: editorSchema,
      trailingBlock: false,
      links: { onClick: () => true },
      resolveFileUrl: async (url) => localImageUrl(url),
      domAttributes: {
        editor: {
          role: "textbox",
          "aria-multiline": "true",
          "aria-label": "Page body",
          "data-testid": "page-editor",
        },
      },
      extensions: [inputWatch(), pagePicker({ onKey: onPickerKey })],
      collaboration: {
        fragment: ydoc.getXmlFragment("prosemirror"),
        user: { name: "Aelomi", color: "#6b6b73" },
      },
    }),
  );
  const check = checkBody(ydoc, editor.pmSchema);
  if (check.kind === "readOnly") {
    const body = readShownBody(ydoc);
    ydoc.destroy();
    return { kind: "readOnly", reason: check.problem, body };
  }
  const stopSync = syncPage({
    docId: pageId,
    ydoc,
    hasUserInput: () => userInput,
    onTrip: () =>
      queueMicrotask(() =>
        onTrip({
          kind: "readOnly",
          reason: { kind: "tripwire" },
          body: readOnlyBody(raw),
        }),
      ),
  });
  const stopDrain = onQuitDrain({
    finish: () => {
      const view = editor.prosemirrorView;
      if (view?.composing && view.dom instanceof HTMLElement) view.dom.blur();
    },
    freeze: () => {
      editor.isEditable = false;
    },
  });
  // The doc outlives the editor's view: whichever of close and unmount runs
  // last ends sync and destroys the doc, so a mounted editor never writes to
  // a destroyed doc, whatever order its owners release it in.
  let opened = true;
  let mounted = false;
  let ended = false;
  const end = () => {
    if (ended || opened || mounted) return;
    ended = true;
    stopDrain();
    stopSync();
    ydoc.destroy();
  };
  const mount = (host: HTMLElement) => {
    editor.mount(host);
    mounted = true;
    let active = true;
    return () => {
      if (!active) return;
      active = false;
      editor.unmount();
      mounted = false;
      end();
    };
  };
  const close = () => {
    opened = false;
    end();
  };
  return { kind: "editable", editor, mount, close };
}
