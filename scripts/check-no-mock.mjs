import { readdir, readFile } from "node:fs/promises";
import { join } from "node:path";

const marker = "__aelomiMock";
const files = await readdir("dist", { recursive: true, withFileTypes: true });
let checked = 0;
for (const file of files) {
  if (!file.isFile()) continue;
  const path = join(file.parentPath, file.name);
  const content = await readFile(path);
  if (content.includes(marker))
    throw new Error(`Mock marker ${marker} in ${path}`);
  checked += 1;
}
if (checked === 0) throw new Error("No production files found in dist");
console.log(`Checked ${checked} production files. No IPC mock marker found.`);
