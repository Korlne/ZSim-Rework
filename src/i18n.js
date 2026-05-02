/**
 * i18n module — flat key-value translations with fallback chain:
 *   current language → en-US → raw key
 *
 * Features:
 *   - Named-parameter templates:  `t("key", { name: "value" })`
 *   - data-i18n DOM attribute for static HTML elements
 *   - localStorage persistence
 *   - "langchange" custom event on switch
 *
 * Adding a new language: simply drop a <code>.json file in src/locales/.
 * No JS code changes needed.
 */

// Vite glob import — auto-discovers all JSON files in locales/
const localeData = import.meta.glob("./locales/*.json", { eager: true });

const FALLBACK_LANG = "en-US";

/** @type {string[]} */
export const supportedLangs = [];

/** @type {Record<string, Record<string, string>>} */
const translations = {};

// Load all discovered locale files
for (const [path, mod] of Object.entries(localeData)) {
  // path is like "./locales/en-US.json"
  const langCode = path.match(/\.\/locales\/(.+)\.json$/)[1];
  if (langCode) {
    supportedLangs.push(langCode);
    translations[langCode] = mod.default || mod;
  }
}

/** @type {string} */
export let lang = FALLBACK_LANG;

/**
 * Translate a key to the current language.
 *
 * Supports named-parameter templates via `{name}` placeholders:
 *   t("greeting", { name: "World" })  ->  "Hello, World"
 *
 * @param {string} key  Translation key
 * @param {Record<string, string>} [params]  Named parameter values
 * @returns {string}
 */
export function t(key, params) {
  let value =
    translations[lang]?.[key] ??
    translations[FALLBACK_LANG]?.[key] ??
    key;

  if (params) {
    for (const [k, v] of Object.entries(params)) {
      value = value.replace(new RegExp(`\\{${k}\\}`, "g"), String(v ?? ""));
    }
  }

  return value;
}

/**
 * Walk all elements with a `data-i18n` attribute and update their text
 * content with the translation for the active language.
 */
function applyToDOM() {
  const els = document.querySelectorAll("[data-i18n]");
  for (const el of els) {
    const key = el.getAttribute("data-i18n");
    if (!key) continue;
    el.textContent = t(key);
  }
}

/**
 * Switch the active language.
 * Persists to localStorage, re-translates DOM, dispatches "langchange".
 *
 * @param {string} langCode  e.g. "en-US", "zh-CN", "ja-JP"
 */
export function switchLang(langCode) {
  if (!translations[langCode] || langCode === lang) return;
  lang = langCode;
  try {
    localStorage.setItem("zsim-lang", langCode);
  } catch (_) {
    /* quota / privacy mode - non-critical */
  }
  document.documentElement.lang = langCode;
  applyToDOM();
  window.dispatchEvent(new CustomEvent("langchange", { detail: { lang: langCode } }));
}

/**
 * Initialise i18n from persisted preference, then apply to DOM.
 * Call once at app startup.
 *
 * @returns {Promise<string>} Resolves with the active language code
 */
export function init() {
  let saved;
  try {
    saved = localStorage.getItem("zsim-lang");
  } catch (_) {
    /* ignore */
  }
  if (saved && translations[saved]) {
    lang = saved;
  }
  document.documentElement.lang = lang;
  applyToDOM();
  return Promise.resolve(lang);
}
