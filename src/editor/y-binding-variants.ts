import { createHash } from "node:crypto";
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  realpathSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import type { EditorState, Plugin } from "prosemirror-state";

/**
 * Test support: loads y-prosemirror's sync and undo plugins twice, as the
 * pinned upstream 1.3.7 source and as the patched source the app runs, so a
 * test can drive both through the same edits. Both copies import the
 * installed yjs, lib0, ProseMirror and y-prosemirror helpers, so documents
 * and nodes cross between them.
 */
export type BindingModule = {
  ySyncPlugin: (type: unknown) => Plugin;
  yUndoPlugin: () => Plugin;
  undo: (state: EditorState) => boolean;
  redo: (state: EditorState) => boolean;
  getRelativeSelection: (
    binding: unknown,
    state: EditorState,
  ) => { type: unknown; anchor: unknown; head: unknown };
};

/** SHA-256 of `src/plugins/sync-plugin.js` in the y-prosemirror 1.3.7 tarball. */
export const upstreamSyncPluginSha256 =
  "2525b13d7b9cae04e8e81fc59674493dddb5017b68f3097a76994ea65ff264f8";

const repo = resolve(dirname(fileURLToPath(import.meta.url)), "../..");

export function repoFile(path: string): string {
  return join(repo, path);
}

function packageDir(): string {
  return realpathSync(join(repo, "node_modules/y-prosemirror"));
}

type Hunk = { before: string; after: string };

function hunksFor(patch: string, file: string): Hunk[] {
  const sections = patch.split(/^diff --git /m).slice(1);
  const section = sections.find((s) => s.startsWith(`a/${file} b/${file}`));
  if (!section) throw new Error(`the patch does not touch ${file}`);
  const hunks: Hunk[] = [];
  for (const chunk of section.split(/^@@[^\n]*@@[^\n]*\n/m).slice(1)) {
    const before: string[] = [];
    const after: string[] = [];
    for (const line of chunk.split("\n")) {
      if (line.startsWith("\\")) continue;
      const mark = line[0];
      const text = line.slice(1);
      if (mark === " " || line === "") {
        before.push(text);
        after.push(text);
      } else if (mark === "-") before.push(text);
      else if (mark === "+") after.push(text);
    }
    while (before.at(-1) === "" && after.at(-1) === "") {
      before.pop();
      after.pop();
    }
    hunks.push({ before: before.join("\n"), after: after.join("\n") });
  }
  return hunks;
}

function swapOnce(source: string, from: string, to: string): string {
  const at = source.indexOf(from);
  if (at < 0 || source.indexOf(from, at + 1) >= 0)
    throw new Error("a patch hunk does not match the installed source once");
  return source.slice(0, at) + to + source.slice(at + from.length);
}

export function sha256(text: string): string {
  return createHash("sha256").update(text).digest("hex");
}

export function syncPluginSources(): { upstream: string; patched: string } {
  const patched = readFileSync(
    join(packageDir(), "src/plugins/sync-plugin.js"),
    "utf8",
  );
  const patch = readFileSync(
    join(repo, "patches/y-prosemirror@1.3.7.patch"),
    "utf8",
  );
  const hunks = hunksFor(patch, "src/plugins/sync-plugin.js");
  let upstream = patched;
  for (const hunk of hunks)
    upstream = swapOnce(upstream, hunk.after, hunk.before);
  let reapplied = upstream;
  for (const hunk of hunks)
    reapplied = swapOnce(reapplied, hunk.before, hunk.after);
  if (reapplied !== patched)
    throw new Error("the committed patch does not rebuild the installed file");
  return { upstream, patched };
}

/** Loads both copies. Call `dispose` to remove the temporary files. */
export async function loadBindings(): Promise<{
  upstream: BindingModule;
  patched: BindingModule;
  dispose: () => void;
}> {
  const pkg = packageDir();
  const cache = join(repo, "node_modules/.cache");
  mkdirSync(cache, { recursive: true });
  const root = mkdtempSync(join(cache, "aelomi-y-binding-"));
  symlinkSync(dirname(pkg), join(root, "node_modules"), "dir");
  const absolute = (path: string) => pathToFileURL(join(pkg, path)).href;
  const relink = (source: string) =>
    source
      .replaceAll(
        "from './keys.js'",
        `from '${absolute("src/plugins/keys.js")}'`,
      )
      .replaceAll("from '../lib.js'", `from '${absolute("src/lib.js")}'`)
      .replaceAll("from '../utils.js'", `from '${absolute("src/utils.js")}'`);
  const undoSource = readFileSync(
    join(pkg, "src/plugins/undo-plugin.js"),
    "utf8",
  );
  const sources = syncPluginSources();
  const override = process.env.AELOMI_Y_BINDING_PATCHED_SOURCE;
  if (override) sources.patched = readFileSync(override, "utf8");
  const load = async (name: "upstream" | "patched") => {
    const dir = join(root, name);
    mkdirSync(dir);
    writeFileSync(join(dir, "sync-plugin.js"), relink(sources[name]));
    writeFileSync(join(dir, "undo-plugin.js"), relink(undoSource));
    const sync = await import(/* @vite-ignore */ join(dir, "sync-plugin.js"));
    const undo = await import(/* @vite-ignore */ join(dir, "undo-plugin.js"));
    return {
      ySyncPlugin: sync.ySyncPlugin,
      getRelativeSelection: sync.getRelativeSelection,
      yUndoPlugin: undo.yUndoPlugin,
      undo: undo.undo,
      redo: undo.redo,
    } as BindingModule;
  };
  const upstream = await load("upstream");
  const patched = await load("patched");
  return {
    upstream,
    patched,
    dispose: () => rmSync(root, { recursive: true, force: true }),
  };
}
