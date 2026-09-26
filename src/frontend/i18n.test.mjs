import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { LOCALES, MESSAGES, FALLBACK_LOCALE, normalizeLocale, makeT, htmlLang } from "./i18n.js";

test("normalizeLocale maps system tags onto the locales we ship", () => {
  assert.equal(normalizeLocale("zh-Hans-CN"), "zh");
  assert.equal(normalizeLocale("zh_CN"), "zh");
  assert.equal(normalizeLocale("zh"), "zh");
  assert.equal(normalizeLocale("en-US"), "en");
  assert.equal(normalizeLocale("EN"), "en");
  assert.equal(normalizeLocale("  en-GB  "), "en");
});

test("normalizeLocale returns null for anything we ship no bundle for", () => {
  for (const raw of [null, undefined, "", "   ", "fr-FR", "ja", 42, {}, []]) {
    assert.equal(normalizeLocale(raw), null, `expected null for ${JSON.stringify(raw)}`);
  }
});

test("every locale is complete — same keys in both directions", () => {
  const en = Object.keys(MESSAGES[FALLBACK_LOCALE]).sort();
  for (const locale of LOCALES) {
    const keys = Object.keys(MESSAGES[locale]).sort();
    assert.deepEqual(keys, en, `locale '${locale}' key set differs from '${FALLBACK_LOCALE}'`);
  }
});

test("no translation is empty or left as a raw key", () => {
  for (const locale of LOCALES) {
    for (const [key, value] of Object.entries(MESSAGES[locale])) {
      assert.equal(typeof value, "string", `${locale}/${key} is not a string`);
      assert.ok(value.trim().length > 0, `${locale}/${key} is empty`);
      assert.notEqual(value, key, `${locale}/${key} still holds the raw key`);
    }
  }
});

test("makeT interpolates, falls back per key, and never leaks a raw key as a crash", () => {
  const t = makeT("zh");
  assert.equal(t("foot.version", { value: "2.46.0" }), "版本：2.46.0");
  assert.equal(t("line.openWebui", { url: "http://127.0.0.1:3000/" }), "已打开 PixivFlow WebUI：http://127.0.0.1:3000/");
  assert.equal(t("meta.port"), "端口");
  assert.equal(t("totally.unknown"), "totally.unknown");
  // a locale we do not ship degrades to English rather than to a blank UI
  assert.equal(makeT("fr")("action.stop"), MESSAGES.en["action.stop"]);
  // a missing key in a partial dictionary falls back per key
  assert.equal(makeT("zh", { en: { a: "A" }, zh: {} })("a"), "A");
});

test("htmlLang tags <html lang> with a real BCP-47 tag", () => {
  assert.equal(htmlLang("zh"), "zh-CN");
  assert.equal(htmlLang("en"), "en");
  assert.equal(htmlLang("fr"), "en");
});

test("index.html only references keys that exist in every locale", () => {
  const html = readFileSync(new URL("./index.html", import.meta.url), "utf8");
  const keys = [...html.matchAll(/data-i18n(?:-title)?="([^"]+)"/g)].map((m) => m[1]);
  assert.ok(keys.length > 0, "no data-i18n attributes found in index.html");
  for (const key of keys) {
    for (const locale of LOCALES) {
      assert.ok(
        Object.prototype.hasOwnProperty.call(MESSAGES[locale], key),
        `${locale} is missing '${key}' referenced by index.html`,
      );
    }
  }
});
