import { spawnSync } from "node:child_process";
import { loadavg } from "node:os";

const start = Date.now();
for (;;) {
  const load1 = loadavg()[0] ?? 0;
  const elapsed = Math.floor((Date.now() - start) / 1000);
  console.log(`t+${elapsed}s load1 ${load1.toFixed(2)}`);
  if (load1 < 2.5) break;
  if (elapsed >= 120) {
    console.log("load stayed above 2.5 for 120 s; measuring anyway");
    break;
  }
  spawnSync("sleep", ["10"]);
}
