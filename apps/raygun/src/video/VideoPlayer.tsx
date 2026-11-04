import { commands } from '@raygun/bindings';
import type {
  ExternalPlayer,
  FoundSubtitles,
  VideoAspect,
  VideoChapter,
  VideoSubtitle,
} from '@raygun/bindings';
import { useQuery } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { getCurrentWindow } from '@tauri-apps/api/window';
import {
  ArrowLeft,
  AudioLines,
  Captions,
  ExternalLink,
  FastForward,
  Keyboard,
  Layers,
  ListVideo,
  LoaderCircle,
  Maximize,
  Minimize,
  Pause,
  PictureInPicture2,
  Play,
  Ratio,
  Rewind,
  Server,
  SkipBack,
  SkipForward,
  Sparkles,
  Volume1,
  Volume2,
  VolumeX,
  X,
} from 'lucide-react';
import { useEffect, useEffectEvent, useRef, useState } from 'react';
import type { FC, ReactNode, RefObject } from 'react';
import { useTranslation } from 'react-i18next';

import { useAiModels } from '../ai/useAi';
import { formatTime } from '../player/formatTime';
import { useCaptions, useStopCaptions } from '../player/spoken';
import { useSettings, useUpdateSetting } from '../settings/useSettings';
import { attachMedia, embeddedCueLines, engineKind } from './mediaEngine';
import type { MediaEngine, StreamProblem } from './mediaEngine';
import { PanelHeading, PanelOption, PlayerPanel } from './PlayerPanel';
import { sendReport, sendRequest } from './protocol';
import type { PlayerRequest, Position, TitleState } from './protocol';
import { SubtitleLines } from './SubtitleLines';
import { CueEditor } from './CueEditor';
import { applyCueEdits, editKey, withCueEdit } from './cueEdits';
import type { CueEdit, CueEdits } from './cueEdits';
import { cuesAt, parseSubtitles } from './subtitles';
import type { Cue } from './subtitles';
import { TimeBar } from './TimeBar';
import { choiceOf, languageOf, languagePrefs, pickAudio, pickSubtitles } from './trackPicking';
import type { TrackOption } from './trackPicking';
import { useVideoPlayerStore } from './videoPlayerStore';
import { sourceName, sourceNote, sourcesChecked } from './waiting';
import type { WaitingNote } from './waiting';
import { useAppActive } from '../shell/deviceState';

type PanelKind =
  | 'episodes'
  | 'sources'
  | 'speed'
  | 'subtitles'
  | 'audio'
  | 'quality'
  | 'aspect'
  | 'players'
  | 'keys';

const VOLUME_KEY = 'raygun.video.volume';
/** The other player last picked (M46.33). */
const PLAYER_KEY = 'raygun.video.externalPlayer';
/** Up to 200 % of the original sound (M46.33). */
const MAX_VOLUME = 2;

const readPlayer = () => {
  try {
    return localStorage.getItem(PLAYER_KEY);
  } catch {
    return null;
  }
};
const savePlayer = (id: string) => {
  try {
    localStorage.setItem(PLAYER_KEY, id);
  } catch {
    // Remembering is a convenience.
  }
};
const REPORT_INTERVAL_MS = 5_000;
const FRAME_SECONDS = 1 / 30;
const ASPECTS: VideoAspect[] = ['fit', 'crop', 'stretch'];
const FIT: Record<VideoAspect, string> = {
  fit: 'object-contain',
  crop: 'object-cover',
  stretch: 'object-fill',
};

const readVolume = () => {
  try {
    const saved = Number(localStorage.getItem(VOLUME_KEY));
    return Number.isFinite(saved) && saved > 0 ? saved : 1;
  } catch {
    return 1;
  }
};

const positionOf = (video: HTMLVideoElement | null): Position => ({
  positionMs: (video?.currentTime ?? 0) * 1000,
  durationMs: video && Number.isFinite(video.duration) ? video.duration * 1000 : null,
});

/** The window's own full screen (Tauri), else the page's. */
const setWindowFullscreen = async (on: boolean) => {
  try {
    await getCurrentWindow().setFullscreen(on);
  } catch {
    try {
      if (on) await document.documentElement.requestFullscreen?.();
      else if (document.fullscreenElement) await document.exitFullscreen();
    } catch {
      // Neither is available (tests); the player still fills the window.
    }
  }
};

const isTyping = (target: EventTarget | null) =>
  target instanceof HTMLElement &&
  (target.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName));

/**
 * The webview's video player (desktop, M46.4). Rust opens it and feeds it with
 * `videoPlayerEvent`s (the Android player's requests and updates) and it reports back the same
 * way, so sessions, resuming, Continue watching and title memory work as on Android.
 */
export const VideoPlayer: FC = () => {
  const open = useVideoPlayerStore((state) => state.open);
  return open ? <PlayerScreen /> : null;
};

const PlayerScreen: FC = () => {
  const { t } = useTranslation();
  const request = useVideoPlayerStore((state) => state.request);
  const loads = useVideoPlayerStore((state) => state.loads);
  const showSources = useVideoPlayerStore((state) => state.showSources);
  const videoRef = useRef<HTMLVideoElement>(null);
  const [panel, setPanel] = useState<PanelKind | null>(null);
  const [volume, setVolumeState] = useState(readVolume);
  const [muted, setMuted] = useState(false);
  const [fullscreen, setFullscreen] = useState(false);
  // An update saying no link worked opens Sources.
  const [sourcesAsked, setSourcesAsked] = useState(showSources);
  if (sourcesAsked !== showSources) {
    setSourcesAsked(showSources);
    setPanel('sources');
  }

  const setVolume = (level: number) => {
    setVolumeState(level);
    setMuted(level <= 0);
    try {
      localStorage.setItem(VOLUME_KEY, String(level));
    } catch {
      // A per-device convenience; playing goes on without it.
    }
  };
  const toggleFullscreen = (on = !fullscreen) => {
    setFullscreen(on);
    void setWindowFullscreen(on);
  };
  const close = () => {
    const state = useVideoPlayerStore.getState();
    sendReport(state.request?.id ?? '', 'closed', positionOf(videoRef.current));
    if (fullscreen) void setWindowFullscreen(false);
    state.close();
  };

  // A session asks for its episodes and sources once the player shows (the Android player does
  // the same, as updates sent before it existed are lost there).
  useEffect(() => {
    const state = useVideoPlayerStore.getState();
    if (state.session) sendRequest(state.request?.id ?? '', 'ready', 0);
  }, []);

  if (request?.url) {
    return (
      <VideoStage
        key={loads}
        request={request}
        videoRef={videoRef}
        panel={panel}
        setPanel={setPanel}
        volume={volume}
        setVolume={setVolume}
        muted={muted}
        setMuted={setMuted}
        fullscreen={fullscreen}
        toggleFullscreen={toggleFullscreen}
        onClose={close}
      />
    );
  }
  return (
    <WaitingScreen
      panel={panel}
      setPanel={setPanel}
      onClose={close}
      fullscreen={fullscreen}
      toggleFullscreen={toggleFullscreen}
      label={t('desktopPlayer.player')}
    />
  );
};

type WaitingProps = {
  panel: PanelKind | null;
  setPanel: (panel: PanelKind | null) => void;
  onClose: () => void;
  fullscreen: boolean;
  toggleFullscreen: (on?: boolean) => void;
  label: string;
};

/** A session finding its first link: the title, a spinner, and Episodes and Sources. */
const WaitingScreen: FC<WaitingProps> = ({
  panel,
  setPanel,
  onClose,
  fullscreen,
  toggleFullscreen,
  label,
}) => {
  const { t } = useTranslation();
  const loadingText = useVideoPlayerStore((state) => state.loadingText);
  const notice = useVideoPlayerStore((state) => state.notice);
  const sources = useVideoPlayerStore((state) => state.sources);

  const onKey = useEffectEvent((event: KeyboardEvent) => {
    if (event.key !== 'Escape') return;
    if (panel) setPanel(null);
    else if (fullscreen) toggleFullscreen(false);
    else onClose();
  });
  useEffect(() => {
    const listener = (event: KeyboardEvent) => onKey(event);
    window.addEventListener('keydown', listener);
    return () => window.removeEventListener('keydown', listener);
  }, []);

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label={label}
      className="fixed inset-0 z-[70] bg-black text-white select-none"
    >
      <TopBar onClose={onClose} setPanel={setPanel} positionMs={0} />
      <Middle>
        {notice ? (
          <NoticeBox onSources={() => setPanel('sources')} />
        ) : (
          <LoadingRing
            text={loadingText || t('videoPlayer.findingSource')}
            note={sourcesChecked(sources)}
          />
        )}
      </Middle>
      {(panel === 'episodes' || panel === 'sources') && (
        <SessionPanel kind={panel} positionMs={0} onClose={() => setPanel(null)} />
      )}
    </div>
  );
};

type StageProps = {
  request: PlayerRequest;
  videoRef: RefObject<HTMLVideoElement | null>;
  panel: PanelKind | null;
  setPanel: (panel: PanelKind | null) => void;
  volume: number;
  setVolume: (level: number) => void;
  muted: boolean;
  setMuted: (muted: boolean) => void;
  fullscreen: boolean;
  toggleFullscreen: (on?: boolean) => void;
  onClose: () => void;
};

type SubtitleOption = TrackOption & { cues?: Cue[] };

/** One video: the picture, its subtitles, and the controls. Mounted again for each load. */
const VideoStage: FC<StageProps> = ({
  request,
  videoRef,
  panel,
  setPanel,
  volume,
  setVolume,
  muted,
  setMuted,
  fullscreen,
  toggleFullscreen,
  onClose,
}) => {
  const { t } = useTranslation();
  const session = useVideoPlayerStore((state) => state.session);
  const prefs = useVideoPlayerStore((state) => state.prefs);
  const seekStepMs = useVideoPlayerStore((state) => state.seekStepMs);
  const appLanguage = useVideoPlayerStore((state) => state.appLanguage);
  const subtitleLanguage = useVideoPlayerStore((state) => state.subtitleLanguage);
  const subtitleMode = useVideoPlayerStore((state) => state.subtitleMode);
  const autoplay = useVideoPlayerStore((state) => state.autoplay);
  const nextPromptMs = useVideoPlayerStore((state) => state.nextPromptMs);
  const loadingText = useVideoPlayerStore((state) => state.loadingText);
  const notice = useVideoPlayerStore((state) => state.notice);
  const setNotice = useVideoPlayerStore((state) => state.setNotice);
  const hasNext = useVideoPlayerStore((state) => state.hasNext);
  const hasPrevious = useVideoPlayerStore((state) => state.hasPrevious);
  const chapters = useVideoPlayerStore((state) => state.chapters);
  // Intros are skipped in series; a film in a session is its only episode.
  const series = useVideoPlayerStore((state) => state.episodes.length > 1);

  const id = request.id;
  const titleState: TitleState = request.titleState ?? {};
  const group = request.group ?? id;
  // The engine for rendering; the ref for callbacks that may run before a render.
  const [engine, setEngine] = useState<MediaEngine | null>(null);
  const engineRef = useRef<MediaEngine | null>(null);
  const boostRef = useRef<{ context: AudioContext; gain: GainNode } | null>(null);
  const hideTimer = useRef<number | undefined>(undefined);
  const clickTimer = useRef<number | undefined>(undefined);
  const hintTimer = useRef<number | undefined>(undefined);
  const flags = useRef({
    started: false,
    picked: false,
    aniskip: false,
    next: false,
    elsewhere: false,
  });
  const skipped = useRef(new Set<number>());
  const subtitleFiles = useRef(new Map<string, Cue[]>());

  const [playing, setPlaying] = useState(false);
  const [waiting, setWaiting] = useState(true);
  const [timeMs, setTimeMs] = useState(request.startMs ?? 0);
  const [durationMs, setDurationMs] = useState(0);
  const [bufferedMs, setBufferedMs] = useState(0);
  const [speed, setSpeed] = useState(titleState.speed ?? prefs.defaultSpeed);
  const [aspect, setAspect] = useState<VideoAspect>(titleState.aspect ?? prefs.aspect);
  const [delayMs, setDelayMs] = useState(titleState.subtitleDelayMs ?? 0);
  const [cueEdits, setCueEdits] = useState<CueEdits>(titleState.subtitleEdits ?? {});
  const [subtitleKey, setSubtitleKey] = useState<string | null>(null);
  const [lastSubtitle, setLastSubtitle] = useState<string | null>(null);
  const [loadedCues, setLoadedCues] = useState<{ key: string; cues: Cue[] } | null>(null);
  const [added, setAdded] = useState<SubtitleOption[]>([]);
  const [embeddedLines, setEmbeddedLines] = useState<string[]>([]);
  const [, setTracksVersion] = useState(0);
  const [ended, setEnded] = useState(false);
  const [nextCancelled, setNextCancelled] = useState(false);
  const [controlsShown, setControlsShown] = useState(true);
  const [overControls, setOverControls] = useState(false);
  const [showRemaining, setShowRemaining] = useState(prefs.showRemaining);
  const [hint, setHint] = useState<string | null>(null);
  /** The video has played: the source it comes from no longer shows under the ring. */
  const [begun, setBegun] = useState(false);
  const sources = useVideoPlayerStore((state) => state.sources);
  const pickedSource = useVideoPlayerStore((state) => state.pickedSource);

  const heading = request.subtitle
    ? `${request.title ?? ''} · ${request.subtitle}`
    : (request.title ?? '');
  const maxVolume = MAX_VOLUME;

  // ---- AI subtitles (M46.33): the lines so far, a track like any other ----
  const ai = useCaptions(id);
  const aiCues: Cue[] = (ai.data?.cues ?? []).map((cue) => ({
    startMs: cue.startMs,
    endMs: cue.endMs,
    lines: [cue.text],
  }));
  const aiOptions: SubtitleOption[] =
    aiCues.length > 0 || ai.data?.running
      ? [{ key: 'ai', label: t('desktopPlayer.ai.track'), language: null, cues: aiCues }]
      : [];

  // ---- What can be chosen ----
  const fileOptions: SubtitleOption[] = (request.subtitles ?? []).map((subtitle, index) => {
    const label = subtitle.label || subtitle.lang || t('videoPlayer.track', { n: index + 1 });
    return { key: `file:${index}`, label, language: subtitle.lang, forced: /forced/i.test(label) };
  });
  const subtitleOptions = (current: MediaEngine | null): SubtitleOption[] => [
    ...(current?.textTracks() ?? []).map((track) => ({
      key: `embedded:${track.id}`,
      label: track.label,
      language: track.language,
      original: track.original,
      forced: track.forced || /forced/i.test(track.label),
    })),
    // The saved AI track is the live one when that's there.
    ...fileOptions.filter(
      (option) => !(aiOptions.length > 0 && option.label === t('desktopPlayer.ai.track')),
    ),
    ...aiOptions,
    ...added,
  ];

  // ---- Small helpers ----
  const remember = (change: Record<string, unknown>) =>
    sendRequest(id, 'state', positionOf(videoRef.current).positionMs, { group, change });
  const savePrefs = (change: Record<string, unknown>) =>
    sendRequest(id, 'prefs', positionOf(videoRef.current).positionMs, { change });
  const showHint = (text: string) => {
    setHint(text);
    window.clearTimeout(hintTimer.current);
    hintTimer.current = window.setTimeout(() => setHint(null), 1_200);
  };
  const bump = () => {
    setControlsShown(true);
    window.clearTimeout(hideTimer.current);
    hideTimer.current = window.setTimeout(
      () => setControlsShown(false),
      prefs.controlsSeconds * 1_000,
    );
  };
  const seekTo = (positionMs: number) => {
    const video = videoRef.current;
    if (!video) return;
    const end = Number.isFinite(video.duration) ? video.duration * 1000 : positionMs;
    const target = Math.max(0, Math.min(positionMs, end));
    video.currentTime = target / 1000;
    setTimeMs(target);
  };
  const seekBy = (deltaMs: number) => seekTo(positionOf(videoRef.current).positionMs + deltaMs);
  const togglePlay = () => {
    const video = videoRef.current;
    if (!video) return;
    if (video.paused || video.ended) void video.play().catch(() => {});
    else video.pause();
  };
  const requestEpisode = (action: 'next' | 'previous') => {
    if (action === 'next') {
      if (flags.current.next) return;
      flags.current.next = true;
    }
    sendRequest(id, action, positionOf(videoRef.current).positionMs);
  };
  const changeSpeed = (next: number) => {
    setSpeed(next);
    if (videoRef.current) videoRef.current.playbackRate = next / 100;
    remember({ speed: next });
    showHint(`${next / 100}×`);
  };
  const changeAspect = (next: VideoAspect) => {
    setAspect(next);
    remember({ aspect: next });
  };
  const changeDelay = (next: number) => {
    setDelayMs(next);
    remember({ subtitleDelayMs: next });
  };
  const changeVolume = (level: number) => {
    const clamped = Math.max(0, Math.min(maxVolume, Math.round(level * 100) / 100));
    setVolume(clamped);
    showHint(t('desktopPlayer.volumeLevel', { n: Math.round(clamped * 100) }));
  };
  const selectSubtitle = (key: string | null, save = true) => {
    setSubtitleKey(key);
    if (key) setLastSubtitle(key);
    engineRef.current?.setTextTrack(key?.startsWith('embedded:') ? Number(key.slice(9)) : -1);
    if (!save) return;
    const option = subtitleOptions(engineRef.current).find((candidate) => candidate.key === key);
    remember({ subtitle: option ? choiceOf(option) : { off: true } });
  };
  const playElsewhere = async (player: string | null = readPlayer()) => {
    const video = videoRef.current;
    if (player) savePlayer(player);
    const result = await commands.videoPlayerExternal(
      {
        url: request.url ?? '',
        title: heading,
        subtitles: (request.subtitles ?? []).map((subtitle) => subtitle.url),
        startMs: positionOf(video).positionMs,
      },
      player,
    );
    if (result.status === 'ok') {
      video?.pause();
      setNotice({ text: t('desktopPlayer.playingIn', { app: result.data }) });
    } else {
      setNotice({ text: result.error });
    }
  };

  // ---- Which tracks play, once the stream is known ----
  const pickTracks = () => {
    if (flags.current.picked) return;
    flags.current.picked = true;
    const current = engineRef.current;
    const languages = languagePrefs({
      audioLanguage: prefs.audioLanguage,
      audioFallback: prefs.audioFallback,
      subtitleFallback: prefs.subtitleFallback,
      appLanguage,
      subtitleLanguage,
      subtitleMode,
      spoken: request.spokenLanguage ?? null,
      device: typeof navigator === 'undefined' ? [] : navigator.languages,
    });
    // Audio first: whether subtitles turn on depends on the language it's in.
    const audio = (current?.audioTracks() ?? []).map((track) => ({
      key: String(track.id),
      label: track.label,
      language: track.language,
      original: track.original,
      extra: track.extra,
    }));
    const playing = current ? String(current.audioTrack()) : undefined;
    const chosenAudio = pickAudio(audio, titleState.audio, languages, playing);
    if (chosenAudio && chosenAudio.key !== playing) current?.setAudioTrack(Number(chosenAudio.key));
    const heard = chosenAudio ?? audio.find((option) => option.key === playing);
    const spoken = (heard ? languageOf(heard) : null) ?? languages.spoken;
    const chosen = pickSubtitles(
      subtitleOptions(current),
      titleState.subtitle,
      languages,
      spoken,
      prefs.subtitlePreferWords,
      prefs.subtitleSkipWords,
    );
    selectSubtitle(chosen?.key ?? null, false);
  };

  // ---- The stream ----
  const onTracks = useEffectEvent(() => {
    setTracksVersion((version) => version + 1);
    // HLS and DASH know their tracks before the video's metadata: picking now spares a switch.
    if (engineRef.current?.tracksKnown()) pickTracks();
  });
  const onFatal = useEffectEvent((message: string, problem: StreamProblem) => {
    const video = videoRef.current;
    sendReport(id, 'error', positionOf(video), message);
    if (session) {
      useVideoPlayerStore.setState({ loadingText: t('videoPlayer.tryingNext'), notice: null });
    } else {
      setNotice({ text: t(PROBLEM_TEXT[problem]), detail: message, elsewhere: true });
    }
  });
  const startElsewhere = useEffectEvent(() => {
    if (flags.current.elsewhere) return;
    flags.current.elsewhere = true;
    void playElsewhere();
  });
  useEffect(() => {
    const video = videoRef.current;
    const url = request.url;
    if (!video || !url) return;
    let cancelled = false;
    let attached: MediaEngine | null = null;
    let triedHls = false;
    const start = (kind: MediaEngine['kind']) => {
      attachMedia(video, url, kind, {
        onTracks: () => {
          if (!cancelled) onTracks();
        },
        onFatal: (message, problem) => {
          if (cancelled) return;
          // An address without an extension may be HLS after all.
          if (problem === 'unsupported' && kind === 'native' && !request.format && !triedHls) {
            triedHls = true;
            attached?.destroy();
            start('hls');
            return;
          }
          onFatal(message, problem);
        },
      })
        .then((engineAttached) => {
          if (cancelled) {
            engineAttached.destroy();
            return;
          }
          attached = engineAttached;
          engineRef.current = engineAttached;
          setEngine(engineAttached);
          onTracks();
        })
        .catch((error: unknown) => {
          if (!cancelled) onFatal(String(error), 'other');
        });
    };
    start(engineKind(url, request.format));
    if (request.external) startElsewhere();
    return () => {
      cancelled = true;
      const state = useVideoPlayerStore.getState();
      // Another video replaces this one (a closing player reports "closed" itself).
      if (state.open && state.request?.id !== request.id) {
        sendReport(request.id, 'switched', positionOf(video));
      }
      attached?.destroy();
      engineRef.current = null;
    };
  }, [request, videoRef]);

  /** Time, the stream's own subtitles, and skipping openings by themselves. */
  const tick = () => {
    const video = videoRef.current;
    if (!video) return;
    const now = video.currentTime * 1000;
    setTimeMs(now);
    if (subtitleKey?.startsWith('embedded:')) setEmbeddedLines(embeddedCueLines(video));
    const chapter = skippableAt(chapters, now, durationMs);
    if (prefs.autoSkip && chapter && !skipped.current.has(chapter.startMs)) {
      skipped.current.add(chapter.startMs);
      seekTo(chapterEnd(chapters, chapter, durationMs));
      showHint(t('videoPlayer.skipped'));
    }
  };
  // ---- The element's events ----
  const onMedia = useEffectEvent((type: string) => {
    const video = videoRef.current;
    if (!video) return;
    switch (type) {
      case 'play':
        setPlaying(true);
        setEnded(false);
        break;
      case 'playing':
        setPlaying(!video.paused);
        setWaiting(false);
        setBegun(true);
        if (useVideoPlayerStore.getState().pickedSource)
          useVideoPlayerStore.setState({ pickedSource: false });
        break;
      case 'canplay':
        setPlaying(!video.paused);
        setWaiting(false);
        break;
      case 'waiting':
        setWaiting(true);
        break;
      case 'pause':
        setPlaying(false);
        if (!video.ended) sendReport(id, 'progress', positionOf(video));
        break;
      case 'timeupdate':
      case 'seeked':
        tick();
        break;
      case 'durationchange':
        setDurationMs(Number.isFinite(video.duration) ? video.duration * 1000 : 0);
        break;
      case 'progress':
        if (video.buffered.length > 0)
          setBufferedMs(video.buffered.end(video.buffered.length - 1) * 1000);
        break;
      case 'ended':
        setEnded(true);
        setPlaying(false);
        sendReport(id, 'ended', positionOf(video));
        if (session && hasNext && autoplay && !nextCancelled) requestEpisode('next');
        break;
      case 'loadedmetadata': {
        setWaiting(false);
        setDurationMs(Number.isFinite(video.duration) ? video.duration * 1000 : 0);
        if (!flags.current.started) {
          flags.current.started = true;
          const startMs = request.startMs ?? 0;
          if (startMs > 0) video.currentTime = startMs / 1000;
          video.playbackRate = speed / 100;
          video.preservesPitch = prefs.pitchCorrection;
          if (!request.external) void video.play().catch(() => {});
        }
        pickTracks();
        if (request.anime && !flags.current.aniskip && Number.isFinite(video.duration)) {
          flags.current.aniskip = true;
          sendRequest(id, 'duration', video.currentTime * 1000, {
            durationMs: video.duration * 1000,
            anime: request.anime,
          });
        }
        break;
      }
      default:
        break;
    }
  });
  useEffect(() => {
    const video = videoRef.current;
    if (!video) return;
    const types = [
      'play',
      'playing',
      'canplay',
      'waiting',
      'pause',
      'timeupdate',
      'seeked',
      'durationchange',
      'progress',
      'ended',
      'loadedmetadata',
    ];
    const listeners = types.map((type) => [type, () => onMedia(type)] as const);
    for (const [type, listener] of listeners) video.addEventListener(type, listener);
    return () => {
      for (const [type, listener] of listeners) video.removeEventListener(type, listener);
    };
  }, [videoRef]);

  const onTick = useEffectEvent(tick);
  const onReportTick = useEffectEvent(() =>
    sendReport(id, 'progress', positionOf(videoRef.current)),
  );
  // Out of sight the time shown needn't move: a slow tick still skips intros.
  const onScreen = useAppActive();
  useEffect(() => {
    if (!playing) return;
    const fast = window.setInterval(() => onTick(), onScreen ? 100 : 1000);
    const reports = window.setInterval(() => onReportTick(), REPORT_INTERVAL_MS);
    return () => {
      window.clearInterval(fast);
      window.clearInterval(reports);
    };
  }, [playing, onScreen]);

  // Another episode is on its way: this one stops here.
  useEffect(() => {
    if (loadingText !== null) videoRef.current?.pause();
  }, [loadingText, videoRef]);

  // Volume, past 100 % through a gain node when boosting.
  useEffect(() => {
    const video = videoRef.current;
    if (!video) return;
    video.volume = Math.min(1, volume);
    video.muted = muted;
    if (volume > 1 && !boostRef.current && typeof AudioContext !== 'undefined') {
      try {
        const context = new AudioContext();
        const gain = context.createGain();
        context.createMediaElementSource(video).connect(gain).connect(context.destination);
        boostRef.current = { context, gain };
      } catch {
        // No boost; 100 % is the most.
      }
    }
    if (boostRef.current) boostRef.current.gain.gain.value = Math.max(1, volume);
  }, [volume, muted, videoRef]);
  useEffect(
    () => () => {
      void boostRef.current?.context.close();
      window.clearTimeout(hideTimer.current);
      window.clearTimeout(clickTimer.current);
      window.clearTimeout(hintTimer.current);
    },
    [],
  );

  // A chosen subtitle file is fetched once (through the proxy's `/subtitle/` route).
  useEffect(() => {
    if (!subtitleKey?.startsWith('file:')) return;
    const subtitle = request.subtitles?.[Number(subtitleKey.slice(5))];
    if (!subtitle) return;
    const cached = subtitleFiles.current.get(subtitle.url);
    let cancelled = false;
    const loaded = cached
      ? Promise.resolve(cached)
      : fetch(subtitle.url)
          .then((response) => response.text())
          .then((text) => parseSubtitles(text, subtitle.format))
          .catch(() => [] as Cue[]);
    void loaded.then((cues) => {
      subtitleFiles.current.set(subtitle.url, cues);
      if (!cancelled) setLoadedCues({ key: subtitleKey, cues });
    });
    return () => {
      cancelled = true;
    };
  }, [subtitleKey, request.subtitles]);

  // ---- Keys ----
  const onKey = useEffectEvent((event: KeyboardEvent) => {
    if (event.key !== 'Escape' && isTyping(event.target)) return;
    if (event.ctrlKey || event.metaKey || event.altKey) return;
    const video = videoRef.current;
    let handled = true;
    switch (event.key) {
      case ' ':
      case 'k':
      case 'K':
        togglePlay();
        break;
      case 'ArrowLeft':
        seekBy(-seekStepMs);
        break;
      case 'ArrowRight':
        seekBy(seekStepMs);
        break;
      case 'j':
      case 'J':
        seekBy(-10_000);
        break;
      case 'l':
      case 'L':
        seekBy(10_000);
        break;
      case 'ArrowUp':
        changeVolume(volume + 0.05);
        break;
      case 'ArrowDown':
        changeVolume(volume - 0.05);
        break;
      case 'm':
      case 'M':
        setMuted(!muted);
        break;
      case 'f':
      case 'F':
        toggleFullscreen();
        break;
      case 'Escape':
        if (panel) setPanel(null);
        else if (fullscreen) toggleFullscreen(false);
        else onClose();
        break;
      case 'N':
        if (event.shiftKey && session && hasNext) requestEpisode('next');
        else handled = false;
        break;
      case 'P':
        if (event.shiftKey && session && hasPrevious) requestEpisode('previous');
        else handled = false;
        break;
      case 'c':
      case 'C': {
        const options = subtitleOptions(engineRef.current);
        const back =
          lastSubtitle && options.some((option) => option.key === lastSubtitle)
            ? lastSubtitle
            : options[0]?.key;
        selectSubtitle(subtitleKey ? null : (back ?? null));
        break;
      }
      case '[':
      case ']': {
        const presets = [...prefs.speedPresets].sort((a, b) => a - b);
        const next =
          event.key === ']'
            ? presets.find((preset) => preset > speed)
            : [...presets].reverse().find((preset) => preset < speed);
        if (next) changeSpeed(next);
        break;
      }
      case ',':
      case '.':
        if (video?.paused) video.currentTime += event.key === '.' ? FRAME_SECONDS : -FRAME_SECONDS;
        break;
      case '?':
        setPanel(panel === 'keys' ? null : 'keys');
        break;
      case 's':
      case 'S': {
        const chapter = skippableAt(chapters, timeMs, durationMs);
        if (chapter) seekTo(chapterEnd(chapters, chapter, durationMs));
        else handled = false;
        break;
      }
      default:
        if (/^[0-9]$/.test(event.key) && durationMs > 0)
          seekTo((durationMs * Number(event.key)) / 10);
        else handled = false;
    }
    if (handled) {
      event.preventDefault();
      bump();
    }
  });
  useEffect(() => {
    const listener = (event: KeyboardEvent) => onKey(event);
    window.addEventListener('keydown', listener);
    return () => window.removeEventListener('keydown', listener);
  }, []);

  // ---- What shows ----
  const originalCues =
    subtitleKey === 'ai'
      ? aiCues
      : subtitleKey?.startsWith('added:')
        ? (added.find((option) => option.key === subtitleKey)?.cues ?? [])
        : loadedCues?.key === subtitleKey
          ? loadedCues.cues
          : [];
  // Lines changed by hand (M46.12) are kept for this episode and this track.
  const trackOption = subtitleOptions(engine).find((option) => option.key === subtitleKey);
  const trackEditKey = trackOption ? editKey(id, trackOption.label, trackOption.language) : null;
  const trackEdits = trackEditKey ? (cueEdits[trackEditKey] ?? []) : [];
  const currentCues = applyCueEdits(originalCues, trackEdits);
  const changeCue = (at: number, edit: Omit<CueEdit, 'at'> | null) => {
    if (!trackEditKey) return;
    const next: CueEdits = { ...cueEdits, [trackEditKey]: withCueEdit(trackEdits, at, edit) };
    if (next[trackEditKey]?.length === 0) delete next[trackEditKey];
    setCueEdits(next);
    remember({ subtitleEdits: next });
  };
  const lines = !subtitleKey
    ? []
    : subtitleKey.startsWith('embedded:')
      ? embeddedLines
      : cuesAt(currentCues, timeMs - delayMs);
  const controlsVisible =
    controlsShown || !playing || panel !== null || overControls || notice !== null;
  const skippable = prefs.skipButton ? skippableAt(chapters, timeMs, durationMs) : null;
  const introMs = titleState.introMs ?? prefs.introSeconds * 1000;
  const offerIntro =
    prefs.skipButton &&
    !skippable &&
    session &&
    series &&
    timeMs > 1_000 &&
    timeMs < 5 * 60_000 &&
    !chapters.some((chapter) => chapter.kind === 'opening');
  const remainingMs = durationMs - timeMs;
  // Over credits (an ending chapter that runs to the end) the prompt shows from their start; it
  // counts down in its last seconds.
  const countdownMs = nextPromptMs > 0 ? nextPromptMs : 1_000;
  const credits = [...chapters]
    .reverse()
    .find(
      (chapter) =>
        chapter.kind === 'ending' && (chapter.endMs ?? durationMs) >= durationMs - CREDITS_SLACK_MS,
    );
  const promptWindowMs = Math.max(countdownMs, credits ? durationMs - credits.startMs : 0);
  const nextPrompt =
    session &&
    hasNext &&
    durationMs > 0 &&
    !ended &&
    !nextCancelled &&
    remainingMs <= promptWindowMs;
  const chapterNow = [...chapters].reverse().find((chapter) => chapter.startMs <= timeMs);
  const audioTracks = engine?.audioTracks() ?? [];
  const qualities = engine?.qualities() ?? [];
  const busy = (waiting || loadingText !== null) && !notice;
  // Before the picture: which source it comes from ("Trying source 2 of 5 · Vidstream").
  const waitingNote = begun || loadingText !== null ? null : sourceNote(sources, pickedSource);
  const VolumeIcon = muted || volume <= 0 ? VolumeX : volume < 0.5 ? Volume1 : Volume2;

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label={t('desktopPlayer.player')}
      onMouseMove={bump}
      onWheel={(event) => {
        if (panel) return;
        changeVolume(volume + (event.deltaY < 0 ? 0.05 : -0.05));
      }}
      className={`fixed inset-0 z-[70] overflow-hidden bg-black text-white select-none ${controlsVisible ? '' : 'cursor-none'}`}
    >
      <video
        ref={videoRef}
        data-testid="desktop-video"
        crossOrigin="anonymous"
        playsInline
        className={`absolute inset-0 size-full ${FIT[aspect]}`}
        onClick={() => {
          window.clearTimeout(clickTimer.current);
          clickTimer.current = window.setTimeout(togglePlay, 220);
        }}
        onDoubleClick={() => {
          window.clearTimeout(clickTimer.current);
          toggleFullscreen();
        }}
      />
      <SubtitleLines lines={lines} style={prefs.subtitleStyle} lifted={controlsVisible} />

      <Middle>
        {notice ? (
          <NoticeBox
            onSources={() => setPanel('sources')}
            onRetry={() =>
              session
                ? sendRequest(id, 'retry', timeMs)
                : useVideoPlayerStore
                    .getState()
                    .apply({ type: 'load', request: { ...request, startMs: timeMs } })
            }
            onElsewhere={notice.elsewhere ? () => setPanel('players') : undefined}
          />
        ) : busy ? (
          <LoadingRing text={loadingText} note={waitingNote} />
        ) : null}
      </Middle>

      {hint && (
        <p
          role="status"
          className="pointer-events-none absolute top-16 left-1/2 -translate-x-1/2 rounded-full bg-black/70 px-4 py-1.5 text-sm"
        >
          {hint}
        </p>
      )}

      {/* Over the credits the next-episode prompt takes the skip button's place. */}
      {(skippable || offerIntro) && !nextPrompt && (
        <button
          type="button"
          onClick={() =>
            skippable ? seekTo(chapterEnd(chapters, skippable, durationMs)) : seekBy(introMs)
          }
          className="absolute right-6 bottom-28 z-10 rounded-xl bg-white/90 px-5 py-2.5 text-sm font-semibold text-black shadow-lg hover:bg-white"
        >
          {skippable
            ? t(SKIP_LABEL[skippable.kind])
            : t('videoPlayer.skipBy', { n: Math.round(introMs / 1000) })}
        </button>
      )}

      {nextPrompt && (
        <div className="absolute right-6 bottom-28 z-10 flex items-center gap-2 rounded-xl bg-neutral-900/95 p-2 pl-4 shadow-xl ring-1 ring-white/10">
          <span className="text-sm">
            {autoplay && remainingMs <= countdownMs
              ? t('videoPlayer.nextIn', { seconds: Math.max(0, Math.ceil(remainingMs / 1000)) })
              : t('videoPlayer.nextEpisode')}
          </span>
          <button
            type="button"
            onClick={() => requestEpisode('next')}
            className="bg-primary text-background rounded-lg px-3 py-1.5 text-sm font-semibold"
          >
            {t('desktopPlayer.playNow')}
          </button>
          <button
            type="button"
            aria-label={t('videoPlayer.cancel')}
            onClick={() => setNextCancelled(true)}
            className="grid size-8 place-items-center rounded-lg hover:bg-white/10"
          >
            <X aria-hidden size={16} />
          </button>
        </div>
      )}

      <div
        className={`transition-opacity duration-200 ${controlsVisible ? 'opacity-100' : 'pointer-events-none opacity-0'}`}
        onMouseEnter={() => setOverControls(true)}
        onMouseLeave={() => setOverControls(false)}
      >
        <TopBar
          onClose={onClose}
          setPanel={setPanel}
          positionMs={timeMs}
          onElsewhere={() => setPanel(panel === 'players' ? null : 'players')}
        />
        <div className="absolute inset-x-0 bottom-0 bg-gradient-to-t from-black/85 via-black/50 to-transparent px-5 pt-12 pb-3">
          <TimeBar
            timeMs={timeMs}
            durationMs={durationMs}
            bufferedMs={bufferedMs}
            chapters={chapters}
            seekStepMs={seekStepMs}
            onSeek={seekTo}
          />
          <div className="mt-1 flex items-center gap-1">
            <IconButton
              label={playing ? t('videoPlayer.pause') : t('videoPlayer.play')}
              onClick={togglePlay}
            >
              {playing ? (
                <Pause aria-hidden size={22} fill="currentColor" />
              ) : (
                <Play aria-hidden size={22} fill="currentColor" />
              )}
            </IconButton>
            {session && (
              <>
                <IconButton
                  label={t('videoPlayer.previous')}
                  disabled={!hasPrevious}
                  onClick={() => requestEpisode('previous')}
                >
                  <SkipBack aria-hidden size={20} />
                </IconButton>
                <IconButton
                  label={t('videoPlayer.next')}
                  disabled={!hasNext}
                  onClick={() => requestEpisode('next')}
                >
                  <SkipForward aria-hidden size={20} />
                </IconButton>
              </>
            )}
            <IconButton
              label={t('desktopPlayer.seekBack', { n: seekStepMs / 1000 })}
              onClick={() => seekBy(-seekStepMs)}
            >
              <Rewind aria-hidden size={20} />
            </IconButton>
            <IconButton
              label={t('desktopPlayer.seekForward', { n: seekStepMs / 1000 })}
              onClick={() => seekBy(seekStepMs)}
            >
              <FastForward aria-hidden size={20} />
            </IconButton>
            <div className="group/volume flex items-center">
              <IconButton
                label={muted ? t('desktopPlayer.unmute') : t('desktopPlayer.mute')}
                onClick={() => setMuted(!muted)}
              >
                <VolumeIcon aria-hidden size={20} />
              </IconButton>
              <input
                type="range"
                aria-label={t('videoPlayer.volume')}
                min={0}
                max={maxVolume}
                step={0.01}
                value={muted ? 0 : volume}
                onChange={(event) => setVolume(Number(event.target.value))}
                className="accent-primary w-0 opacity-0 transition-all group-hover/volume:w-24 group-hover/volume:opacity-100 focus:w-24 focus:opacity-100"
              />
            </div>
            <button
              type="button"
              onClick={() => {
                setShowRemaining(!showRemaining);
                savePrefs({ showRemaining: !showRemaining });
              }}
              className="ml-2 rounded px-1 text-sm tabular-nums text-white/90 hover:text-white"
            >
              {showRemaining ? `−${formatTime(remainingMs)}` : formatTime(timeMs)} /{' '}
              {formatTime(durationMs)}
            </button>
            {chapterNow?.name && (
              <span className="ml-3 truncate text-sm text-white/70">{chapterNow.name}</span>
            )}
            <span className="flex-1" />
            <button
              type="button"
              aria-label={t('videoPlayer.speed')}
              onClick={() => setPanel(panel === 'speed' ? null : 'speed')}
              className="rounded-lg px-2 py-1 text-sm font-semibold tabular-nums hover:bg-white/10"
            >
              {speed / 100}×
            </button>
            <IconButton
              label={t('videoPlayer.subtitles')}
              pressed={subtitleKey !== null}
              onClick={() => setPanel(panel === 'subtitles' ? null : 'subtitles')}
            >
              <Captions aria-hidden size={20} />
            </IconButton>
            {audioTracks.length > 1 && (
              <IconButton
                label={t('videoPlayer.audio')}
                onClick={() => setPanel(panel === 'audio' ? null : 'audio')}
              >
                <AudioLines aria-hidden size={20} />
              </IconButton>
            )}
            {qualities.length > 1 && (
              <IconButton
                label={t('desktopPlayer.quality')}
                onClick={() => setPanel(panel === 'quality' ? null : 'quality')}
              >
                <Layers aria-hidden size={20} />
              </IconButton>
            )}
            <IconButton
              label={t('videoPlayer.aspect')}
              onClick={() => setPanel(panel === 'aspect' ? null : 'aspect')}
            >
              <Ratio aria-hidden size={20} />
            </IconButton>
            {typeof document !== 'undefined' && document.pictureInPictureEnabled && (
              <IconButton
                label={t('videoPlayer.pip')}
                onClick={() => void videoRef.current?.requestPictureInPicture().catch(() => {})}
              >
                <PictureInPicture2 aria-hidden size={20} />
              </IconButton>
            )}
            <IconButton
              label={fullscreen ? t('desktopPlayer.exitFullscreen') : t('desktopPlayer.fullscreen')}
              onClick={() => toggleFullscreen()}
            >
              {fullscreen ? <Minimize aria-hidden size={20} /> : <Maximize aria-hidden size={20} />}
            </IconButton>
          </div>
        </div>
      </div>

      {(panel === 'episodes' || panel === 'sources') && (
        <SessionPanel kind={panel} positionMs={timeMs} onClose={() => setPanel(null)} />
      )}
      {panel === 'players' && (
        <PlayersPanel
          onPick={(player) => {
            setPanel(null);
            void playElsewhere(player);
          }}
          onClose={() => setPanel(null)}
        />
      )}
      {panel === 'keys' && <KeysPanel onClose={() => setPanel(null)} />}
      {panel === 'speed' && (
        <PlayerPanel title={t('videoPlayer.speed')} onClose={() => setPanel(null)}>
          {[...prefs.speedPresets]
            .sort((a, b) => a - b)
            .map((preset) => (
              <PanelOption
                key={preset}
                label={`${preset / 100}×`}
                selected={preset === speed}
                onSelect={() => changeSpeed(preset)}
              />
            ))}
          <PanelHeading>{t('videoPlayer.more')}</PanelHeading>
          <PanelOption
            label={t('videoPlayer.makeDefault')}
            hint={t('videoPlayer.madeDefault', { n: `${prefs.defaultSpeed / 100}×` })}
            selected={prefs.defaultSpeed === speed}
            onSelect={() => {
              savePrefs({ defaultSpeed: speed });
              useVideoPlayerStore.setState({ prefs: { ...prefs, defaultSpeed: speed } });
            }}
          />
        </PlayerPanel>
      )}
      {panel === 'subtitles' && (
        <PlayerPanel title={t('videoPlayer.subtitles')} onClose={() => setPanel(null)}>
          <PanelOption
            label={t('videoPlayer.off')}
            selected={subtitleKey === null}
            onSelect={() => selectSubtitle(null)}
          />
          {subtitleOptions(engine).map((option) => (
            <PanelOption
              key={option.key}
              label={option.label}
              hint={
                option.key.startsWith('embedded:') ? t('desktopPlayer.inTheVideo') : option.language
              }
              selected={option.key === subtitleKey}
              onSelect={() => selectSubtitle(option.key)}
            />
          ))}
          <label className="flex cursor-pointer items-center gap-3 rounded-lg px-3 py-2 text-sm hover:bg-white/10">
            {t('videoPlayer.addSubtitleFile')}
            <input
              type="file"
              accept=".srt,.vtt,.ass,.ssa,.sub,.txt"
              className="sr-only"
              onChange={(event) => {
                const file = event.target.files?.[0];
                if (!file) return;
                void file.text().then((text) => {
                  const key = `added:${added.length}`;
                  const extension = file.name.split('.').pop() ?? null;
                  setAdded([
                    ...added,
                    {
                      key,
                      label: file.name,
                      language: null,
                      cues: parseSubtitles(text, extension),
                    },
                  ]);
                  setSubtitleKey(key);
                  setLastSubtitle(key);
                  engineRef.current?.setTextTrack(-1);
                });
              }}
            />
          </label>
          <FindSubtitles
            videoKey={id}
            onPick={(label, language, cues) => {
              const key = `added:${added.length}`;
              setAdded([...added, { key, label, language, cues }]);
              setSubtitleKey(key);
              setLastSubtitle(key);
              engineRef.current?.setTextTrack(-1);
            }}
          />
          <AiSubtitles
            videoKey={id}
            url={request.url ?? ''}
            format={request.format ?? null}
            positionMs={timeMs}
            onStarted={() => selectSubtitle('ai')}
            onClose={onClose}
          />
          <PanelHeading>{t('videoPlayer.delay')}</PanelHeading>
          <div className="flex items-center gap-2 px-3 py-1">
            <button
              type="button"
              onClick={() => changeDelay(delayMs - 100)}
              className="rounded-lg bg-white/10 px-3 py-1.5 text-sm hover:bg-white/20"
            >
              −0.1 s
            </button>
            <DelayField delayMs={delayMs} onChange={changeDelay} />
            <button
              type="button"
              onClick={() => changeDelay(delayMs + 100)}
              className="rounded-lg bg-white/10 px-3 py-1.5 text-sm hover:bg-white/20"
            >
              +0.1 s
            </button>
            <button
              type="button"
              disabled={delayMs === 0}
              onClick={() => changeDelay(0)}
              className="rounded-lg px-2 py-1.5 text-sm hover:bg-white/10 disabled:opacity-40"
            >
              {t('videoPlayer.reset')}
            </button>
          </div>
          {subtitleKey && (
            <>
              <PanelHeading>{t('cueEdit.title')}</PanelHeading>
              {subtitleKey.startsWith('embedded:') ? (
                <p className="px-3 py-1 text-xs text-white/60">{t('cueEdit.embedded')}</p>
              ) : (
                <CueEditor
                  original={originalCues}
                  edits={trackEdits}
                  timeMs={timeMs}
                  delayMs={delayMs}
                  onDelay={changeDelay}
                  onEdit={changeCue}
                />
              )}
            </>
          )}
        </PlayerPanel>
      )}
      {panel === 'audio' && (
        <PlayerPanel title={t('videoPlayer.audio')} onClose={() => setPanel(null)}>
          {audioTracks.map((track) => (
            <PanelOption
              key={track.id}
              label={track.label}
              hint={track.language}
              selected={engine?.audioTrack() === track.id}
              onSelect={() => {
                engineRef.current?.setAudioTrack(track.id);
                remember({ audio: { language: track.language, label: track.label } });
                setTracksVersion((version) => version + 1);
              }}
            />
          ))}
        </PlayerPanel>
      )}
      {panel === 'quality' && (
        <PlayerPanel title={t('desktopPlayer.quality')} onClose={() => setPanel(null)}>
          <PanelOption
            label={t('desktopPlayer.auto')}
            selected={engine?.quality() === -1}
            onSelect={() => {
              engineRef.current?.setQuality(-1);
              setTracksVersion((version) => version + 1);
            }}
          />
          {[...qualities].reverse().map((quality) => (
            <PanelOption
              key={quality.id}
              label={quality.label}
              selected={engine?.quality() === quality.id}
              onSelect={() => {
                engineRef.current?.setQuality(quality.id);
                setTracksVersion((version) => version + 1);
              }}
            />
          ))}
        </PlayerPanel>
      )}
      {panel === 'aspect' && (
        <PlayerPanel title={t('videoPlayer.aspect')} onClose={() => setPanel(null)}>
          {ASPECTS.map((option) => (
            <PanelOption
              key={option}
              label={t(ASPECT_LABEL[option])}
              selected={option === aspect}
              onSelect={() => changeAspect(option)}
            />
          ))}
        </PlayerPanel>
      )}
    </div>
  );
};

const SKIP_LABEL = {
  opening: 'videoPlayer.skipOpening',
  ending: 'videoPlayer.skipEnding',
  recap: 'videoPlayer.skipRecap',
  other: 'videoPlayer.skipOpening',
} as const;

/** What a stream's failure means for the viewer (as `describeProblem` says on Android). */
const PROBLEM_TEXT: Record<StreamProblem, string> = {
  unsupported: 'desktopPlayer.cantPlayHere',
  network: 'videoPlayer.problemNetwork',
  refused: 'videoPlayer.problemRefused',
  gone: 'videoPlayer.problemGone',
  server: 'videoPlayer.problemServer',
  other: 'videoPlayer.problemOther',
};

/** An ending that reaches this close to the end is the credits: the next-episode prompt shows. */
const CREDITS_SLACK_MS = 5_000;

const ASPECT_LABEL = {
  fit: 'videoPlayer.aspectFit',
  crop: 'videoPlayer.aspectCrop',
  stretch: 'videoPlayer.aspectStretch',
} as const;

/** Where a chapter ends: its own end, the next chapter's start, or the video's end. */
const chapterEnd = (chapters: VideoChapter[], chapter: VideoChapter, durationMs: number) =>
  chapter.endMs ?? chapters.find((other) => other.startMs > chapter.startMs)?.startMs ?? durationMs;

/** The opening, ending or recap playing at `timeMs`, if any. */
const skippableAt = (chapters: VideoChapter[], timeMs: number, durationMs: number) =>
  chapters.find(
    (chapter) =>
      chapter.kind !== 'other' &&
      chapter.startMs <= timeMs &&
      timeMs < chapterEnd(chapters, chapter, durationMs) - 500,
  ) ?? null;

type TopBarProps = {
  onClose: () => void;
  setPanel: (panel: PanelKind | null) => void;
  positionMs: number;
  onElsewhere?: () => void;
};

const TopBar: FC<TopBarProps> = ({ onClose, setPanel, onElsewhere }) => {
  const { t } = useTranslation();
  const session = useVideoPlayerStore((state) => state.session);
  const title = useVideoPlayerStore((state) => state.title);
  const subtitle = useVideoPlayerStore((state) => state.subtitle);
  return (
    <div className="absolute inset-x-0 top-0 flex items-center gap-2 bg-gradient-to-b from-black/80 to-transparent px-3 pt-3 pb-10">
      <IconButton label={t('desktopPlayer.close')} onClick={onClose}>
        <ArrowLeft aria-hidden size={22} />
      </IconButton>
      <div className="flex min-w-0 flex-1 flex-col">
        <h1 className="truncate text-base font-semibold">{title}</h1>
        {subtitle && <p className="truncate text-sm text-white/70">{subtitle}</p>}
      </div>
      {session && (
        <>
          <IconButton label={t('videoPlayer.episodes')} onClick={() => setPanel('episodes')}>
            <ListVideo aria-hidden size={20} />
          </IconButton>
          <IconButton label={t('videoPlayer.sources')} onClick={() => setPanel('sources')}>
            <Server aria-hidden size={20} />
          </IconButton>
        </>
      )}
      {onElsewhere && (
        <IconButton label={t('desktopPlayer.playElsewhere')} onClick={onElsewhere}>
          <ExternalLink aria-hidden size={20} />
        </IconButton>
      )}
      <IconButton label={t('desktopPlayer.keys.title')} onClick={() => setPanel('keys')}>
        <Keyboard aria-hidden size={20} />
      </IconButton>
    </div>
  );
};

/** The installed players to hand the video to (M46.33); the last one picked is ticked. */
const PlayersPanel: FC<{ onPick: (player: string) => void; onClose: () => void }> = ({
  onPick,
  onClose,
}) => {
  const { t } = useTranslation();
  const players = useQuery({
    queryKey: ['externalPlayers'],
    queryFn: (): Promise<ExternalPlayer[]> => commands.videoExternalPlayers(),
  });
  const last = readPlayer();
  return (
    <PlayerPanel title={t('desktopPlayer.players.title')} onClose={onClose} at="top">
      {players.data?.length === 0 && (
        <p className="px-3 py-2 text-sm text-white/70">{t('desktopPlayer.players.none')}</p>
      )}
      {players.data?.map((player) => (
        <PanelOption
          key={player.id}
          label={player.name}
          selected={player.id === last}
          onSelect={() => onPick(player.id)}
        />
      ))}
    </PlayerPanel>
  );
};

const KEYS: [string, string][] = [
  ['Space · K', 'playPause'],
  ['← →', 'seek'],
  ['J · L', 'seek10'],
  ['↑ ↓', 'volume'],
  ['M', 'mute'],
  ['F', 'fullscreen'],
  ['C', 'subtitles'],
  ['[ ]', 'speed'],
  ['Shift+N · Shift+P', 'episodes'],
  [', .', 'frame'],
  ['S', 'skip'],
  ['0–9', 'jump'],
  ['?', 'keys'],
  ['Esc', 'close'],
];

/** The player's keyboard shortcuts (M46.33). */
const KeysPanel: FC<{ onClose: () => void }> = ({ onClose }) => {
  const { t } = useTranslation();
  return (
    <PlayerPanel title={t('desktopPlayer.keys.title')} onClose={onClose} at="top">
      <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-2 px-3 py-2 text-sm">
        {KEYS.map(([keys, action]) => (
          <div key={action} className="contents">
            <dt className="font-mono text-xs text-white/80">{keys}</dt>
            <dd className="text-white/70">{t(`desktopPlayer.keys.${action}`)}</dd>
          </div>
        ))}
      </dl>
    </PlayerPanel>
  );
};

/** The subtitle delay as a typed number of seconds (M46.33), −60 to 60. */
const DelayField: FC<{ delayMs: number; onChange: (ms: number) => void }> = ({
  delayMs,
  onChange,
}) => {
  const { t } = useTranslation();
  const [draft, setDraft] = useState<string | null>(null);
  const commit = () => {
    if (draft === null) return;
    const seconds = Number(draft.replace(',', '.'));
    if (Number.isFinite(seconds)) {
      onChange(Math.round(Math.max(-60, Math.min(60, seconds)) * 1000));
    }
    setDraft(null);
  };
  return (
    <label className="flex flex-1 items-center justify-center gap-1 text-sm tabular-nums">
      <input
        type="text"
        inputMode="decimal"
        aria-label={t('desktopPlayer.delaySeconds')}
        value={draft ?? (delayMs / 1000).toFixed(2)}
        onChange={(event) => setDraft(event.target.value)}
        onBlur={commit}
        onKeyDown={(event) => {
          if (event.key === 'Enter') {
            commit();
            (event.target as HTMLInputElement).blur();
          }
          event.stopPropagation();
        }}
        className="w-16 rounded-md bg-white/10 px-2 py-1 text-center outline-none focus:ring-1 focus:ring-white/40"
      />
      s
    </label>
  );
};

/**
 * Find subtitles (M46.33): every added addon's subtitles for this film or episode; with no addon
 * that has subtitles, the suggested one (OpenSubtitles) can be added in one tap.
 */
const FindSubtitles: FC<{
  videoKey: string;
  onPick: (label: string, language: string | null, cues: Cue[]) => void;
}> = ({ videoKey, onPick }) => {
  const { t } = useTranslation();
  const [found, setFound] = useState<FoundSubtitles | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const look = async () => {
    setBusy(true);
    setError(null);
    const result = await commands.videoFindSubtitles(videoKey);
    setBusy(false);
    if (result.status === 'ok') setFound(result.data);
    else setError(result.error);
  };
  const addSuggested = async (url: string) => {
    setBusy(true);
    const added = await commands.stremioAdd(url);
    if (added.status === 'error') {
      setBusy(false);
      setError(added.error);
      return;
    }
    await look();
  };
  const pick = async (track: VideoSubtitle) => {
    const text = await fetch(track.url)
      .then((response) => response.text())
      .catch(() => '');
    onPick(
      track.label ?? track.lang ?? t('desktopPlayer.find.track'),
      track.lang,
      parseSubtitles(text, track.format),
    );
  };
  return (
    <>
      <PanelHeading>{t('desktopPlayer.find.title')}</PanelHeading>
      {!found ? (
        <PanelOption
          label={busy ? t('desktopPlayer.find.looking') : t('desktopPlayer.find.look')}
          disabled={busy}
          onSelect={() => void look()}
        />
      ) : (
        <>
          {found.note && <p className="px-3 py-1 text-xs text-white/60">{found.note}</p>}
          {found.tracks.length === 0 && !found.note && (
            <p className="px-3 py-1 text-xs text-white/60">{t('desktopPlayer.find.none')}</p>
          )}
          {found.tracks.map((track, index) => (
            <PanelOption
              key={`${track.url}-${index}`}
              label={track.label ?? track.lang ?? t('desktopPlayer.find.track')}
              hint={track.lang}
              onSelect={() => void pick(track)}
            />
          ))}
          {found.suggestion && (
            <div className="flex flex-col gap-1 px-3 py-1.5 text-xs text-white/70">
              <p>{t('desktopPlayer.find.suggest', { name: found.suggestion.name })}</p>
              <button
                type="button"
                disabled={busy}
                onClick={() => void addSuggested(found.suggestion?.url ?? '')}
                className="bg-primary text-background self-start rounded-full px-3 py-1.5 font-semibold disabled:opacity-50"
              >
                {t('desktopPlayer.find.add', { name: found.suggestion.name })}
              </button>
            </div>
          )}
        </>
      )}
      {error && (
        <p role="alert" className="px-3 pb-1 text-xs text-white/70">
          {error}
        </p>
      )}
    </>
  );
};

type AiSubtitlesProps = {
  videoKey: string;
  url: string;
  format: string | null;
  positionMs: number;
  onStarted: () => void;
  onClose: () => void;
};

/**
 * AI subtitles while watching (M46.33): pick the speech model, then make subtitles from here on;
 * lines appear as each part is heard. Needs a downloaded model (or the online provider).
 */
const AiSubtitles: FC<AiSubtitlesProps> = ({
  videoKey,
  url,
  format,
  positionMs,
  onStarted,
  onClose,
}) => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const settings = useSettings();
  const update = useUpdateSetting();
  const models = useAiModels();
  const captions = useCaptions(videoKey);
  const stop = useStopCaptions();
  const [error, setError] = useState<string | null>(null);
  const prefs = settings.data?.transcription;
  const speech = (models.data ?? []).filter((model) => model.kind === 'speech' && model.installed);
  const running = captions.data?.running ?? false;
  const online = prefs?.engine === 'online' && Boolean(prefs.providerId);

  const start = async () => {
    setError(null);
    const result = await commands.captionsVideoTranscribe(
      videoKey,
      url,
      format,
      Math.round(positionMs),
    );
    if (result.status === 'error') setError(result.error);
    else onStarted();
  };
  // While it runs, it keeps up with where the viewer is.
  const latest = useRef(positionMs);
  useEffect(() => {
    latest.current = positionMs;
  });
  useEffect(() => {
    if (!running || !url) return;
    const timer = window.setInterval(() => {
      void commands.captionsVideoTranscribe(videoKey, url, format, Math.round(latest.current));
    }, 15_000);
    return () => window.clearInterval(timer);
  }, [running, videoKey, url, format]);

  const pick = (model: string) => {
    if (!prefs) return;
    update.mutate({
      key: 'transcription',
      value: { ...prefs, engine: 'local', localModel: model },
    });
  };

  return (
    <>
      <PanelHeading>{t('desktopPlayer.ai.title')}</PanelHeading>
      {speech.length === 0 && !online ? (
        <div className="flex flex-col gap-2 px-3 py-1 text-xs text-white/70">
          <p>{t('desktopPlayer.ai.needsModel')}</p>
          <button
            type="button"
            onClick={() => {
              onClose();
              void navigate({ to: '/settings/ai', hash: 'transcription' });
            }}
            className="text-primary self-start font-semibold"
          >
            {t('desktopPlayer.ai.setUp')}
          </button>
        </div>
      ) : (
        <>
          {speech.map((model) => (
            <PanelOption
              key={model.id}
              label={model.name}
              selected={prefs?.engine === 'local' && prefs.localModel === model.id}
              onSelect={() => pick(model.id)}
            />
          ))}
          {online && prefs && (
            <PanelOption
              label={t('desktopPlayer.ai.online')}
              hint={prefs.onlineModel}
              selected
              onSelect={() => undefined}
            />
          )}
          <div className="flex items-center gap-2 px-3 py-1.5">
            {running ? (
              <>
                <LoaderCircle aria-hidden size={14} className="animate-spin text-white/70" />
                <span className="flex-1 text-xs text-white/70">
                  {t('desktopPlayer.ai.listening', { n: captions.data?.cues.length ?? 0 })}
                </span>
                <button
                  type="button"
                  onClick={() => stop.mutate(videoKey)}
                  className="rounded-lg bg-white/10 px-3 py-1.5 text-xs hover:bg-white/20"
                >
                  {t('captions.stop')}
                </button>
              </>
            ) : (
              <button
                type="button"
                disabled={!url}
                onClick={() => void start()}
                className="bg-primary text-background flex items-center gap-2 rounded-full px-3 py-1.5 text-xs font-semibold disabled:opacity-50"
              >
                <Sparkles aria-hidden size={13} />
                {(captions.data?.cues.length ?? 0) > 0
                  ? t('desktopPlayer.ai.continue')
                  : t('desktopPlayer.ai.make')}
              </button>
            )}
          </div>
          {(error ?? captions.data?.error) && (
            <p role="alert" className="px-3 pb-1 text-xs text-white/70">
              {error ?? captions.data?.error}
            </p>
          )}
        </>
      )}
    </>
  );
};

type SessionPanelProps = { kind: 'episodes' | 'sources'; positionMs: number; onClose: () => void };

/** A session's episodes or sources; picking one asks Rust for it. */
const SessionPanel: FC<SessionPanelProps> = ({ kind, positionMs, onClose }) => {
  const { t } = useTranslation();
  const id = useVideoPlayerStore((state) => state.request?.id ?? '');
  const episodes = useVideoPlayerStore((state) => state.episodes);
  const sources = useVideoPlayerStore((state) => state.sources);
  const playingFrom = useVideoPlayerStore((state) => state.playingFrom);
  const autoplay = useVideoPlayerStore((state) => state.autoplay);
  const prefs = useVideoPlayerStore((state) => state.prefs);
  if (kind === 'episodes') {
    return (
      <PlayerPanel title={t('videoPlayer.episodes')} onClose={onClose} at="top">
        <PanelOption
          label={autoplay ? t('videoPlayer.autoplayOn') : t('videoPlayer.autoplayOff')}
          selected={autoplay}
          onSelect={() => {
            useVideoPlayerStore.setState({ autoplay: !autoplay });
            sendRequest(id, 'autoplay', positionMs, { on: !autoplay });
          }}
        />
        <PanelHeading>{t('videoPlayer.episodes')}</PanelHeading>
        {episodes.map((episode, index) => (
          <PanelOption
            key={`${index}-${episode.label}`}
            label={episode.label}
            hint={
              episode.downloaded
                ? t('videoPlayer.downloaded')
                : episode.watched
                  ? t('desktopPlayer.watched')
                  : null
            }
            selected={episode.current}
            onSelect={() => {
              if (!episode.current) sendRequest(id, 'episode', positionMs, { index });
            }}
          />
        ))}
      </PlayerPanel>
    );
  }
  const shown = sources
    .map((group, index) => ({ group, index }))
    .filter(
      ({ group }) =>
        (group.state !== 'failed' || prefs.showFailedSources) &&
        (group.state !== 'empty' || prefs.showEmptySources),
    );
  return (
    <PlayerPanel title={t('videoPlayer.sources')} onClose={onClose} at="top">
      {playingFrom && (
        <p className="text-muted px-4 py-2 text-sm">
          {t('videoPlayer.playingFrom', { n: playingFrom })}
        </p>
      )}
      {shown.map(({ group, index }) => (
        <div key={`${index}-${group.name}`} className="flex flex-col">
          <PanelHeading>
            {group.name}
            {group.state === 'resolving' && ` · ${t('videoPlayer.resolving')}`}
            {group.state === 'failed' && ` · ${t('videoPlayer.failed')}`}
            {group.state === 'empty' && ` · ${t('videoPlayer.empty')}`}
            {group.unreliable && ` · ${t('videoPlayer.unreliable')}`}
            {group.worked && !group.unreliable && ` · ${t('videoPlayer.workedLastTime')}`}
          </PanelHeading>
          {group.links.map((link, linkIndex) => (
            <PanelOption
              key={`${linkIndex}-${link.label}`}
              label={link.label}
              hint={
                link.failed
                  ? t('videoPlayer.failed')
                  : link.worked
                    ? t('videoPlayer.workedLastTime')
                    : null
              }
              selected={link.current}
              onSelect={() => {
                if (link.current) return;
                // Loads from where the video is; the ring names the source while it does.
                useVideoPlayerStore.setState({
                  pickedSource: true,
                  notice: null,
                  loadingText: t('videoPlayer.loadingSource', {
                    n: sourceName(group, linkIndex) ?? group.name,
                  }),
                });
                sendRequest(id, 'source', positionMs, { group: index, link: linkIndex });
                onClose();
              }}
            />
          ))}
        </div>
      ))}
    </PlayerPanel>
  );
};

type NoticeProps = { onSources: () => void; onRetry?: () => void; onElsewhere?: () => void };

const NoticeBox: FC<NoticeProps> = ({ onSources, onRetry, onElsewhere }) => {
  const { t } = useTranslation();
  const notice = useVideoPlayerStore((state) => state.notice);
  const session = useVideoPlayerStore((state) => state.session);
  const [details, setDetails] = useState(false);
  if (!notice) return null;
  return (
    <div
      role="alert"
      className="pointer-events-auto flex max-w-md flex-col items-center gap-3 rounded-2xl bg-neutral-900/90 px-6 py-5 text-center"
    >
      <p className="text-sm wrap-anywhere">{notice.text}</p>
      {notice.detail && (
        <>
          <button
            type="button"
            aria-expanded={details}
            onClick={() => setDetails(!details)}
            className="text-xs text-white/70 underline-offset-2 hover:underline"
          >
            {t('videoPlayer.moreInfo')}
          </button>
          {details && (
            <p className="selectable max-w-full font-mono text-xs wrap-anywhere text-white/60">
              {notice.detail}
            </p>
          )}
        </>
      )}
      <div className="flex flex-wrap justify-center gap-2">
        {onRetry && (
          <button
            type="button"
            onClick={onRetry}
            className="rounded-full bg-white/15 px-4 py-2 text-sm hover:bg-white/25"
          >
            {t('desktopPlayer.retry')}
          </button>
        )}
        {session && (
          <button
            type="button"
            onClick={onSources}
            className="rounded-full bg-white/15 px-4 py-2 text-sm hover:bg-white/25"
          >
            {t('videoPlayer.sources')}
          </button>
        )}
        {onElsewhere && (
          <button
            type="button"
            onClick={onElsewhere}
            className="bg-primary text-background rounded-full px-4 py-2 text-sm font-semibold"
          >
            {t('desktopPlayer.playElsewhere')}
          </button>
        )}
      </div>
    </div>
  );
};

/** A note under the loading ring, in words. */
const useNoteText = () => {
  const { t } = useTranslation();
  return (note: WaitingNote | null): string | null => {
    if (!note) return null;
    switch (note.kind) {
      case 'checked':
        return t('videoPlayer.sourcesChecked', { n: note.answered, of: note.total });
      case 'trying':
        return `${t('videoPlayer.tryingSource', { n: note.number, of: note.of })} · ${note.source}`;
      default:
        return note.source;
    }
  };
};

/**
 * The player's waiting ring (as the Android player draws it): an accent arc turning over a faint
 * track (only its rotation animates), with what's happening and a note under it.
 */
const LoadingRing: FC<{ text?: string | null; note?: WaitingNote | null }> = ({ text, note }) => {
  const { t } = useTranslation();
  const noteText = useNoteText()(note ?? null);
  return (
    <div role="status" className="flex flex-col items-center gap-3">
      <svg
        aria-hidden
        viewBox="0 0 48 48"
        className="text-primary size-14 animate-spin drop-shadow [animation-duration:1.1s]"
      >
        <circle cx="24" cy="24" r="20" fill="none" strokeWidth="4" className="stroke-white/20" />
        <circle
          cx="24"
          cy="24"
          r="20"
          fill="none"
          strokeWidth="4"
          stroke="currentColor"
          strokeLinecap="round"
          strokeDasharray="42 84"
        />
      </svg>
      {text ? (
        <p className="max-w-sm text-center text-sm whitespace-pre-line text-white/90">{text}</p>
      ) : (
        <span className="sr-only">{t('videoPlayer.loading')}</span>
      )}
      {noteText && (
        <p className="max-w-sm truncate text-center text-xs text-white/70">{noteText}</p>
      )}
    </div>
  );
};

const Middle: FC<{ children: ReactNode }> = ({ children }) => (
  <div className="pointer-events-none absolute inset-0 grid place-items-center">{children}</div>
);

type IconButtonProps = {
  label: string;
  onClick: () => void;
  disabled?: boolean;
  pressed?: boolean;
  children: ReactNode;
};

const IconButton: FC<IconButtonProps> = ({ label, onClick, disabled, pressed, children }) => (
  <button
    type="button"
    aria-label={label}
    title={label}
    aria-pressed={pressed}
    disabled={disabled}
    onClick={onClick}
    className={`grid size-10 shrink-0 place-items-center rounded-full hover:bg-white/10 disabled:opacity-35 ${pressed ? 'text-primary' : ''}`}
  >
    {children}
  </button>
);
