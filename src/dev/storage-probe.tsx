import { useEffect, useState } from "react";
import * as Y from "yjs";
import { commands, events } from "../ipc";
import { fromBase64, toBase64 } from "../ipc/base64";

const PROBE_MARKER = "aelomi-dev-storage-probe";

async function load(docId: string): Promise<Uint8Array> {
  const result = await commands.docLoad(docId);
  if (result.status === "error")
    throw new Error(`doc_load ${JSON.stringify(result.error)}`);
  return fromBase64(result.data);
}

async function append(docId: string, update: Uint8Array): Promise<void> {
  const result = await commands.docAppend(docId, toBase64(update));
  if (result.status === "error")
    throw new Error(`doc_append ${JSON.stringify(result.error)}`);
}

function docFrom(state: Uint8Array): Y.Doc {
  const doc = new Y.Doc();
  Y.applyUpdate(doc, state);
  return doc;
}

const ms = (value: number) => value.toFixed(1);

const median = (values: number[]) => {
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.floor(sorted.length / 2)] ?? Number.NaN;
};

async function runProbe(): Promise<string> {
  const doc = docFrom(await load("probe"));
  const markers = doc.getArray<string>("markers");
  const before = markers.toArray();
  const marker = `${PROBE_MARKER}-${Date.now().toString(36)}`;
  const known = Y.encodeStateVector(doc);
  markers.push([marker]);
  const start = performance.now();
  await append("probe", Y.encodeStateAsUpdate(doc, known));
  const appended = performance.now();
  const after = docFrom(await load("probe"));
  const loaded = performance.now();
  const present = after.getArray<string>("markers").toArray().includes(marker);
  return `storage probe present=${present} marker=${marker} before=[${before.join(",")}] append_ms=${ms(appended - start)} load_ms=${ms(loaded - appended)}`;
}

async function measureTransport(): Promise<string> {
  let state = await load("probe-1mb");
  if (state.length < 1_000_000) {
    const doc = docFrom(state);
    const known = Y.encodeStateVector(doc);
    doc.getText("text").insert(0, "x".repeat(1_000_000));
    await append("probe-1mb", Y.encodeStateAsUpdate(doc, known));
    state = await load("probe-1mb");
  }
  const loads: number[] = [];
  for (let i = 0; i < 20; i++) {
    const start = performance.now();
    await load("probe-1mb");
    loads.push(performance.now() - start);
  }
  const small = new Y.Doc();
  const text = small.getText("text");
  const appends: number[] = [];
  let bytes = 0;
  for (let i = 0; i < 50; i++) {
    const known = Y.encodeStateVector(small);
    text.insert(text.length, "y".repeat(80));
    const update = Y.encodeStateAsUpdate(small, known);
    bytes = update.length;
    const start = performance.now();
    await append("probe-append", update);
    appends.push(performance.now() - start);
  }
  return `storage transport load_bytes=${state.length} load_ms_median=${ms(median(loads))} append_bytes=${bytes} append_ms_median=${ms(median(appends))}`;
}

export default function StorageProbe() {
  const [text, setText] = useState("Storage probe idle");
  const [busy, setBusy] = useState(false);
  const [heard, setHeard] = useState("No storage events yet");

  useEffect(() => {
    const hear = (line: string) => {
      setHeard(line);
      void commands.devProbeLog(line);
    };
    const unlisten = [
      events.storageHealthChanged.listen(({ payload }) =>
        hear(`storage event health=${JSON.stringify(payload)}`),
      ),
      events.quitStatusChanged.listen(({ payload }) =>
        hear(`storage event quit_status=${JSON.stringify(payload)}`),
      ),
    ];
    return () => {
      for (const stop of unlisten) void stop.then((f) => f());
    };
  }, []);

  const run = (probe: () => Promise<string>) => async () => {
    setBusy(true);
    let line: string;
    try {
      line = await probe();
    } catch (error: unknown) {
      line = `storage probe failed: ${error instanceof Error ? error.message : String(error)}`;
    }
    setText(line);
    setBusy(false);
    await commands.devProbeLog(line);
  };

  return (
    <section className="w-full max-w-2xl space-y-4">
      <p role="status" className="text-sm break-all tabular-nums">
        {text}
      </p>
      <p className="text-sm break-all text-[var(--muted)]">{heard}</p>
      <div className="flex gap-2">
        <button
          type="button"
          disabled={busy}
          onClick={run(runProbe)}
          className="rounded-md border border-[var(--border)] bg-[var(--button)] px-3 py-1.5 text-sm font-medium hover:bg-[var(--button-hover)] disabled:opacity-50"
        >
          Run probe
        </button>
        <button
          type="button"
          disabled={busy}
          onClick={run(measureTransport)}
          className="rounded-md border border-[var(--border)] bg-[var(--button)] px-3 py-1.5 text-sm font-medium hover:bg-[var(--button-hover)] disabled:opacity-50"
        >
          Measure transport
        </button>
      </div>
    </section>
  );
}
