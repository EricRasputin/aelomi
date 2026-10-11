import { describe, expect, it } from "vitest";
import { checkReview } from "./check-review.ts";

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
            body: "Looks good to me.",
            submitted_at: "2026-10-11T10:00:00Z",
          },
          { id: 2, body: null, submitted_at: "2026-10-11T11:00:00Z" },
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
            body: "Reviewer verdict: APPROVE\nHead: 1111111111111111111111111111111111111111\n\nNo findings.",
            submitted_at: "2026-10-11T10:00:00Z",
          },
          {
            id: 2,
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
            body: "Reviewer verdict: APPROVE\nHead: 1111111111111111111111111111111111111111",
            submitted_at: "2026-10-11T10:00:00Z",
          },
          {
            id: 2,
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
            body: "Reviewer verdict: APPROVE\nHead: 1111111111111111111111111111111111111111",
            submitted_at: "2026-10-11T11:00:00Z",
          },
          {
            id: 2,
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
        [{ id: 1, body, submitted_at: "2026-10-11T10:00:00Z" }],
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
            body: "Reviewer verdict: CHANGES REQUESTED\nHead: 1111111111111111111111111111111111111111",
            submitted_at: "2026-10-11T10:00:00Z",
          },
          {
            id: 1,
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
});
