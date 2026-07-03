import { describe, it, expect } from "vitest";
import { diffTypedAnswer } from "./typedAnswerDiff";

describe("diffTypedAnswer", () => {
  it("marks an exact match fully good on both lines", () => {
    const html = diffTypedAnswer("hello", "hello");
    expect(html).toContain('<span class="typeGood">hello</span>');
    expect(html).not.toContain("typeBad");
    expect(html).not.toContain("typeMissed");
  });

  it("flags wrong typed characters and missed expected characters", () => {
    const html = diffTypedAnswer("helno", "hello");
    // "hel" matches, "no" vs "lo" diverge.
    expect(html).toContain('<span class="typeGood">hel</span>');
    expect(html).toContain("typeBad");
    expect(html).toContain("typeMissed");
  });

  it("trims and collapses whitespace before comparing", () => {
    const html = diffTypedAnswer("  hello  ", "hello");
    expect(html).toContain('<span class="typeGood">hello</span>');
    expect(html).not.toContain("typeBad");
  });

  it("returns empty string when both sides are empty", () => {
    expect(diffTypedAnswer("", "")).toBe("");
  });

  it("escapes HTML in typed/expected text", () => {
    const html = diffTypedAnswer("<b>", "<b>");
    expect(html).not.toContain("<b>hello");
    expect(html).toContain("&lt;b&gt;");
  });
});
