// __NAME__: a dictionary (`dictionary`) for the book reader's word lookups, from the Free
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
    name: '__NAME__',

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
