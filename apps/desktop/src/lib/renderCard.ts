/**
 * Card render pipeline — runs after the Rust template engine produces HTML.
 *
 * Responsibilities:
 *  1. Walk the HTML in document order (one `DOMParser` pass) to build a
 *     single ordered playback queue mixing `[sound:name]` markers and
 *     `{{tts …:Field}}` markers (rendered by the engine as
 *     `.synapse-tts` spans) — so mixed sound+TTS cards play back in the
 *     order they appear, not "files first, then speech".
 *  2. Rewrite bare-filename `<img src>` to synapse-media:// URLs.
 */

/** Absolute URL for a media file served through the Tauri protocol handler. */
export function mediaUrl(filename: string): string {
  const encoded = encodeURIComponent(filename);
  return navigator.userAgent.includes("Windows")
    ? `http://synapse-media.localhost/${encoded}`
    : `synapse-media://localhost/${encoded}`;
}

export type QueueEntry =
  | { kind: "file"; name: string }
  | { kind: "tts"; text: string; lang: string; voices: string[]; rate: number };

export interface PreparedCard {
  /** HTML ready to inject into the DOM. */
  html: string;
  /** Sound files and TTS utterances, in document order, for sequenced playback. */
  queue: QueueEntry[];
}

const SOUND_RE = /\[sound:([^\]]+)\]/g;

/**
 * Prepare raw card HTML for display:
 *  - extract `[sound:...]` markers and `.synapse-tts` spans into one ordered queue
 *  - rewrite relative image src URLs (Tauri only)
 */
export function prepareCard(html: string, tauri: boolean): PreparedCard {
  const doc = new DOMParser().parseFromString(html, "text/html");
  const queue: QueueEntry[] = [];

  const walker = doc.createTreeWalker(doc.body, NodeFilter.SHOW_ELEMENT | NodeFilter.SHOW_TEXT);
  let node: Node | null;
  const textNodesWithSounds: Text[] = [];
  while ((node = walker.nextNode())) {
    if (node.nodeType === Node.TEXT_NODE) {
      const text = node.textContent ?? "";
      if (text.includes("[sound:")) {
        for (const m of text.matchAll(SOUND_RE)) {
          queue.push({ kind: "file", name: m[1] });
        }
        textNodesWithSounds.push(node as Text);
      }
    } else if (node.nodeType === Node.ELEMENT_NODE) {
      const el = node as HTMLElement;
      if (el.classList.contains("synapse-tts")) {
        const text = el.dataset.text ?? "";
        if (text) {
          queue.push({
            kind: "tts",
            text,
            lang: el.dataset.lang ?? "",
            voices: (el.dataset.voices ?? "")
              .split(",")
              .map((s) => s.trim())
              .filter(Boolean),
            rate: Number(el.dataset.rate) || 1,
          });
        }
      }
    }
  }
  for (const textNode of textNodesWithSounds) {
    textNode.textContent = (textNode.textContent ?? "").replace(SOUND_RE, "");
  }

  if (tauri) {
    doc.querySelectorAll("img").forEach((img) => {
      const src = img.getAttribute("src") ?? "";
      if (src && !/^(data:|https?:|synapse-media:)/i.test(src)) {
        img.setAttribute("src", mediaUrl(src));
      }
    });
  }

  return { html: doc.body.innerHTML, queue };
}
