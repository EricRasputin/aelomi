export type FocusRequest = {
  pageId: string;
  target: "title" | "body" | "bodyStart";
};

type FocusContext =
  | { kind: "request"; bodyReady: boolean }
  | { kind: "titleFocus" }
  | { kind: "bodyMount"; textFieldFocused: boolean };

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
          : { ...request, target: "title" };
    case "titleFocus":
      return null;
    case "bodyMount":
      return request.target === "body" && context.textFieldFocused
        ? null
        : request;
  }
}
