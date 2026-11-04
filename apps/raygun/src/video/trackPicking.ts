import type { SubtitleMode, VideoResolution } from '@raygun/bindings';

import type { TrackChoice } from './protocol';

/**
 * Which subtitle and audio tracks the webview's player turns on by itself (M46.4), by the same
 * rules as the Android player (`PlayerTracks.kt`, `PlayerLanguages.kt`; mpv's `alang`, `slang`
 * and `subs-with-matching-audio`):
 * - audio: the title's remembered track, else the first audio language the video has (the
 *   device's languages, then the app's, unless Settings → Player → Audio names one), else the
 *   track marked default, else the first;
 * - subtitles: the title's remembered choice, else, when the audio isn't in a subtitle language
 *   (or always, as set), the first subtitle language the video has, leaving out skip words and
 *   preferring prefer words. Audio in a subtitle language only brings its forced subtitles.
 */

export type TrackOption = {
  key: string;
  label: string;
  language: string | null;
  /** Marked default (HLS `DEFAULT`) or main (DASH's role). */
  original?: boolean;
  /** Only what's said in other languages, and signs. */
  forced?: boolean;
  /** Commentary or audio description: not played by itself. */
  extra?: boolean;
};

/** `audioLanguage` for the video's own track (`ORIGINAL_AUDIO` in raygun-model). */
export const ORIGINAL_AUDIO = 'original';

/** ISO 639-1 codes, as Java lists them (with its old `in`, `iw`, `ji` and `mo`). */
const TWO_LETTER =
  'aa ab ae af ak am an ar as av ay az ba be bg bh bi bm bn bo br bs ca ce ch co cr cs cu cv cy da de dv dz ee el en eo es et eu fa ff fi fj fo fr fy ga gd gl gn gu gv ha he hi ho hr ht hu hy hz ia id ie ig ii ik in io is it iu iw ja ji jv ka kg ki kj kk kl km kn ko kr ks ku kv kw ky la lb lg li ln lo lt lu lv mg mh mi mk ml mn mo mr ms mt my na nb nd ne ng nl nn no nr nv ny oc oj om or os pa pi pl ps pt qu rm rn ro ru rw sa sc sd se sg si sk sl sm sn so sq sr ss st su sv sw ta te tg th ti tk tl tn to tr ts tt tw ty ug uk ur uz ve vi vo wa wo xh yi yo za zh zu';

/** ISO 639-2/T codes to ISO 639-1. */
const THREE_LETTER =
  'aar:aa abk:ab ave:ae afr:af aka:ak amh:am arg:an ara:ar asm:as ava:av aym:ay aze:az bak:ba bel:be bul:bg bih:bh bis:bi bam:bm ben:bn bod:bo bre:br bos:bs cat:ca che:ce cha:ch cos:co cre:cr ces:cs chu:cu chv:cv cym:cy dan:da deu:de div:dv dzo:dz ewe:ee ell:el eng:en epo:eo spa:es est:et eus:eu fas:fa ful:ff fin:fi fij:fj fao:fo fra:fr fry:fy gle:ga gla:gd glg:gl grn:gn guj:gu glv:gv hau:ha heb:he hin:hi hmo:ho hrv:hr hat:ht hun:hu hye:hy her:hz ina:ia ind:id ile:ie ibo:ig iii:ii ipk:ik ido:io isl:is ita:it iku:iu jpn:ja jav:jv kat:ka kon:kg kik:ki kua:kj kaz:kk kal:kl khm:km kan:kn kor:ko kau:kr kas:ks kur:ku kom:kv cor:kw kir:ky lat:la ltz:lb lug:lg lim:li lin:ln lao:lo lit:lt lub:lu lav:lv mlg:mg mah:mh mri:mi mkd:mk mal:ml mon:mn mol:mo mar:mr msa:ms mlt:mt mya:my nau:na nob:nb nde:nd nep:ne ndo:ng nld:nl nno:nn nor:no nbl:nr nav:nv nya:ny oci:oc oji:oj orm:om ori:or oss:os pan:pa pli:pi pol:pl pus:ps por:pt que:qu roh:rm run:rn ron:ro rus:ru kin:rw san:sa srd:sc snd:sd sme:se sag:sg sin:si slk:sk slv:sl smo:sm sna:sn som:so sqi:sq srp:sr ssw:ss sot:st sun:su swe:sv swa:sw tam:ta tel:te tgk:tg tha:th tir:ti tuk:tk tgl:tl tsn:tn ton:to tur:tr tso:ts tat:tt twi:tw tah:ty uig:ug ukr:uk urd:ur uzb:uz ven:ve vie:vi vol:vo wln:wa wol:wo xho:xh yid:yi yor:yo zha:za zho:zh zul:zu';

/** Bibliographic ISO 639-2 codes and older or regional ones, to the code used here. */
const ALIASES = new Map(
  Object.entries({
    alb: 'sq',
    arm: 'hy',
    baq: 'eu',
    bur: 'my',
    chi: 'zh',
    cze: 'cs',
    dut: 'nl',
    fre: 'fr',
    geo: 'ka',
    ger: 'de',
    gre: 'el',
    ice: 'is',
    mac: 'mk',
    mao: 'mi',
    may: 'ms',
    per: 'fa',
    rum: 'ro',
    slo: 'sk',
    tib: 'bo',
    wel: 'cy',
    // Codes Java and old streams still use.
    iw: 'he',
    in: 'id',
    ji: 'yi',
    jw: 'jv',
    mo: 'ro',
    // Bokmål and Nynorsk read as Norwegian; Tagalog as Filipino.
    nb: 'no',
    nn: 'no',
    nob: 'no',
    nno: 'no',
    tl: 'fil',
    tgl: 'fil',
    sh: 'sr',
    scc: 'sr',
    scr: 'hr',
    // Chinese spoken varieties written as Chinese, and OpenSubtitles' own codes.
    cmn: 'zh',
    zht: 'zh',
    zhs: 'zh',
    zhe: 'zh',
    ze: 'zh',
    pob: 'pt',
    pom: 'pt',
    spn: 'es',
    spl: 'es',
    sea: 'es',
  }),
);

/** Names that the language data doesn't give but labels use. */
const EXTRA_NAMES: [string, string][] = [
  ['castilian', 'es'],
  ['castellano', 'es'],
  ['latino', 'es'],
  ['brazilian', 'pt'],
  ['mandarin', 'zh'],
  ['cantonese', 'yue'],
  ['farsi', 'fa'],
  ['flemish', 'nl'],
  ['filipino', 'fil'],
  ['tagalog', 'fil'],
  ['bokmål', 'no'],
  ['nynorsk', 'no'],
];

/** No language, several, or not said. */
const UNDETERMINED = new Set(['und', 'zxx', 'mul', 'mis', 'qaa', 'xx', 'unknown', 'none']);
/** Capital words in labels that are codes of languages but rarely mean them (SDH, HI, SD). */
const NOT_LANGUAGES = new Set(['sd', 'hi', 'sdh', 'ad', 'cc', 'tv']);
/** A whole value that is a code or a tag: `en`, `eng`, `en-us`, `zh-hant-tw`. */
const CODE_SHAPE = /^[a-z]{2,3}(-[a-z0-9]{1,8})*$/;
const BRACKETS = /[([{][^)\]}]*[)\]}]/g;
/** Scripts written without spaces between words. */
const UNSPACED = /[⺀-￿]/;

type Tables = { two: Set<string>; three: Map<string, string>; names: Map<string, string> };
let tables: Tables | null = null;

const displayNames = (locale: string) => {
  try {
    return new Intl.DisplayNames([locale], { type: 'language', fallback: 'none' });
  } catch {
    return null;
  }
};

/** Built on first use: the code tables, and language names in English, the device's language
 * and their own, each to its code. */
const getTables = (): Tables => {
  if (tables) return tables;
  const twoLetter = TWO_LETTER.split(' ');
  const two = new Set([...twoLetter.map((code) => ALIASES.get(code) ?? code), 'fil', 'yue']);
  const three = new Map<string, string>();
  for (const pair of THREE_LETTER.split(' ')) {
    const [long, short] = pair.split(':') as [string, string];
    three.set(long, ALIASES.get(short) ?? short);
  }
  const names = new Map<string, string>(EXTRA_NAMES);
  const device = typeof navigator === 'undefined' ? 'en' : navigator.language || 'en';
  const shared = [displayNames('en'), displayNames(device)];
  for (const raw of [...twoLetter, 'fil', 'yue']) {
    const code = ALIASES.get(raw) ?? raw;
    for (const display of [...shared, displayNames(raw)]) {
      let name: string | undefined;
      try {
        name = display?.of(raw);
      } catch {
        name = undefined;
      }
      const lower = name?.toLowerCase();
      if (lower && lower.length > 1 && lower !== raw && !names.has(lower)) names.set(lower, code);
    }
  }
  tables = { two, three, names };
  return tables;
};

const canonical = (primary: string): string | null => {
  if (UNDETERMINED.has(primary)) return null;
  const code =
    ALIASES.get(primary) ??
    (primary.length === 3 ? (getTables().three.get(primary) ?? primary) : primary);
  return ALIASES.get(code) ?? code;
};

/**
 * The language `value` names, as one primary code: ISO 639-1, 639-2/T and /B and 639-3 codes,
 * tags with a region or script (`en-US`, `pt_BR`, `zh-Hant`), OpenSubtitles' own (`pob`), and
 * names or labels (`English`, `Español (Latinoamérica)`, `Japanese - Original`, `ENG SDH`,
 * `日本語`). Null when it names none.
 */
export const languageCode = (value: string | null | undefined): string | null => {
  const raw = value?.trim();
  if (!raw) return null;
  const lower = raw.toLowerCase().replace(/_/g, '-');
  if (UNDETERMINED.has(lower)) return null;
  const { names, two } = getTables();
  // A code or tag of a known language; "Dub" or "SDH" as a whole is none.
  if (CODE_SHAPE.test(lower)) {
    const code = canonical(lower.split('-')[0] ?? lower);
    return code && two.has(code) ? code : null;
  }
  // A name or a label: the whole of it without what's in brackets, then word by word.
  const plain = lower.replace(BRACKETS, ' ').trim();
  const whole = names.get(plain);
  if (whole) return whole;
  const words = plain.match(/\p{L}+/gu) ?? [];
  for (const word of words) {
    const found = names.get(word);
    if (found) return found;
  }
  if (words.some((word) => UNSPACED.test(word))) {
    for (const [name, code] of names) {
      if (name.length > 1 && UNSPACED.test(name) && plain.includes(name)) return code;
    }
  }
  // A code in capitals inside a label: "ENG SDH", "[JPN]".
  for (const match of raw.matchAll(/(?<!\p{L})[A-Z]{2,3}(?!\p{L})/gu)) {
    const code = match[0].toLowerCase();
    if (NOT_LANGUAGES.has(code)) continue;
    const found = canonical(code);
    if (found && two.has(found)) return found;
  }
  return null;
};

/** A track's language: what it says, else what its name says ("English"). */
export const languageOf = (option: TrackOption): string | null =>
  languageCode(option.language) ?? languageCode(option.label);

/** Which languages tracks are chosen in (`LanguagePrefs` in the Android player). */
export type LanguagePrefs = {
  /** Audio languages in order; empty plays the video's own track. */
  audio: string[];
  subtitles: string[];
  subtitleMode: SubtitleMode;
  /** The audio's language when a stream doesn't say (Japanese for anime). */
  spoken: string | null;
};

type LanguageInput = {
  audioLanguage: string;
  audioFallback: string;
  subtitleFallback: string;
  /** The app's language; empty when it follows the device. */
  appLanguage: string;
  /** Settings' subtitle language; empty follows the device's languages, then the app's. */
  subtitleLanguage: string;
  subtitleMode: SubtitleMode;
  spoken: string | null;
  /** The device's languages in order (`navigator.languages`). */
  device: readonly string[];
};

const codes = (values: (string | null | undefined)[]) => [
  ...new Set(values.map(languageCode).filter((code): code is string => code !== null)),
];

export const languagePrefs = (input: LanguageInput): LanguagePrefs => {
  const yours = codes([...input.device, input.appLanguage]);
  const audio =
    input.audioLanguage === ''
      ? codes([...yours, input.audioFallback])
      : input.audioLanguage === ORIGINAL_AUDIO
        ? []
        : codes([input.audioLanguage, input.audioFallback]);
  const first = input.subtitleLanguage === '' ? yours : codes([input.subtitleLanguage]);
  return {
    audio,
    subtitles: codes([...first, input.subtitleFallback]),
    subtitleMode: input.subtitleMode,
    spoken: languageCode(input.spoken),
  };
};

const words = (list: string) =>
  list
    .split(',')
    .map((word) => word.trim().toLowerCase())
    .filter(Boolean);

/** The remembered track among `options`: the same language and name, else the same name, else
 * the same language. */
export const matchChoice = <T extends TrackOption>(options: T[], choice: TrackChoice): T | null => {
  const language = languageCode(choice.language);
  return (
    options.find(
      (option) => language && languageOf(option) === language && option.label === choice.label,
    ) ??
    options.find((option) => choice.label && option.label === choice.label) ??
    options.find((option) => language && languageOf(option) === language && !option.forced) ??
    null
  );
};

/** The best full subtitles in the first of `languages` that has any. */
export const pickByLanguage = <T extends TrackOption>(
  options: T[],
  languages: (string | null | undefined)[],
  preferWords = '',
  skipWords = '',
): T | null => {
  const prefer = words(preferWords);
  const skip = words(skipWords);
  for (const language of languages) {
    const code = languageCode(language);
    if (!code) continue;
    const matching = options
      .filter((option) => languageOf(option) === code)
      .filter((option) => !skip.some((word) => option.label.toLowerCase().includes(word)));
    const full = matching.filter((option) => !option.forced);
    const pool = full.length > 0 ? full : matching;
    const preferred = pool.find((option) =>
      prefer.some((word) => option.label.toLowerCase().includes(word)),
    );
    if (preferred ?? pool[0]) return preferred ?? pool[0]!;
  }
  return null;
};

/** The audio track to play; null when there's no choice to make. `playing` is the engine's own
 * pick, kept when it's as good. */
export const pickAudio = <T extends TrackOption>(
  options: T[],
  remembered: TrackChoice | undefined,
  prefs: LanguagePrefs,
  playing?: string,
): T | null => {
  if (options.length < 2) return null;
  const kept = remembered ? matchChoice(options, remembered) : null;
  if (kept) return kept;
  for (const language of prefs.audio) {
    const matching = options.filter((option) => languageOf(option) === language);
    if (matching.length === 0) continue;
    const plain = matching.filter((option) => !option.extra);
    const pool = plain.length > 0 ? plain : matching;
    return (
      pool.find((option) => option.key === playing) ??
      pool.find((option) => option.original) ??
      pool[0]!
    );
  }
  return (
    options.find((option) => option.original && !option.extra) ??
    options.find((option) => !option.extra) ??
    options[0]!
  );
};

/** The subtitles to show; null for none. `spoken` is the audio's language. */
export const pickSubtitles = <T extends TrackOption>(
  options: T[],
  remembered: TrackChoice | { off: true } | undefined,
  prefs: LanguagePrefs,
  spoken: string | null,
  preferWords = '',
  skipWords = '',
): T | null => {
  if (remembered && 'off' in remembered) return null;
  const kept = remembered ? matchChoice(options, remembered) : null;
  if (kept) return kept;
  if (prefs.subtitleMode === 'never') return null;
  const understood = spoken === null || prefs.subtitles.includes(spoken);
  if (prefs.subtitleMode === 'foreign' && understood) {
    // Only what's said in other languages, and signs.
    return spoken
      ? (options.find((option) => option.forced && languageOf(option) === spoken) ?? null)
      : null;
  }
  return pickByLanguage(options, prefs.subtitles, preferWords, skipWords);
};

/** The track a remembered choice describes. */
export const choiceOf = (option: TrackOption): TrackChoice => ({
  language: option.language ?? languageOf(option),
  label: option.label,
});

/** A stream's variant as its engine describes it; sizes and bit rates may be unknown (0). */
export type Variant = { width?: number; height?: number; bitrate?: number };

/** The height a picture counts as: a 16:9 picture's with the same long side, so 1920×800 is
 * 1080p (`video_quality::picture_height` in raygun-app). */
export const pictureHeight = (width: number, height: number): number =>
  Math.round((Math.max(width, height) * 9) / 16);

/** A rough height from a bit rate, for variants without a size (`bandwidth_height`). */
export const bitrateHeight = (bitsPerSecond: number): number => {
  if (bitsPerSecond >= 12_000_000) return 2160;
  if (bitsPerSecond >= 7_000_000) return 1440;
  if (bitsPerSecond >= 3_500_000) return 1080;
  if (bitsPerSecond >= 1_800_000) return 720;
  if (bitsPerSecond >= 900_000) return 480;
  return 360;
};

const variantHeight = ({ width = 0, height = 0, bitrate = 0 }: Variant): number | null => {
  if (height > 0) return width > 0 ? pictureHeight(width, height) : height;
  return bitrate > 0 ? bitrateHeight(bitrate) : null;
};

/** The largest height wanted; null for the highest. */
const resolutionHeight = (resolution: VideoResolution): number | null =>
  resolution === 'highest' ? null : Number.parseInt(resolution, 10);

/** Lower suits better (`video_quality::rank`): the highest at or below the preferred height,
 * then the lowest above it, unknown sizes last; with `highest`, the highest. */
const rank = (height: number | null, limit: number | null): [number, number] => {
  if (height === null) return [2, 0];
  if (limit === null) return [0, -height];
  return height <= limit ? [0, limit - height] : [1, height - limit];
};

export type VariantPick = {
  /** The variant to start at. */
  start: number;
  /** The highest variant automatic switching may use; -1 for no cap. */
  cap: number;
  /** The highest bit rate automatic switching may use, in kb/s; null for no cap. */
  maxKbps: number | null;
};

/**
 * Where an adaptive stream starts and how far automatic switching may go, as the Android player
 * caps it (`PlayerTracks.kt`): the variant the preferred resolution picks, and nothing larger
 * than it unless `highest`. Null when there's nothing to choose.
 */
export const pickVariant = (
  variants: Variant[],
  resolution: VideoResolution,
): VariantPick | null => {
  if (variants.length < 2) return null;
  const limit = resolutionHeight(resolution);
  const heights = variants.map(variantHeight);
  let start = 0;
  heights.forEach((height, index) => {
    const [tier, distance] = rank(height, limit);
    const [bestTier, bestDistance] = rank(heights[start] ?? null, limit);
    const better =
      tier < bestTier ||
      (tier === bestTier &&
        (distance < bestDistance ||
          (distance === bestDistance &&
            (variants[index]?.bitrate ?? 0) > (variants[start]?.bitrate ?? 0))));
    if (better) start = index;
  });
  const chosen = heights[start] ?? null;
  if (limit === null || chosen === null) return { start, cap: -1, maxKbps: null };
  let cap = start;
  let maxBitrate = variants[start]?.bitrate ?? 0;
  heights.forEach((height, index) => {
    if (height === null || height > chosen) return;
    cap = Math.max(cap, index);
    maxBitrate = Math.max(maxBitrate, variants[index]?.bitrate ?? 0);
  });
  return { start, cap, maxKbps: maxBitrate > 0 ? Math.ceil(maxBitrate / 1000) : null };
};
