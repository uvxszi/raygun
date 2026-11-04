// Videos from Commons: every file is a title with one episode, played from Commons' WebM copies
// at each height (and the file itself when it is WebM), with the subtitles people wrote for it.
// Popular and latest list the videos once shown as Commons' Media of the day, a reviewed set.
import { defineProvider } from '@raygun/sdk';
import type { FilterValues, MangaPage, SourceFilter, Video, VideoSubtitle } from '@raygun/sdk';

import {
  COMMONS,
  MEDIA_HEADERS,
  adult,
  clean,
  creatorOf,
  field,
  fileByName,
  nameOf,
  openlyLicensed,
  searchFiles,
  titleOf,
} from './commons';
import type { CommonsFile, FileInfo } from './commons';

const PAGE_SIZE = 24;
const FEATURED = 'incategory:"Media_of_the_day"';

const COLLECTIONS = [
  { value: 'all', label: 'All of Commons' },
  { value: 'featured', label: 'Media of the day' },
];
const SORTS = [
  { value: 'relevance', label: 'Best match' },
  { value: 'incoming_links_desc', label: 'Most used' },
  { value: 'create_timestamp_desc', label: 'Newest' },
];

// Search syntax would turn a viewer's words into a different search.
const words = (text: string) =>
  text
    .replace(/[\\"~*?:()[\]{}^|/-]/g, ' ')
    .replace(/\s+/g, ' ')
    .trim();

/** Categories about the file (its kind, format, size, origin or captions), not what it shows. */
const isTopic = (name: string) =>
  !/(videos?$|^videos?\b|^audio\b|^\d{4}(-\d\d){0,2}$|^media\b|^files\b|^uploaded|self-published|^cc-|^pd-|resolution|^webm|^ogg|^vp\d|^opus|^av1|captioning|^license|^quality|^featured|^valued|^pages with|^commons|wikipedia|youtube|flickr|vimeo)/i.test(
    name,
  );

const listed = (file: CommonsFile) => {
  const info = file.videoinfo?.[0];
  return (
    info !== undefined &&
    info.mediatype === 'VIDEO' &&
    openlyLicensed(info) &&
    (!adult(info) || raygun.content.showNsfw())
  );
};

const listPage = async (search: string, sort: string, number: number): Promise<MangaPage> => {
  const offset = (Math.max(1, number) - 1) * PAGE_SIZE;
  const { files, more } = await searchFiles(search, { limit: PAGE_SIZE, offset, sort });
  return {
    items: files.filter(listed).map((file) => ({
      id: nameOf(file),
      title: titleOf(file),
      coverUrl: clean(file.videoinfo?.[0]?.thumburl) ?? null,
      nsfw: file.videoinfo?.[0] ? adult(file.videoinfo[0]) : false,
    })),
    hasNextPage: more,
  };
};

const pick = (filters: FilterValues | undefined, key: string, options: { value: string }[]) => {
  const value = filters?.[key];
  return typeof value === 'string' && options.some((option) => option.value === value)
    ? value
    : null;
};

/** WebM copies' heights come from their names ("1080p.vp9.webm"). */
const nominalHeight = (key: string | undefined) => Number.parseInt(key ?? '', 10) || undefined;

/**
 * The file's videos, the best first: Commons' VP9 and VP8 WebM copies (players on phones and in
 * browsers read them; its Ogg Theora and MPEG-4 copies they often can't), then the file itself
 * when it is WebM.
 */
export const videosOf = (info: FileInfo): Video[] => {
  const subtitles: VideoSubtitle[] = (info.timedtext ?? [])
    .filter((track) => track.src && track.kind !== 'metadata')
    .map((track) => ({ url: track.src, lang: track.srclang, label: track.label }));
  const copies = (info.derivatives ?? [])
    .filter((copy) => /^video\/webm/.test(copy.type ?? '') && /vp9|vp8|av01/i.test(copy.type ?? ''))
    .map((copy): Video => {
      const height = nominalHeight(copy.transcodekey) ?? copy.height;
      const codec = /vp9/i.test(copy.type ?? '')
        ? 'VP9'
        : /av01/i.test(copy.type ?? '')
          ? 'AV1'
          : 'VP8';
      return {
        url: copy.src,
        label: height ? `${height}p · ${codec}` : codec,
        quality: height,
        headers: MEDIA_HEADERS,
        subtitles,
      };
    });
  const original = clean(info.url);
  if (original && info.mime === 'video/webm') {
    copies.push({
      url: original,
      label: info.height ? `Original · ${info.width}×${info.height}` : 'Original',
      quality: info.height || undefined,
      headers: MEDIA_HEADERS,
      subtitles,
    });
  }
  return copies.sort((a, b) => (b.quality ?? 0) - (a.quality ?? 0));
};

const filters: SourceFilter[] = [
  { kind: 'select', key: 'collection', label: 'Collection', options: COLLECTIONS },
  { kind: 'select', key: 'sort', label: 'Sort', options: SORTS },
];

const server = async (name: string) => {
  const info = (await fileByName(name)).videoinfo![0]!;
  const videos = videosOf(info);
  if (videos.length === 0)
    throw new Error('Commons has no copy of this video that Raygun can play');
  return { id: name, name: 'Wikimedia Commons', videos };
};

export const videos = defineProvider({
  kind: 'video.source',
  id: 'videos',
  name: 'Wikimedia Commons',

  popular: ({ page: number }) =>
    listPage(`filetype:video ${FEATURED}`, 'incoming_links_desc', number),

  latest: ({ page: number }) =>
    listPage(`filetype:video ${FEATURED}`, 'create_timestamp_desc', number),

  search: ({ query, page: number, filters: values }) => {
    const asked = words(query);
    const featured = pick(values, 'collection', COLLECTIONS) === 'featured' || asked === '';
    const sort = pick(values, 'sort', SORTS) ?? (asked ? 'relevance' : 'incoming_links_desc');
    const search = ['filetype:video', featured && FEATURED, asked].filter(Boolean).join(' ');
    return listPage(search, sort, number);
  },

  filters: async () => filters,

  async details({ id }) {
    const file = await fileByName(id);
    const info = file.videoinfo![0]!;
    const about = field(info, 'ImageDescription');
    const licence = field(info, 'LicenseShortName');
    const creator = creatorOf(info);
    const year = /\b(1[89]\d\d|20\d\d)\b/.exec(
      `${field(info, 'DateTimeOriginal')} ${field(info, 'DateTime')}`,
    )?.[1];
    const topics = field(info, 'Categories')
      .split('|')
      .map((name) => name.trim())
      .filter((name) => name && isTopic(name));
    return {
      id,
      title: titleOf(file),
      coverUrl: clean(info.thumburl) ?? null,
      description:
        [
          about.slice(0, 2000),
          [licence && `Licence: ${licence}`, creator && `By ${creator}`]
            .filter(Boolean)
            .join(' · '),
        ]
          .filter(Boolean)
          .join('\n\n') || null,
      genres: topics.slice(0, 6),
      directors: creator ? [creator] : [],
      type: 'other',
      year: year ? Number(year) : null,
      status: 'completed',
      url: `${COMMONS}/wiki/File:${encodeURIComponent(id.replace(/ /g, '_'))}`,
    };
  },

  episodes: async ({ id }) => ({ episodes: [{ id, name: 'Video' }] }),

  servers: async ({ episodeId }) => ({ servers: [await server(episodeId)] }),

  videos: async ({ serverId }) => ({ videos: (await server(serverId)).videos }),
});
