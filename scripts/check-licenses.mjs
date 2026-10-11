import { execFileSync } from "node:child_process";

const allowed = new Set([
  "MIT",
  "Apache-2.0",
  "Apache-2.0 WITH LLVM-exception",
  "BSD-2-Clause",
  "BSD-3-Clause",
  "ISC",
  "MPL-2.0",
]);
const exceptions = new Map([["Unlicense", new Set(["isbot"])]]);
const tree = JSON.parse(
  execFileSync("pnpm", ["licenses", "list", "--json"], {
    encoding: "utf8",
    maxBuffer: 16 * 1024 * 1024,
  }),
);

let checked = 0;
const failures = [];
for (const packages of Object.values(tree)) {
  for (const dependency of packages) {
    checked += 1;
    const { name, license } = dependency;
    const terms = license.replace(/[()]/g, "").split(/\s+(?:AND|OR)\s+/);
    if (
      name.startsWith("@blocknote/xl-") ||
      !terms.every(
        (term) => allowed.has(term) || exceptions.get(term)?.has(name),
      )
    ) {
      failures.push(`${name}: ${license}`);
    }
  }
}

if (checked === 0) throw new Error("No dependencies returned by pnpm licenses");
if (failures.length > 0) {
  throw new Error(`Disallowed dependencies:\n${failures.join("\n")}`);
}
console.log(`Checked ${checked} dependencies. All licenses allowed.`);
