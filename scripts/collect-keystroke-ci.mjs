import { execFile } from "node:child_process";
import { writeFileSync } from "node:fs";
import { promisify, stripVTControlCharacters } from "node:util";

const exec = promisify(execFile);
const [from, until, output] = process.argv.slice(2);
if (!from || !until || !output || !(Date.parse(from) < Date.parse(until))) {
  throw new Error(
    "Usage: node scripts/collect-keystroke-ci.mjs FROM UNTIL OUTPUT.json",
  );
}
const repository = "aelomi/aelomi";
async function gh(path, json = true) {
  const { stdout } = await exec(
    "gh",
    ["api", path, "--paginate", ...(json ? ["--slurp"] : [])],
    {
      maxBuffer: 32 * 1024 * 1024,
    },
  );
  return json ? JSON.parse(stdout) : stdout;
}
const number = (match, index = 1) => (match ? Number(match[index]) : null);
function measurements(log) {
  const lines = stripVTControlCharacters(log).split("\n");
  const waits = [...log.matchAll(/t\+(\d+)s load1 ([\d.]+)/g)];
  const first = waits.at(0);
  const last = waits.at(-1);
  const typing = {};
  for (const [i, line] of lines.entries()) {
    const surface = line.match(
      /\b(editor-empty|editor-page-5000-blocks|empty|page-5000-blocks|layout-delay): full key-to-paint p95/,
    );
    if (surface) {
      const block = lines.slice(i, i + 12).join("\n");
      typing[surface[1]] = {
        p95: number(block.match(/app-owned p95 ([\d.]+) ms/)),
        samples: number(block.match(/over (\d+) samples from 3 loads/)),
        load1: number(block.match(/load average ([\d.]+)/)),
      };
    }
  }
  const paletteLine =
    lines.find((line) => line.includes("palette results p50")) ?? "";
  const scrollIndex = lines.findIndex((line) =>
    /scroll frames \d+, rendering work/.test(line),
  );
  const scroll =
    scrollIndex < 0 ? "" : lines.slice(scrollIndex, scrollIndex + 6).join("\n");
  return {
    checkout:
      log.match(/git log -1 --format=%H\r?\n\S+ ([0-9a-f]{40})/)?.[1] ?? null,
    image: log.match(/Image Release: (\S+)/)?.[1] ?? null,
    wait:
      first && last
        ? {
            startLoad1: number(first, 2),
            endLoad1: number(last, 2),
            seconds: number(last),
          }
        : null,
    palette: {
      p95: number(paletteLine.match(/p95 ([\d.]+) ms/)),
      keys: number(paletteLine.match(/over (\d+) keys/)),
      load1: number(paletteLine.match(/load average ([\d.]+)/)),
    },
    scroll: {
      p95: number(scroll.match(/rendering work p50 [\d.]+ ms p95 ([\d.]+) ms/)),
      longest: number(scroll.match(/longest interval ([\d.]+) ms/)),
      frames: number(scroll.match(/scroll frames (\d+)/)),
      previousWork: number(scroll.match(/previous frame's work ([\d.-]+) ms/)),
      betweenFrames: number(
        scroll.match(/then ([\d.-]+) ms to the next stamp/),
      ),
      over33: number(scroll.match(/intervals over 33 ms (\d+)/)),
      load1: number(scroll.match(/load average ([\d.]+)/)),
    },
    typing,
    passed: number(log.match(/\b(\d+) passed \(/)),
    failed: number(log.match(/\b(\d+) failed\b/)),
  };
}
const query = new URLSearchParams({
  per_page: "100",
  created: `${from}..${until}`,
});
const pages = await gh(
  `repos/${repository}/actions/workflows/ci.yml/runs?${query}`,
);
const runs = pages
  .flatMap((page) => page.workflow_runs)
  .filter(
    (run) =>
      run.event === "pull_request" ||
      (run.event === "push" && run.head_branch === "main"),
  );
const attempts = runs.flatMap((run) =>
  Array.from({ length: run.run_attempt }, (_, i) => ({ run, attempt: i + 1 })),
);
const rows = [];
let next = 0;
await Promise.all(
  Array.from({ length: 4 }, async () => {
    while (next < attempts.length) {
      const { run, attempt } = attempts[next++];
      const base = {
        run: run.id,
        attempt,
        branch: run.head_branch,
        head: run.head_sha,
        event: run.event,
        created: run.created_at,
      };
      const jobs = (
        await gh(
          `repos/${repository}/actions/runs/${run.id}/attempts/${attempt}/jobs?per_page=100`,
        )
      )
        .flatMap((page) => page.jobs)
        .filter((job) => job.name === "keystroke");
      if (jobs.length === 0)
        rows.push({ ...base, job: null, status: "no-keystroke-job" });
      for (const job of jobs) {
        if (job.started_at > until) continue;
        const row = {
          ...base,
          job: job.id,
          head: job.head_sha,
          started: job.started_at,
          status: job.conclusion ?? job.status,
        };
        if (job.status !== "completed") {
          rows.push({ ...row, log: "not-completed" });
          continue;
        }
        try {
          const log = await gh(
            `repos/${repository}/actions/jobs/${job.id}/logs`,
            false,
          );
          rows.push({ ...row, log: "read", ...measurements(log) });
        } catch (error) {
          if (!/HTTP (404|410)/.test(error.message)) throw error;
          rows.push({ ...row, log: "unavailable" });
        }
      }
    }
  }),
);
rows.sort((a, b) => a.run - b.run || a.attempt - b.attempt);
writeFileSync(
  output,
  `${JSON.stringify({ repository, from, until, runs: runs.length, rows }, null, 2)}\n`,
);
console.log(
  `${runs.length} runs, ${rows.length} attempts written to ${output}`,
);
