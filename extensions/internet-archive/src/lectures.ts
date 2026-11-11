// MIT OpenCourseWare's video lectures on the Internet Archive (collection `mit_ocw`), as a video
// source with a Search pill of its own. Only the courses MIT uploaded itself (from OpenCourseWare's
// own account, `ocw-prod@mit.edu`) are listed, each under the Creative Commons licence its record
// names (most CC BY-NC-SA); copies others uploaded, or a record without a Creative Commons
// licence, are left out, and an item reached another way is held to the same rule. A course is a
// series: its lectures are the episodes, with MIT's subtitles.
import { defineProvider, selectValue } from '@raygun/sdk';
import type { SelectFilter, Video, VideoEpisode, VideoServer, VideoSubtitle } from '@raygun/sdk';

import {
  API,
  cover,
  escapeQuery,
  fetchItem,
  fileUrl,
  firstString,
  plainText,
  searchPage,
  subjectsOf,
} from './archive';
import type { ItemFile, ItemMetadata } from './archive';

/** MIT OpenCourseWare's own account on the Internet Archive. */
const MIT = 'ocw-prod@mit.edu';
const CREATIVE_COMMONS = /^https?:\/\/creativecommons\.org\/(?:licenses|publicdomain)\//i;
/** The search terms for MIT's own uploads under a Creative Commons licence. */
const LECTURES = `collection:(mit_ocw) AND mediatype:(movies) AND licenseurl:*creativecommons* AND uploader:"${MIT}"`;

const SORT: SelectFilter = {
  kind: 'select',
  key: 'sort',
  label: 'Sort',
  options: [
    { value: 'downloads desc', label: 'Most viewed' },
    { value: 'date desc', label: 'Newest courses' },
    { value: 'addeddate desc', label: 'Recently added' },
    { value: 'titleSorter asc', label: 'Title' },
  ],
  default: 'downloads desc',
};

/** Whether an item is one of MIT's own uploads under a Creative Commons licence. */
const isOpen = (item: ItemMetadata) => {
  const meta = item.metadata ?? {};
  const collections = [meta.collection ?? []].flat();
  return (
    collections.includes('mit_ocw') &&
    meta.uploader?.trim().toLowerCase() === MIT &&
    CREATIVE_COMMONS.test(meta.licenseurl?.trim() ?? '')
  );
};

const course = async (identifier: string): Promise<ItemMetadata> => {
  const item = await fetchItem(identifier);
  if (!item.metadata) throw new Error('The Internet Archive has no such item');
  if (!isOpen(item)) {
    throw new Error(
      "This isn't one of MIT OpenCourseWare's own uploads under a Creative Commons licence, so Raygun doesn't offer it",
    );
  }
  return item;
};

/** "http://creativecommons.org/licenses/by-nc-sa/3.0/us/" → "CC BY-NC-SA 3.0 US". */
export const licenceName = (url: string) => {
  const parts =
    /creativecommons\.org\/(licenses|publicdomain)\/([a-z-]+)\/([\d.]+)(?:\/([a-z]{2}))?/i.exec(
      url,
    );
  if (!parts) return 'Creative Commons';
  const [, kind = '', name = '', version = '', port] = parts;
  if (kind.toLowerCase() === 'publicdomain') {
    return name === 'zero' ? `CC0 ${version}` : 'Public Domain Mark';
  }
  return `CC ${name.toUpperCase()} ${version}${port ? ` ${port.toUpperCase()}` : ''}`;
};

/** MP4 files Android's player handles; RealMedia and Ogg copies are left out. */
const isPlayable = (file: ItemFile) =>
  /\.(mp4|m4v)$/i.test(file.name) && /h\.264|mpeg4|mpeg-4/i.test(file.format ?? '');

/**
 * A lecture's files share a base name: "MIT6_034F10_lec01_300k" for its MP4 and subtitles, and
 * an older lecture's "_512kb" copy made from its RealMedia original.
 */
const lectureOf = (name: string) => name.replace(/\.[^./]+$/, '').replace(/_512kb$/i, '');

/** Names in their natural order: "lec2" before "lec10", "lec12" before "lec12A". */
const naturally = (one: string, other: string) => {
  const parts = (name: string) =>
    name
      .toLowerCase()
      .replace(/[_.\s-]+/g, ' ')
      .match(/\d+|\D+/g) ?? [];
  const [a, b] = [parts(one), parts(other)];
  for (let at = 0; at < Math.min(a.length, b.length); at += 1) {
    const [x, y] = [a[at]!, b[at]!];
    if (x === y) continue;
    if (/^\d/.test(x) && /^\d/.test(y) && Number(x) !== Number(y)) return Number(x) - Number(y);
    return x < y ? -1 : 1;
  }
  return a.length - b.length;
};

/** Playable files by lecture, in the course's order. */
const lectureFiles = (item: ItemMetadata): Map<string, ItemFile[]> => {
  const groups = new Map<string, ItemFile[]>();
  for (const file of item.files ?? []) {
    if (!isPlayable(file)) continue;
    const key = lectureOf(
      file.source === 'derivative' && file.original ? file.original : file.name,
    );
    groups.set(key, [...(groups.get(key) ?? []), file]);
  }
  return new Map([...groups].sort(([a], [b]) => naturally(a, b)));
};

/** A lecture's name: the one its files carry, else "Lecture 3" from "…_lec03_…". */
const lectureName = (item: ItemMetadata, lecture: string) => {
  const titled = (item.files ?? []).find(
    (file) => file.title?.trim() && lectureOf(file.name) === lecture,
  );
  if (titled?.title) return titled.title.trim();
  const number = /lec(?:ture)?[\s_-]*0*(\d+[a-z]?)/i.exec(lecture)?.[1];
  return number ? `Lecture ${number.toUpperCase()}` : lecture.replace(/_/g, ' ');
};

/** `<identifier>/<lecture>`; the identifier never holds a slash. */
const splitEpisode = (episodeId: string) => {
  const slash = episodeId.indexOf('/');
  return slash < 0
    ? { identifier: episodeId, lecture: null }
    : { identifier: episodeId.slice(0, slash), lecture: episodeId.slice(slash + 1) };
};

const resolveServer = async (episodeId: string): Promise<VideoServer> => {
  const { identifier, lecture } = splitEpisode(episodeId);
  const item = await course(identifier);
  const groups = lectureFiles(item);
  const key = lecture ?? [...groups.keys()][0] ?? '';
  const files = groups.get(key);
  if (!files) throw new Error('This course no longer has that lecture');
  const subtitles = (item.files ?? [])
    .filter((file) => /\.(srt|vtt)$/i.test(file.name) && lectureOf(file.name) === key)
    .map((file): VideoSubtitle => ({
      url: fileUrl(identifier, file.name),
      label: /\.asr\.(srt|vtt)$/i.test(file.name) ? 'Automatic (English)' : 'English',
      lang: 'en',
    }));
  const videos = files
    .map((file): Video => {
      const height = Number.parseInt(file.height ?? '', 10);
      const known = Number.isFinite(height) && height > 0;
      return {
        url: fileUrl(identifier, file.name),
        label: known ? `${height}p · ${file.format}` : file.format,
        quality: known ? height : undefined,
        subtitles,
      };
    })
    .sort((a, b) => (b.quality ?? 0) - (a.quality ?? 0));
  return { id: episodeId, name: 'Internet Archive', videos };
};

/** "Instructor: Patrick Winston", "Instructor/speaker: Prof. Gilbert Strang" in a description. */
const instructorsOf = (description: string) =>
  (/Instructors?(?:\s*\/\s*speakers?)?:\s*([^\n]+)/i.exec(description)?.[1] ?? '')
    .split(/\s*(?:,|;|\band\b)\s*/)
    .map((name) => name.replace(/^(?:Prof|Professor|Dr)\.?\s+/i, '').trim())
    .filter((name) => name.length > 1 && name.length < 60);

export const lectures = defineProvider({
  kind: 'video.source',
  id: 'lectures',
  name: 'MIT OpenCourseWare',
  searchPill: 'Lectures',

  popular: ({ page }) => searchPage(LECTURES, 'downloads desc', page),

  latest: ({ page }) => searchPage(LECTURES, 'addeddate desc', page),

  search({ query, page, filters }) {
    const sort = selectValue(filters, SORT);
    const cleaned = escapeQuery(query);
    // Instructors are named in the descriptions.
    const q = cleaned
      ? `(title:(${cleaned}) OR subject:(${cleaned}) OR description:(${cleaned})) AND ${LECTURES}`
      : LECTURES;
    return searchPage(q, sort, page);
  },

  filters: async () => [SORT],

  async details({ id }) {
    const item = await course(id);
    const meta = item.metadata ?? {};
    const description = plainText(firstString(meta.description) ?? '');
    const year = Number.parseInt((meta.year ?? meta.date ?? '').slice(0, 4), 10);
    const licence = meta.licenseurl?.trim() ?? '';
    return {
      id,
      title: firstString(meta.title) ?? id,
      coverUrl: cover(id),
      description: [
        description,
        `Licence: ${licenceName(licence)} (${licence}). Credit: MIT OpenCourseWare, ocw.mit.edu.`,
      ]
        .filter(Boolean)
        .join('\n\n'),
      genres: subjectsOf(item).slice(0, 8),
      directors: instructorsOf(description),
      type: 'series',
      year: Number.isFinite(year) ? year : null,
      status: 'completed',
      url: `${API}/details/${encodeURIComponent(id)}`,
    };
  },

  async episodes({ id }) {
    const item = await course(id);
    const groups = [...lectureFiles(item).keys()];
    if (groups.length === 0) throw new Error('This course has no video Raygun can play');
    return {
      episodes: groups.map((lecture, index): VideoEpisode => ({
        id: `${id}/${lecture}`,
        name: lectureName(item, lecture),
        number: index + 1,
      })),
    };
  },

  // One server, resolved at once: the archive serves every file itself.
  servers: async ({ episodeId }) => ({ servers: [await resolveServer(episodeId)] }),

  videos: async ({ serverId }) => ({ videos: (await resolveServer(serverId)).videos ?? [] }),
});
