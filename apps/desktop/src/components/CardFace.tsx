import { useLayoutEffect, useRef } from "react";
import renderMathInElement from "katex/contrib/auto-render";
// Vite `?inline` returns the file's CSS as a string, for adoptedStyleSheets.
import katexCss from "katex/dist/katex.min.css?inline";
import cardBaseCss from "@/styles/card-base.css?inline";
import { prepareCard, type QueueEntry } from "@/lib/renderCard";
import { diffTypedAnswer } from "@/lib/typedAnswerDiff";
import { loadVoices } from "@/lib/tts";
import { wireOcclusionShapes } from "@/lib/imageOcclusion";

const KATEX_OPTIONS = {
  delimiters: [
    { left: "\\(", right: "\\)", display: false },
    { left: "\\[", right: "\\]", display: true },
    { left: "$$", right: "$$", display: true },
    { left: "$", right: "$", display: false },
  ],
  throwOnError: false,
} as const;

let sharedBaseSheet: CSSStyleSheet | null = null;
let sharedKatexSheet: CSSStyleSheet | null = null;

function getSharedSheets(): [CSSStyleSheet, CSSStyleSheet] {
  if (!sharedBaseSheet) {
    sharedBaseSheet = new CSSStyleSheet();
    sharedBaseSheet.replaceSync(cardBaseCss);
  }
  if (!sharedKatexSheet) {
    sharedKatexSheet = new CSSStyleSheet();
    sharedKatexSheet.replaceSync(katexCss);
  }
  return [sharedBaseSheet, sharedKatexSheet];
}

interface CardFaceProps {
  /** Rendered question/answer HTML from the Rust template engine. */
  html: string;
  /** The active notetype's custom card CSS (scoped to this face via shadow DOM). */
  css: string;
  tauri: boolean;
  night: boolean;
  className?: string;
  style?: React.CSSProperties;
  /** Called with the sound+TTS playback queue, in document order, whenever `html` changes. */
  onQueue?: (queue: QueueEntry[]) => void;
  /** Called on every keystroke in a `{{type:Field}}` input (front side only). */
  onTypedInput?: (value: string) => void;
  /** What the user typed on the front side, to diff against `.synapse-typeans` (back side only). */
  typedAnswer?: string;
}

/**
 * Renders one card face inside a shadow root so per-notetype CSS (from
 * imported Anki decks or user edits) can use global selectors like `.card`
 * or `img {}` without leaking into app chrome. Theme CSS custom properties
 * still inherit through the shadow boundary.
 */
export function CardFace({
  html,
  css,
  tauri,
  night,
  className,
  style,
  onQueue,
  onTypedInput,
  typedAnswer,
}: CardFaceProps) {
  const hostRef = useRef<HTMLDivElement>(null);
  const containerRef = useRef<HTMLDivElement | null>(null);
  const notetypeSheetRef = useRef<CSSStyleSheet | null>(null);

  // One-time shadow root + adopted stylesheets setup.
  useLayoutEffect(() => {
    const host = hostRef.current;
    if (!host) return;
    const shadow = host.shadowRoot ?? host.attachShadow({ mode: "open" });

    let container = shadow.querySelector<HTMLDivElement>(".synapse-card-root");
    if (!container) {
      container = document.createElement("div");
      container.className = "card synapse-card-root";
      shadow.appendChild(container);
    }
    containerRef.current = container;

    const [base, katex] = getSharedSheets();
    const notetypeSheet = new CSSStyleSheet();
    notetypeSheetRef.current = notetypeSheet;
    shadow.adoptedStyleSheets = [base, katex, notetypeSheet];
  }, []);

  // Keep the per-notetype stylesheet in sync.
  useLayoutEffect(() => {
    notetypeSheetRef.current?.replaceSync(css || "");
  }, [css]);

  // Anki-CSS-compatible night mode classes.
  useLayoutEffect(() => {
    containerRef.current?.classList.toggle("night_mode", night);
    containerRef.current?.classList.toggle("nightMode", night);
  }, [night]);

  // Inject prepared HTML and wire post-render behavior (KaTeX, hints, type input).
  useLayoutEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    const prepared = prepareCard(html, tauri);
    container.innerHTML = prepared.html;
    onQueue?.(prepared.queue);

    renderMathInElement(container, KATEX_OPTIONS);

    container.querySelectorAll<HTMLAnchorElement>(".synapse-hint").forEach((link) => {
      link.addEventListener("click", (e) => {
        e.preventDefault();
        const body = link.nextElementSibling as HTMLElement | null;
        if (body?.classList.contains("synapse-hint-body")) {
          body.hidden = false;
          link.style.display = "none";
        }
      });
    });

    const input = container.querySelector<HTMLInputElement>(".synapse-type-input");
    if (input && onTypedInput) {
      input.addEventListener("input", () => onTypedInput(input.value));
      input.focus();
    }

    wireOcclusionShapes(container);

    const voicesEl = container.querySelectorAll<HTMLElement>(".synapse-tts-voices");
    if (voicesEl.length > 0) {
      void loadVoices().then((voices) => {
        const names = voices.map((v) => `${v.name} (${v.lang})`).join(", ");
        voicesEl.forEach((el) => {
          el.textContent = names || "No voices installed";
        });
      });
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [html, tauri]);

  // Fill in the typed-answer diff on the back side once we know both sides.
  useLayoutEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    const target = container.querySelector<HTMLElement>(".synapse-typeans");
    if (!target) return;
    const expected = target.dataset.expected ?? "";
    target.innerHTML = diffTypedAnswer(typedAnswer ?? "", expected);
  }, [html, typedAnswer]);

  return <div ref={hostRef} className={className} style={style} />;
}
