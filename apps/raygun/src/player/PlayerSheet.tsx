import type { PlaybackState, QueueItem, RepeatMode } from '@raygun/bindings';
import {
  ChevronDown,
  ImageIcon,
  ListMusic,
  ListPlus,
  Pause,
  Play,
  Repeat,
  Repeat1,
  Shuffle,
  SkipBack,
  SkipForward,
  SlidersHorizontal,
} from 'lucide-react';
import { useRouterState } from '@tanstack/react-router';
import { AnimatePresence, motion, useDragControls } from 'motion/react';
import type { PanInfo } from 'motion/react';
import { useEffect, useRef, useState } from 'react';
import type { CSSProperties, FC, ReactNode, SyntheticEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { useTrackActionsStore } from '../collections/trackActionsStore';
import { fromQueueItem } from '../collections/useCollections';
import { Artwork } from '../library/Artwork';
import { ArtistLinks } from '../music/ArtistLinks';
import { Marquee } from '../shell/Marquee';
import { ResizeHandle } from '../shell/ResizeHandle';
import { Sheet } from '../shell/Sheet';
import { usePanelSizes } from '../shell/usePanelSizes';
import { useSideRail, useWideScreen } from '../shell/useWideScreen';
import { FavoriteButton } from './FavoriteButton';
import { formatTime } from './formatTime';
import { CaptionsView } from './CaptionsView';
import { LyricsView } from './LyricsView';
import type { SheetView } from './playerUiStore';
import { usePlayerUiStore } from './playerUiStore';
import { usePlayerView } from './playerView';
import { useSpokenItem } from './spoken';
import { SpokenByline } from './SpokenByline';
import type { PlayerView } from './playerView';
import { SoundButton } from '../sound/SoundButton';
import { PlaybackExtras } from './PlaybackExtras';
import { PlayerCards } from './PlayerCards';
import { PlayingOn } from '../connect/PlayingOn';
import { IncognitoBadge } from '../privacy/IncognitoBadge';
import { PlayerViewSettings } from './PlayerViewSettings';
import { useCloseOnBack } from './useCloseOnBack';
import { onSeekKey } from './seekKeys';
import { useLivePosition } from './useLivePosition';
import { hasVideo } from './hasVideo';
import { MediaArt, VideoToggle } from './MediaArt';
import { isActive, sendControl, sourceNote, useNowPlaying } from './usePlayback';
import { SourceIssue } from '../shell/SourceIssue';
import { songCover } from '../library/coverTargets';

const DISMISS_DISTANCE = 120;
const DISMISS_VELOCITY = 500;

const NEXT_REPEAT: Record<RepeatMode, RepeatMode> = { off: 'all', all: 'one', one: 'off' };

/** Lists scroll by touch without dragging the sheet down. */
const keepDrag = (event: SyntheticEvent) => event.stopPropagation();

/** The player's column: the chosen share of the window, or a width that suits the screen. */
const columnWidth = (view: PlayerView, sideBySide: boolean) => ({
  style: view.width > 0 ? ({ maxWidth: `${view.width}%` } as CSSProperties) : undefined,
  className:
    view.width > 0 ? '' : sideBySide ? 'max-w-[90rem]' : 'max-w-md md:max-w-2xl lg:max-w-4xl',
});

/**
 * Full-screen player. Drag it down (or press Escape / the close button) to dismiss.
 *
 * Side by side (wide windows by default), the artwork and controls sit on the left and the lyrics
 * or queue fill the right (M46.7). Stacked (phones), it scrolls as Spotify's does (M46.14): the
 * cover with an "Up next" pill under it and the controls high on the screen, then cards for the
 * lyrics, the artist and the credits; the lyrics and the queue open full screen above the controls.
 * Stacked, it drags from its header, so the rest can scroll. Settings → Player arranges it.
 */
export const PlayerSheet: FC = () => {
  const { t } = useTranslation();
  const sheetOpen = usePlayerUiStore((state) => state.sheetOpen);
  const closeSheet = usePlayerUiStore((state) => state.closeSheet);
  const view = usePlayerUiStore((state) => state.sheetView);
  const setView = usePlayerUiStore((state) => state.setSheetView);
  const { state, item, upNext } = useNowPlaying();
  const spoken = useSpokenItem(item ?? undefined);
  const { view: layout } = usePlayerView();
  const { panels, save: savePanels } = usePanelSizes();
  const [split, setSplit] = useState<number | null>(null);
  const splitRow = useRef<HTMLDivElement>(null);
  const wide = useWideScreen();
  const large = useSideRail();
  const sideBySide =
    wide && (layout.layout === 'sideBySide' || (layout.layout === 'auto' && large));
  const videoShown = usePlayerUiStore((state) => state.videoShown);
  // A YouTube song shows its video only once asked for (M46.8).
  const isVideo = hasVideo(item) && videoShown;
  const [layoutAnchor, setLayoutAnchor] = useState<HTMLElement | null>(null);
  const [scrubbing, setScrubbing] = useState(false);
  const dragControls = useDragControls();
  const href = useRouterState({ select: (router) => router.location.href });
  const openedAt = useRef(href);

  useCloseOnBack(sheetOpen, closeSheet);

  useEffect(() => {
    // Following a link from the player (an artist, a credit) shows that page instead.
    if (sheetOpen && href !== openedAt.current) closeSheet();
    openedAt.current = href;
  }, [href, sheetOpen, closeSheet]);

  useEffect(() => {
    if (!sheetOpen) {
      return;
    }
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        closeSheet();
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [sheetOpen, closeSheet]);

  const handleDragEnd = (_event: PointerEvent, info: PanInfo) => {
    if (info.offset.y > DISMISS_DISTANCE || info.velocity.y > DISMISS_VELOCITY) {
      closeSheet();
    }
  };

  /** A header button that shows `target` in the main area, or the artwork again. */
  const viewButton = (target: SheetView, label: string, icon: ReactNode) => {
    const shown = view === target;
    return (
      <button
        type="button"
        aria-label={shown ? t('player.showArtwork') : label}
        aria-pressed={shown}
        onClick={() => setView(shown ? 'artwork' : target)}
        className={`grid size-11 place-items-center rounded-full ${shown ? 'text-primary' : 'text-muted hover:text-foreground'}`}
      >
        {shown ? <ImageIcon aria-hidden size={22} /> : icon}
      </button>
    );
  };

  const width = columnWidth(layout, sideBySide);
  const media = item ? (
    <MediaArt
      item={item}
      active={sheetOpen}
      videoClassName={`mx-auto aspect-video w-full rounded-2xl bg-black ${
        // A short window keeps the video small enough for the controls, at 16:9.
        sideBySide
          ? 'max-w-[min(100%,calc((100dvh-22rem)*16/9))]'
          : 'max-w-[min(100%,calc((100dvh-28rem)*16/9))]'
      }`}
      artworkClassName={`mx-auto aspect-square w-full rounded-2xl shadow-xl ${
        sideBySide ? 'max-w-[min(100%,calc(100dvh-22rem))]' : 'max-w-[min(100%,calc(100dvh-28rem))]'
      }`}
      iconSize={96}
    />
  ) : (
    <Artwork url={null} className="aspect-square w-full max-w-xs rounded-2xl" iconSize={96} />
  );
  // Podcasts and audiobooks show captions here (M46.32).
  const lyrics = !item ? null : spoken ? (
    <CaptionsView
      spoken={spoken}
      state={state}
      size={layout.lyricsSize}
      align={layout.lyricsAlign}
    />
  ) : (
    <LyricsView item={item} state={state} size={layout.lyricsSize} align={layout.lyricsAlign} />
  );
  const textLabel = spoken ? t('captions.title') : t('player.lyrics');
  const queue = <UpNextList items={upNext} />;

  return (
    <AnimatePresence>
      {sheetOpen && (
        <motion.div
          key="player-sheet"
          role="dialog"
          aria-modal="true"
          aria-label={t('player.nowPlaying')}
          data-testid="player-sheet"
          data-layout={sideBySide ? 'sideBySide' : 'stacked'}
          initial={{ y: '100%' }}
          animate={{ y: 0 }}
          exit={{ y: '100%' }}
          transition={{ type: 'spring', damping: 32, stiffness: 320 }}
          drag="y"
          dragControls={dragControls}
          dragListener={sideBySide && !scrubbing}
          dragConstraints={{ top: 0, bottom: 0 }}
          dragElastic={{ top: 0, bottom: 0.7 }}
          onDragEnd={handleDragEnd}
          className="bg-background fixed inset-x-0 top-[var(--app-bar,0px)] bottom-0 isolate z-50 flex flex-col overflow-hidden px-6 pt-[max(0.75rem,env(safe-area-inset-top))] pb-[max(1.5rem,env(safe-area-inset-bottom))] lg:px-10"
        >
          {layout.background && item?.artworkUrl && !isVideo && (
            <div aria-hidden className="pointer-events-none absolute inset-0 -z-10">
              <img
                src={item.artworkUrl}
                alt=""
                className="size-full scale-125 object-cover opacity-45 blur-3xl"
              />
              <div className="from-background/30 via-background/60 to-background absolute inset-0 bg-gradient-to-b" />
            </div>
          )}
          <div
            data-testid="player-sheet-header"
            onPointerDown={sideBySide ? undefined : (event) => dragControls.start(event)}
            className={`flex items-center justify-between ${sideBySide ? '' : 'touch-none'}`}
          >
            <button
              type="button"
              aria-label={t('player.closePlayer')}
              onClick={closeSheet}
              className="text-muted hover:text-foreground grid size-11 place-items-center rounded-full"
            >
              <ChevronDown aria-hidden size={28} />
            </button>
            <span aria-hidden className="bg-muted/40 h-1.5 w-10 rounded-full" />
            <div className="flex items-center">
              <IncognitoBadge className="mr-1" />
              <PlaybackExtras state={state} className="h-11" />
              <SoundButton className="size-11" />
              <VideoToggle item={item} className="size-11" />
              {item &&
                !sideBySide &&
                view !== 'artwork' &&
                viewButton(view, '', <ImageIcon aria-hidden size={22} />)}
              <button
                type="button"
                aria-label={t('player.view.open')}
                aria-haspopup="dialog"
                aria-expanded={layoutAnchor !== null}
                onClick={(event) => setLayoutAnchor(layoutAnchor ? null : event.currentTarget)}
                className="text-muted hover:text-foreground grid size-11 place-items-center rounded-full"
              >
                <SlidersHorizontal aria-hidden size={20} />
              </button>
            </div>
          </div>

          <PlayingOn className="mx-auto mt-1 w-fit max-w-full rounded-full" />
          <div
            style={width.style}
            className={`mx-auto flex min-h-0 w-full flex-1 ${width.className}`}
          >
            {sideBySide ? (
              <div ref={splitRow} className="flex min-h-0 flex-1 gap-4 py-4 xl:gap-8">
                <div
                  style={
                    (split ?? panels.playerSplit)
                      ? { width: `${split ?? panels.playerSplit}%` }
                      : undefined
                  }
                  className={`flex min-h-0 shrink-0 flex-col justify-center gap-6 ${
                    (split ?? panels.playerSplit)
                      ? ''
                      : isVideo
                        ? 'w-[min(58%,60rem)]'
                        : 'w-[min(42%,34rem)]'
                  }`}
                >
                  <div className="flex justify-center">{media}</div>
                  <Controls
                    state={state}
                    item={item}
                    onScrub={setScrubbing}
                    note={sourceNote(state, item)}
                  />
                </div>
                <ResizeHandle
                  label={t('player.view.resizeSplit')}
                  value={split ?? (panels.playerSplit || (isVideo ? 58 : 42))}
                  min={30}
                  max={70}
                  direction={1}
                  scale={() => 100 / Math.max(1, splitRow.current?.clientWidth ?? 1000)}
                  onPreview={setSplit}
                  onCommit={(playerSplit) => {
                    setSplit(null);
                    savePanels({ playerSplit });
                  }}
                />
                <section
                  aria-label={view === 'queue' ? t('player.upNextView') : textLabel}
                  className="flex min-h-0 min-w-0 flex-1 flex-col"
                >
                  <div role="tablist" className="flex gap-1 pb-2">
                    {(['lyrics', 'queue'] as const).map((tab) => {
                      const current = tab === 'queue' ? view === 'queue' : view !== 'queue';
                      return (
                        <button
                          key={tab}
                          type="button"
                          role="tab"
                          aria-selected={current}
                          onClick={() => setView(tab)}
                          className={`rounded-full px-4 py-1.5 text-sm font-medium ${
                            current
                              ? 'bg-surface text-foreground'
                              : 'text-muted hover:text-foreground'
                          }`}
                        >
                          {tab === 'queue' ? t('player.upNextView') : textLabel}
                        </button>
                      );
                    })}
                  </div>
                  <div onPointerDownCapture={keepDrag} className="flex min-h-0 flex-1 flex-col">
                    {view === 'queue' ? queue : lyrics}
                  </div>
                </section>
              </div>
            ) : view === 'artwork' ? (
              <div
                data-testid="player-scroll"
                className="no-scrollbar -mx-6 flex min-h-0 flex-1 flex-col overflow-y-auto overscroll-contain px-6 lg:-mx-10 lg:px-10"
              >
                <div className="flex shrink-0 flex-col items-center gap-4 pt-4">
                  {media}
                  {item && <UpNextPill next={upNext[0]} onOpen={() => setView('queue')} />}
                </div>
                <div className="shrink-0 pt-5">
                  <Controls
                    state={state}
                    item={item}
                    onScrub={setScrubbing}
                    note={sourceNote(state, item)}
                  />
                </div>
                {item && (
                  <PlayerCards
                    item={item}
                    state={state}
                    spoken={spoken}
                    onLyrics={() => setView('lyrics')}
                  />
                )}
              </div>
            ) : (
              <div className="flex min-h-0 flex-1 flex-col gap-5 pt-4">
                <div className="flex min-h-0 flex-1 flex-col items-center justify-center">
                  {view === 'lyrics' ? lyrics : queue}
                </div>
                <Controls
                  state={state}
                  item={item}
                  onScrub={setScrubbing}
                  note={sourceNote(state, item)}
                />
              </div>
            )}
          </div>
          {layoutAnchor && (
            <Sheet
              title={t('player.view.title')}
              anchor={layoutAnchor}
              popoverWidth={340}
              onClose={() => setLayoutAnchor(null)}
            >
              <PlayerViewSettings title="" />
            </Sheet>
          )}
        </motion.div>
      )}
    </AnimatePresence>
  );
};

type ControlsProps = {
  state: PlaybackState | undefined;
  item: QueueItem | undefined;
  note: ReturnType<typeof sourceNote>;
  /** Whether the seek slider is being dragged (the sheet doesn't drag meanwhile). */
  onScrub: (scrubbing: boolean) => void;
};

/** Title, seek bar and buttons: the same in both layouts, and never moved by the views. */
const Controls: FC<ControlsProps> = ({ state, item, note, onScrub }) => {
  const { t } = useTranslation();
  const spoken = useSpokenItem(item ?? undefined);
  const openTrackActions = useTrackActionsStore((store) => store.open);
  const [scrubMs, setScrubMs] = useState<number | null>(null);
  const livePositionMs = useLivePosition(state);
  const durationMs = state?.durationMs ?? 0;
  const positionMs = scrubMs ?? livePositionMs;
  const repeat = state?.repeat ?? 'off';
  const scrub = (value: number | null) => {
    setScrubMs(value);
    onScrub(value !== null);
  };
  const commitScrub = () => {
    if (scrubMs !== null) {
      void sendControl({ type: 'seek', positionMs: scrubMs });
      scrub(null);
    }
  };

  return (
    <div className="mx-auto flex w-full max-w-xl shrink-0 flex-col gap-5">
      <div className="flex w-full items-center gap-1">
        <div className="flex min-w-0 flex-1 flex-col gap-1">
          <Marquee
            testId="sheet-title"
            text={item?.title ?? t('player.nothingPlaying')}
            className="w-full text-2xl font-bold"
          />
          {spoken ? (
            <SpokenByline spoken={spoken} className="text-muted block w-full truncate text-lg" />
          ) : (
            item?.artist && (
              <ArtistLinks
                credit={item.artist}
                className="text-muted block w-full truncate text-lg"
              />
            )
          )}
          {note && (
            <p data-testid="sheet-source-note" className="text-muted text-xs">
              {note.kind === 'finding'
                ? t('player.findingSource')
                : t('player.via', { source: note.source })}
            </p>
          )}
          {state?.status === 'error' && state.error && <SourceIssue>{state.error}</SourceIssue>}
        </div>
        {item && !spoken && <FavoriteButton item={item} download />}
        {item && !spoken ? (
          <button
            type="button"
            aria-label={t('player.addToPlaylist')}
            aria-haspopup="dialog"
            onClick={(event) =>
              openTrackActions(fromQueueItem(item), { anchor: event.currentTarget })
            }
            className="text-muted hover:text-foreground grid size-11 shrink-0 place-items-center rounded-full"
          >
            <ListPlus aria-hidden size={22} />
          </button>
        ) : (
          <span className="size-11 shrink-0" />
        )}
      </div>

      <div className="flex w-full flex-col gap-1">
        <input
          type="range"
          aria-label={t('player.seek')}
          min={0}
          max={Math.max(durationMs, 1)}
          step={250}
          value={Math.min(positionMs, Math.max(durationMs, 1))}
          disabled={!item || durationMs <= 0}
          onChange={(event) => scrub(Number(event.target.value))}
          onKeyDown={onSeekKey(positionMs, durationMs, scrub)}
          onPointerUp={commitScrub}
          onKeyUp={commitScrub}
          onPointerDownCapture={keepDrag}
          className="accent-primary w-full"
        />
        <div className="text-muted flex justify-between text-xs tabular-nums">
          <span data-testid="position">{formatTime(positionMs)}</span>
          <span data-testid="duration">{formatTime(durationMs)}</span>
        </div>
      </div>

      <div className="flex w-full items-center justify-between">
        <button
          type="button"
          aria-label={t('player.shuffle')}
          aria-pressed={state?.shuffle ?? false}
          onClick={() => void sendControl({ type: 'setShuffle', enabled: !state?.shuffle })}
          className={`grid size-11 place-items-center rounded-full ${state?.shuffle ? 'text-primary' : 'text-muted hover:text-foreground'}`}
        >
          <Shuffle aria-hidden size={22} />
        </button>
        <button
          type="button"
          disabled={!item}
          aria-label={t('player.previous')}
          onClick={() => void sendControl({ type: 'previous' })}
          className="grid size-12 place-items-center rounded-full disabled:opacity-40"
        >
          <SkipBack aria-hidden size={32} />
        </button>
        <button
          type="button"
          disabled={!item}
          aria-label={isActive(state) ? t('player.pause') : t('player.play')}
          onClick={() => void sendControl({ type: 'toggle' })}
          className="bg-primary text-background grid size-18 place-items-center rounded-3xl shadow-lg shadow-black/20 disabled:opacity-40"
        >
          {isActive(state) ? (
            <Pause aria-hidden size={32} fill="currentColor" />
          ) : (
            <Play aria-hidden size={32} fill="currentColor" />
          )}
        </button>
        <button
          type="button"
          disabled={!item}
          aria-label={t('player.next')}
          onClick={() => void sendControl({ type: 'next' })}
          className="grid size-12 place-items-center rounded-full disabled:opacity-40"
        >
          <SkipForward aria-hidden size={32} />
        </button>
        <button
          type="button"
          aria-label={t(`player.repeat.${repeat}`)}
          onClick={() => void sendControl({ type: 'setRepeat', mode: NEXT_REPEAT[repeat] })}
          className={`grid size-11 place-items-center rounded-full ${repeat === 'off' ? 'text-muted hover:text-foreground' : 'text-primary'}`}
        >
          {repeat === 'one' ? <Repeat1 aria-hidden size={22} /> : <Repeat aria-hidden size={22} />}
        </button>
      </div>
    </div>
  );
};

/** A pill under the cover (M46.14): "Up next" and the song after this one; opens the queue. */
const UpNextPill: FC<{ next: QueueItem | undefined; onOpen: () => void }> = ({ next, onOpen }) => {
  const { t } = useTranslation();
  return (
    <button
      type="button"
      aria-label={t('player.upNextView')}
      onClick={onOpen}
      className="bg-surface/80 hover:bg-surface flex max-w-full items-center gap-2 rounded-full px-4 py-1.5 text-sm shadow-sm"
    >
      <ListMusic aria-hidden size={16} className="shrink-0" />
      <span className="shrink-0 font-semibold">{t('player.upNextView')}</span>
      {next && <span className="text-muted truncate">{next.title}</span>}
    </button>
  );
};

/** What plays after the current item, filling the space it's given and scrolling in it. */
const UpNextList: FC<{ items: QueueItem[] }> = ({ items }) => {
  const { t } = useTranslation();
  return (
    <section
      aria-label={t('player.upNext')}
      className="flex min-h-0 w-full flex-1 flex-col gap-1 self-stretch overflow-y-auto pr-2"
    >
      <h2 className="text-muted text-xs font-semibold tracking-wide uppercase">
        {t('player.upNext')}
      </h2>
      {items.length === 0 ? (
        <p className="text-muted py-2 text-sm">{t('player.queueEmpty')}</p>
      ) : (
        <ul data-testid="up-next" className="flex flex-col">
          {items.map((next) => (
            <li key={next.id}>
              <button
                type="button"
                aria-label={next.title}
                onClick={() => void sendControl({ type: 'playItem', itemId: next.id })}
                className="hover:bg-surface/60 flex w-full items-center gap-3 rounded-lg p-1.5 text-left"
              >
                <Artwork
                  url={next.artworkUrl}
                  fallback={songCover(next)}
                  className="size-12 shrink-0 rounded-md"
                  iconSize={20}
                />
                <span className="flex min-w-0 flex-col">
                  <span className="truncate font-medium">{next.title}</span>
                  {next.artist && (
                    <span className="text-muted truncate text-sm">{next.artist}</span>
                  )}
                </span>
              </button>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
};
