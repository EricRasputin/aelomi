import { mkdirSync, writeFileSync } from "node:fs";
import { generateFixtures } from "./generator.ts";

const dir = new URL("./generated/", import.meta.url);
mkdirSync(dir, { recursive: true });
for (const [name, content] of Object.entries(generateFixtures())) {
  const bytes = Buffer.from(content, "utf8");
  writeFileSync(new URL(name, dir), bytes);
  console.log(`${name}\t${bytes.length} bytes`);
}
