const localHosts = new Set(["127.0.0.1", "localhost", "[::1]"]);
const localProtocols = new Set(["data:", "blob:", "about:"]);

export function isLocalUrl(raw: string): boolean {
  const url = new URL(raw);
  return localProtocols.has(url.protocol) || localHosts.has(url.hostname);
}
