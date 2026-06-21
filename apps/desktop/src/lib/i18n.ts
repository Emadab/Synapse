/**
 * Minimal i18n scaffold — no runtime dep required.
 *
 * Usage:
 *   import { t } from "@/lib/i18n";
 *   <p>{t("settings.title")}</p>
 *   <p>{t("app.version", { version: "1.0.0" })}</p>
 *
 * To add a new locale:
 *   1. Duplicate `src/locales/en.ts` → `src/locales/fr.ts` and translate.
 *   2. Import it here and add to `LOCALES`.
 *   3. Call `setLocale("fr")` from a settings toggle.
 */

import { en, type LocaleKey } from "@/locales/en";

type Locale = "en";
type Strings = typeof en;

const LOCALES: Record<Locale, Strings> = { en };

let activeLocale: Locale = "en";

export function setLocale(locale: Locale) {
  activeLocale = locale;
}

export function getLocale(): Locale {
  return activeLocale;
}

/** Translate a key, interpolating `{{var}}` placeholders from `vars`. */
export function t(key: LocaleKey, vars?: Record<string, string | number>): string {
  let str: string = LOCALES[activeLocale][key] ?? key;
  if (vars) {
    for (const [k, v] of Object.entries(vars)) {
      str = str.replace(new RegExp(`\\{\\{${k}\\}\\}`, "g"), String(v));
    }
  }
  return str;
}
