/**
 * The image URLs the editor may load: bytes already inside the webview, as
 * `data:` or `blob:` URLs. Every other URL, remote or file, resolves to
 * nothing, so the editor never fetches it (SHELL-13). Hash-addressed images
 * on disk (DATA-3) belong to the image feature's own loader.
 */
export function localImageUrl(url: string): string {
  return /^(data|blob):/.test(url) ? url : "";
}
