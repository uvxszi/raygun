# Translating Raygun

Raygun's interface text lives in one JSON file per language in
[`apps/raygun/src/i18n/locales/`](../apps/raygun/src/i18n/locales/).
- **`en.json`** is the source: every key the app uses, in English.
- **Other files** hold translations. Any key missing from them shows in English, so a partial translation is fine.

People pick a language in **Settings → Appearance → Language**. "System default" follows the phone's language.

## Editing a translation

1. Open the language's file, for example `de.json` or `pt-BR.json`.
2. Copy any key you want to translate from `en.json`, keeping its place in the nesting (`settings.theme` lives in `"settings": { "theme": ... }`), and write the translation.
3. Keep placeholders exactly as they are. `{{count}}`, `{{name}}` and the like are filled in by the app. You can move them within the sentence, but don't rename or translate them.
4. Check your work with `pnpm --filter @raygun/app test locales`. It fails on keys that don't exist in English, empty strings and unknown placeholders.

## Plurals

Keys ending in `_one` and `_other` are plural forms ([i18next plurals](https://www.i18next.com/translation-function/plurals)). Add the forms your language uses (`_zero`, `_one`, `_two`, `_few`, `_many`, `_other`) next to them:

```json
"summary_one": "{{count}} Übersetzer",
"summary_other": "{{count}} Übersetzer"
```

## Adding a language

Create `<tag>.json` with the [BCP 47 tag](https://www.w3.org/International/questions/qa-choosing-language-tags) as its name. Use the plain language (`fr`, `ja`) unless the regional variants differ a lot (`pt-BR`, `zh-CN`, `zh-TW`). Start with `{}` and add keys; the language appears in the picker once the file exists.

## Where the existing translations come from

Many strings were not translated for Raygun. They were copied from other open-source apps that already ship the same English text translated: Nuclear, Mihon, aniyomi, CloudStream, Shosetsu and RiPlay. [`SOURCES.md`](../apps/raygun/src/i18n/locales/SOURCES.md) lists, per language, how many strings came from each app and under which licence. All of those licences can be combined with Raygun's AGPL-3.0; [`NOTICE`](../NOTICE) credits the Apache-2.0 ones.

Maintainers can repeat the import after adding English strings. It needs the reference apps cloned into `reference/`:

```bash
pnpm i18n:import
```

The import only fills keys that are missing, so it never overwrites a translation someone wrote or corrected. It only takes strings whose English text matches Raygun's exactly, and nothing is machine-translated.

## Not yet supported

- **Right-to-left layout** (Arabic, Hebrew, Persian, Urdu): the text is translated, but the layout stays left-to-right until those languages are far enough along to be tested.
- **Extensions' own text** (source names, setting labels) comes from each extension and isn't translated here.
