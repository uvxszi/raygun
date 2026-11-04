// __NAME__: streams (`music.stream`) from the Internet Archive (https://archive.org/details/audio)
// for songs other sources found, from recordings their uploaders put under a Creative Commons
// license or in the public domain. Raygun asks every stream provider for candidates for a song
// (title, artist, length), scores them and plays the best. This one offers the recordings of
// archive items whose title and creator match; when Raygun plays one, `resolve` picks the copy of
// it nearest the user's Audio quality setting.
//
// Extensions run in QuickJS, not a browser or Node: there is no `fetch`, `URL`, `setTimeout` or
// `require`. Use the `raygun` global (requests reach only the manifest's `permissions.net`) and
// the helpers from '@raygun/sdk'. Try a call with
// `pnpm exec raygun-ext test archive candidates title="Maple Leaf Rag" artist="Scott Joplin"`.
import { defineProvider, fetchJson, query, registerProvider } from '@raygun/sdk';
import type { StreamCandidate } from '@raygun/sdk';

const API = 'https://archive.org';
/** Recordings under a Creative Commons license, or marked public domain. */
const OPEN = 'licenseurl:(*creativecommons.org*)';
/** Items whose files are read for one song. */
const ITEMS = 4;

type SearchAnswer = { response?: { docs?: { identifier: string }[] } };
type ItemFile = {
  name: string;
  format?: string;
  original?: string;
  title?: string;
  length?: string;
  creator?: string;
};
type Item = { files?: ItemFile[]; metadata?: { creator?: string | string[] } };

/** The MP3 copies the archive makes of a recording, by their bitrates. */
const KBPS: Record<string, number> = { 'VBR MP3': 200, '128Kbps MP3': 128, '64Kbps MP3': 64 };

// Lucene's special characters would change what a search means.
const escape = (text: string) => text.replace(/[+\-!(){}[\]^"~*?:\\/&|]/g, ' ').trim();
const words = (text: string) => escape(text).toLowerCase().split(/\s+/).filter(Boolean);

/** `length` is seconds ("245.3") or a clock ("4:05"). */
const lengthMs = (length: string | undefined) => {
  const seconds = (length ?? '').split(':').reduce((total, part) => total * 60 + Number(part), 0);
  return Number.isFinite(seconds) && seconds > 0 ? Math.round(seconds * 1000) : null;
};

const encodePath = (path: string) => path.split('/').map(encodeURIComponent).join('/');

// Items read for candidates are read again by `resolve`; the last few are kept.
const items = new Map<string, Promise<Item>>();
const item = (identifier: string) => {
  let found = items.get(identifier);
  if (!found) {
    found = fetchJson<Item>(`${API}/metadata/${encodeURIComponent(identifier)}`);
    found.catch(() => items.delete(identifier));
    items.set(identifier, found);
    if (items.size > 20) items.delete(items.keys().next().value!);
  }
  return found;
};

/** An item's MP3 copies, by the recording (the original file) each was made from. */
const recordings = (files: ItemFile[] = []) => {
  const found = new Map<string, ItemFile[]>();
  for (const file of files) {
    if (file.format === undefined || !(file.format in KBPS)) continue;
    const recording = file.original ?? file.name;
    found.set(recording, [...(found.get(recording) ?? []), file]);
  }
  return found;
};

registerProvider(
  defineProvider({
    kind: 'music.stream',
    id: 'archive',
    name: '__NAME__',

    async candidates({ title, artist, limit }) {
      const wanted = words(title);
      if (wanted.length === 0) return { candidates: [] };
      const byArtist = artist ? ` AND creator:(${escape(artist)})` : '';
      const params = query({
        q: `title:(${escape(title)})${byArtist} AND mediatype:audio AND ${OPEN}`,
        'fl[]': 'identifier',
        rows: ITEMS,
        'sort[]': 'downloads desc',
        output: 'json',
      });
      const answer = await fetchJson<SearchAnswer>(`${API}/advancedsearch.php?${params}`);
      const candidates: StreamCandidate[] = [];
      // Every item's files at once; one that fails is left out rather than failing the rest.
      const all = Promise.allSettled(
        (answer.response?.docs ?? []).map(async ({ identifier }) => {
          const { files, metadata } = await item(identifier);
          const creator = metadata?.creator;
          for (const [recording, copies] of recordings(files)) {
            const best = copies.find((copy) => copy.title) ?? copies[0];
            const named = best?.title ?? recording.replace(/\.[^.]+$/, '');
            if (!best || !wanted.every((word) => words(named).includes(word))) continue;
            candidates.push({
              // `resolve` gets this back when Raygun plays the candidate.
              id: `${identifier}/${recording}`,
              title: named,
              artist: best.creator ?? (Array.isArray(creator) ? creator[0] : creator) ?? null,
              durationMs: lengthMs(best.length),
              quality: copies.map((copy) => copy.format).join(', '),
              bitrateKbps: Math.max(...copies.map((copy) => KBPS[copy.format ?? ''] ?? 0)),
            });
          }
        }),
      );
      // Raygun waits 12 s for candidates: a slow item shouldn't cost the ones already read.
      await Promise.race([all, raygun.time.sleep(8000)]);
      return { candidates: candidates.slice(0, limit) };
    },

    // Candidates without a `source` are resolved when they play. A source whose stream is known
    // at once can put it in the candidate (`source`) and leave `resolve` out.
    async resolve({ id, targetKbps }) {
      const slash = id.indexOf('/');
      const identifier = id.slice(0, slash);
      const copies = recordings((await item(identifier)).files).get(id.slice(slash + 1)) ?? [];
      // The nearest bitrate to the setting; the best for `null`, the VBR copy when it's left out.
      const kbps = (copy: ItemFile) => KBPS[copy.format ?? ''] ?? 0;
      const distance = (copy: ItemFile) =>
        targetKbps === null ? -kbps(copy) : Math.abs(kbps(copy) - (targetKbps ?? 200));
      const chosen = [...copies].sort((a, b) => distance(a) - distance(b))[0];
      if (!chosen) throw new Error('This recording is no longer on the Internet Archive');
      return {
        source: {
          kind: 'url',
          url: `${API}/download/${encodeURIComponent(identifier)}/${encodePath(chosen.name)}`,
          headers: {},
        },
      };
    },
  }),
);
