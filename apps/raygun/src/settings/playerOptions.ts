import type {
  DecoderMode,
  MediaKeyAction,
  PlayerOrientation,
  SubtitleEdge,
  SubtitleFont,
  SubtitleStyle,
  TapAction,
  VideoAspect,
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
} from 'lucide-react';
import type { LucideIcon } from 'lucide-react';

/** Settings → Player's screens after Playback (`/settings/player/<id>`), as in Aniyomi. */
export type PlayerSettingsPage =
  'controls' | 'gestures' | 'subtitles' | 'audio' | 'skip' | 'pip' | 'advanced';

export const PLAYER_PAGES: { id: PlayerSettingsPage; icon: LucideIcon }[] = [
  { id: 'controls', icon: SlidersHorizontal },
  { id: 'gestures', icon: Hand },
  { id: 'subtitles', icon: Captions },
  { id: 'audio', icon: AudioLines },
  { id: 'skip', icon: SkipForward },
  { id: 'pip', icon: PictureInPicture2 },
  { id: 'advanced', icon: Cog },
];

export const isPlayerPage = (value: unknown): value is PlayerSettingsPage =>
  PLAYER_PAGES.some((page) => page.id === value);

/** These match `CONTROLS_SECONDS`, `HOLD_SPEEDS` and `SKIP_WAIT_SECONDS` in raygun-model. */
export const CONTROLS_SECONDS = [2, 3, 4, 5, 8];
export const HOLD_SPEEDS = [0, 150, 200, 300];
export const SKIP_WAIT_SECONDS = [3, 5, 7, 10];

export const ORIENTATIONS: PlayerOrientation[] = [
  'sensorLandscape',
  'landscape',
  'reverseLandscape',
  'video',
  'free',
  'sensorPortrait',
  'portrait',
  'reversePortrait',
];
export const ASPECTS: VideoAspect[] = ['fit', 'crop', 'stretch'];
export const TAP_ACTIONS: TapAction[] = ['seek', 'playPause', 'switchEpisode', 'off'];
export const MEDIA_KEY_ACTIONS: MediaKeyAction[] = ['switchEpisode', 'seek'];
export const DECODERS: DecoderMode[] = ['auto', 'software'];
export const SUBTITLE_EDGES: SubtitleEdge[] = ['none', 'outline', 'shadow', 'raised', 'depressed'];
export const SUBTITLE_FONTS: SubtitleFont[] = ['default', 'sans', 'serif', 'mono'];

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

/** The player's settings with every field: Rust always sends them all, but the generated type
 * marks them optional because the stored row may lack ones added later. */
export type PlayerPrefs = Required<Omit<VideoPlayerPrefs, 'subtitleStyle'>> & {
  subtitleStyle: Required<SubtitleStyle>;
};

/** Matches `SubtitleStyle::default()` in raygun-model. */
export const DEFAULT_SUBTITLE_STYLE: Required<SubtitleStyle> = {
  size: 100,
  font: 'default',
  bold: false,
  italic: false,
  textColor: 0xffffffff,
  edge: 'outline',
  edgeColor: 0xff000000,
  backgroundColor: 0,
  position: 8,
  ignoreEmbedded: false,
};

/** Matches `VideoPlayerPrefs::default()` in raygun-model. */
export const PLAYER_DEFAULTS: PlayerPrefs = {
  orientation: 'sensorLandscape',
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
  doubleTapLeft: 'seek',
  doubleTapCenter: 'playPause',
  doubleTapRight: 'seek',
  mediaKeys: 'switchEpisode',
  volumeBoost: 100,
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
  subtitleStyle: DEFAULT_SUBTITLE_STYLE,
  audioLanguage: '',
  audioFallback: '',
  pipOnLeave: true,
  pipOnBack: false,
  decoder: 'auto',
  externalPlayer: false,
  externalPackage: '',
  titleTrailers: true,
  showFailedSources: true,
  showEmptySources: true,
};

/** Fills anything missing from the saved settings with the defaults. */
export const withDefaults = (prefs: VideoPlayerPrefs | undefined): PlayerPrefs => ({
  ...PLAYER_DEFAULTS,
  ...prefs,
  subtitleStyle: { ...DEFAULT_SUBTITLE_STYLE, ...prefs?.subtitleStyle },
});
