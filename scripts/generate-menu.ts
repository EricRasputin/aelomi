import { writeFileSync } from "node:fs";
import { serializeNativeMenu } from "../src/shortcuts.ts";

writeFileSync(
  new URL("../src-tauri/menu.json", import.meta.url),
  serializeNativeMenu(),
);
