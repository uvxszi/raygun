// PeerTube (M46.35): videos and live streams from PeerTube's federated instances, found through
// SepiaSearch (sepiasearch.org, Framasoft's open index of PeerTube) and played from the instance
// that hosts them: its HLS playlist, else its MP4 files. Keyless; adult videos stay out unless
// the app shows 18+ content. It has its own pill in Search.
import { defineProvider, fetchJson, registerProvider } from '@raygun/sdk';
import type { MangaPage, Video } from '@raygun/sdk';

const SEARCH = 'https://sepiasearch.org/api/v1/search/videos';
const COUNT = 24;

type Found = {
  uuid: string;
  name: string;
  url: string;
  thumbnailUrl?: string | null;
  isLive?: boolean;
  nsfw?: boolean;
};
type SearchResponse = { total?: number; data?: Found[] };

type Resolution = { id: number; label?: string };
type FileEntry = { resolution: Resolution; fileUrl?: string };
type VideoDetails = {
  name: string;
  description?: string | null;
  duration?: number;
  publishedAt?: string;
  thumbnailPath?: string | null;
  previewPath?: string | null;
  isLive?: boolean;
  category?: { label?: string } | null;
  tags?: string[];
  account?: { displayName?: string };
  channel?: { displayName?: string };
  streamingPlaylists?: { playlistUrl?: string; files?: FileEntry[] }[];
  files?: FileEntry[];
};

/** A title's id is `<instance host>/<video uuid>`: details are read from that instance. */
const idOf = (found: Found) => {
  const host = /^https?:\/\/([^/?#]+)/i.exec(found.url)?.[1] ?? '';
  return `${host}/${found.uuid}`;
};
const splitId = (id: string) => {
  const slash = id.indexOf('/');
  if (slash < 1) throw new Error('Not a PeerTube video id');
  const host = id.slice(0, slash);
  if (!/^[a-z0-9.-]+(:\d+)?$/i.test(host)) throw new Error('Not a PeerTube instance');
  return { host, uuid: id.slice(slash + 1) };
};

const searchPage = async (query: string, sort: string, page: number): Promise<MangaPage> => {
  const params = [
    `search=${encodeURIComponent(query)}`,
    `start=${(Math.max(1, page) - 1) * COUNT}`,
    `count=${COUNT}`,
    `sort=${sort}`,
    `nsfw=${raygun.content.showNsfw() ? 'both' : 'false'}`,
  ].join('&');
  const result = await fetchJson<SearchResponse>(`${SEARCH}?${params}`);
  const items = (result.data ?? [])
    .filter((found) => raygun.content.showNsfw() || !found.nsfw)
    .map((found) => ({
      id: idOf(found),
      title: found.isLive ? `● ${found.name}` : found.name,
      coverUrl: found.thumbnailUrl ?? null,
      nsfw: found.nsfw ?? false,
    }));
  return { items, hasNextPage: page * COUNT < (result.total ?? 0) };
};

const details = async (id: string): Promise<{ host: string; video: VideoDetails }> => {
  const { host, uuid } = splitId(id);
  const video = await fetchJson<VideoDetails>(
    `https://${host}/api/v1/videos/${encodeURIComponent(uuid)}`,
  );
  return { host, video };
};

/** The instance's HLS playlist first (every quality in one), then its MP4 files, best first. */
const linksOf = (video: VideoDetails): Video[] => {
  const links: Video[] = [];
  for (const playlist of video.streamingPlaylists ?? []) {
    if (playlist.playlistUrl) {
      links.push({ url: playlist.playlistUrl, label: 'HLS (every quality)', format: 'hls' });
    }
  }
  const files = [
    ...(video.files ?? []),
    ...(video.streamingPlaylists ?? []).flatMap((p) => p.files ?? []),
  ]
    .filter((file) => file.fileUrl && /^https?:\/\//.test(file.fileUrl))
    .sort((a, b) => b.resolution.id - a.resolution.id);
  for (const file of files) {
    links.push({
      url: file.fileUrl!,
      label: file.resolution.label ?? `${file.resolution.id}p`,
      quality: file.resolution.id || undefined,
    });
  }
  return links;
};

const peertube = defineProvider({
  kind: 'video.source',
  id: 'videos',
  name: 'PeerTube',
  searchPill: 'PeerTube',

  popular: ({ page }) => searchPage('', '-views', page),

  latest: ({ page }) => searchPage('', '-publishedAt', page),

  search: ({ query, page }) => searchPage(query.trim(), query.trim() ? '-match' : '-views', page),

  async details({ id }) {
    const { host, video } = await details(id);
    const cover = video.previewPath ?? video.thumbnailPath;
    const by = video.channel?.displayName ?? video.account?.displayName;
    const year = Number.parseInt((video.publishedAt ?? '').slice(0, 4), 10);
    return {
      id,
      title: video.name,
      coverUrl: cover ? `https://${host}${cover}` : null,
      description: [by && `By ${by} on ${host}`, video.description].filter(Boolean).join('\n\n'),
      genres: [video.category?.label, ...(video.tags ?? [])].filter((tag): tag is string =>
        Boolean(tag),
      ),
      type: 'movie' as const,
      year: Number.isFinite(year) ? year : null,
      status: video.isLive ? ('ongoing' as const) : ('completed' as const),
      url: `https://${host}/w/${splitId(id).uuid}`,
    };
  },

  episodes: async ({ id }) => ({ episodes: [{ id, name: 'Video' }] }),

  servers: async ({ episodeId }) => {
    const { host, video } = await details(episodeId);
    return { servers: [{ id: episodeId, name: host, videos: linksOf(video) }] };
  },

  videos: async ({ serverId }) => ({ videos: linksOf((await details(serverId)).video) }),
});

registerProvider(peertube);
