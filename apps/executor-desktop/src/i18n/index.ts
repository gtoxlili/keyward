import { createContext, useContext } from "react";
import { en, type Dict } from "./en";
import { zh } from "./zh";

export type Lang = "en" | "zh";
export const dicts: Record<Lang, Dict> = { en, zh };

type I18n = { lang: Lang; setLang: (l: Lang) => void; d: Dict };
export const I18nContext = createContext<I18n>({
  lang: "en",
  setLang: () => {},
  d: en,
});
export const useI18n = () => useContext(I18nContext);

/** Minimal interpolation: fill("…{n}…", { n: 3 }). */
export function fill(s: string, vars: Record<string, string | number>): string {
  return s.replace(/\{(\w+)\}/g, (_, k) => String(vars[k] ?? `{${k}}`));
}

export type { Dict };
