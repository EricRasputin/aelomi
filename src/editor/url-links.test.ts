// @vitest-environment happy-dom
import { expect, test, vi } from "vitest";
import { commands } from "../ipc";
import { allowedUrl, openPageLink, recordPageLinkPointer } from "./url-links";

test.each([
  "http://example.com",
  "https://example.com",
  "mailto:hello@example.com",
  "HTTPS://example.com/path?q=test#part",
])("allows a browser URL %s", (url) => {
  expect(allowedUrl(url)).toBe(true);
});
test.each([
  "javascript:alert(1)",
  "file:///tmp/secret",
  "data:text/html,test",
  "ftp://example.com",
  "/page/page-a",
  "not a URL",
  "tauri://localhost",
])("blocks a non-browser URL %s", (url) => {
  expect(allowedUrl(url)).toBe(false);
});

test("keyboard and accessibility activation opens once even with selected text", async () => {
  const root = document.createElement("div");
  const link = document.createElement("a");
  link.href = "https://example.com";
  link.textContent = "Example";
  root.append(link);
  document.body.append(root);
  const opener = vi
    .spyOn(commands, "openUrl")
    .mockResolvedValue({ status: "ok", data: null });
  const preventDefault = vi.fn();
  try {
    window.getSelection()?.selectAllChildren(link);
    openPageLink({
      target: link,
      currentTarget: root,
      preventDefault,
      type: "click",
      button: 0,
      detail: 0,
      clientX: 0,
      clientY: 0,
    });
    expect(preventDefault.mock.calls).toEqual([[]]);
    expect(opener.mock.calls).toEqual([["https://example.com"]]);
  } finally {
    window.getSelection()?.removeAllRanges();
    vi.restoreAllMocks();
    document.body.replaceChildren();
  }
});

test("double-click then single click opens twice, while drag and context menu only prevent navigation", async () => {
  const root = document.createElement("div");
  const link = document.createElement("a");
  link.href = "https://example.com";
  link.textContent = "Example";
  root.append(link);
  document.body.append(root);
  const opener = vi
    .spyOn(commands, "openUrl")
    .mockResolvedValue({ status: "ok", data: null });
  const dispatch = (type: string, detail: number, x = 10, y = 10) => {
    recordPageLinkPointer({ currentTarget: root, clientX: 10, clientY: 10 });
    const event = new MouseEvent(type, {
      detail,
      bubbles: true,
      cancelable: true,
    });
    const handle = () =>
      openPageLink({
        target: link,
        currentTarget: root,
        type,
        detail,
        button: 0,
        clientX: x,
        clientY: y,
        preventDefault: () => event.preventDefault(),
      });
    link.addEventListener(type, handle, { once: true });
    link.dispatchEvent(event);
    return event.defaultPrevented;
  };
  try {
    expect(dispatch("click", 1)).toBe(true);
    expect(dispatch("click", 2)).toBe(true);
    const selection = window.getSelection();
    selection?.selectAllChildren(link);
    expect(selection?.isCollapsed).toBe(false);
    expect(dispatch("click", 1)).toBe(true);
    expect(dispatch("click", 1, 30)).toBe(true);
    selection?.collapseToStart();
    expect(dispatch("contextmenu", 1)).toBe(true);
    expect(opener.mock.calls).toEqual([
      ["https://example.com"],
      ["https://example.com"],
    ]);
  } finally {
    window.getSelection()?.removeAllRanges();
    vi.restoreAllMocks();
    document.body.replaceChildren();
  }
});
