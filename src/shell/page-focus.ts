export type FocusRequest =
  | { pageId: string; target: "title" | "body" | "bodyStart" }
  | { pageId: string; target: "block"; blockId: string };

type FocusContext =
  | { kind: "request"; bodyReady: boolean }
  | { kind: "titleFocus" }
  | { kind: "bodyFocus"; deferred: boolean; textFieldFocused: boolean };

export function resolvePageFocus(
  request: FocusRequest,
  context: FocusContext,
): FocusRequest | null {
  if (request.target === "title") return request;
  switch (context.kind) {
    case "request":
      return context.bodyReady
        ? request
        : request.target === "bodyStart"
          ? null
          : { pageId: request.pageId, target: "title" };
    case "titleFocus":
      return null;
    case "bodyFocus":
      return context.textFieldFocused &&
        (context.deferred || request.target !== "bodyStart")
        ? null
        : request;
  }
}
