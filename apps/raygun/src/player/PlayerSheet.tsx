import type { QueueItem, SpokenItem } from '@raygun/bindings';
import {
  BookOpen,
  EllipsisVertical,
  ImageIcon,
  ListMusic,
  MicVocal,
  Podcast,
  SlidersHorizontal,
  Subtitles,
} from 'lucide-react';
import { useNavigate, useRouterState } from '@tanstack/react-router';
import { AnimatePresence, motion, useDragControls } from 'motion/react';
import type { DragControls, PanInfo } from 'motion/react';
import { useEffect, useRef, useState } from 'react';
import type {
  CSSProperties,
  FC,
  MouseEvent,
  PointerEvent as ReactPointerEvent,
  ReactNode,
} from 'react';
import { useTranslation } from 'react-i18next';

import { useTrackActionsStore } from '../collections/trackActionsStore';
import { fromQueueItem } from '../collections/useCollections';
import { PlayingOn } from '../connect/PlayingOn';
import { ArtistLinks } from '../music/ArtistLinks';
import { useTrackMenuItems } from '../music/useTrackMenu';
import { useContextMenuStore } from '../shell/contextMenuStore';
import { Marquee } from '../shell/Marquee';
import { ResizeHandle } from '../shell/ResizeHandle';
import { Sheet } from '../shell/Sheet';
import { SourceIssue } from '../shell/SourceIssue';
import { usePanelSizes } from '../shell/usePanelSizes';
import { SoundButton } from '../sound/SoundButton';
import { CaptionsView } from './CaptionsView';
import { FavoriteButton } from './FavoriteButton';
import { hasVideo } from './hasVideo';
import { LyricsView } from './LyricsView';
import { MediaArt, VideoToggle } from './MediaArt';
import { PlaybackExtras } from './PlaybackExtras';
import { PlayerCards } from './PlayerCards';
import type { SheetLayout } from './playerLayout';
import { usePlayerWide, useSheetLayout } from './playerLayout';
import type { SheetView } from './playerUiStore';
import { usePlayerUiStore } from './playerUiStore';
import { usePlayerView } from './playerView';
import { IncognitoBadge } from '../privacy/IncognitoBadge';
import { PlayerViewSettings } from './PlayerViewSettings';
import { QueueActions, QueueList } from './QueueList';
import { SeekBar } from './SeekBar';
import { spokenName, useSpokenItem } from './spoken';
import { SpokenByline } from './SpokenByline';
import { isHandleTap, sheetDismissed } from './swipe';
import {
  NextButton,
  PlayPauseButton,
  PreviousButton,
  RepeatButton,
  ShuffleButton,
} from './transport';
import { useCloseOnBack } from './useCloseOnBack';
import { useCoverFit } from './useCoverFit';
import { useCurrentItem, usePlayerSelect, useSourceNote, useUpNext } from './usePlayback';

/** The sheet's sides keep clear of a phone's cutout and its navigation bar when on its side. */
const GUTTER =
  'pl-[max(1.5rem,env(safe-area-inset-left))] pr-[max(1.5rem,env(safe-area-inset-right))] lg:pl-[max(2.5rem,env(safe-area-inset-left))] lg:pr-[max(2.5rem,env(safe-area-inset-right))]';

/**
 * Full-screen player. Its handle closes it on a tap, or as it's dragged down; so do Back and
 * Escape.
 *
 * Every size follows the room it has, at any window size, zoom, text size and orientation; the
 * player's settings move them from there (`--cover-scale`, `--player-width-scale`, lyrics'
 * `--lyrics-offset`). Three arrangements (`useSheetLayout`):
 * - side by side (wide windows): the cover and controls on the left, the lyrics or queue on the
 *   right (M46.7);
 * - stacked (phones): one column that scrolls as Spotify's does (M46.14), the cover as large as
 *   the controls under it leave room for, an "Up next" pill, then cards for the lyrics, the artist
 *   and the credits; the lyrics and the queue open full screen above smaller controls;
 * - on its side (a phone in landscape, a short window): the cover, lyrics or queue beside the
 *   controls.
 *
 * It follows the song and the view, not the playhead: only the seek bar and the lyric lines move
 * as a song plays.
 */
export const PlayerSheet: FC = () => {
  const { t } = useTranslation();
  const sheetOpen = usePlayerUiStore((state) => state.sheetOpen);
  const closeSheet = usePlayerUiStore((state) => state.closeSheet);
  const view = usePlayerUiStore((state) => state.sheetView);
  const setView = usePlayerUiStore((state) => state.setSheetView);
  const item = useCurrentItem();
  const spoken = useSpokenItem(item);
  const { view: prefs } = usePlayerView();
  const layout = useSheetLayout(prefs.layout);
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
    if (sheetDismissed(info.offset, info.velocity)) {
      closeSheet();
    }
  };

  const sizes = {
    '--cover-scale': 1 + prefs.coverOffset / 100,
    '--player-width-scale': 1 + prefs.widthOffset / 100,
  } as CSSProperties;
  const onScrub = setScrubbing;
  const parts: LayoutParts = { item, spoken, view, setView, onScrub, isVideo };

  return (
    <AnimatePresence>
      {sheetOpen && (
        <motion.div
          key="player-sheet"
          role="dialog"
          aria-modal="true"
          aria-label={t('player.nowPlaying')}
          data-testid="player-sheet"
          data-layout={layout}
          initial={{ y: '100%' }}
          animate={{ y: 0 }}
          exit={{ y: '100%' }}
          transition={{ type: 'spring', damping: 32, stiffness: 320 }}
          drag="y"
          dragControls={dragControls}
          dragListener={layout === 'sideBySide' && !scrubbing}
          dragConstraints={{ top: 0, bottom: 0 }}
          dragElastic={{ top: 0, bottom: 0.7 }}
          onDragEnd={handleDragEnd}
          style={sizes}
          className="bg-background fixed inset-x-0 top-[var(--app-bar,0px)] bottom-0 isolate z-50 flex flex-col overflow-hidden pt-[max(0.75rem,env(safe-area-inset-top))] pb-[max(1rem,env(safe-area-inset-bottom))]"
        >
          {prefs.background && item?.artworkUrl && !isVideo && (
            // Its own layer, painted once: the controls changing over it don't blur it again.
            <div
              aria-hidden
              className="pointer-events-none absolute inset-0 -z-10 transform-gpu overflow-hidden [contain:strict]"
            >
              <img
                src={item.artworkUrl}
                alt=""
                className="size-full scale-125 object-cover opacity-45 blur-3xl"
              />
              <div className="from-background/30 via-background/60 to-background absolute inset-0 bg-gradient-to-b" />
            </div>
          )}
          <SheetHeader
            layout={layout}
            dragControls={dragControls}
            onClose={closeSheet}
            actions={
              <>
                <IncognitoBadge className="mr-1" />
                {item && layout === 'landscape' && (
                  <>
                    <ViewToggle
                      target="lyrics"
                      view={view}
                      onView={setView}
                      label={spoken ? t('captions.title') : t('player.lyrics')}
                      icon={
                        spoken ? (
                          <Subtitles aria-hidden size={20} />
                        ) : (
                          <MicVocal aria-hidden size={20} />
                        )
                      }
                    />
                    <ViewToggle
                      target="queue"
                      view={view}
                      onView={setView}
                      label={t('player.upNextView')}
                      icon={<ListMusic aria-hidden size={20} />}
                    />
                  </>
                )}
                {item && layout === 'stacked' && view !== 'artwork' && (
                  <button
                    type="button"
                    aria-label={t('player.showArtwork')}
                    aria-pressed
                    onClick={() => setView('artwork')}
                    className="text-primary grid size-[min(2.75rem,11cqw)] place-items-center rounded-full"
                  >
                    <ImageIcon aria-hidden size={22} />
                  </button>
                )}
                <PlaybackExtras className="h-[min(2.75rem,11cqw)]" />
                <SoundButton className="size-[min(2.75rem,11cqw)]" />
                <VideoToggle item={item} className="size-[min(2.75rem,11cqw)]" />
                <button
                  type="button"
                  aria-label={t('player.view.open')}
                  aria-haspopup="dialog"
                  aria-expanded={layoutAnchor !== null}
                  onClick={(event) => setLayoutAnchor(layoutAnchor ? null : event.currentTarget)}
                  className="text-muted hover:text-foreground grid size-[min(2.75rem,11cqw)] place-items-center rounded-full"
                >
                  <SlidersHorizontal aria-hidden size={20} />
                </button>
              </>
            }
          />
          <PlayingOn className="mx-auto mt-1 w-fit max-w-full rounded-full" />
          {layout === 'sideBySide' ? (
            <SideBySide {...parts} />
          ) : layout === 'landscape' ? (
            <Landscape {...parts} />
          ) : (
            <Stacked {...parts} />
          )}
          {layoutAnchor && (
            <Sheet
              title={t('player.view.title')}
              anchor={layoutAnchor}
              popoverWidth={360}
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

type LayoutParts = {
  item: QueueItem | undefined;
  spoken: SpokenItem | null;
  view: SheetView;
  setView: (view: SheetView) => void;
  onScrub: (scrubbing: boolean) => void;
  isVideo: boolean;
};

/**
 * The top of the sheet. Its handle spans it and is at least 44 px tall: a tap closes the player
 * (as do Enter and Space), a drag down closes it too, and a drag that snaps back changes nothing.
 * The buttons sit on top of it at the right.
 */
const SheetHeader: FC<{
  layout: SheetLayout;
  dragControls: DragControls;
  onClose: () => void;
  actions: ReactNode;
}> = ({ layout, dragControls, onClose, actions }) => {
  const { t } = useTranslation();
  const pressed = useRef<{ x: number; y: number; at: number } | null>(null);
  const onPointerDown = (event: ReactPointerEvent) => {
    pressed.current = { x: event.clientX, y: event.clientY, at: event.timeStamp };
    // Side by side the whole sheet drags; otherwise the handle drags it.
    if (layout !== 'sideBySide') dragControls.start(event);
  };
  const onPointerUp = (event: ReactPointerEvent) => {
    const from = pressed.current;
    pressed.current = null;
    if (from && isHandleTap(from, { x: event.clientX, y: event.clientY, at: event.timeStamp })) {
      onClose();
    }
  };
  return (
    <div
      data-testid="player-sheet-header"
      className={`@container relative flex min-h-11 shrink-0 items-center justify-end ${GUTTER}`}
    >
      <button
        type="button"
        aria-label={t('player.closePlayer')}
        title={t('player.closePlayer')}
        onPointerDown={onPointerDown}
        onPointerUp={onPointerUp}
        onPointerCancel={() => {
          pressed.current = null;
        }}
        onClick={(event) => {
          // Keys (Enter, Space) click with no pointer; pointer taps close on release above.
          if (event.detail === 0) onClose();
        }}
        className="group absolute inset-x-0 -top-3 bottom-0 cursor-pointer touch-none focus-visible:outline-none"
      >
        <span
          aria-hidden
          className="bg-muted/50 group-hover:bg-muted group-focus-visible:bg-primary absolute top-1.5 left-1/2 h-1.5 w-12 -translate-x-1/2 rounded-full transition-colors"
        />
      </button>
      <div className="relative flex min-w-0 items-center">{actions}</div>
    </div>
  );
};

/** A header button showing `target` instead of the cover, or the cover again. */
const ViewToggle: FC<{
  target: SheetView;
  view: SheetView;
  onView: (view: SheetView) => void;
  label: string;
  icon: ReactNode;
}> = ({ target, view, onView, label, icon }) => {
  const shown = view === target;
  return (
    <button
      type="button"
      aria-label={label}
      aria-pressed={shown}
      onClick={() => onView(shown ? 'artwork' : target)}
      className={`grid size-[min(2.75rem,11cqw)] place-items-center rounded-full ${
        shown ? 'text-primary' : 'text-muted hover:text-foreground'
      }`}
    >
      {icon}
    </button>
  );
};

/** The cover or the video, sized by `fill` (its own column) or by the room the controls leave. */
const Media: FC<{ item: QueueItem | undefined; fill?: boolean }> = ({ item, fill = false }) => {
  const sheetOpen = usePlayerUiStore((state) => state.sheetOpen);
  if (!item) return null;
  return (
    <MediaArt
      item={item}
      active={sheetOpen}
      videoClassName={`mx-auto rounded-2xl bg-black ${fill ? 'player-video-fill' : 'player-video'}`}
      artworkClassName={`mx-auto rounded-2xl shadow-xl ${fill ? 'player-cover-fill' : 'player-cover'}`}
      iconSize={96}
    />
  );
};

/** Lyrics, or captions for podcasts and audiobooks (M46.32). */
const TextView: FC<{ item: QueueItem; spoken: SpokenItem | null }> = ({ item, spoken }) =>
  spoken ? <CaptionsView spoken={spoken} /> : <LyricsView item={item} />;

const Stacked: FC<LayoutParts> = ({ item, spoken, view, setView, onScrub }) => {
  const upNext = useUpNext();
  const { containerRef, contentRef, coverRef } = useCoverFit();
  if (view !== 'artwork' && item) {
    return (
      <div
        className={`player-column flex min-h-0 w-full flex-1 flex-col gap-4 pt-3 [--player-column-base:44rem] ${GUTTER}`}
      >
        <div className="flex min-h-0 flex-1 flex-col">
          {view === 'lyrics' ? (
            <TextView item={item} spoken={spoken} />
          ) : (
            <QueuePane item={item} items={upNext} />
          )}
        </div>
        <Controls item={item} spoken={spoken} onScrub={onScrub} compact />
      </div>
    );
  }
  return (
    <div
      ref={containerRef}
      data-testid="player-scroll"
      className={`no-scrollbar flex min-h-0 flex-1 flex-col overflow-y-auto overscroll-contain ${GUTTER}`}
    >
      <div className="player-column flex flex-col [--player-column-base:44rem]">
        <div ref={contentRef} className="flex shrink-0 flex-col items-center gap-4 pt-4">
          <div ref={coverRef} className="flex w-full justify-center">
            <Media item={item} />
          </div>
          {item && <UpNextPill next={upNext[0]} onOpen={() => setView('queue')} />}
          <Controls item={item} spoken={spoken} onScrub={onScrub} />
        </div>
        {item && <PlayerCards item={item} spoken={spoken} onLyrics={() => setView('lyrics')} />}
      </div>
    </div>
  );
};

const Landscape: FC<LayoutParts> = ({ item, spoken, view, onScrub }) => {
  const upNext = useUpNext();
  return (
    <div className={`flex min-h-0 flex-1 gap-[min(2rem,4vw)] pt-2 ${GUTTER}`}>
      <div className="flex min-h-0 min-w-0 flex-1 flex-col items-center justify-center [container-type:size]">
        {view === 'artwork' || !item ? (
          <Media item={item} fill />
        ) : view === 'lyrics' ? (
          <TextView item={item} spoken={spoken} />
        ) : (
          <QueuePane item={item} items={upNext} />
        )}
      </div>
      <div className="no-scrollbar flex min-h-0 min-w-0 flex-1 flex-col overflow-y-auto">
        <div className="my-auto py-2">
          <Controls item={item} spoken={spoken} onScrub={onScrub} compact={view !== 'artwork'} />
        </div>
      </div>
    </div>
  );
};

const SideBySide: FC<LayoutParts> = ({ item, spoken, view, setView, onScrub, isVideo }) => {
  const { t } = useTranslation();
  const upNext = useUpNext();
  const { panels, save: savePanels } = usePanelSizes();
  const [split, setSplit] = useState<number | null>(null);
  const splitRow = useRef<HTMLDivElement>(null);
  const { containerRef, contentRef, coverRef } = useCoverFit();
  const share = split ?? panels.playerSplit;
  const textLabel = spoken ? t('captions.title') : t('player.lyrics');
  return (
    <div
      className={`player-column flex min-h-0 w-full flex-1 [--player-column-base:96rem] ${GUTTER}`}
    >
      <div ref={splitRow} className="flex min-h-0 flex-1 gap-4 py-4 xl:gap-8">
        <div
          ref={containerRef}
          style={share ? { width: `${share}%` } : undefined}
          className={`no-scrollbar flex min-h-0 shrink-0 flex-col overflow-y-auto ${
            share ? '' : isVideo ? 'w-[min(58%,60rem)]' : 'w-[min(42%,34rem)]'
          }`}
        >
          <div ref={contentRef} className="my-auto flex flex-col gap-6">
            <div ref={coverRef} className="flex justify-center">
              <Media item={item} />
            </div>
            <Controls item={item} spoken={spoken} onScrub={onScrub} />
          </div>
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
                    current ? 'bg-surface text-foreground' : 'text-muted hover:text-foreground'
                  }`}
                >
                  {tab === 'queue' ? t('player.upNextView') : textLabel}
                </button>
              );
            })}
          </div>
          {/* Lists scroll by touch without dragging the sheet down. */}
          <div
            onPointerDownCapture={(event) => event.stopPropagation()}
            className="flex min-h-0 flex-1 flex-col"
          >
            {item &&
              (view === 'queue' ? (
                <QueuePane item={item} items={upNext} />
              ) : (
                <TextView item={item} spoken={spoken} />
              ))}
          </div>
        </section>
      </div>
    </div>
  );
};

type ControlsProps = {
  item: QueueItem | undefined;
  spoken: SpokenItem | null;
  /** Whether the seek slider is being dragged (the sheet doesn't drag meanwhile). */
  onScrub: (scrubbing: boolean) => void;
  /** Smaller, under the lyrics or the queue, so they get the room. */
  compact?: boolean;
};

/**
 * Title, seek bar and buttons: the same in every layout. Sizes follow its own width (a
 * container), so five buttons always fit in a row, whatever the interface size.
 */
const Controls: FC<ControlsProps> = ({ item, spoken, onScrub, compact = false }) => {
  const { t } = useTranslation();
  const note = useSourceNote(item);
  const error = usePlayerSelect((state) => (state?.status === 'error' ? state.error : null));
  const wide = usePlayerWide();

  return (
    <div
      className={`@container mx-auto flex w-full max-w-xl shrink-0 flex-col ${
        compact ? 'gap-[min(0.75rem,2svh)]' : 'gap-[min(1.25rem,3svh)]'
      }`}
    >
      <div className="flex w-full items-center gap-1">
        <div className="title-gap flex min-w-0 flex-1 flex-col">
          <Marquee
            testId="sheet-title"
            text={item?.title ?? t('player.nothingPlaying')}
            className={`w-full font-bold ${compact ? 'text-lg' : 'text-2xl'}`}
          />
          {/* On a phone the names are text (the menu goes to them); with a pointer, only the
              names themselves are links, kept apart from the title. */}
          {spoken ? (
            wide ? (
              <SpokenByline
                spoken={spoken}
                className={`text-muted block w-full truncate ${compact ? 'text-sm' : 'text-lg'}`}
              />
            ) : (
              <span
                data-testid="spoken-byline"
                className={`text-muted block w-full truncate ${compact ? 'text-sm' : 'text-lg'}`}
              >
                {spokenName(spoken)}
              </span>
            )
          ) : (
            item?.artist &&
            (wide ? (
              <ArtistLinks
                credit={item.artist}
                className={`text-muted block w-full truncate ${compact ? 'text-sm' : 'text-lg'}`}
              />
            ) : (
              <span
                data-testid="sheet-artist"
                className={`text-muted block w-full truncate ${compact ? 'text-sm' : 'text-lg'}`}
              >
                {item.artist}
              </span>
            ))
          )}
          {note && (
            <p data-testid="sheet-source-note" className="text-muted text-xs">
              {note.kind === 'finding'
                ? t('player.findingSource')
                : t('player.via', { source: note.source })}
            </p>
          )}
          {error && <SourceIssue>{error}</SourceIssue>}
        </div>
        {item && !spoken && <FavoriteButton item={item} download />}
        {item && <MoreButton item={item} spoken={spoken} />}
      </div>

      <SeekBar variant="sheet" onScrub={onScrub} />

      <div className="flex w-full items-center justify-between">
        <ShuffleButton
          className={compact ? 'size-[min(2.5rem,12cqw)]' : 'size-[min(2.75rem,13cqw)]'}
          iconSize={compact ? 20 : 22}
        />
        <PreviousButton
          className={compact ? 'size-[min(2.75rem,13cqw)]' : 'size-[min(3rem,15cqw)]'}
          iconSize={compact ? 26 : 32}
        />
        <PlayPauseButton
          className={`bg-primary text-background shadow-lg shadow-black/20 ${
            compact
              ? 'size-[min(3.5rem,17cqw)] rounded-2xl'
              : 'size-[min(4.5rem,21cqw)] rounded-3xl'
          }`}
          iconSize={compact ? 26 : 32}
        />
        <NextButton
          className={compact ? 'size-[min(2.75rem,13cqw)]' : 'size-[min(3rem,15cqw)]'}
          iconSize={compact ? 26 : 32}
        />
        <RepeatButton
          className={compact ? 'size-[min(2.5rem,12cqw)]' : 'size-[min(2.75rem,13cqw)]'}
          iconSize={compact ? 20 : 22}
        />
      </div>
    </div>
  );
};

/**
 * The song's other options: on a phone the actions sheet (playlists, play next, download, go to
 * the album or the artist...), with a pointer its menu beside the button; for a chapter or an
 * episode, the book or the show.
 */
const MoreButton: FC<{ item: QueueItem; spoken: SpokenItem | null }> = ({ item, spoken }) => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const wide = usePlayerWide();
  const menuItems = useTrackMenuItems();
  const openTrackActions = useTrackActionsStore((store) => store.open);
  const open = (event: MouseEvent<HTMLButtonElement>) => {
    const button = event.currentTarget;
    const box = button.getBoundingClientRect();
    const at = { x: box.left, y: box.bottom + 4 };
    if (spoken) {
      const book = spoken.kind === 'audiobook';
      useContextMenuStore.getState().show(at.x, at.y, [
        { kind: 'heading', label: item.title },
        {
          label: t(book ? 'captions.goToBook' : 'captions.goToShow'),
          icon: book ? BookOpen : Podcast,
          onSelect: () =>
            void navigate({
              to: book ? '/audiobook' : '/podcast',
              search: { id: spoken.podcastId },
            }),
        },
      ]);
      return;
    }
    const track = fromQueueItem(item);
    if (wide) useContextMenuStore.getState().show(at.x, at.y, menuItems(track, {}, at));
    else openTrackActions(track, { anchor: button });
  };
  return (
    <button
      type="button"
      aria-label={t('player.moreOptions')}
      title={t('player.moreOptions')}
      aria-haspopup="menu"
      onClick={open}
      className="text-muted hover:text-foreground grid size-11 shrink-0 place-items-center rounded-full"
    >
      <EllipsisVertical aria-hidden size={22} />
    </button>
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

/** What plays after the current item: its actions, then the list filling the space it has. */
const QueuePane: FC<{ item: QueueItem; items: QueueItem[] }> = ({ item, items }) => {
  const { t } = useTranslation();
  return (
    <section
      aria-label={t('player.upNext')}
      className="flex min-h-0 w-full flex-1 flex-col gap-1 self-stretch"
    >
      <div className="flex flex-wrap items-center justify-between gap-x-2">
        <h2 className="text-muted text-xs font-semibold tracking-wide uppercase">
          {t('player.upNext')}
          {items.length > 0 && (
            <span className="ml-1.5 font-normal normal-case">
              {t('player.queueCount', { count: items.length })}
            </span>
          )}
        </h2>
        <QueueActions items={items} current={item} />
      </div>
      <QueueList items={items} variant="sheet" />
    </section>
  );
};
