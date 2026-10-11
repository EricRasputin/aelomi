import { describe, expect, it } from "vitest";
import { buildStatusPayload, checkReview } from "./check-review.ts";

describe("checkReview", () => {
  it("fails when there are no reviews", () => {
    expect(checkReview([], "1111111111111111111111111111111111111111")).toEqual(
      {
        pass: false,
        reason:
          "No reviewer verdict found. FAIL; run the reviewer agent on head 1111111111111111111111111111111111111111.",
      },
    );
  });

  it("ignores reviews without the verdict format", () => {
    expect(
      checkReview(
        [
          {
            id: 1,
            author_association: "OWNER",
            body: "Looks good to me.",
            submitted_at: "2026-10-11T10:00:00Z",
          },
          {
            id: 2,
            author_association: "OWNER",
            body: null,
            submitted_at: "2026-10-11T11:00:00Z",
          },
        ],
        "1111111111111111111111111111111111111111",
      ),
    ).toEqual({
      pass: false,
      reason:
        "No reviewer verdict found. FAIL; run the reviewer agent on head 1111111111111111111111111111111111111111.",
    });
  });

  it("fails when approval is for an old head", () => {
    expect(
      checkReview(
        [
          {
            id: 1,
            author_association: "OWNER",
            body: "Reviewer verdict: APPROVE\nHead: 2222222222222222222222222222222222222222\n\nNo findings.",
            submitted_at: "2026-10-11T10:00:00Z",
          },
        ],
        "1111111111111111111111111111111111111111",
      ),
    ).toEqual({
      pass: false,
      reason:
        "Latest reviewer verdict: APPROVE for head 2222222222222222222222222222222222222222. FAIL; run the reviewer agent on head 1111111111111111111111111111111111111111.",
    });
  });

  it("passes with approval for the current head", () => {
    expect(
      checkReview(
        [
          {
            id: 1,
            author_association: "OWNER",
            body: "Reviewer verdict: APPROVE\nHead: 1111111111111111111111111111111111111111\n\nNo findings.",
            submitted_at: "2026-10-11T10:00:00Z",
          },
          {
            id: 2,
            author_association: "OWNER",
            body: "A follow-up without a verdict.",
            submitted_at: "2026-10-11T11:00:00Z",
          },
        ],
        "1111111111111111111111111111111111111111",
      ),
    ).toEqual({
      pass: true,
      reason:
        "Latest reviewer verdict: APPROVE for head 1111111111111111111111111111111111111111. PASS.",
    });
  });

  it("fails when CHANGES REQUESTED follows APPROVE", () => {
    expect(
      checkReview(
        [
          {
            id: 1,
            author_association: "OWNER",
            body: "Reviewer verdict: APPROVE\nHead: 1111111111111111111111111111111111111111",
            submitted_at: "2026-10-11T10:00:00Z",
          },
          {
            id: 2,
            author_association: "OWNER",
            body: "Reviewer verdict: CHANGES REQUESTED\nHead: 1111111111111111111111111111111111111111",
            submitted_at: "2026-10-11T11:00:00Z",
          },
        ],
        "1111111111111111111111111111111111111111",
      ),
    ).toEqual({
      pass: false,
      reason:
        "Latest reviewer verdict: CHANGES REQUESTED for head 1111111111111111111111111111111111111111. FAIL; run the reviewer agent on head 1111111111111111111111111111111111111111.",
    });
  });

  it("passes when APPROVE follows CHANGES REQUESTED, regardless of input order", () => {
    expect(
      checkReview(
        [
          {
            id: 1,
            author_association: "OWNER",
            body: "Reviewer verdict: APPROVE\nHead: 1111111111111111111111111111111111111111",
            submitted_at: "2026-10-11T11:00:00Z",
          },
          {
            id: 2,
            author_association: "OWNER",
            body: "Reviewer verdict: CHANGES REQUESTED\nHead: 1111111111111111111111111111111111111111",
            submitted_at: "2026-10-11T10:00:00Z",
          },
        ],
        "1111111111111111111111111111111111111111",
      ),
    ).toEqual({
      pass: true,
      reason:
        "Latest reviewer verdict: APPROVE for head 1111111111111111111111111111111111111111. PASS.",
    });
  });

  it.each([
    "Reviewer verdict: APPROVE\nHead: 1111111",
    "Reviewer verdict: APPROVE \nHead: 1111111111111111111111111111111111111111",
    "Reviewer verdict: APPROVE\nHead: 1111111111111111111111111111111111111111 ",
    "Reviewer verdict: APPROVE\nHead: z111111111111111111111111111111111111111",
    "Reviewer verdict: APPROVED\nHead: 1111111111111111111111111111111111111111",
    "Preface\nReviewer verdict: APPROVE\nHead: 1111111111111111111111111111111111111111",
    "Reviewer verdict: APPROVE\n\nHead: 1111111111111111111111111111111111111111",
  ])("ignores the malformed verdict body %j", (body) => {
    expect(
      checkReview(
        [
          {
            id: 1,
            author_association: "OWNER",
            body,
            submitted_at: "2026-10-11T10:00:00Z",
          },
        ],
        "1111111111111111111111111111111111111111",
      ),
    ).toEqual({
      pass: false,
      reason:
        "No reviewer verdict found. FAIL; run the reviewer agent on head 1111111111111111111111111111111111111111.",
    });
  });

  it("breaks timestamp ties by the larger review id", () => {
    expect(
      checkReview(
        [
          {
            id: 2,
            author_association: "OWNER",
            body: "Reviewer verdict: CHANGES REQUESTED\nHead: 1111111111111111111111111111111111111111",
            submitted_at: "2026-10-11T10:00:00Z",
          },
          {
            id: 1,
            author_association: "OWNER",
            body: "Reviewer verdict: APPROVE\nHead: 1111111111111111111111111111111111111111",
            submitted_at: "2026-10-11T10:00:00Z",
          },
        ],
        "1111111111111111111111111111111111111111",
      ),
    ).toEqual({
      pass: false,
      reason:
        "Latest reviewer verdict: CHANGES REQUESTED for head 1111111111111111111111111111111111111111. FAIL; run the reviewer agent on head 1111111111111111111111111111111111111111.",
    });
  });

  it("ignores an unsubmitted review", () => {
    expect(
      checkReview(
        [
          {
            id: 1,
            author_association: "OWNER",
            body: "Reviewer verdict: APPROVE\nHead: 1111111111111111111111111111111111111111",
            submitted_at: null,
          },
        ],
        "1111111111111111111111111111111111111111",
      ),
    ).toEqual({
      pass: false,
      reason:
        "No reviewer verdict found. FAIL; run the reviewer agent on head 1111111111111111111111111111111111111111.",
    });
  });

  it("ignores verdicts from accounts outside the repository", () => {
    expect(
      checkReview(
        [
          {
            id: 1,
            author_association: "NONE",
            body: "Reviewer verdict: APPROVE\nHead: 1111111111111111111111111111111111111111",
            submitted_at: "2026-10-11T10:00:00Z",
          },
        ],
        "1111111111111111111111111111111111111111",
      ),
    ).toEqual({
      pass: false,
      reason:
        "No reviewer verdict found. FAIL; run the reviewer agent on head 1111111111111111111111111111111111111111.",
    });
  });

  it("matches the head regardless of hex case", () => {
    expect(
      checkReview(
        [
          {
            id: 1,
            author_association: "OWNER",
            body: "Reviewer verdict: APPROVE\nHead: ABCDEFABCDEFABCDEFABCDEFABCDEFABCDEFABCD",
            submitted_at: "2026-10-11T10:00:00Z",
          },
        ],
        "abcdefabcdefabcdefabcdefabcdefabcdefabcd",
      ),
    ).toEqual({
      pass: true,
      reason:
        "Latest reviewer verdict: APPROVE for head abcdefabcdefabcdefabcdefabcdefabcdefabcd. PASS.",
    });
  });
});

describe("buildStatusPayload", () => {
  it("reports an approval with the workflow run URL", () => {
    expect(
      buildStatusPayload(
        { pass: true, reason: "Approved for the current head." },
        "https://github.com/aelomi/aelomi/actions/runs/123",
      ),
    ).toEqual({
      context: "reviewer-approved",
      state: "success",
      description: "Approved for the current head.",
      target_url: "https://github.com/aelomi/aelomi/actions/runs/123",
    });
  });

  it("truncates a failure reason to 140 characters", () => {
    expect(
      buildStatusPayload(
        {
          pass: false,
          reason:
            "Latest reviewer verdict: CHANGES REQUESTED for head 1111111111111111111111111111111111111111. FAIL; run the reviewer agent on head 1111111111111111111111111111111111111111.",
        },
        "https://github.com/aelomi/aelomi/actions/runs/456",
      ),
    ).toEqual({
      context: "reviewer-approved",
      state: "failure",
      description:
        "Latest reviewer verdict: CHANGES REQUESTED for head 1111111111111111111111111111111111111111. FAIL; run the reviewer agent on head 111111111",
      target_url: "https://github.com/aelomi/aelomi/actions/runs/456",
    });
  });

  it("keeps a Unicode character whole at the truncation boundary", () => {
    expect(
      buildStatusPayload(
        {
          pass: false,
          reason:
            "Latest reviewer verdict: CHANGES REQUESTED for head 1111111111111111111111111111111111111111. FAIL; run the reviewer agent on head 11111111🤖extra",
        },
        "https://github.com/aelomi/aelomi/actions/runs/789",
      ),
    ).toEqual({
      context: "reviewer-approved",
      state: "failure",
      description:
        "Latest reviewer verdict: CHANGES REQUESTED for head 1111111111111111111111111111111111111111. FAIL; run the reviewer agent on head 11111111🤖",
      target_url: "https://github.com/aelomi/aelomi/actions/runs/789",
    });
  });
});
