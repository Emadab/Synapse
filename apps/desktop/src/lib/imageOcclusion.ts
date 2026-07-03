/**
 * Image-occlusion shape codec + front-side runtime. Shapes are stored in the
 * notetype's `Occlusion` field as cloze markers whose text begins with
 * `image-occlusion:` (see `crates/synapse-render/src/cloze.rs`), so card
 * generation, scheduling and .apkg round-trip all reuse the existing cloze
 * machinery — this module only translates between that text encoding and a
 * shape list an editor UI can draw/drag.
 */

export interface OcclusionShape {
  /** Cloze ordinal — which card (`{{cN::…}}`) this shape belongs to. */
  ord: number;
  kind: "rect" | "ellipse";
  /** Normalized 0–1 coordinates, relative to the occluded image. */
  left: number;
  top: number;
  width: number;
  height: number;
}

const SHAPE_RE = /\{\{c(\d+)::image-occlusion:(rect|ellipse):([^{}]+)\}\}/g;

export function parseOcclusionField(html: string): OcclusionShape[] {
  const shapes: OcclusionShape[] = [];
  for (const m of html.matchAll(SHAPE_RE)) {
    const props: Record<string, number> = {};
    for (const part of m[3].split(":")) {
      const [k, v] = part.split("=");
      if (k && v !== undefined) props[k] = Number(v);
    }
    shapes.push({
      ord: Number(m[1]),
      kind: m[2] as "rect" | "ellipse",
      left: props.left ?? 0,
      top: props.top ?? 0,
      width: props.width ?? 0.1,
      height: props.height ?? 0.1,
    });
  }
  return shapes.sort((a, b) => a.ord - b.ord);
}

const clamp01 = (n: number) => Math.max(0, Math.min(1, n));

export function serializeOcclusionField(shapes: OcclusionShape[]): string {
  return shapes
    .map(
      (s) =>
        `{{c${s.ord}::image-occlusion:${s.kind}:left=${clamp01(s.left).toFixed(4)}:top=${clamp01(s.top).toFixed(4)}:width=${clamp01(s.width).toFixed(4)}:height=${clamp01(s.height).toFixed(4)}}}`,
    )
    .join("");
}

/**
 * Front-side "click to peek" — reveal one hidden shape without flipping the
 * whole card to the answer, matching Anki's occlusion review behavior. Only
 * shapes rendered hidden (`data-revealed="false"`) get the affordance.
 */
export function wireOcclusionShapes(container: HTMLElement): void {
  container
    .querySelectorAll<HTMLElement>('.synapse-io-shape[data-revealed="false"]')
    .forEach((shape) => {
      shape.style.cursor = "pointer";
      shape.addEventListener("click", () => {
        shape.dataset.revealed = "true";
      });
    });
}
