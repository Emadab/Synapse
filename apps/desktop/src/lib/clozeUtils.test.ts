import { describe, it, expect } from "vitest";
import { maxClozeIndex } from "./clozeUtils";

describe("maxClozeIndex", () => {
  it("returns 0 when no cloze markers exist", () => {
    expect(maxClozeIndex(["plain text", "<b>bold</b>"])).toBe(0);
  });

  it("finds the highest index within one field", () => {
    expect(maxClozeIndex(["{{c1::a}} {{c3::b}} {{c2::c}}"])).toBe(3);
  });

  it("finds the highest index across multiple fields", () => {
    expect(maxClozeIndex(["{{c1::a}}", "{{c5::b}}", "{{c2::c}}"])).toBe(5);
  });
});
