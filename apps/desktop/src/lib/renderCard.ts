/**
 * Card render pipeline — runs after the Rust template engine produces HTML.
 *
 * Responsibilities:
 *  1. Extract `[sound:name]` filenames in document order (return to caller for
 *     sequenced playback).
 *  2. Rewrite bare-filename `<img src>` to synapse-media:// URLs.
 *  3. Strip residual `[sound:...]` markup (audio is driven by caller).
 */

/** Absolute URL for a media file served through the Tauri protocol handler. */
export function mediaUrl(filename: string): string {
  const encoded = encodeURIComponent(filename);
  return navigator.userAgent.includes("Windows")
    ? `http://synapse-media.localhost/${encoded}`
    : `synapse-media://localhost/${encoded}`;
}

export interface PreparedCard {
  /** HTML ready to inject into the DOM. */
  html: string;
  /** Sound filenames in document order, to be played sequentially. */
  sounds: string[];
}

/**
 * Prepare raw card HTML for display:
 *  - extract and strip `[sound:...]` markers
 *  - rewrite relative image src URLs (Tauri only)
 */
export function prepareCard(html: string, tauri: boolean): PreparedCard {
  const sounds: string[] = [];

  // Extract and remove [sound:name] markers.
  const withoutSounds = html.replace(/\[sound:([^\]]+)\]/g, (_, name: string) => {
    sounds.push(name);
    return "";
  });

  // Rewrite bare image filenames to synapse-media:// URLs (skip data: and http:).
  const withImages = tauri
    ? withoutSounds.replace(
        /(<img\b[^>]*?\s)src="([^":/][^"]*)"/gi,
        (_, pre: string, name: string) => `${pre}src="${mediaUrl(name)}"`,
      )
    : withoutSounds;

  return { html: withImages, sounds };
}
