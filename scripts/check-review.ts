import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";

type Review = {
  id: number;
  body: string | null;
  submitted_at: string | null;
};

type Verdict = {
  id: number;
  verdict: "APPROVE" | "CHANGES REQUESTED";
  head: string;
  submittedAt: number;
};

export function checkReview(
  reviews: readonly Review[],
  head: string,
): { pass: boolean; reason: string } {
  let latest: Verdict | undefined;
  for (const review of reviews) {
    const [first, second] = (review.body ?? "").split(/\r?\n/);
    if (
      first !== "Reviewer verdict: APPROVE" &&
      first !== "Reviewer verdict: CHANGES REQUESTED"
    ) {
      continue;
    }
    const sha = second?.match(/^Head: ([0-9a-fA-F]{40})$/)?.[1];
    if (!sha || !review.submitted_at) continue;
    const candidate: Verdict = {
      id: review.id,
      verdict:
        first === "Reviewer verdict: APPROVE" ? "APPROVE" : "CHANGES REQUESTED",
      head: sha,
      submittedAt: Date.parse(review.submitted_at),
    };
    if (
      !latest ||
      candidate.submittedAt > latest.submittedAt ||
      (candidate.submittedAt === latest.submittedAt && candidate.id > latest.id)
    ) {
      latest = candidate;
    }
  }

  const found = latest
    ? `Latest reviewer verdict: ${latest.verdict} for head ${latest.head}.`
    : "No reviewer verdict found.";
  if (latest?.verdict === "APPROVE" && latest.head === head) {
    return { pass: true, reason: `${found} PASS.` };
  }
  return {
    pass: false,
    reason: `${found} FAIL; run the reviewer agent on head ${head}.`,
  };
}

function isReview(raw: unknown): raw is Review {
  return (
    typeof raw === "object" &&
    raw !== null &&
    "id" in raw &&
    typeof raw.id === "number" &&
    Number.isSafeInteger(raw.id) &&
    "body" in raw &&
    (raw.body === null || typeof raw.body === "string") &&
    "submitted_at" in raw &&
    (raw.submitted_at === null ||
      (typeof raw.submitted_at === "string" &&
        Number.isFinite(Date.parse(raw.submitted_at))))
  );
}

function main(): void {
  const repository = process.env.GITHUB_REPOSITORY;
  const eventPath = process.env.GITHUB_EVENT_PATH;
  if (!repository || !/^[\w.-]+\/[\w.-]+$/.test(repository) || !eventPath) {
    throw new Error("Set GITHUB_REPOSITORY and GITHUB_EVENT_PATH.");
  }
  const event: unknown = JSON.parse(readFileSync(eventPath, "utf8"));
  if (
    typeof event !== "object" ||
    event === null ||
    !("pull_request" in event) ||
    typeof event.pull_request !== "object" ||
    event.pull_request === null
  ) {
    throw new Error("Event must contain a pull_request.");
  }
  const pr = event.pull_request;
  if (
    !("number" in pr) ||
    typeof pr.number !== "number" ||
    !Number.isSafeInteger(pr.number) ||
    pr.number < 1 ||
    !("head" in pr) ||
    typeof pr.head !== "object" ||
    pr.head === null ||
    !("sha" in pr.head) ||
    typeof pr.head.sha !== "string" ||
    !/^[0-9a-fA-F]{40}$/.test(pr.head.sha)
  ) {
    throw new Error("Event must contain a PR number and a 40-hex head SHA.");
  }
  const pages: unknown = JSON.parse(
    execFileSync(
      "gh",
      [
        "api",
        "--paginate",
        "--slurp",
        `repos/${repository}/pulls/${pr.number}/reviews?per_page=100`,
      ],
      { encoding: "utf8" },
    ),
  );
  if (!Array.isArray(pages)) throw new Error("Expected review pages.");
  const reviews: Review[] = [];
  for (const page of pages) {
    if (!Array.isArray(page)) throw new Error("Expected a page of reviews.");
    for (const review of page) {
      if (!isReview(review)) throw new Error("Invalid review from GitHub.");
      reviews.push(review);
    }
  }
  const result = checkReview(reviews, pr.head.sha);
  console.log(result.reason);
  process.exitCode = result.pass ? 0 : 1;
}

if (import.meta.main) {
  try {
    main();
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  }
}
