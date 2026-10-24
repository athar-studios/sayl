import { writable, derived } from "svelte/store";
import ar from "../i18n/ar.js";
import en from "../i18n/en.js";

const dictionaries = { ar, en };

export const currentLang = writable("ar");

export function setLanguage(lang) {
  if (lang !== "ar" && lang !== "en") {
    lang = "ar";
  }
  currentLang.set(lang);
  if (typeof document !== "undefined") {
    document.documentElement.lang = lang;
    document.documentElement.dir = lang === "ar" ? "rtl" : "ltr";
  }
}

export function toggleLanguage() {
  currentLang.update((lang) => {
    const next = lang === "ar" ? "en" : "ar";
    if (typeof document !== "undefined") {
      document.documentElement.lang = next;
      document.documentElement.dir = next === "ar" ? "rtl" : "ltr";
    }
    return next;
  });
}

export function resolveTranslation(lang, path) {
  const dict = dictionaries[lang] || dictionaries.ar;
  const parts = path.split(".");
  let cur = dict;
  for (const part of parts) {
    if (cur && typeof cur === "object" && part in cur) {
      cur = cur[part];
    } else {
      return path;
    }
  }
  return cur;
}

export const t = derived(currentLang, ($lang) => {
  return (path) => resolveTranslation($lang, path);
});
