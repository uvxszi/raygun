import { commands } from '@raygun/bindings';
import type { CastControl, CastDevice, CastKind, CastState, CastStatus } from '@raygun/bindings';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import {
  ArrowLeft,
  FastForward,
  Pause,
  Play,
  Rewind,
  Square,
  Tv,
  Volume2,
  VolumeX,
} from 'lucide-react';
import { useEffect, useEffectEvent, useRef, useState } from 'react';
import type { FC, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { useLocalNetworkGate } from '../network/useLocalNetworkGate';
import { formatTime } from '../player/formatTime';
import { PanelOption, PlayerPanel } from './PlayerPanel';
import { sendRequest } from './protocol';
import { TimeBar } from './TimeBar';
import { useVideoPlayerStore } from './videoPlayerStore';

const KIND_LABEL: Record<CastKind, string> = {
  googleCast: 'videoPlayer.castGoogleCast',
  dlna: 'videoPlayer.castDlna',
  airplay: 'videoPlayer.castAirplay',
};

const STATE_LABEL: Record<CastState, string> = {
  connecting: 'videoPlayer.castConnecting',
  loading: 'videoPlayer.castLoading',
  playing: 'videoPlayer.castPlaying',
  paused: 'videoPlayer.castPaused',
  buffering: 'videoPlayer.castBuffering',
  stopped: 'videoPlayer.castStopped',
  finished: 'videoPlayer.castFinished',
  failed: 'videoPlayer.castFailed',
};

type PanelProps = {
  /** Where the video is: the TV starts there. */
  positionMs: number;
  /** The subtitle file on, by its index in the video's subtitles. */
  subtitle: number | null;
  onClose: () => void;
};

/** Play on TV: the TVs and other devices found, and adding one by its address. */
export const CastPanel: FC<PanelProps> = ({ positionMs, subtitle, onClose }) => {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const devices = useQuery({
    queryKey: ['castDevices'],
    queryFn: () => commands.castDevices(),
    // The search goes on while the panel is open; devices show as they answer.
    refetchInterval: 2_000,
  });
  const [adding, setAdding] = useState(false);
  const [address, setAddress] = useState('');
  const [starting, setStarting] = useState<string | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  const { guard, sheet } = useLocalNetworkGate('cast');

  const start = (device: CastDevice) =>
    void guard(device.address, async () => {
      setStarting(device.id);
      setProblem(null);
      const result = await commands.castStart(device.id, positionMs, subtitle);
      setStarting(null);
      if (result.status === 'error') setProblem(result.error);
    });
  const add = () =>
    void guard(address.trim(), async () => {
      setProblem(null);
      setStarting('address');
      const result = await commands.castAddDevice(address.trim());
      setStarting(null);
      if (result.status === 'error') {
        setProblem(result.error);
        return;
      }
      setAdding(false);
      setAddress('');
      void queryClient.invalidateQueries({ queryKey: ['castDevices'] });
      start(result.data);
    });

  const list = devices.data?.devices ?? [];
  const searching = devices.data?.searching ?? devices.isPending;
  return (
    <PlayerPanel title={t('videoPlayer.playOnTv')} onClose={onClose}>
      {list.map((device) => (
        <PanelOption
          key={device.id}
          label={device.name}
          hint={[
            t(KIND_LABEL[device.kind]),
            device.model,
            device.added ? t('videoPlayer.castAdded') : null,
          ]
            .filter(Boolean)
            .join(' · ')}
          disabled={starting !== null}
          onSelect={() => start(device)}
        />
      ))}
      {searching ? (
        <p role="status" className="px-3 py-2 text-sm text-white/60">
          {t('videoPlayer.castSearching')}
        </p>
      ) : (
        list.length === 0 && (
          <p className="px-3 py-2 text-sm text-white/60">{t('videoPlayer.castNothingFound')}</p>
        )
      )}
      {adding ? (
        <form
          className="flex items-center gap-2 px-3 py-2"
          onSubmit={(event) => {
            event.preventDefault();
            if (address.trim()) add();
          }}
        >
          <input
            aria-label={t('videoPlayer.castAddress')}
            placeholder="192.168.1.20"
            value={address}
            autoFocus
            inputMode="url"
            onChange={(event) => setAddress(event.target.value)}
            className="min-w-0 flex-1 rounded-lg bg-white/10 px-3 py-1.5 text-sm outline-none focus:ring-2 focus:ring-white/40"
          />
          <button
            type="submit"
            disabled={!address.trim() || starting !== null}
            className="rounded-lg bg-white/15 px-3 py-1.5 text-sm font-semibold hover:bg-white/25 disabled:opacity-40"
          >
            {t('videoPlayer.castAdd')}
          </button>
        </form>
      ) : (
        <PanelOption label={t('videoPlayer.castAddByAddress')} onSelect={() => setAdding(true)} />
      )}
      {problem && (
        <p role="alert" className="px-3 py-2 text-sm text-white/80">
          {problem}
        </p>
      )}
      {sheet}
    </PlayerPanel>
  );
};

/** Where the TV is now: it moves on from its last report while playing. */
const useCastPosition = (cast: CastStatus) => {
  const playing = cast.state === 'playing';
  // The clock, read twice a second while the TV plays; until its first read nothing moves.
  const [now, setNow] = useState(0);
  useEffect(() => {
    if (!playing) return;
    const timer = window.setInterval(() => setNow(Date.now()), 500);
    return () => window.clearInterval(timer);
  }, [playing]);
  const reported = cast.positionMs ?? 0;
  const moved = playing && cast.atMs ? Math.max(0, now - cast.atMs) : 0;
  const position = reported + moved;
  return cast.durationMs ? Math.min(position, cast.durationMs) : position;
};

type StageProps = {
  cast: CastStatus;
  /** Closes the player; the TV plays on. */
  onClose: () => void;
};

/** While a TV plays: the player is its remote, with the TV's own volume. */
export const CastStage: FC<StageProps> = ({ cast, onClose }) => {
  const { t } = useTranslation();
  const seekStepMs = useVideoPlayerStore((state) => state.seekStepMs);
  const session = useVideoPlayerStore((state) => state.session);
  const hasNext = useVideoPlayerStore((state) => state.hasNext);
  const autoplay = useVideoPlayerStore((state) => state.autoplay);
  const loadingText = useVideoPlayerStore((state) => state.loadingText);
  // A session's word when no source played on the TV either.
  const notice = useVideoPlayerStore((state) => state.notice);
  const chapters = useVideoPlayerStore((state) => state.chapters);
  const positionMs = useCastPosition(cast);
  const [problem, setProblem] = useState<string | null>(null);
  const [volume, setVolume] = useState<number | null>(null);
  const volumeTimer = useRef<number | undefined>(undefined);
  const advanced = useRef<string | null>(null);

  const control = async (value: CastControl) => {
    const result = await commands.castControl(value);
    setProblem(result.status === 'error' ? result.error : null);
  };
  const disconnect = async () => {
    const result = await commands.castDisconnect(true);
    if (result.status === 'error') setProblem(result.error);
  };
  const playing = cast.state === 'playing' || cast.state === 'buffering';
  const toggle = () => void control(playing ? { type: 'pause' } : { type: 'play' });
  const durationMs = cast.durationMs ?? 0;
  const seekTo = (target: number) =>
    void control({
      type: 'seek',
      positionMs: Math.max(0, durationMs > 0 ? Math.min(target, durationMs) : target),
    });
  const level = volume ?? cast.volume ?? null;
  const changeVolume = (next: number) => {
    setVolume(next);
    window.clearTimeout(volumeTimer.current);
    volumeTimer.current = window.setTimeout(() => {
      void control({ type: 'volume', level: next });
      setVolume(null);
    }, 150);
  };
  useEffect(() => () => window.clearTimeout(volumeTimer.current), []);

  // Played to the end on the TV: the next episode follows there, as in the player.
  useEffect(() => {
    if (cast.state !== 'finished' || !session || !hasNext || !autoplay) return;
    if (advanced.current === cast.key) return;
    advanced.current = cast.key;
    sendRequest(cast.key, 'next', cast.positionMs ?? 0);
  }, [cast.state, cast.key, cast.positionMs, session, hasNext, autoplay]);

  const onKey = useEffectEvent((event: KeyboardEvent) => {
    if (event.target instanceof HTMLInputElement && event.target.type !== 'range') return;
    if (event.key === 'Escape') onClose();
    else if (event.key === ' ' || event.key === 'k') toggle();
    else if (event.key === 'ArrowLeft') seekTo(positionMs - seekStepMs);
    else if (event.key === 'ArrowRight') seekTo(positionMs + seekStepMs);
    else return;
    event.preventDefault();
  });
  useEffect(() => {
    const listener = (event: KeyboardEvent) => onKey(event);
    window.addEventListener('keydown', listener);
    return () => window.removeEventListener('keydown', listener);
  }, []);

  const note = loadingText || notice?.text || problem || cast.error;
  const title = cast.subtitle ? `${cast.title} · ${cast.subtitle}` : cast.title;
  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label={t('desktopPlayer.player')}
      className="fixed inset-0 z-[70] flex flex-col bg-black text-white select-none"
    >
      <div className="flex items-center gap-2 px-3 pt-3">
        <RoundButton label={t('desktopPlayer.close')} onClick={onClose}>
          <ArrowLeft aria-hidden size={22} />
        </RoundButton>
        <h1 className="min-w-0 flex-1 truncate text-base font-semibold">{title}</h1>
      </div>
      <div className="flex flex-1 flex-col items-center justify-center gap-2 px-6 text-center">
        <Tv aria-hidden size={56} className="text-white/70" />
        <p className="text-lg font-semibold">
          {t('videoPlayer.castPlayingOn', { n: cast.device.name })}
        </p>
        <p role="status" className="text-sm text-white/70">
          {t(STATE_LABEL[cast.state])}
        </p>
        {note && (
          <p role="alert" className="max-w-md text-sm text-white/80">
            {note}
          </p>
        )}
      </div>
      <div className="px-5 pb-4">
        <TimeBar
          timeMs={positionMs}
          durationMs={durationMs}
          bufferedMs={0}
          chapters={chapters}
          seekStepMs={seekStepMs}
          onSeek={seekTo}
        />
        <div className="mt-1 flex flex-wrap items-center gap-1">
          <RoundButton
            label={playing ? t('videoPlayer.pause') : t('videoPlayer.play')}
            onClick={toggle}
          >
            {playing ? (
              <Pause aria-hidden size={22} fill="currentColor" />
            ) : (
              <Play aria-hidden size={22} fill="currentColor" />
            )}
          </RoundButton>
          <RoundButton
            label={t('desktopPlayer.seekBack', { n: seekStepMs / 1000 })}
            onClick={() => seekTo(positionMs - seekStepMs)}
          >
            <Rewind aria-hidden size={20} />
          </RoundButton>
          <RoundButton
            label={t('desktopPlayer.seekForward', { n: seekStepMs / 1000 })}
            onClick={() => seekTo(positionMs + seekStepMs)}
          >
            <FastForward aria-hidden size={20} />
          </RoundButton>
          <RoundButton
            label={t('videoPlayer.castStop')}
            onClick={() => void control({ type: 'stop' })}
          >
            <Square aria-hidden size={18} fill="currentColor" />
          </RoundButton>
          <span className="ml-2 text-sm text-white/90 tabular-nums">
            {formatTime(positionMs)} / {formatTime(durationMs)}
          </span>
          <span className="flex-1" />
          {level !== null && (
            <div className="flex items-center">
              <RoundButton
                label={cast.muted ? t('videoPlayer.castUnmute') : t('videoPlayer.castMute')}
                onClick={() => void control({ type: 'mute', muted: !cast.muted })}
              >
                {cast.muted ? <VolumeX aria-hidden size={20} /> : <Volume2 aria-hidden size={20} />}
              </RoundButton>
              <input
                type="range"
                aria-label={t('videoPlayer.castTvVolume')}
                min={0}
                max={1}
                step={0.05}
                value={level}
                onChange={(event) => changeVolume(Number(event.target.value))}
                className="accent-primary w-28"
              />
            </div>
          )}
          <button
            type="button"
            onClick={() => void disconnect()}
            className="ml-2 rounded-full bg-white/15 px-4 py-2 text-sm font-semibold hover:bg-white/25"
          >
            {t('videoPlayer.castDisconnect')}
          </button>
        </div>
      </div>
    </div>
  );
};

const RoundButton: FC<{ label: string; onClick: () => void; children: ReactNode }> = ({
  label,
  onClick,
  children,
}) => (
  <button
    type="button"
    aria-label={label}
    title={label}
    onClick={onClick}
    className="grid size-10 shrink-0 place-items-center rounded-full hover:bg-white/10"
  >
    {children}
  </button>
);
