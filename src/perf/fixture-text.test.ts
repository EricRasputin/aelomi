import { expect, test } from "vitest";
import { fixtureParagraphs } from "./fixture-text";

test("flattens block text depth first and skips blocks without content", () => {
  const blocks = [
    {
      id: "a",
      type: "paragraph",
      content: [
        { type: "text", text: "Hello ", styles: { bold: true } },
        {
          type: "link",
          content: [{ type: "text", text: "world", styles: {} }],
          href: "https://example.com/x",
        },
      ],
      children: [
        {
          id: "b",
          type: "bulletListItem",
          content: [{ type: "text", text: "child", styles: {} }],
          children: [],
        },
      ],
    },
    { id: "c", type: "divider", props: {}, children: [] },
    {
      id: "d",
      type: "codeBlock",
      content: [{ type: "text", text: "let x = 1;", styles: {} }],
      children: [],
    },
  ];
  expect(fixtureParagraphs(blocks)).toEqual([
    { id: "a", text: "Hello world" },
    { id: "b", text: "child" },
    { id: "d", text: "let x = 1;" },
  ]);
});

test("rejects shapes that are not block arrays", () => {
  expect(() => fixtureParagraphs({})).toThrow("not a block array");
  expect(() => fixtureParagraphs([{ type: "paragraph" }])).toThrow("no id");
});
