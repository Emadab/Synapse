/**
 * Anki-style typed-answer diff: compares what the user typed for
 * `{{type:Field}}` against the expected answer and produces a two-line HTML
 * block — the typed text with wrong characters flagged, and the correct
 * answer with missing characters flagged. Matching runs (the longest common
 * subsequence) are shown as "good" on both lines.
 */

function longestCommonSubsequenceMask(a: string, b: string): { inA: boolean[]; inB: boolean[] } {
  const n = a.length;
  const m = b.length;
  const dp: number[][] = Array.from({ length: n + 1 }, () => new Array<number>(m + 1).fill(0));

  for (let i = n - 1; i >= 0; i--) {
    for (let j = m - 1; j >= 0; j--) {
      dp[i][j] =
        a[i] === b[j] ? dp[i + 1][j + 1] + 1 : Math.max(dp[i + 1][j], dp[i][j + 1]);
    }
  }

  const inA = new Array<boolean>(n).fill(false);
  const inB = new Array<boolean>(m).fill(false);
  let i = 0;
  let j = 0;
  while (i < n && j < m) {
    if (a[i] === b[j]) {
      inA[i] = true;
      inB[j] = true;
      i++;
      j++;
    } else if (dp[i + 1][j] >= dp[i][j + 1]) {
      i++;
    } else {
      j++;
    }
  }
  return { inA, inB };
}

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}

function buildLine(text: string, mask: boolean[], goodClass: string, badClass: string): string {
  let out = "";
  let i = 0;
  while (i < text.length) {
    const good = mask[i];
    let j = i;
    while (j < text.length && mask[j] === good) j++;
    const run = escapeHtml(text.slice(i, j));
    out += `<span class="${good ? goodClass : badClass}">${run}</span>`;
    i = j;
  }
  return out;
}

/** Normalize whitespace/case the way Anki compares typed answers. */
function normalize(s: string): string {
  return s.trim().replace(/\s+/g, " ");
}

/**
 * Returns HTML for the two-row typed-answer comparison:
 * line 1 = what was typed (correct chars `typeGood`, wrong chars `typeBad`),
 * line 2 = the expected answer (matched chars `typeGood`, missing chars `typeMissed`).
 */
export function diffTypedAnswer(typed: string, expected: string): string {
  const a = normalize(typed);
  const b = normalize(expected);
  if (a.length === 0 && b.length === 0) return "";

  const { inA, inB } = longestCommonSubsequenceMask(a, b);
  const userLine = buildLine(a, inA, "typeGood", "typeBad");
  const correctLine = buildLine(b, inB, "typeGood", "typeMissed");
  return `<code class="synapse-typeans-diff">${userLine}<br>${correctLine}</code>`;
}
