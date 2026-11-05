/** Sources' kinds: the pills after All (M46.52). */
export type SourceType = 'repositories' | 'addons' | 'servers' | 'extensions';

export const SOURCE_TYPES: SourceType[] = ['repositories', 'addons', 'servers', 'extensions'];

export const isSourceType = (value: unknown): value is SourceType =>
  typeof value === 'string' && (SOURCE_TYPES as string[]).includes(value);

/**
 * What sources are for, as Sources' and Browse → Sources' pills group them: Audio gathers music
 * (`music.*`), lyrics, podcast and audiobook directories, and with them the servers that bring
 * any (Subsonic, Audiobookshelf, Jellyfin's music).
 */
export type SourceMedia = 'audio' | 'video' | 'manga' | 'novel' | 'other';

export const SOURCE_MEDIA: SourceMedia[] = ['audio', 'video', 'manga', 'novel', 'other'];

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
  return found.size ? SOURCE_MEDIA.filter((media) => found.has(media)) : ['other'];
};
