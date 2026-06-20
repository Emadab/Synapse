/** Convert a media filename to its synapse-media:// URL. */
function mediaUrl(filename: string): string {
  const encoded = encodeURIComponent(filename);
  // On Windows Tauri uses http://scheme.localhost/, elsewhere scheme://localhost/
  return navigator.userAgent.includes("Windows")
    ? `http://synapse-media.localhost/${encoded}`
    : `synapse-media://localhost/${encoded}`;
}

/**
 * Rewrite card HTML so that bare-filename image src attributes resolve to the
 * synapse-media protocol, and [sound:name] tags become hidden autoplay audio
 * elements.
 */
export function resolveCardMedia(html: string): string {
  // Rewrite <img src="bare-filename"> — skip anything already schema-prefixed
  const withImages = html.replace(
    /(<img\b[^>]*?\s)src="([^":/][^"]*)"/gi,
    (_, pre, name) => `${pre}src="${mediaUrl(name)}"`,
  );
  // Replace [sound:name] with hidden autoplay audio
  const withAudio = withImages.replace(/\[sound:([^\]]+)\]/g, (_, name) => {
    const url = mediaUrl(name);
    return `<audio src="${url}" autoplay style="display:none" onended="this.remove()"></audio>`;
  });
  return withAudio;
}
