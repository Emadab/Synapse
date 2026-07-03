import { describe, it, expect } from "vitest";
import { prepareCard } from "./renderCard";

describe("prepareCard", () => {
  it("extracts sound markers in document order and strips them from the HTML", () => {
    const { html, queue } = prepareCard("word [sound:a.mp3] more [sound:b.ogg] end", false);
    expect(queue).toEqual([
      { kind: "file", name: "a.mp3" },
      { kind: "file", name: "b.ogg" },
    ]);
    expect(html).not.toContain("[sound:");
    expect(html).toContain("word");
    expect(html).toContain("end");
  });

  it("extracts .synapse-tts markers with their data attributes", () => {
    const html =
      '<span class="synapse-tts" data-lang="en_US" data-voices="Alice" data-rate="1.2" data-text="hola"></span>';
    const { queue } = prepareCard(html, false);
    expect(queue).toEqual([
      { kind: "tts", text: "hola", lang: "en_US", voices: ["Alice"], rate: 1.2 },
    ]);
  });

  it("interleaves sound and tts entries in true document order", () => {
    const html =
      'a [sound:one.mp3] b <span class="synapse-tts" data-text="two"></span> c [sound:three.mp3]';
    const { queue } = prepareCard(html, false);
    expect(queue.map((e) => (e.kind === "file" ? e.name : e.text))).toEqual([
      "one.mp3",
      "two",
      "three.mp3",
    ]);
  });

  it("returns an empty queue when there is nothing to play", () => {
    expect(prepareCard("<b>plain</b>", false).queue).toEqual([]);
  });

  it("leaves data:/http(s):/synapse-media: image sources untouched", () => {
    const html = '<img src="data:image/png;base64,AAA"><img src="https://x.test/a.png">';
    const { html: out } = prepareCard(html, true);
    expect(out).toContain('src="data:image/png;base64,AAA"');
    expect(out).toContain('src="https://x.test/a.png"');
  });
});
