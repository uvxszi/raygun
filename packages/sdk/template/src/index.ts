// __NAME__: a Raygun extension. Build with `raygun-ext build`; while developing, `raygun-ext dev
// --android` reinstalls it into the app on the emulator on every save.
//
// Extensions run in QuickJS, not a browser or Node: there is no `fetch`, `URL`, `setTimeout` or
// `require`. Use the `raygun` global (network access is limited to the manifest's
// `permissions.net`) and helpers from '@raygun/sdk'.
import { defineProvider, fetchJson, registerProvider } from '@raygun/sdk';
import type { Track } from '@raygun/sdk';

type ApiSong = { id: number; name: string; artist: string; streamUrl: string };

const search = defineProvider({
  kind: 'music.metadata',
  id: 'search',
  name: '__NAME__',
  async search({ query, limit }) {
    const songs = await fetchJson<ApiSong[]>(
      `https://api.example.org/search?q=${encodeURIComponent(query)}&limit=${limit}`,
    );
    const tracks: Track[] = songs.map((song) => ({
      id: song.id,
      title: song.name,
      artist: song.artist,
      source: { kind: 'url', url: song.streamUrl, headers: {} },
    }));
    return { tracks };
  },
});

registerProvider(search);
