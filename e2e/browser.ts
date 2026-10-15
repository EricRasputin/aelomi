import { surfaceOf, web } from "@e2e-dev/web";

export const acceptanceEngine = web({ browser: "webkit", headers: {} });

export function acceptanceContext() {
  const surface = surfaceOf(acceptanceEngine);
  if (!surface) throw new Error("The acceptance WebKit engine is unavailable");
  return surface.context();
}
