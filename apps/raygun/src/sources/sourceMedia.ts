/** Sources' kinds: the pills after All (M46.52). */
export type SourceType = 'repositories' | 'addons' | 'servers' | 'extensions';

export const SOURCE_TYPES: SourceType[] = ['repositories', 'addons', 'servers', 'extensions'];

export const isSourceType = (value: unknown): value is SourceType =>
  typeof value === 'string' && (SOURCE_TYPES as string[]).includes(value);

/**
 * What sources are for, as Sources' and Browse → Sources' pills group them: Audio gathers music
 * (`music.*`), lyrics, podcast and audiobook directories, and with them the servers that bring
 * any (Subsonic, Audiobookshelf, Jellyfin's music). Data gathers what describes rather than
 * plays: details, tags, charts, lyrics, subtitles and ratings, beside what they're for.
 */
export type SourceMedia = 'audio' | 'video' | 'manga' | 'novel' | 'data' | 'other';

export const SOURCE_MEDIA: SourceMedia[] = ['audio', 'video', 'manga', 'novel', 'data', 'other'];

/** Provider kinds that are data: details, tags, charts, recommendations, lyrics, subtitles. */
const DATA_KINDS = new Set([
  'music.metadata',
  'music.info',
  'music.tags',
  'music.charts',
  'music.recommend',
  'lyrics',
  'stremio.subtitles',
  'stremio.meta',
]);

/** Whether a provider kind is data (as well as being for what `mediaOfKind` says). */
export const isDataKind = (kind: string) => DATA_KINDS.has(kind);

/**
 * Whether something providing these kinds is a source of data. Sources that play music find
 * their songs through `music.metadata` too, so that kind alone makes one a data source only when
 * it plays nothing (`music.stream`).
 */
export const isDataSource = (provides: string[]) =>
  provides.some(
    (kind) => isDataKind(kind) && !(kind === 'music.metadata' && provides.includes('music.stream')),
  );

export const isSourceMedia = (value: unknown): value is SourceMedia =>
  typeof value === 'string' && (SOURCE_MEDIA as string[]).includes(value);

/** What a provider kind (`music.metadata`, `audiobook.directory`…) is for. */
export const mediaOfKind = (kind: string): SourceMedia => {
  if (
    kind.startsWith('music.') ||
    kind.startsWith('podcast.') ||
    kind.startsWith('audiobook.') ||
    kind === 'lyrics'
  ) {
    return 'audio';
  }
  if (kind.startsWith('video.') || kind.startsWith('stremio.')) return 'video';
  if (kind === 'manga.source') return 'manga';
  if (kind === 'novel.source') return 'novel';
  return 'other';
};

/** What something providing these kinds is for, in the pills' order. */
export const mediaOf = (provides: string[]): SourceMedia[] => {
  const found = new Set(provides.map(mediaOfKind));
  if (isDataSource(provides)) found.add('data');
  return found.size ? SOURCE_MEDIA.filter((media) => found.has(media)) : ['other'];
};

/** What a repository's sources (each one's `provides`) are for, in the pills' order. */
export const mediaOfAll = (sources: string[][]): SourceMedia[] => {
  const found = new Set(sources.flatMap(mediaOf));
  return found.size ? SOURCE_MEDIA.filter((media) => found.has(media)) : ['other'];
};

/**
 * What a Stremio addon is for: videos, and data when it gives subtitles or details (with their
 * ratings).
 */
export const addonMedia = (resources: string[]): SourceMedia[] =>
  resources.some((resource) => resource === 'subtitles' || resource === 'meta')
    ? ['video', 'data']
    : ['video'];
