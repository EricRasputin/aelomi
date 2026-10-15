import { commands } from "../ipc";
import {
  commandFailures,
  drainCommands,
  hasPendingCommands,
  setCommandAdmission,
} from "../ipc/pending-commands";
import { useQuitStore } from "../stores/quit";
import { drainDeliveries, hasOutstandingDeliveries } from "./delivery";

type OpenEditor = {
  /** Commits input still in flight, such as an IME composition. */
  finish: () => void;
  /** Stops accepting edits until the quit store opens again. */
  freeze: () => void;
};

const editors = new Set<OpenEditor>();

export function onQuitDrain(editor: OpenEditor): () => void {
  editors.add(editor);
  return () => {
    editors.delete(editor);
  };
}

async function drain(revision: number) {
  for (const editor of editors) editor.finish();
  // A committed composition reaches Yjs through ProseMirror's DOM observer;
  // let that run before collecting what is in flight.
  await new Promise((done) => setTimeout(done, 0));
  for (const editor of editors) editor.freeze();
  let failed: string[];
  do {
    const [deliveries] = await Promise.all([
      drainDeliveries(),
      drainCommands(),
    ]);
    failed = deliveries;
  } while (hasOutstandingDeliveries() || hasPendingCommands());
  const quit = useQuitStore.getState();
  if (quit.decision.kind !== "draining" || quit.decision.revision !== revision)
    return;
  setCommandAdmission("completing");
  failed = [...new Set([...failed, ...commandFailures()])];
  const reply =
    failed.length === 0
      ? await commands.completeQuit(revision)
      : await commands.quitDrainFailed(revision, failed);
  if ("status" in reply) {
    if (reply.status === "ok") quit.receiveQuit(reply.data);
  } else quit.receiveQuit(reply);
}

/**
 * Answers each Draining revision once: every edit this webview painted is
 * handed to the store before the shell may flush and exit.
 */
export function connectQuitDrain(): () => void {
  let answered = 0;
  const run = () => {
    const { decision } = useQuitStore.getState();
    if (decision.kind === "open") setCommandAdmission("open");
    if (decision.kind !== "draining" || decision.revision <= answered) return;
    setCommandAdmission("open");
    answered = decision.revision;
    void drain(decision.revision).catch((error: unknown) => {
      console.error("Quit drain failed", error);
    });
  };
  run();
  return useQuitStore.subscribe(run);
}
