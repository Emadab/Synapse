/** Highest `{{cN::…}}` index found across the given field HTML strings, or 0 if none. */
export function maxClozeIndex(htmlFields: string[]): number {
  const re = /\{\{c(\d+)::/g;
  let max = 0;
  for (const html of htmlFields) {
    for (const match of html.matchAll(re)) {
      max = Math.max(max, Number(match[1]));
    }
  }
  return max;
}
