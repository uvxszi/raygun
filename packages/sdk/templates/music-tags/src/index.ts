// __NAME__: song genres (`music.tags`) from MusicBrainz (https://musicbrainz.org), the open
// music database: the recording's own tags, else its artist's genres. Raygun asks tag providers
// about saved and played songs in the background, maps the names it gets onto its own genres and
// derives moods from them: they fill genre pages, the Tags column and Made for You. Answer no
// tags when unsure; a wrong genre does more harm than none.
//
// Extensions run in QuickJS, not a browser or Node: there is no `fetch`, `URL`, `setTimeout` or
// `require`. Use the `raygun` global (requests reach only the manifest's `permissions.net`) and
// the helpers from '@raygun/sdk'. Try a call with
// `pnpm exec raygun-ext test genres tags title="Maple Leaf Rag" artist="Scott Joplin"`.
import { defineProvider, query, registerProvider } from '@raygun/sdk';
import type { MusicTag } from '@raygun/sdk';

const API = 'https://musicbrainz.org/ws/2';
// MusicBrainz asks every client to name itself and a way to reach its author: put yours here.
const CONTACT = 'https://example.org/your-extension';
const HEADERS = { 'User-Agent': `__ID__/0.1.0 ( ${CONTACT} )`, Accept: 'application/json' };
/** MusicBrainz takes one request a second from each address. */
const INTERVAL_MS = 1100;

type Tag = { name: string; count?: number };
type Recording = {
  title: string;
  tags?: Tag[];
  'artist-credit'?: { name: string; artist?: { id: string } }[];
};

// Lucene's special characters would change what a search means.
const escape = (text: string) => text.replace(/[+\-!(){}[\]^"~*?:\\/&|]/g, ' ').trim();
const simple = (text: string) =>
  text
    .toLowerCase()
    .replace(/[^\p{L}\p{N}]+/gu, ' ')
    .trim();

let lastRequest = 0;

/** One request at a time, a second apart, as MusicBrainz asks. */
const paced = async <T>(path: string, params: Record<string, string | number>): Promise<T> => {
  const wait = lastRequest + INTERVAL_MS - Date.now();
  if (wait > 0) await raygun.time.sleep(wait);
  lastRequest = Date.now();
  const response = await raygun.http.fetch(`${API}/${path}?${query({ ...params, fmt: 'json' })}`, {
    headers: HEADERS,
  });
  if (response.status === 503) throw new Error('MusicBrainz is busy; try again in a moment');
  if (!response.ok) throw new Error(`HTTP ${response.status} from MusicBrainz`);
  return response.json<T>();
};

/** Tags weighted by their votes, the most voted 1. */
const weighted = (tags: Tag[] = []): MusicTag[] => {
  const most = Math.max(1, ...tags.map((tag) => tag.count ?? 1));
  return tags
    .filter((tag) => (tag.count ?? 1) > 0)
    .map((tag) => ({ name: tag.name, weight: (tag.count ?? 1) / most }));
};

registerProvider(
  defineProvider({
    kind: 'music.tags',
    id: 'genres',
    name: '__NAME__',

    async tags({ title, artist }) {
      const search = [`recording:"${escape(title)}"`];
      if (artist) search.push(`artist:"${escape(artist)}"`);
      const found = await paced<{ recordings?: Recording[] }>('recording', {
        query: search.join(' AND '),
        limit: 10,
      });
      // Only recordings of this very song, by this artist when one is known.
      const matches = (found.recordings ?? []).filter(
        (recording) =>
          simple(recording.title) === simple(title) &&
          (!artist ||
            (recording['artist-credit'] ?? []).some(
              (credit) => simple(credit.name) === simple(artist),
            )),
      );
      // The same song is on many releases; some of its copies carry the tags.
      const tagged = matches.find((recording) => recording.tags?.length);
      if (tagged) return { tags: weighted(tagged.tags) };
      // Nobody tagged the recording: its artist's genres stand in.
      const artistId = matches[0]?.['artist-credit']?.[0]?.artist?.id;
      if (!artistId) return { tags: [] };
      const byArtist = await paced<{ genres?: Tag[] }>(`artist/${artistId}`, { inc: 'genres' });
      return { tags: weighted(byArtist.genres) };
    },
  }),
);
