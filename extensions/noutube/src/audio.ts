// Adapted from nonbili/NouTube (https://github.com/nonbili/NouTube), AGPL-3.0.
// From content/audio.ts (the word "original" in YouTube's languages, after
// https://github.com/Bilalkamal/YouTube-Original-Audio-Track) and playDefaultAudio in
// content/player.ts.
//
// Videos with dubbed audio tracks (many made by YouTube's automatic dubbing) start in the track of
// the viewer's language. The player names the track the video was made with "… (original)", in
// the language of YouTube's interface; that one is picked once per video, so a track the listener
// picks afterwards stays.

import { isObject } from './lib/json';
import { onTimeUpdate, onVideoChange, playerVideoId, playsMainVideo, type Player } from './player';

const l10nOriginal: Record<string, string> = {
  en: 'original',
  es: 'original',
  fr: 'originale',
  de: 'original',
  it: 'originale',
  pt: 'original',
  ru: 'оригинал',
  ja: 'オリジナル',
  ko: '원본',
  zh: '原始',
  ar: 'أصلي',
  hi: 'मूल',
  bn: 'মূল',
  id: 'asli',
  tr: 'orijinal',
  vi: 'nguyên bản',
  th: 'ต้นฉบับ',
  nl: 'origineel',
  pl: 'oryginalny',
  sv: 'original',
  fi: 'alkuperäinen',
  no: 'original',
  da: 'original',
  cs: 'originál',
  el: 'πρωτότυπο',
  he: 'מקורי',
  ro: 'original',
  hu: 'eredeti',
  uk: 'оригінал',
  fa: 'اصلی',
  ms: 'asal',
  tl: 'orihinal',
  ta: 'அசல்',
  ur: 'اصل',
  hr: 'izvorni',
  bg: 'оригинален',
  sk: 'originál',
  sl: 'izvirnik',
  sr: 'оригинал',
  lt: 'originalus',
  lv: 'oriģināls',
  et: 'originaal',
  ka: 'ორიგინალი',
  az: 'orijinal',
  uz: 'asl',
  kk: 'түпнұсқа',
  hy: 'բնօրինակ',
  km: 'ដើម',
  my: 'မူရ်း',
  si: 'මුල්',
  am: 'የመጀመሪያው',
  ne: 'मौलिक',
  lo: 'ຕົ້ນສະບັບ',
  mn: 'эх',
  bo: 'དངོས་གཞི།',
  cy: 'gwreiddiol',
  eo: 'originala',
  eu: 'jatorrizko',
  gl: 'orixinal',
  gu: 'મૂળ',
  ha: 'asali',
  ig: 'izugbe',
  is: 'upprunalegt',
  jv: 'asli',
  kn: 'ಮೂಲ',
  ku: 'orîjînal',
  ky: 'түп нуска',
  lb: 'original',
  mi: 'taketake',
  ml: 'യഥാർത്ഥ',
  mr: 'मूळ',
  mt: 'oriġinali',
  or: 'ମୌଳିକ',
  pa: 'ਅਸਲ',
  ps: 'اصلي',
  sd: 'اصل',
  sm: 'muamua',
  sn: 'chaicho',
  so: 'asalka ah',
  sq: 'origjinal',
  su: 'asli',
  sw: 'asili',
  te: 'అసలు',
  tk: 'asyl',
  tt: 'оригиналь',
  ug: 'ئەسلى',
  yi: 'אריגינעל',
  yo: 'atilẹba',
  zu: 'okwangempela',
  af: 'oorspronklik',
  bs: 'originalni',
  ca: 'original',
  ceb: 'orihinal',
  co: 'uriginale',
  fy: 'oarspronklik',
  ga: 'bunaidh',
  gd: 'tùsail',
  haw: 'kumu',
  hmn: 'thawj',
  ht: 'orijinal',
  ia: 'original',
  ie: 'original',
  io: 'originala',
  kw: 'gwredhek',
  la: 'originalis',
  ln: 'ya liboso',
  mg: 'tany am-boalohany',
  mk: 'оригинален',
  ny: 'yoyamba',
  oc: 'original',
  qu: 'qallariy',
  rm: 'original',
  rw: "cy'umwimerere",
  sa: 'मूल',
  sc: 'originale',
  st: 'ea pele',
  tg: 'аслӣ',
  to: 'ʻuluaki',
  wo: 'njëkk',
  xh: 'yokuqala',
  'zh-tw': '原始',
};

/** "original" in every language listed, lowercase (German, for one, writes "Original"). */
const originalLabels = [
  ...new Set(Object.values(l10nOriginal).map((label) => label.toLocaleLowerCase())),
];

const isOriginal = (name: string) => {
  const lower = name.toLocaleLowerCase();
  return originalLabels.some((label) => lower.includes(label));
};

/**
 * A track's name. The player's track objects keep it in an inner object (`name`, `id`,
 * `isDefault`, `isAutoDubbed`) under a minified field name, so it is looked for among the
 * track's values. Caption tracks inside the object have a `name` and `isDefault` too, so one
 * with `isAutoDubbed` is preferred.
 */
function trackName(track: unknown): string | undefined {
  if (!isObject(track)) return undefined;
  const infos = Object.values(track).filter(
    (value) => isObject(value) && 'isDefault' in value && typeof value.name === 'string',
  );
  const info = infos.find((value) => isObject(value) && 'isAutoDubbed' in value) ?? infos[0];
  return isObject(info) && typeof info.name === 'string' && info.name ? info.name : undefined;
}

function audioTracks(player: Player): unknown[] {
  try {
    const tracks = player.getAvailableAudioTracks?.();
    return Array.isArray(tracks) ? tracks : [];
  } catch {
    return [];
  }
}

/** The video already seen to (or given up on), so the listener's own choice stays. */
let handled = '';
let attemptsFor = '';
let attempts = 0;

function pickOriginal(player: Player) {
  const videoId = playerVideoId(player);
  if (!videoId || videoId === handled || !playsMainVideo(player)) return;
  if (attemptsFor !== videoId) {
    attemptsFor = videoId;
    attempts = 0;
  }
  const tracks = audioTracks(player);
  // The tracks come a moment after the video; a video with one track has nothing to pick.
  if (tracks.length < 2) {
    if (++attempts >= 40) handled = videoId;
    return;
  }
  handled = videoId;
  const current = trackName(player.getAudioTrack?.());
  if (current && isOriginal(current)) return;
  const original = tracks.find((track) => {
    const name = trackName(track);
    return name !== undefined && isOriginal(name);
  });
  if (original) player.setAudioTrack?.(original);
}

export function installOriginalAudio() {
  onVideoChange((_video, player) => {
    if (player) pickOriginal(player);
  });
  // Tried again on the time updates until the player lists the tracks (about ten seconds).
  onTimeUpdate((player) => pickOriginal(player));
}
