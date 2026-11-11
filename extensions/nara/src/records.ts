// The National Archives Catalog's records as the API answers them, and what Raygun reads of
// them: which films it may offer, their reels, titles and descriptions. No requests here.
import type { MangaSummary, VideoEpisode } from '@raygun/sdk';

export type DigitalObject = {
  objectType?: string;
  objectUrl?: string;
  objectFilename?: string;
  objectDesignator?: string;
  objectDescription?: string;
};
type Dated = { year?: number; logicalDate?: string };
type Ancestor = {
  naId?: number;
  title?: string;
  levelOfDescription?: string;
  recordGroupNumber?: number;
  creators?: { heading?: string; creatorType?: string }[];
};
export type CatalogRecord = {
  naId?: number | string;
  title?: string;
  levelOfDescription?: string;
  generalRecordsTypes?: string[];
  productionDates?: Dated[];
  scopeAndContentNote?: string;
  shotList?: string;
  useRestriction?: { status?: string; note?: string };
  accessRestriction?: { status?: string };
  digitalObjects?: DigitalObject[];
  ancestors?: Ancestor[];
  creators?: { heading?: string; creatorType?: string }[];
  localIdentifier?: string;
};
type Hit = { _source?: { record?: CatalogRecord } };
type Hits = { total?: { value?: number } | number; hits?: Hit[] };
/** `records/search` answers `{ body: { hits }, statusCode }`. */
export type SearchAnswer = {
  body?: { hits?: Hits } | string;
  hits?: Hits;
  statusCode?: number;
  message?: string;
};

/** The records an answer holds, and how many the search found in all. */
export const hitsOf = (answer: SearchAnswer): { records: CatalogRecord[]; total: number } => {
  const hits = (typeof answer.body === 'object' ? answer.body.hits : undefined) ?? answer.hits;
  const total = typeof hits?.total === 'number' ? hits.total : (hits?.total?.value ?? 0);
  const records = (hits?.hits ?? []).flatMap((hit) =>
    hit._source?.record ? [hit._source.record] : [],
  );
  return { records, total };
};

const isMp4 = (object: DigitalObject) =>
  /\bmp4\b/i.test(object.objectType ?? '') || /\.mp4$/i.test(object.objectFilename ?? '');

/**
 * Older answers name the storage bucket directly, which refuses outside requests; the catalogue
 * serves the same files under its own address.
 */
export const fileUrl = (address: string) =>
  address.replace(
    /^https?:\/\/s3(?:\.dualstack\.[a-z0-9-]+)?\.amazonaws\.com\/NARAprodstorage\/lz\//i,
    'https://catalog.archives.gov/medialz/',
  );

/** "Reel 2 of 3", or "-r2" in the file's name. */
const reelNumber = (object: DigitalObject) => {
  const named = /reel\s*(\d+)/i.exec(object.objectDesignator ?? '');
  if (named) return Number(named[1]);
  const file = /[-_]r(\d+)\.mp4$/i.exec(object.objectFilename ?? '');
  return file ? Number(file[1]) : null;
};

/** A record's MP4 files, each once (answers sometimes list a file twice), in reel order. */
export const reelsOf = (record: CatalogRecord): DigitalObject[] => {
  const seen = new Set<string>();
  const reels = (record.digitalObjects ?? []).filter((object) => {
    if (!isMp4(object) || !object.objectUrl || !/^https?:\/\//i.test(object.objectUrl)) {
      return false;
    }
    const name = (object.objectFilename ?? object.objectUrl.split('/').pop() ?? '').toLowerCase();
    if (seen.has(name)) return false;
    seen.add(name);
    return true;
  });
  return reels
    .map((object, index) => ({ object, index, reel: reelNumber(object) }))
    .sort(
      (one, other) =>
        (one.reel ?? Number.MAX_SAFE_INTEGER) - (other.reel ?? Number.MAX_SAFE_INTEGER) ||
        one.index - other.index,
    )
    .map(({ object }) => object);
};

/**
 * A film Raygun may offer: the National Archives marks both its use and its access unrestricted
 * (a film it says may be under copyright, or hasn't judged, is left out), and it has an MP4.
 */
export const isOpen = (record: CatalogRecord): boolean =>
  record.useRestriction?.status === 'Unrestricted' &&
  record.accessRestriction?.status === 'Unrestricted' &&
  reelsOf(record).length > 0;

/** Words that stay in capitals when a title written in capitals is retyped. */
const KEEP_CAPS = new Set([
  'US',
  'USA',
  'USS',
  'USN',
  'USAF',
  'USMC',
  'USCG',
  'UN',
  'NATO',
  'NASA',
  'FBI',
  'CIA',
  'TV',
  'WW',
  'WWI',
  'WWII',
  'II',
  'III',
  'IV',
  'VI',
  'VII',
  'VIII',
  'IX',
  'XI',
  'XII',
  'DC',
  'NY',
  'SW',
  'SE',
  'NW',
  'NE',
  'ETO',
  'CBI',
  'POW',
  'POWS',
  'GI',
  'GIS',
  'ROTC',
  'NCO',
  'AEF',
  'MP',
]);
const SMALL = new Set(['a', 'an', 'and', 'at', 'by', 'for', 'in', 'of', 'on', 'or', 'the', 'to']);

/** Catalogue titles are often in capitals: "CARRIER ACTIVITY ABOARD USS PHILIPPINE SEA". */
export const tidyTitle = (title: string): string => {
  const trimmed = title.replace(/\s+/g, ' ').trim();
  if (/[a-z]/.test(trimmed)) return trimmed;
  let first = true;
  return trimmed.replace(/[A-Z][A-Z'’]*|\d+[A-Z]*/g, (word, at: number) => {
    const bare = word.replace(/['’].*$/, '');
    // Designations keep their capitals: "CVA-47", "B-17".
    const designation = /^-\d/.test(trimmed.slice(at + word.length));
    let out: string;
    if (KEEP_CAPS.has(bare) || designation || /\d/.test(word)) out = word;
    else if (!first && SMALL.has(word.toLowerCase())) out = word.toLowerCase();
    else out = word.charAt(0) + word.slice(1).toLowerCase();
    first = false;
    return out;
  });
};

export const idOf = (record: CatalogRecord): string | null =>
  record.naId === undefined || record.naId === null || String(record.naId).trim() === ''
    ? null
    : String(record.naId).trim();

export const summaryOf = (record: CatalogRecord): MangaSummary[] => {
  const id = idOf(record);
  if (!id || !record.title || !isOpen(record)) return [];
  return [{ id, title: tidyTitle(record.title), coverUrl: null }];
};

export const yearOf = (record: CatalogRecord): number | null => {
  const dated = (record.productionDates ?? []).find((date) => date.year);
  const year = dated?.year ?? Number((dated?.logicalDate ?? '').slice(0, 4));
  return year && year >= 1800 && year <= 2200 ? year : null;
};

/** "Department of the Navy. Naval Photographic Center. (12/01/1959 - ca. 1998)" without dates. */
const heading = (text: string) => text.replace(/\s*\([^)]*\d{4}[^)]*\)\s*$/, '').replace(/\.$/, '');

const paragraphs = (text: string | undefined, most: number) => {
  const plain = (text ?? '')
    .replace(/<[^>]+>/g, ' ')
    .replace(/[ \t]+/g, ' ')
    .trim();
  return plain.length > most ? `${plain.slice(0, most).replace(/\s+\S*$/, '')}…` : plain;
};

export const descriptionOf = (record: CatalogRecord): string => {
  const series = (record.ancestors ?? []).find(
    (ancestor) => ancestor.levelOfDescription === 'series',
  );
  const group = (record.ancestors ?? []).find(
    (ancestor) => ancestor.levelOfDescription === 'recordGroup',
  );
  const makers = [
    ...(record.creators ?? []),
    ...(series?.creators ?? []).filter((creator) => creator.creatorType !== 'Predecessor'),
  ]
    .map((creator) => (creator.heading ? heading(creator.heading) : ''))
    .filter(Boolean);
  const id = idOf(record);
  return [
    paragraphs(record.scopeAndContentNote, 2000),
    record.shotList ? `Shot list: ${paragraphs(record.shotList, 2000)}` : '',
    makers.length > 0 ? `Made by: ${[...new Set(makers)].slice(0, 2).join('; ')}` : '',
    [
      series?.title ? `Series: ${series.title}` : '',
      group?.title
        ? `Record Group ${group.recordGroupNumber ?? ''}: ${group.title}`.replace('  ', ' ')
        : '',
    ]
      .filter(Boolean)
      .join('\n'),
    `National Archives Identifier ${id}${record.localIdentifier ? ` (${record.localIdentifier})` : ''}. Unrestricted use and access.`,
  ]
    .filter(Boolean)
    .join('\n\n');
};

/** A record's reels as episodes: one film is one episode, "Film". */
export const episodesOf = (record: CatalogRecord): VideoEpisode[] => {
  const id = idOf(record)!;
  const reels = reelsOf(record);
  if (reels.length === 1) return [{ id: `${id}/0`, name: 'Film' }];
  return reels.map((reel, index) => ({
    id: `${id}/${index}`,
    name: reel.objectDesignator?.trim() || `Reel ${index + 1}`,
    number: index + 1,
  }));
};
