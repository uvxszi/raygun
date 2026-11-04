import { titleScore } from '../search/rankResults';

/**
 * Season identity of video titles, as `raygun_resolver::video_match` has it in Rust (the two
 * are kept in step, with the same test cases): the season a title names ("Title II", "2nd
 * Season", "S2", "Part 2", "第2期"), the title without it, and whether two titles are one show's
 * same season. Anime sites and Kitsu list each season as a title of its own.
 */
export type TitleInfo = {
  /** The title without its season, part, year and decorations: lower case, accents folded. */
  base: string;
  /** The same words as written. */
  name: string;
  season: number | null;
  /** A split cour: "Part 2", "Cour 2". */
  part: number | null;
  finalSeason: boolean;
  /** "Hunter x Hunter (2011)". */
  year: number | null;
};

const SEASON_WORDS = ['season', 'saison', 'temporada', 'staffel', 'stagione', 'sezon', 'сезон'];
const PART_WORDS = ['part', 'partie', 'parte', 'teil', 'cour'];
const DECORATIONS = [
  'tv',
  'sub',
  'subs',
  'subbed',
  'dub',
  'dubs',
  'dubbed',
  'raw',
  'uncensored',
  'uncut',
  'english',
  'eng',
  'japanese',
  'multi',
  'audio',
  'dual',
  'vostfr',
  'legendado',
  'dublado',
  'hd',
  'sd',
  'fhd',
  'uhd',
  '4k',
  '480p',
  '720p',
  '1080p',
  '2160p',
  'bd',
  'web',
];
const NUMBER_PREFIXES = [
  'no',
  'vol',
  'volume',
  'episode',
  'ep',
  'chapter',
  'ch',
  'case',
  'number',
  'movie',
  'film',
  'ova',
  'oad',
  'special',
];
const NUMBER_WORDS = [
  'one',
  'two',
  'three',
  'four',
  'five',
  'six',
  'seven',
  'eight',
  'nine',
  'ten',
  'eleven',
  'twelve',
  'thirteen',
  'fourteen',
  'fifteen',
  'sixteen',
  'seventeen',
  'eighteen',
  'nineteen',
  'twenty',
];
const ORDINAL_WORDS = [
  'first',
  'second',
  'third',
  'fourth',
  'fifth',
  'sixth',
  'seventh',
  'eighth',
  'ninth',
  'tenth',
  'eleventh',
  'twelfth',
];
/** A lone "X" ends many names ("Gundam X"), so it never counts. */
const ROMAN = [
  'i',
  'ii',
  'iii',
  'iv',
  'v',
  'vi',
  'vii',
  'viii',
  'ix',
  'x',
  'xi',
  'xii',
  'xiii',
  'xiv',
  'xv',
  'xvi',
  'xvii',
  'xviii',
  'xix',
  'xx',
];

type Ordinal = number | 'final';

const ordinal = (word: string): Ordinal | null => {
  if (word === 'final' || word === 'last') return 'final';
  const index = ORDINAL_WORDS.indexOf(word);
  if (index >= 0) return index + 1;
  const numeric = /^(\d+)(?:st|nd|rd|th)$/.exec(word);
  const value = numeric ? Number(numeric[1]) : 0;
  return value >= 1 && value <= 99 ? value : null;
};

const romanValue = (word: string) => {
  const index = ROMAN.indexOf(word);
  return index >= 0 ? index + 1 : null;
};

/** A count after "Season" or "Part": digits, a roman numeral or a word. */
const count = (word: string): number | null => {
  if (/^\d+$/.test(word)) {
    const value = Number(word);
    return value <= 99 ? value : null;
  }
  const roman = romanValue(word);
  if (roman !== null) return roman;
  const index = NUMBER_WORDS.indexOf(word);
  return index >= 0 ? index + 1 : null;
};

const sNumber = (word: string) => {
  const found = /^s(\d{1,2})$/.exec(word);
  return found ? Number(found[1]) : null;
};

const yearOf = (word: string) => {
  if (!/^\d{4}$/.test(word)) return null;
  const year = Number(word);
  return year >= 1900 && year <= 2035 ? year : null;
};

type Token = { text: string; lower: string; breakBefore: boolean; group: number };

/** Accents dropped, the rest recomposed (Hangul stays syllables). */
const fold = (word: string) =>
  word.normalize('NFD').replace(/[̀-ͯ]/g, '').normalize('NFC').toLowerCase();

/** Rust's `char::is_alphanumeric`. */
const WORD_CHARACTER = /[\p{Alphabetic}\p{N}]/u;
const ALWAYS_BREAKS = '~～|()[]{}【】「」〔〕–—';

const isBreak = (separator: string) => {
  const spaced = /\s/u.test(separator);
  return [...separator].some(
    (char) => ALWAYS_BREAKS.includes(char) || ((char === ':' || char === '-') && spaced),
  );
};

const tokenize = (text: string): Token[] => {
  const tokens: Token[] = [];
  let word = '';
  let separator = '';
  let depth = 0;
  let groups = 0;
  let group = 0;
  let breakBefore = true;
  const push = () => {
    if (word) tokens.push({ text: word, lower: fold(word), breakBefore, group });
    word = '';
  };
  for (const char of text) {
    if (WORD_CHARACTER.test(char)) {
      if (!word) {
        breakBefore = tokens.length === 0 || isBreak(separator);
        group = depth > 0 ? groups : 0;
        separator = '';
      }
      word += char;
      continue;
    }
    push();
    separator += char;
    if ('([{【「〔'.includes(char)) {
      depth += 1;
      if (depth === 1) groups += 1;
    } else if (')]}】」〕'.includes(char)) {
      depth = Math.max(0, depth - 1);
    }
  }
  push();
  return tokens;
};

const CHINESE_DIGITS = '一二三四五六七八九';

const chineseNumber = (text: string): number | null => {
  if (/^\d+$/.test(text)) return Number(text);
  const digit = (char: string | undefined) =>
    char === '〇' || char === '零' ? 0 : char ? CHINESE_DIGITS.indexOf(char) + 1 || null : null;
  const chars = [...text];
  if (chars.length === 1) return chars[0] === '十' ? 10 : digit(chars[0]);
  if (chars.length === 2 && chars[0] === '十') {
    const one = digit(chars[1]);
    return one === null ? null : 10 + one;
  }
  if (chars.length === 2 && chars[1] === '十') {
    const tens = digit(chars[0]);
    return tens === null ? null : tens * 10;
  }
  if (chars.length === 3 && chars[1] === '十') {
    const tens = digit(chars[0]);
    const one = digit(chars[2]);
    return tens === null || one === null ? null : tens * 10 + one;
  }
  return null;
};

const CJK_ORDINAL =
  /第\s*([0-9]+|[〇零一二三四五六七八九十]+)\s*(期|季|シーズン|シリーズ|部|章|クール|幕)/gu;
const CJK_COUNTED = /([0-9]+)\s*(?:st|nd|rd|th)?\s*(期|季|シーズン|クール|기)/gu;
const CJK_WORD = /(シーズン|시즌|ซีซั่น|ภาค)\s*([0-9]+)/gu;
const PART_KINDS = ['部', '章', 'クール', '幕'];

/** What a title says about its season; see `TitleInfo`. */
export const parseTitle = (title: string): TitleInfo => {
  const info: TitleInfo = {
    base: '',
    name: '',
    season: null,
    part: null,
    finalSeason: false,
    year: null,
  };
  const mark = (value: number | null, kind: string) => {
    if (value === null) return;
    if (PART_KINDS.includes(kind)) info.part ??= value;
    else info.season ??= value;
  };
  const text = title
    .normalize('NFKC')
    .replaceAll('×', ' x ')
    .replaceAll('&', ' and ')
    .replace(CJK_ORDINAL, (_, value: string, kind: string) => {
      mark(chineseNumber(value), kind);
      return ' ～ ';
    })
    .replace(CJK_COUNTED, (_, value: string, kind: string) => {
      mark(Number(value), kind);
      return ' ～ ';
    })
    .replace(CJK_WORD, (_, _word: string, value: string) => {
      mark(Number(value), '期');
      return ' ～ ';
    });
  const tokens = tokenize(text);
  const used = tokens.map(() => false);

  // Markers that say so anywhere: "Season 2", "2nd Season", "The Final Season", "Part 2", "S2".
  tokens.forEach((token, index) => {
    if (used[index]) return;
    const word = token.lower;
    const next = index + 1 < tokens.length && !used[index + 1] ? tokens[index + 1]!.lower : null;
    const previous = index > 0 && !used[index - 1] ? tokens[index - 1]!.lower : null;
    const seasonWord = SEASON_WORDS.includes(word);
    const partWord = PART_WORDS.includes(word);
    const after = next === null ? null : count(next);
    if ((seasonWord || partWord || word === 'series') && after !== null) {
      if (partWord) info.part ??= after;
      else info.season ??= after;
      used[index] = true;
      used[index + 1] = true;
      return;
    }
    const before = previous === null ? null : ordinal(previous);
    if ((seasonWord || partWord || word === 'stage') && before !== null) {
      // "Final Part": the last cour, its number unknown.
      if (before === 'final' && partWord) return;
      if (before === 'final') info.finalSeason = true;
      else if (partWord) info.part ??= before;
      else info.season ??= before;
      used[index] = true;
      used[index - 1] = true;
      if (index >= 2 && !used[index - 2] && tokens[index - 2]!.lower === 'the') {
        used[index - 2] = true;
      }
      return;
    }
    const season = sNumber(word);
    if (season !== null) {
      info.season ??= season;
      used[index] = true;
    }
  });

  // Bracketed years and versions: "(2019)", "(TV)", "[Dub]".
  const groups = Math.max(0, ...tokens.map((token) => token.group));
  for (let group = 1; group <= groups; group += 1) {
    const members = tokens
      .map((token, index) => ({ token, index }))
      .filter(({ token, index }) => token.group === group && !used[index]);
    const year = members.length === 1 ? yearOf(members[0]!.token.lower) : null;
    if (year !== null) {
      info.year ??= year;
      used[members[0]!.index] = true;
    } else if (
      members.length > 0 &&
      members.every(({ token }) => DECORATIONS.includes(token.lower))
    ) {
      members.forEach(({ index }) => (used[index] = true));
    }
  }

  // A trailing "Dub" or year: "Naruto Shippuden Dub", "Hunter x Hunter 2011".
  const remaining = () => tokens.map((_, index) => index).filter((index) => !used[index]);
  let left = remaining();
  const lastDub = left.at(-1);
  if (
    lastDub !== undefined &&
    left.length >= 3 &&
    ['dub', 'dubbed', 'sub', 'subbed'].includes(tokens[lastDub]!.lower)
  ) {
    used[lastDub] = true;
  }
  left = remaining();
  const lastYear = left.at(-1);
  const year = lastYear === undefined ? null : yearOf(tokens[lastYear]!.lower);
  if (lastYear !== undefined && left.length >= 2 && year !== null) {
    info.year ??= year;
    used[lastYear] = true;
  }

  // A roman numeral or small number ending the title or its main part, after other words:
  // "Overlord II", "Mushoku Tensei II: Isekai Ittara Honki Dasu", "Psycho-Pass 3".
  left = remaining();
  if (info.season === null) {
    const boundary = (before: number, after: number) =>
      after > before + 1 || tokens[after]!.breakBefore;
    for (let position = 0; position < left.length; position += 1) {
      const index = left[position]!;
      const starts = position === 0 || boundary(left[position - 1]!, index);
      const ends = position + 1 === left.length || boundary(index, left[position + 1]!);
      if (starts || !ends) continue;
      if (NUMBER_PREFIXES.includes(tokens[left[position - 1]!]!.lower)) continue;
      const token = tokens[index]!;
      const roman =
        token.lower !== 'x' &&
        (token.lower.length >= 2 || [...token.text].every((char) => char !== char.toLowerCase()))
          ? romanValue(token.lower)
          : null;
      const bare =
        /^\d{1,2}$/.test(token.lower) && Number(token.lower) >= 1 && Number(token.lower) <= 20
          ? Number(token.lower)
          : null;
      const season = roman ?? bare;
      if (season !== null) {
        info.season = season;
        used[index] = true;
        break;
      }
    }
  }

  const kept = tokens.filter((_, index) => !used[index]);
  if (kept.length > 1 && kept[0]!.lower === 'the') kept.shift();
  info.base = kept.map((token) => token.lower).join(' ');
  info.name = kept.map((token) => token.text).join(' ');
  return info;
};

/** Whether a title names a season, a part or the final season. */
export const namesSeason = (info: TitleInfo) =>
  info.season !== null || info.part !== null || info.finalSeason;

/** The base as compared: no spaces, long vowels folded ("Kyoukai" and "Kyokai" agree). */
const compareKey = (base: string) =>
  base.replaceAll(' ', '').replaceAll('ou', 'o').replaceAll('uu', 'u');

/** The same season: number (none is the first), part (none is the first), final, years close. */
export const sameSeason = (a: TitleInfo, b: TitleInfo) =>
  (a.year === null || b.year === null || Math.abs(a.year - b.year) <= 1) &&
  a.finalSeason === b.finalSeason &&
  (a.finalSeason || (a.season ?? 1) === (b.season ?? 1)) &&
  (a.part ?? 1) === (b.part ?? 1);

/** Whether two titles are one show's same season. */
export const sameTitle = (a: string, b: string) => {
  const first = parseTitle(a);
  const second = parseTitle(b);
  return (
    first.base !== '' &&
    compareKey(first.base) === compareKey(second.base) &&
    sameSeason(first, second)
  );
};

/** Where a title of another season than the one looked for scores: below every close match. */
const OTHER_SEASON = 30;

/**
 * `titleScore` for video titles: their bases compared ("II" is "Season 2" is "2nd Season"), and
 * a title of another season than the query names scored below every close match, so a season-2
 * query never ranks season 1 as close. With nothing asked, the first season comes before the
 * others. `strict` (the same title in other sources) takes a query without a season as the
 * first season.
 */
export const videoTitleScore = (title: string, query: string, strict = false) => {
  const wanted = parseTitle(query);
  const found = parseTitle(title);
  const base = titleScore(found.base || title, wanted.base || query);
  const asked = namesSeason(wanted);
  if ((asked || strict) && !sameSeason(found, wanted)) return Math.min(base, OTHER_SEASON);
  if (!asked && namesSeason(found)) return Math.max(0, base - Math.min(found.season ?? 2, 10));
  return base;
};
