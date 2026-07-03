import { mediaUrl } from "./renderCard";

const BARE_SRC_RE = /(<img\b[^>]*?\s)src="([^":/][^"]*)"/gi;
const SERVED_SRC_RE =
  /(<img\b[^>]*?\s)src="(?:synapse-media:\/\/localhost\/|http:\/\/synapse-media\.localhost\/)([^"]+)"/gi;

/** Rewrite bare media filenames to servable URLs, for showing stored field HTML in the editor. */
export function toDisplayHtml(html: string, tauri: boolean): string {
  if (!tauri) return html;
  return html.replace(BARE_SRC_RE, (_, pre: string, name: string) => `${pre}src="${mediaUrl(name)}"`);
}

/** Rewrite servable media URLs back to bare filenames, before persisting editor HTML. */
export function toStorageHtml(html: string): string {
  return html.replace(
    SERVED_SRC_RE,
    (_, pre: string, encoded: string) => `${pre}src="${decodeURIComponent(encoded)}"`,
  );
}
