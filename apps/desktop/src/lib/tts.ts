/**
 * Text-to-speech playback for `{{tts …:Field}}` template markers, backed by
 * the webview's `SpeechSynthesis` API (WebView2 ships Edge/Windows voices;
 * there is no Rust-side TTS). Voices load asynchronously and can be entirely
 * absent on some systems (e.g. Windows "N" editions) — every entry point
 * degrades gracefully to a no-op rather than throwing.
 */

let voicesPromise: Promise<SpeechSynthesisVoice[]> | null = null;

function synth(): SpeechSynthesis | null {
  return typeof speechSynthesis === "undefined" ? null : speechSynthesis;
}

/** Resolve once the browser's voice list is available (or empty, if none installed). */
export function loadVoices(): Promise<SpeechSynthesisVoice[]> {
  if (voicesPromise) return voicesPromise;
  const s = synth();
  if (!s) {
    voicesPromise = Promise.resolve([]);
    return voicesPromise;
  }
  voicesPromise = new Promise((resolve) => {
    const existing = s.getVoices();
    if (existing.length > 0) {
      resolve(existing);
      return;
    }
    const onVoicesChanged = () => {
      s.removeEventListener("voiceschanged", onVoicesChanged);
      resolve(s.getVoices());
    };
    s.addEventListener("voiceschanged", onVoicesChanged);
    // Some platforms never fire voiceschanged when there are zero voices.
    setTimeout(() => {
      s.removeEventListener("voiceschanged", onVoicesChanged);
      resolve(s.getVoices());
    }, 1000);
  });
  return voicesPromise;
}

function pickVoice(
  voices: SpeechSynthesisVoice[],
  lang: string,
  names: string[],
): SpeechSynthesisVoice | undefined {
  if (names.length > 0) {
    const byName = voices.find((v) => names.some((n) => v.name.toLowerCase() === n.toLowerCase()));
    if (byName) return byName;
  }
  if (lang) {
    const exact = voices.find((v) => v.lang.toLowerCase() === lang.toLowerCase());
    if (exact) return exact;
    const base = lang.split(/[-_]/)[0].toLowerCase();
    const prefix = voices.find((v) => v.lang.toLowerCase().startsWith(base));
    if (prefix) return prefix;
  }
  return undefined;
}

export interface SpeakOptions {
  lang?: string;
  voices?: string[];
  rate?: number;
}

/** Speak `text` and resolve when playback ends (or immediately if TTS/voices are unavailable). */
export async function speak(text: string, opts: SpeakOptions = {}): Promise<void> {
  const s = synth();
  if (!s || !text.trim()) return;

  const available = await loadVoices();
  if (available.length === 0) return; // no voices installed — skip silently

  const utter = new SpeechSynthesisUtterance(text);
  const voice = pickVoice(available, opts.lang ?? "", opts.voices ?? []);
  if (voice) {
    utter.voice = voice;
    utter.lang = voice.lang;
  } else if (opts.lang) {
    utter.lang = opts.lang;
  }
  if (opts.rate && opts.rate > 0) utter.rate = opts.rate;

  return new Promise((resolve) => {
    utter.onend = () => resolve();
    utter.onerror = () => resolve();
    s.speak(utter);
  });
}

export function cancelSpeech(): void {
  synth()?.cancel();
}
