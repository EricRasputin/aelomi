import { useEffect, useState } from "react";
import { commands, type Settings } from "../ipc";

const marker = "aelomi-dev-settings-probe";

export default function SettingsProbe() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    void commands.settingsGet().then((result) => {
      if (result.status === "ok") setSettings(result.data);
      else setError(JSON.stringify(result.error));
    });
  }, []);

  async function update() {
    setBusy(true);
    const result = await commands.settingsUpdate({
      theme: "Dark",
      open_page: {
        Set: { page_id: "probe-page", top_visible_block_id: "probe-block" },
      },
      recent_pages: ["probe-page", "older-page", "probe-page"],
      expanded_rows: ["probe-page", "parent-page"],
    });
    if (result.status === "ok") {
      setError(null);
      setSettings(result.data);
      await commands.devProbeLog(
        `${marker} updated ${JSON.stringify(result.data)}`,
      );
    } else setError(JSON.stringify(result.error));
    setBusy(false);
  }

  return (
    <section className="max-w-2xl space-y-4" data-probe={marker}>
      <h1>Settings probe</h1>
      <pre role="status" className="whitespace-pre-wrap break-all">
        {error ?? (settings ? JSON.stringify(settings) : "Loading settings")}
      </pre>
      <button
        type="button"
        disabled={busy || !settings}
        onClick={() => void update()}
      >
        Write session settings
      </button>
    </section>
  );
}
