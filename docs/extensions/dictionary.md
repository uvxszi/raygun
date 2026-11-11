# Dictionaries: `dictionary`

A dictionary says what the words a reader selects in a book mean. Raygun looks them up in its own
dictionaries (Wiktionary for every language, the Free Dictionary API for English) and in every
dictionary extension, all at once, and merges the answers in the order Settings → Reader →
Dictionary gives them: senses grouped by part of speech, a sense two dictionaries share shown once,
and each dictionary credited under the answer with its licence and a link to its full entry.

## Calls

| Call     | Receives               | Answers                          | Required |
| -------- | ---------------------- | -------------------------------- | -------- |
| `define` | `{ word, language }`   | `{ entries: DictionaryEntry[] }` | yes      |

`language` is the words' language as a BCP 47 tag: the book's, unless the reader chose another.
`define` has 12 s. Raygun keeps answers on the device (a month for a word found, three days for one
that wasn't), so a word looked up before opens offline.

A word that isn't found as selected is asked again in other forms: in lower case, without an
elided article (`l’homme` → `homme`) and, in English, without its ending (`studies` → `study`).

### The answer

```ts
import type { DictionaryEntry } from '@raygun/sdk';

const entry: DictionaryEntry = {
  word: 'maison',
  language: 'fr',
  languageName: 'French',
  pronunciations: [{ ipa: '/mɛ.zɔ̃/', label: 'Paris' }],
  meanings: [
    {
      partOfSpeech: 'noun',
      definitions: [{ text: 'house', examples: ['La maison est grande.'] }, 'home'],
      synonyms: ['demeure'],
    },
  ],
  etymology: 'From Old French maison, from Latin mansiō.',
  url: 'https://example.org/maison',
  license: { name: 'CC BY-SA 4.0', url: 'https://creativecommons.org/licenses/by-sa/4.0/' },
};

export { entry };
```

- An entry needs a `word` and a meaning with a definition; a definition without examples can be
  its text alone. Answer `{ entries: [] }` for a word or a language the dictionary doesn't know.
- `formOf: { lemma, note }` says the word is a form of another (`went`: `simple past of go`):
  the reader then shows the lemma's entry. With `only: false` the word has senses of its own,
  which are shown, with the lemma offered.
- `audioUrl` and `url` must be `http(s)`. Recordings play through Raygun's proxy.
- `license` is shown with the answer: name the licence the text comes under.

## Example: the Free Dictionary API

English words from the Free Dictionary API, whose text is Wiktionary's. Raygun has it built in;
start from here for a dictionary of another language.

```sh
pnpm raygun-ext init ../my-extensions/free-dictionary --kind dictionary --id dev.example.free-dictionary --name "Free Dictionary"
```

<!-- example: dictionary dev.example.free-dictionary Free Dictionary -->

```json
{
  "id": "dev.example.free-dictionary",
  "name": "Free Dictionary",
  "version": "0.1.0",
  "apiVersion": 1,
  "runtime": "js",
  "entry": "dist/index.js",
  "description": "English words from the Free Dictionary API, with Wiktionary's text.",
  "website": "https://dictionaryapi.dev",
  "provides": ["dictionary"],
  "permissions": { "net": ["api.dictionaryapi.dev"] },
  "contentRating": "safe"
}
```

```ts
// Free Dictionary: a dictionary (`dictionary`) for the book reader's word lookups, from the Free
// Dictionary API (https://dictionaryapi.dev), whose text is Wiktionary's (CC BY-SA). Raygun asks
// every dictionary for the words a reader selects, in their language, and merges the answers;
// answer `{ entries: [] }` for a word or a language this one doesn't know. Start from here for a
// dictionary of another language, or of a field's own words.
//
// Extensions run in QuickJS, not a browser or Node: there is no `fetch`, `URL`, `setTimeout` or
// `require`. Use the `raygun` global (requests reach only the manifest's `permissions.net`) and
// the helpers from '@raygun/sdk'. Try a call with
// `pnpm exec raygun-ext test words define word=happy language=en`.
import { defineProvider, registerProvider } from '@raygun/sdk';
import type { DictionaryEntry, DictionaryMeaning } from '@raygun/sdk';

const API = 'https://api.dictionaryapi.dev/api/v2/entries/en';

type Answer = {
  word: string;
  phonetics?: { text?: string; audio?: string }[];
  meanings?: {
    partOfSpeech: string;
    definitions: { definition: string; example?: string }[];
    synonyms?: string[];
    antonyms?: string[];
  }[];
  sourceUrls?: string[];
  license?: { name: string; url?: string };
}[];

registerProvider(
  defineProvider({
    kind: 'dictionary',
    id: 'words',
    name: 'Free Dictionary',

    async define({ word, language }) {
      if (language.split('-')[0] !== 'en') return { entries: [] };
      const response = await raygun.http.fetch(`${API}/${encodeURIComponent(word)}`);
      // The API answers 404 for a word it doesn't know.
      if (response.status === 404) return { entries: [] };
      if (!response.ok) throw new Error(`HTTP ${response.status} from the Free Dictionary API`);
      const answer = await response.json<Answer>();
      const entries: DictionaryEntry[] = answer.map((entry) => {
        const meanings: DictionaryMeaning[] = (entry.meanings ?? []).map((meaning) => ({
          partOfSpeech: meaning.partOfSpeech,
          definitions: meaning.definitions.map((definition) => ({
            text: definition.definition,
            examples: definition.example ? [definition.example] : [],
          })),
          synonyms: meaning.synonyms ?? [],
          antonyms: meaning.antonyms ?? [],
        }));
        return {
          word: entry.word,
          language: 'en',
          pronunciations: (entry.phonetics ?? []).map((phonetic) => ({
            ipa: phonetic.text ?? null,
            audioUrl: phonetic.audio || null,
          })),
          meanings,
          url: entry.sourceUrls?.[0] ?? null,
          license: entry.license ?? null,
        };
      });
      return { entries };
    },
  }),
);
```

## Try it

```sh
pnpm exec raygun-ext test words define word=happy language=en
```
