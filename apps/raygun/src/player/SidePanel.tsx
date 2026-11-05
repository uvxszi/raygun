import type { QueueItem } from '@raygun/bindings';
import { ListMusic, Maximize2, X } from 'lucide-react';
import { useState } from 'react';
import type { FC, KeyboardEvent, PointerEvent as ReactPointerEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { Artwork } from '../library/Artwork';
import { ExplicitMark } from '../music/ExplicitMark';
import { songCover } from '../library/coverTargets';
import { ArtistLinks } from '../music/ArtistLinks';
import { tidyCredit } from '../music/artists';
import { TrackTagChips } from '../music/TrackTagChips';
import { ResizeHandle } from '../shell/ResizeHandle';
import { useIsDesktop } from '../shell/useIsDesktop';
import { usePanelSizes } from '../shell/usePanelSizes';
import { useWindowWidth } from '../shell/useWideScreen';
import { CaptionsView } from './CaptionsView';
import { FavoriteButton } from './FavoriteButton';
import { LyricsView } from './LyricsView';
import { MediaArt, VideoToggle } from './MediaArt';
import { MusicSettingsButton } from './MusicSettingsButton';
import { ArtistCard, CreditsCard } from './PlayerCards';
import { usePlayerWide } from './playerLayout';
import type { PlayerPanel } from './playerUiStore';
import { usePlayerUiStore } from './playerUiStore';
import { PlayingFrom } from './PlayingFrom';
import { QueueActions, QueueList } from './QueueList';
import { useSpokenItem } from './spoken';
import { SpokenByline } from './SpokenByline';
import { sendControl, useCurrentItem, useSourceNote, useUpNext } from './usePlayback';

const TABS: PlayerPanel[] = ['nowPlaying', 'queue'];

/** Songs "Next in queue" can show (M46.24), and one row's height in rem for dragging. */
const NEXT_UP_MAX = 10;
const NEXT_UP_ROW_REM = 3.25;

/**
 * The panel at the right of the page on wide windows (M46.7), like Spotify's: what plays (the
 * artwork or the video, its title, lyrics and what's next) or the queue. It sits beside the page
 * instead of over it, and stays open between songs and restarts.
 */
export const SidePanel: FC = () => {
  const { t } = useTranslation();
  const panel = usePlayerUiStore((state) => state.panel);
  const setPanel = usePlayerUiStore((state) => state.setPanel);
  const wide = usePlayerWide();
  const desktop = useIsDesktop();
  const windowWidth = useWindowWidth();
  const item = useCurrentItem();
  const { panels, save } = usePanelSizes();
  const [dragging, setDragging] = useState<number | null>(null);
  // Desktop keeps the panel at every window size, narrower when the window is (M46.9).
  if (!panel || !(wide || desktop) || !item) return null;
  // Dragged to a size (M46.8), or by screen: 18, 20 then 24 rem; never more than 30 % of a
  // desktop window, nor less than 13 rem.
  const chosen = dragging ?? (panels.sidePanelWidth || null);
  const width =
    desktop && windowWidth > 0
      ? Math.max(
          208,
          Math.min(
            chosen ?? (windowWidth >= 1280 ? 384 : windowWidth >= 1024 ? 320 : 288),
            Math.round(windowWidth * 0.3),
          ),
        )
      : chosen;

  return (
    <>
      <ResizeHandle
        label={t('player.panel.resize')}
        value={width ?? 320}
        min={280}
        max={560}
        direction={-1}
        onPreview={setDragging}
        onCommit={(sidePanelWidth) => {
          setDragging(null);
          save({ sidePanelWidth });
        }}
        className="my-2"
      />
      {/* A container (`panel`): what's inside sizes itself by the panel's width, not the window's. */}
      <aside
        aria-label={t(`player.panel.${panel}`)}
        data-testid="side-panel"
        style={width ? { width } : undefined}
        className={`bg-surface @container/panel my-2 mr-2 flex shrink-0 flex-col overflow-hidden rounded-2xl lg:mr-4 ${width ? '' : 'w-72 lg:w-80 xl:w-96'}`}
      >
        <header className="flex items-center gap-1 px-2 pt-2 @max-3xs/panel:gap-0.5 @max-3xs/panel:px-1">
          {/* Each label stays on one line: smaller and closer as the panel narrows. */}
          <div
            role="tablist"
            aria-label={t('player.panel.label')}
            className="flex min-w-0 flex-1 gap-1 @max-3xs/panel:gap-0.5"
          >
            {TABS.map((tab) => (
              <button
                key={tab}
                type="button"
                role="tab"
                aria-selected={panel === tab}
                onClick={() => setPanel(tab)}
                className={`min-w-0 rounded-full px-3 py-1.5 text-sm font-medium whitespace-nowrap @max-xs/panel:px-2.5 @max-xs/panel:text-xs @max-3xs/panel:px-1.5 @max-3xs/panel:text-[0.6875rem] ${
                  panel === tab
                    ? 'bg-background text-foreground'
                    : 'text-muted hover:text-foreground'
                }`}
              >
                <span className="block truncate">{t(`player.panel.${tab}`)}</span>
              </button>
            ))}
          </div>
          <MusicSettingsButton className="size-9 shrink-0 @max-3xs/panel:size-7" />
          <button
            type="button"
            aria-label={t('player.panel.close')}
            onClick={() => setPanel(null)}
            className="text-muted hover:text-foreground grid size-9 shrink-0 place-items-center rounded-full @max-3xs/panel:size-7"
          >
            <X aria-hidden size={18} />
          </button>
        </header>
        {panel === 'nowPlaying' ? (
          <div
            role="tabpanel"
            className="no-scrollbar flex min-h-0 flex-1 flex-col overflow-y-auto"
          >
            <NowPlaying item={item} />
          </div>
        ) : (
          <div role="tabpanel" className="flex min-h-0 flex-1 flex-col">
            <Queue item={item} />
          </div>
        )}
      </aside>
    </>
  );
};

const NowPlaying: FC<{ item: QueueItem }> = ({ item }) => {
  const { t } = useTranslation();
  const upNext = useUpNext();
  const sheetOpen = usePlayerUiStore((store) => store.sheetOpen);
  const openSheet = usePlayerUiStore((store) => store.openSheet);
  const setPanel = usePlayerUiStore((store) => store.setPanel);
  const note = useSourceNote(item);
  // Podcasts and audiobooks show their show or book and captions, not artists and lyrics (M46.32).
  const spoken = useSpokenItem(item);
  const { panels, save } = usePanelSizes();
  const [rowsPreview, setRowsPreview] = useState<number | null>(null);
  const rows = rowsPreview ?? (panels.nextUpRows || 1);
  const shown = upNext.slice(0, rows);

  return (
    // Its parts keep their height and the panel scrolls: shrinking cut the cards' last lines.
    <div className="flex shrink-0 flex-col gap-4 px-4 pt-3 pb-4 @max-3xs/panel:px-3 [&>*]:shrink-0">
      <PlayingFrom variant="row" className="-mb-2" />
      {/* The one video dock shows here unless the full player has it. */}
      <MediaArt
        item={item}
        active={!sheetOpen}
        testId="side-panel-video"
        artworkClassName="aspect-square w-full rounded-xl shadow-lg"
        videoClassName="aspect-video w-full overflow-hidden rounded-xl bg-black"
        iconSize={72}
      />
      {/* The song: its title and artists beside its buttons, its tags under them across the
          panel. On a narrow panel the title has the whole width too (two lines at most) and the
          buttons get a row of their own, so nothing is squeezed into a column. */}
      <div className="flex flex-col gap-2">
        <div className="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-x-1 gap-y-0.5 @max-xs/panel:grid-cols-1">
          <div className="title-gap flex min-w-0 flex-col">
            <p
              data-testid="side-panel-title"
              title={item.title}
              className="panel-title line-clamp-2 leading-tight font-bold wrap-anywhere"
            >
              {item.title}
            </p>
            {/* Only the names lead to their pages; the space around them doesn't. */}
            {spoken ? (
              <SpokenByline spoken={spoken} className="text-muted block truncate text-sm" />
            ) : (
              item.artist && (
                <ArtistLinks credit={item.artist} className="text-muted block truncate text-sm" />
              )
            )}
            {note && (
              <span className="text-muted truncate text-xs">
                {note.kind === 'finding'
                  ? t('player.findingSource')
                  : t('player.via', { source: note.source })}
              </span>
            )}
          </div>
          <div data-testid="side-panel-actions" className="flex items-center @max-xs/panel:-ml-2">
            {!spoken && <FavoriteButton item={item} compact download />}
            <VideoToggle item={item} className="size-9" />
            <button
              type="button"
              aria-label={t('player.openPlayer')}
              onClick={() => openSheet({ view: 'artwork' })}
              className="text-muted hover:text-foreground grid size-9 shrink-0 place-items-center rounded-full"
            >
              <Maximize2 aria-hidden size={18} />
            </button>
          </div>
        </div>
        {!spoken && <TrackTagChips track={item} compact />}
      </div>
      <section
        aria-label={spoken ? t('captions.title') : t('player.lyrics')}
        className="bg-background/50 rounded-xl"
      >
        <header className="flex items-center justify-between gap-2 px-4 pt-3">
          <h3 className="min-w-0 truncate text-sm font-semibold">
            {spoken ? t('captions.title') : t('player.lyrics')}
          </h3>
          <button
            type="button"
            onClick={() => openSheet({ view: 'lyrics' })}
            className="text-muted hover:text-foreground shrink-0 text-xs font-medium whitespace-nowrap"
          >
            {spoken ? t('captions.full') : t('player.panel.fullLyrics')}
          </button>
        </header>
        {spoken ? (
          <CaptionsView spoken={spoken} variant="panel" />
        ) : (
          <LyricsView item={item} variant="panel" />
        )}
      </section>
      <section aria-label={t('player.panel.nextUp')} className="bg-background/50 rounded-xl p-3">
        {/* Without room for both, the button goes under the heading rather than cut it. */}
        <header className="flex flex-wrap items-center justify-between gap-x-2 gap-y-1 pb-2">
          <h3 className="min-w-0 truncate text-sm font-semibold">{t('player.panel.nextUp')}</h3>
          <button
            type="button"
            onClick={() => setPanel('queue')}
            className="text-muted hover:text-foreground ml-auto flex shrink-0 items-center gap-1 text-xs font-medium whitespace-nowrap"
          >
            <ListMusic aria-hidden size={14} />
            {t('player.panel.openQueue')}
          </button>
        </header>
        {shown.length > 0 ? (
          <ul data-testid="next-up-list" className="flex flex-col">
            {shown.map((next) => (
              <li key={next.id}>
                <NextRow item={next} />
              </li>
            ))}
          </ul>
        ) : (
          <p className="text-muted text-sm">{t('player.queueEmpty')}</p>
        )}
        <RowsHandle
          rows={rows}
          onPreview={setRowsPreview}
          onCommit={(nextUpRows) => {
            setRowsPreview(null);
            save({ nextUpRows });
          }}
        />
      </section>
      {/* The artist and the credits, as under the phone's player (M46.16). */}
      {!spoken && (
        <>
          <ArtistCard credit={item.artist} />
          <CreditsCard item={item} />
        </>
      )}
    </div>
  );
};

/**
 * The lower edge of "Next in queue" (M46.24): dragging it shows more or fewer songs, a whole row
 * at a time; arrow keys do the same.
 */
const RowsHandle: FC<{
  rows: number;
  onPreview: (rows: number) => void;
  onCommit: (rows: number) => void;
}> = ({ rows, onPreview, onCommit }) => {
  const { t } = useTranslation();
  const clamp = (next: number) => Math.min(NEXT_UP_MAX, Math.max(1, next));
  const start = (event: ReactPointerEvent) => {
    event.preventDefault();
    const from = event.clientY;
    // A row is as tall as the interface size makes it.
    const rowPx =
      NEXT_UP_ROW_REM *
      (Number.parseFloat(getComputedStyle(document.documentElement).fontSize) || 16);
    let latest = rows;
    const move = (moved: PointerEvent) => {
      latest = clamp(rows + Math.round((moved.clientY - from) / rowPx));
      onPreview(latest);
    };
    const up = () => {
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', up);
      document.body.style.cursor = '';
      onCommit(latest);
    };
    document.body.style.cursor = 'row-resize';
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', up);
  };
  const key = (event: KeyboardEvent) => {
    const next =
      event.key === 'ArrowDown'
        ? rows + 1
        : event.key === 'ArrowUp'
          ? rows - 1
          : event.key === 'Home'
            ? 1
            : event.key === 'End'
              ? NEXT_UP_MAX
              : null;
    if (next === null) return;
    event.preventDefault();
    onCommit(clamp(next));
  };
  return (
    <div
      role="separator"
      aria-orientation="horizontal"
      aria-label={t('player.panel.nextUpSize')}
      aria-valuenow={rows}
      aria-valuemin={1}
      aria-valuemax={NEXT_UP_MAX}
      tabIndex={0}
      onPointerDown={start}
      onKeyDown={key}
      className="group -mb-2 flex h-4 cursor-row-resize touch-none items-center justify-center focus:outline-none"
    >
      <span className="bg-muted/40 group-hover:bg-muted group-focus-visible:bg-primary h-1 w-10 rounded-full transition-colors" />
    </div>
  );
};

/**
 * A song in "Next in queue": plays on a tap; its artist is plain text. On a narrow panel the
 * remove button only takes room while the pointer is over the row or it has focus, so the names
 * have it otherwise.
 */
const NextRow: FC<{ item: QueueItem; current?: boolean }> = ({ item, current = false }) => {
  const { t } = useTranslation();
  return (
    <div className="group hover:bg-background/40 flex items-center gap-1 rounded-lg pr-1">
      <button
        type="button"
        aria-label={item.title}
        disabled={current}
        onClick={() => void sendControl({ type: 'playItem', itemId: item.id })}
        className="flex min-w-0 flex-1 items-center gap-3 py-1.5 pl-2 text-left @max-xs/panel:gap-2.5"
      >
        <Artwork
          url={item.artworkUrl}
          fallback={songCover(item)}
          className="size-10 shrink-0 rounded-md @max-xs/panel:size-9"
          iconSize={18}
        />
        <span className="title-gap flex min-w-0 flex-col">
          <span className="flex min-w-0 items-center gap-1.5">
            <span
              className={`truncate text-sm leading-tight font-medium ${current ? 'text-primary' : ''}`}
            >
              {item.title}
            </span>
            {item.explicit && <ExplicitMark />}
          </span>
          {item.artist && (
            <span className="text-muted truncate text-xs leading-tight">
              {tidyCredit(item.artist)}
            </span>
          )}
        </span>
      </button>
      {!current && (
        <button
          type="button"
          aria-label={t('player.removeFromQueue', { title: item.title })}
          onClick={() => void sendControl({ type: 'remove', itemId: item.id })}
          className="text-muted hover:text-foreground grid size-8 shrink-0 place-items-center overflow-hidden rounded-full pointer-fine:opacity-0 pointer-fine:group-hover:opacity-100 pointer-fine:focus-visible:opacity-100 @max-xs/panel:pointer-fine:w-0 @max-xs/panel:pointer-fine:group-hover:w-8 @max-xs/panel:pointer-fine:focus-visible:w-8"
        >
          <X aria-hidden size={16} />
        </button>
      )}
    </div>
  );
};

/** The queue tab: the song playing, then what's next, which moves, goes and saves. */
const Queue: FC<{ item: QueueItem }> = ({ item }) => {
  const { t } = useTranslation();
  const upNext = useUpNext();
  return (
    <div className="flex min-h-0 flex-1 flex-col px-2 pb-2">
      <h3 className="text-muted px-2 pt-3 pb-1 text-xs font-semibold tracking-wide uppercase">
        {t('player.nowPlayingHeading')}
      </h3>
      <NextRow item={item} current />
      <div className="flex flex-wrap items-center justify-between gap-x-2 px-2 pt-4 pb-1">
        <h3 className="text-muted text-xs font-semibold tracking-wide uppercase">
          {t('player.upNext')}
          {upNext.length > 0 && (
            <span className="ml-1.5 font-normal normal-case">
              {t('player.queueCount', { count: upNext.length })}
            </span>
          )}
        </h3>
      </div>
      <div className="px-1 pb-1">
        <QueueActions items={upNext} current={item} />
      </div>
      <QueueList items={upNext} variant="panel" />
    </div>
  );
};
