import { describe, it, expect } from "vitest";
import { parseOcclusionField, serializeOcclusionField } from "./imageOcclusion";

describe("occlusion shape codec", () => {
  it("round-trips shapes through serialize/parse", () => {
    const shapes = [
      { ord: 1, kind: "rect" as const, left: 0.1, top: 0.2, width: 0.3, height: 0.15 },
      { ord: 2, kind: "ellipse" as const, left: 0.5, top: 0.6, width: 0.1, height: 0.1 },
    ];
    const html = serializeOcclusionField(shapes);
    expect(parseOcclusionField(html)).toEqual(shapes);
  });

  it("parses shapes sorted by cloze ordinal regardless of source order", () => {
    const html =
      "{{c2::image-occlusion:rect:left=.5:top=.5:width=.1:height=.1}}" +
      "{{c1::image-occlusion:rect:left=.1:top=.1:width=.1:height=.1}}";
    const shapes = parseOcclusionField(html);
    expect(shapes.map((s) => s.ord)).toEqual([1, 2]);
  });

  it("clamps out-of-range coordinates on serialize", () => {
    const html = serializeOcclusionField([
      { ord: 1, kind: "rect", left: -0.5, top: 1.5, width: 0.1, height: 0.1 },
    ]);
    expect(html).toContain("left=0.0000");
    expect(html).toContain("top=1.0000");
  });

  it("returns an empty array for fields with no occlusion markers", () => {
    expect(parseOcclusionField("plain text")).toEqual([]);
  });
});
