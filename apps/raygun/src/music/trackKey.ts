/**
 * The resolver's key for a song (M46.36), as `raygun_resolver::track_key` makes it: the normalized
 * artist and the title without brackets or `feat.` credits. Downloads, favorites and remembered
 * picks are keyed by it.
 */
const normalize = (text: string) =>
  text
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .toLowerCase()
    .replaceAll('&', ' and ')
    .split(/[^\p{Alphabetic}\p{N}]+/u)
    .filter(Boolean)
    .join(' ');

const coreTitle = (title: string) => {
  let plain = '';
  let depth = 0;
  for (const char of title) {
    if (char === '(' || char === '[' || char === '{') depth += 1;
    else if (char === ')' || char === ']' || char === '}') depth = Math.max(0, depth - 1);
    else if (depth === 0) plain += char;
  }
  const words = normalize(plain).split(' ');
  const cut = words.findIndex((word) => word === 'feat' || word === 'ft' || word === 'featuring');
  return (cut === -1 ? words : words.slice(0, cut)).join(' ');
};

export const trackKey = (title: string, artist: string | null | undefined) =>
  `${normalize(artist ?? '')}|${coreTitle(title)}`;

// ---- One identity for saved songs (M46.48), as `collections::identity` in Rust ----

/** Words that make what follows " - " in a title a decoration, as brackets are. */
const DECORATIONS = new Set([
  'acoustic',
  'album',
  'anniversary',
  'bonus',
  'clean',
  'deluxe',
  'demo',
  'edit',
  'edition',
  'explicit',
  'extended',
  'feat',
  'featuring',
  'from',
  'ft',
  'instrumental',
  'live',
  'mix',
  'mono',
  'original',
  'radio',
  'recorded',
  'remaster',
  'remastered',
  'remix',
  'session',
  'single',
  'stereo',
  'unplugged',
  'version',
]);

const SEPARATORS = [' - ', ' – ', ' — '];

const isDecoration = (rest: string) =>
  normalize(rest)
    .split(' ')
    .some((word) => DECORATIONS.has(word) || /^(19|20)\d\d$/.test(word));

/** A title without a decoration after " - " ("Song - Remastered 2011"); other titles as they are. */
export const plainTitle = (title: string) => {
  let cut = -1;
  for (const separator of SEPARATORS) {
    let from = 0;
    for (;;) {
      const at = title.indexOf(separator, from);
      if (at < 0) break;
      if (isDecoration(title.slice(at + separator.length))) {
        cut = cut < 0 ? at : Math.min(cut, at);
        break;
      }
      from = at + separator.length;
    }
  }
  return cut >= 0 && title.slice(0, cut).trim() ? title.slice(0, cut).trimEnd() : title;
};

/** The keys a song with these names answers to: its song key and the resolver's key. */
export const keysOf = (title: string, artist: string | null | undefined): string[] => {
  const song = trackKey(plainTitle(title), artist);
  const track = trackKey(title, artist);
  return song === track ? [song] : [song, track];
};

const words = (artist: string) => new Set(artist.split(' ').filter(Boolean));
const holds = (big: Set<string>, small: Set<string>) => [...small].every((word) => big.has(word));

/**
 * Whether two keys are one song: the same title, and artists where one credit holds every word of
 * the other ("daft punk" and "daft punk pharrell williams"). A key without an artist is only ever
 * the same as itself.
 */
export const sameSong = (a: string, b: string) => {
  if (a === b) return true;
  const at = a.indexOf('|');
  const bt = b.indexOf('|');
  if (at < 0 || bt < 0) return false;
  const title = a.slice(at + 1);
  if (!title || title !== b.slice(bt + 1)) return false;
  const left = words(a.slice(0, at));
  const right = words(b.slice(0, bt));
  return left.size > 0 && right.size > 0 && (holds(left, right) || holds(right, left));
};

/** A key's title part: only keys with the same title can be one song. */
export const titleOf = (key: string) => key.slice(key.indexOf('|') + 1);
