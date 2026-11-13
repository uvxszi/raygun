import type {
  DecoderMode,
  MediaKeyAction,
  PlayerOrientation,
  SubtitleEdge,
  SubtitleFont,
  SubtitleMode,
  SubtitleStyle,
  TapAction,
  VideoAspect,
  VideoCodec,
  VideoPlayerPrefs,
} from '@raygun/bindings';
import {
  AudioLines,
  Captions,
  Cog,
  Hand,
  PictureInPicture2,
  SkipForward,
  SlidersHorizontal,
  Wand2,
} from 'lucide-react';
import type { LucideIcon } from 'lucide-react';

import type { PlatformOnly } from '../shell/useIsDesktop';
import {
  NO_EFFECTS,
  NO_SMART_AUDIO,
  withAudioDefaults,
  withEffectDefaults,
} from '../video/effects/effectParams';
import type { Audio, Effects } from '../video/effects/effectParams';

/** Settings → Video player's screens after Playback (`/settings/video/<id>`), as in Aniyomi. */
export type PlayerSettingsPage =
  'controls' | 'gestures' | 'subtitles' | 'audio' | 'skip' | 'pip' | 'effects' | 'advanced';

/** Where Settings → Video player and its screens live (`/settings/player/…` redirects there). */
export const VIDEO_SETTINGS_PATH = '/settings/video';
export const playerPagePath = (page: PlayerSettingsPage) => `${VIDEO_SETTINGS_PATH}/${page}`;

/** `only`: swipes, double taps and Android's picture-in-picture are the phone player's. */
export const PLAYER_PAGES: { id: PlayerSettingsPage; icon: LucideIcon; only?: PlatformOnly }[] = [
  { id: 'controls', icon: SlidersHorizontal },
  { id: 'gestures', icon: Hand, only: 'phone' },
  { id: 'subtitles', icon: Captions },
  { id: 'audio', icon: AudioLines },
  { id: 'skip', icon: SkipForward },
  { id: 'pip', icon: PictureInPicture2, only: 'phone' },
  { id: 'effects', icon: Wand2 },
  { id: 'advanced', icon: Cog },
];

export const isPlayerPage = (value: unknown): value is PlayerSettingsPage =>
  PLAYER_PAGES.some((page) => page.id === value);

/** These match `CONTROLS_SECONDS`, `HOLD_SPEEDS` and `SKIP_WAIT_SECONDS` in raygun-model. */
export const CONTROLS_SECONDS = [2, 3, 4, 5, 8];
export const HOLD_SPEEDS = [0, 150, 200, 300];
export const SKIP_WAIT_SECONDS = [3, 5, 7, 10];

export const ORIENTATIONS: PlayerOrientation[] = [
  'free',
  'video',
  'sensorLandscape',
  'landscape',
  'reverseLandscape',
  'sensorPortrait',
  'portrait',
  'reversePortrait',
];
export const ASPECTS: VideoAspect[] = ['fit', 'crop', 'stretch'];
export const TAP_ACTIONS: TapAction[] = ['seek', 'playPause', 'switchEpisode', 'off'];
export const MEDIA_KEY_ACTIONS: MediaKeyAction[] = ['switchEpisode', 'seek'];
export const DECODERS: DecoderMode[] = ['auto', 'software'];
export const VIDEO_CODECS: VideoCodec[] = ['auto', 'h264'];
export const SUBTITLE_EDGES: SubtitleEdge[] = ['none', 'outline', 'shadow', 'raised', 'depressed'];
/** In the order Android's own caption settings list them (`SUBTITLE_FONTS` in the player). */
export const SUBTITLE_FONTS: SubtitleFont[] = [
  'default',
  'sans',
  'condensed',
  'mono',
  'serif',
  'serifMono',
  'casual',
  'cursive',
  'smallCaps',
];
export const SUBTITLE_MODES: SubtitleMode[] = ['foreign', 'always', 'never'];

/** Colours offered for subtitles, as 0xAARRGGBB, named for the choices. */
export const TEXT_COLORS: { value: number; name: string }[] = [
  { value: 0xffffffff, name: 'white' },
  { value: 0xffffeb3b, name: 'yellow' },
  { value: 0xff80deea, name: 'cyan' },
  { value: 0xffa5d6a7, name: 'green' },
  { value: 0xffbdbdbd, name: 'grey' },
  { value: 0xff000000, name: 'black' },
];
export const EDGE_COLORS: { value: number; name: string }[] = [
  { value: 0xff000000, name: 'black' },
  { value: 0xff424242, name: 'darkGrey' },
  { value: 0xffffffff, name: 'white' },
];
export const BACKGROUND_COLORS: { value: number; name: string }[] = [
  { value: 0x00000000, name: 'none' },
  { value: 0x80000000, name: 'blackHalf' },
  { value: 0xbf000000, name: 'blackMost' },
  { value: 0xff000000, name: 'black' },
  { value: 0x80ffffff, name: 'whiteHalf' },
];

/** Other apps videos can open in: their package and name (not translated). */
export const EXTERNAL_PLAYERS: { value: string; label: string }[] = [
  { value: 'org.videolan.vlc', label: 'VLC' },
  { value: 'is.xyz.mpv', label: 'mpv' },
  { value: 'com.mxtech.videoplayer.ad', label: 'MX Player' },
  { value: 'com.mxtech.videoplayer.pro', label: 'MX Player Pro' },
  { value: 'com.brouken.player', label: 'Just Player' },
  { value: 'dev.anilbeesetti.nextplayer', label: 'Next Player' },
  { value: 'com.instantbits.cast.webvideo', label: 'Web Video Caster' },
];

/** `1.25×` from hundredths. */
export const formatSpeed = (hundredths: number) => `${Number((hundredths / 100).toFixed(2))}×`;

/** "0.5, 1, 1.5" as hundredths; null when a part isn't a speed from 0.25 to 4. */
export const parseSpeeds = (text: string): number[] | null => {
  const parts = text
    .split(/[,\s]+/)
    .map((part) => part.replace(/×|x$/i, '').trim())
    .filter(Boolean);
  if (parts.length === 0 || parts.length > 12) return null;
  const speeds = parts.map((part) => Math.round(Number(part) * 100));
  if (speeds.some((speed) => !Number.isFinite(speed) || speed < 25 || speed > 400)) return null;
  return [...new Set(speeds)].sort((left, right) => left - right);
};

/** A 0xAARRGGBB colour as CSS. */
export const cssColor = (argb: number) => {
  const alpha = ((argb >>> 24) & 0xff) / 255;
  const red = (argb >>> 16) & 0xff;
  const green = (argb >>> 8) & 0xff;
  const blue = argb & 0xff;
  return `rgba(${red}, ${green}, ${blue}, ${Number(alpha.toFixed(3))})`;
};

/** `#rrggbb` of a 0xAARRGGBB colour (its opacity left out). */
export const hexOf = (argb: number) => `#${(argb & 0xffffff).toString(16).padStart(6, '0')}`;

/** A typed `#rrggbb` (the # may be left out) as 0xRRGGBB; null when it isn't one. */
export const parseHex = (text: string): number | null => {
  const digits = text.trim().replace(/^#/, '');
  return /^[0-9a-f]{6}$/i.test(digits) ? Number.parseInt(digits, 16) : null;
};

/** `rgb` with opacity `alpha` (0–255), as an unsigned 0xAARRGGBB. */
export const withAlpha = (rgb: number, alpha: number) =>
  (((alpha & 0xff) << 24) | (rgb & 0xffffff)) >>> 0;

/** The player's settings with every field: Rust always sends them all, but the generated type
 * marks them optional because the stored row may lack ones added later. */
export type PlayerPrefs = Required<
  Omit<VideoPlayerPrefs, 'subtitleStyle' | 'pictureEffects' | 'smartAudio'>
> & {
  subtitleStyle: Required<SubtitleStyle>;
  pictureEffects: Effects;
  smartAudio: Audio;
};

/** Matches `SubtitleStyle::default()` in raygun-model. */
export const DEFAULT_SUBTITLE_STYLE: Required<SubtitleStyle> = {
  size: 100,
  font: 'default',
  bold: false,
  italic: false,
  textColor: 0xffffffff,
  textColorChosen: false,
  edge: 'outline',
  edgeColor: 0xff000000,
  backgroundColor: 0,
  backgroundColorChosen: false,
  position: 8,
  maxWidth: 80,
  ignoreEmbedded: false,
};

/** The line widths offered, in percent of the picture's width (`SubtitleStyle::max_width`). */
export const SUBTITLE_MAX_WIDTH = { min: 40, max: 100, step: 5 };

/** Matches `VideoPlayerPrefs::default()` in raygun-model. */
export const PLAYER_DEFAULTS: PlayerPrefs = {
  version: 1,
  preferredResolution: '1080p',
  orientation: 'free',
  aspect: 'fit',
  controlsSeconds: 4,
  showRemaining: false,
  defaultSpeed: 100,
  speedPresets: [25, 50, 75, 100, 125, 150, 175, 200, 250, 300, 350, 400],
  pitchCorrection: true,
  holdSpeed: 200,
  rememberBrightness: false,
  brightnessGesture: true,
  volumeGesture: true,
  swapSides: false,
  seekGesture: true,
  // Unset: on for phones, off on desktop (`seekByDragging`).
  seekByDragging: null,
  doubleTapLeft: 'seek',
  doubleTapCenter: 'playPause',
  doubleTapRight: 'seek',
  mediaKeys: 'switchEpisode',
  volumeBoost: 100,
  pictureEffects: NO_EFFECTS,
  smartAudio: NO_SMART_AUDIO,
  skipButton: true,
  introSeconds: 85,
  autoSkip: false,
  skipCountdown: false,
  skipWaitSeconds: 5,
  aniskip: false,
  aniskipUnlessChapters: true,
  subtitleFallback: '',
  subtitlePreferWords: '',
  subtitleSkipWords: '',
  // Unset: the older `autoSubtitles` switch decides.
  subtitleMode: null,
  subtitleStyle: DEFAULT_SUBTITLE_STYLE,
  audioLanguage: '',
  audioFallback: '',
  pipOnLeave: true,
  pipOnBack: false,
  decoder: 'auto',
  videoCodec: 'auto',
  externalPlayer: false,
  externalPackage: '',
  titleTrailers: true,
  showFailedSources: true,
  showEmptySources: true,
};

/** Whether the seek bar seeks only by dragging: the setting, else on for phones, off on desktop. */
export const seekByDragging = (prefs: { seekByDragging?: boolean | null }, phone: boolean) =>
  prefs.seekByDragging ?? phone;

/** Fills anything missing from the saved settings with the defaults. */
export const withDefaults = (prefs: VideoPlayerPrefs | undefined): PlayerPrefs => ({
  ...PLAYER_DEFAULTS,
  ...prefs,
  subtitleStyle: { ...DEFAULT_SUBTITLE_STYLE, ...prefs?.subtitleStyle },
  pictureEffects: withEffectDefaults(prefs?.pictureEffects),
  smartAudio: withAudioDefaults(prefs?.smartAudio),
});
