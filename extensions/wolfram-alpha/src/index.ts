// Wolfram|Alpha (wolframalpha.com): answers to what's typed in Search from the Full Results API
// (https://products.wolframalpha.com/api/documentation), under its terms
// (https://products.wolframalpha.com/api/termsofuse): each answer links to its results page on
// wolframalpha.com and credits Wolfram|Alpha, and nothing is kept (Raygun keeps no extension's
// answers).
//
// Each person creates their own AppID (free for non-commercial use, at developer.wolframalpha.com)
// and enters it in this source's settings. The setting is required, so Raygun asks nothing while
// it's empty, and an AppID Wolfram|Alpha refuses says so with `needsSetup`.
//
// Results and information from Wolfram|Alpha are not a certified or definitive source of
// information that can be relied on for legal, financial, medical, life-safety or any other
// critical purposes.
import { defineProvider, needsSetup, query, registerProvider } from '@raygun/sdk';
import type { AnswerItem } from '@raygun/sdk';

const API = 'https://api.wolframalpha.com/v2/query';
const SITE = 'https://www.wolframalpha.com';
/** Pods shown as rows at most, after the interpretation and the result. */
const MOST_ROWS = 12;

type Subpod = { title?: string; plaintext?: string };
type Pod = { title?: string; id?: string; primary?: boolean; error?: unknown; subpods?: Subpod[] };
type QueryResult = {
  queryresult?: {
    success?: boolean;
    error?: boolean | { code?: string | number; msg?: string };
    pods?: Pod[];
  };
};

/** The person's AppID; without one the source needs setup. */
const appId = (): string => {
  const typed = raygun.settings.get('appId');
  const id = typeof typed === 'string' ? typed.trim() : '';
  if (!id) {
    return needsSetup(
      "Wolfram|Alpha needs your own AppID: create one at developer.wolframalpha.com and enter it in this source's settings",
    );
  }
  return id;
};

/** A pod's text: its subpods' plain text, one per line. */
const textOf = (pod: Pod): string =>
  (pod.subpods ?? [])
    .map((subpod) => (typeof subpod.plaintext === 'string' ? subpod.plaintext.trim() : ''))
    .filter((text) => text !== '')
    .join('\n');

/** Where the answer is on wolframalpha.com: every answer links to it. */
const pageOf = (words: string) => `${SITE}/input?${query({ i: words })}`;

registerProvider(
  defineProvider({
    kind: 'answers',
    id: 'answers',
    name: 'Wolfram|Alpha',

    async answer({ query: words }) {
      const id = appId();
      const units = raygun.settings.get('units') === 'nonmetric' ? 'nonmetric' : 'metric';
      const response = await raygun.http.fetch(
        `${API}?${query({ appid: id, input: words, output: 'json', format: 'plaintext', units })}`,
      );
      if (response.status === 401 || response.status === 403) {
        return needsSetup("Wolfram|Alpha refused the AppID: check it in this source's settings");
      }
      if (!response.ok) throw new Error(`HTTP ${response.status} from Wolfram|Alpha`);
      const result = (await response.json<QueryResult>()).queryresult;
      const error = result?.error;
      if (error && typeof error === 'object') {
        const message = typeof error.msg === 'string' ? error.msg : 'an error';
        if (/appid/i.test(message)) {
          return needsSetup(
            `Wolfram|Alpha refused the AppID (${message}): check it in this source's settings`,
          );
        }
        throw new Error(`Wolfram|Alpha answered with ${message}`);
      }
      // Words Wolfram|Alpha didn't understand: no answer.
      if (!result?.success) return { answers: [] };
      const pods = (result.pods ?? []).filter((pod) => !pod.error && textOf(pod) !== '');
      const input = pods.find((pod) => pod.id === 'Input');
      const main =
        pods.find((pod) => pod.primary) ??
        pods.find((pod) => pod.id === 'Result') ??
        pods.find((pod) => pod !== input);
      if (!main) return { answers: [] };
      const page = pageOf(words);
      const answer: AnswerItem = {
        interpretation: input ? textOf(input) : null,
        result: textOf(main),
        rows: pods
          .filter((pod) => pod !== input && pod !== main)
          .slice(0, MOST_ROWS)
          .map((pod) => ({ label: pod.title?.trim() || 'More', value: textOf(pod) })),
        url: page,
        credits: [{ name: 'Wolfram|Alpha', url: page }],
      };
      return { answers: [answer] };
    },
  }),
);
