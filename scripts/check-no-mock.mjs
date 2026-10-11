import { readdir, readFile } from "node:fs/promises";
import { join } from "node:path";

const markers = ["__aelomiMock", "aelomi-dev-storage-probe"];
const files = await readdir("dist", { recursive: true, withFileTypes: true });
let checked = 0;
for (const file of files) {
  if (!file.isFile()) continue;
  const path = join(file.parentPath, file.name);
  const content = await readFile(path);
  for (const marker of markers)
    if (content.includes(marker))
      throw new Error(`Dev or mock marker ${marker} in ${path}`);
  checked += 1;
}
if (checked === 0) throw new Error("No production files found in dist");
console.log(
  `Checked ${checked} production files. No dev or mock marker found.`,
);
