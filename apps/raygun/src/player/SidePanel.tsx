import { DndContext, PointerSensor, closestCenter, useSensor, useSensors } from '@dnd-kit/core';
import type { DragEndEvent } from '@dnd-kit/core';
import {
  SortableContext,
  arrayMove,
  useSortable,
  verticalListSortingStrategy,
} from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import type { QueueItem } from '@raygun/bindings';
import { ListMusic, Maximize2, X } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { fromQueueItem, useAddToPlaylist, useCreatePlaylist } from '../collections/useCollections';
import { Artwork } from '../library/Artwork';
import { ArtistLinks } from '../music/ArtistLinks';
import { TrackTagChips } from '../music/TrackTagChips';
import { Marquee } from '../shell/Marquee';
import { useIsDesktop } from '../shell/useIsDesktop';
import { useWideScreen, useWindowWidth } from '../shell/useWideScreen';
import { ResizeHandle } from '../shell/ResizeHandle';
import { usePanelSizes } from '../shell/usePanelSizes';
import { FavoriteButton } from './FavoriteButton';
import { LyricsView } from './LyricsView';
import { ArtistCard, CreditsCard } from './PlayerCards';
import type { PlayerPanel } from './playerUiStore';
import { usePlayerUiStore } from './playerUiStore';
import { sendControl, sourceNote, useNowPlaying } from './usePlayback';
import { MediaArt, VideoToggle } from './MediaArt';

const TABS: PlayerPanel[] = ['nowPlaying', 'queue'];

/**
 * The panel at the right of the page on wide windows (M46.7), like Spotify's: what plays (the
 * artwork or the video, its title, lyrics and what's next) or the queue. It sits beside the page
 * instead of over it, and stays open between songs and restarts.
 */
export const SidePanel: FC = () => {
  const { t } = useTranslation();
  const panel = usePlayerUiStore((state) => state.panel);
  const setPanel = usePlayerUiStore((state) => state.setPanel);
  const wide = useWideScreen();
  const desktop = useIsDesktop();
  const windowWidth = useWindowWidth();
  const { item } = useNowPlaying();
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
      <aside
        aria-label={t(`player.panel.${panel}`)}
        data-testid="side-panel"
        style={width ? { width } : undefined}
        className={`bg-surface my-2 mr-2 flex shrink-0 flex-col overflow-hidden rounded-2xl lg:mr-4 ${width ? '' : 'w-72 lg:w-80 xl:w-96'}`}
      >
        <header className="flex items-center gap-1 px-2 pt-2">
          <div role="tablist" aria-label={t('player.panel.label')} className="flex flex-1 gap-1">
            {TABS.map((tab) => (
              <button
                key={tab}
                type="button"
                role="tab"
                aria-selected={panel === tab}
                onClick={() => setPanel(tab)}
                className={`rounded-full px-3 py-1.5 text-sm font-medium ${
                  panel === tab
                    ? 'bg-background text-foreground'
                    : 'text-muted hover:text-foreground'
                }`}
              >
                {t(`player.panel.${tab}`)}
              </button>
            ))}
          </div>
          <button
            type="button"
            aria-label={t('player.panel.close')}
            onClick={() => setPanel(null)}
            className="text-muted hover:text-foreground grid size-9 place-items-center rounded-full"
          >
            <X aria-hidden size={18} />
          </button>
        </header>
        <div role="tabpanel" className="no-scrollbar flex min-h-0 flex-1 flex-col overflow-y-auto">
          {panel === 'nowPlaying' ? <NowPlaying item={item} /> : <Queue item={item} />}
        </div>
      </aside>
    </>
  );
};

const NowPlaying: FC<{ item: QueueItem }> = ({ item }) => {
  const { t } = useTranslation();
  const { state, upNext } = useNowPlaying();
  const sheetOpen = usePlayerUiStore((store) => store.sheetOpen);
  const openSheet = usePlayerUiStore((store) => store.openSheet);
  const setPanel = usePlayerUiStore((store) => store.setPanel);
  const note = sourceNote(state, item);
  const next = upNext[0];

  return (
    <div className="flex flex-col gap-4 px-4 pt-3 pb-4">
      {/* The one video dock shows here unless the full player has it. */}
      <MediaArt
        item={item}
        active={!sheetOpen}
        testId="side-panel-video"
        artworkClassName="aspect-square w-full rounded-xl shadow-lg"
        videoClassName="aspect-video w-full overflow-hidden rounded-xl bg-black"
        iconSize={72}
      />
      <div className="flex items-center gap-1">
        <div className="flex min-w-0 flex-1 flex-col">
          <Marquee testId="side-panel-title" text={item.title} className="text-xl font-bold" />
          {item.artist && (
            <ArtistLinks credit={item.artist} className="text-muted block truncate text-sm" />
          )}
          {note && (
            <span className="text-muted text-xs">
              {note.kind === 'finding'
                ? t('player.findingSource')
                : t('player.via', { source: note.source })}
            </span>
          )}
          <TrackTagChips track={item} className="pt-1.5" />
        </div>
        <FavoriteButton item={item} compact download />
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
      <section aria-label={t('player.lyrics')} className="bg-background/50 rounded-xl">
        <header className="flex items-center justify-between px-4 pt-3">
          <h3 className="text-sm font-semibold">{t('player.lyrics')}</h3>
          <button
            type="button"
            onClick={() => openSheet({ view: 'lyrics' })}
            className="text-muted hover:text-foreground text-xs font-medium"
          >
            {t('player.panel.fullLyrics')}
          </button>
        </header>
        <LyricsView item={item} state={state} variant="panel" />
      </section>
      <section aria-label={t('player.panel.nextUp')} className="bg-background/50 rounded-xl p-3">
        <header className="flex items-center justify-between pb-2">
          <h3 className="text-sm font-semibold">{t('player.panel.nextUp')}</h3>
          <button
            type="button"
            onClick={() => setPanel('queue')}
            className="text-muted hover:text-foreground flex items-center gap-1 text-xs font-medium"
          >
            <ListMusic aria-hidden size={14} />
            {t('player.panel.openQueue')}
          </button>
        </header>
        {next ? (
          <QueueRow item={next} />
        ) : (
          <p className="text-muted text-sm">{t('player.queueEmpty')}</p>
        )}
      </section>
      {/* The artist and the credits, as under the phone's player (M46.16). */}
      <ArtistCard credit={item.artist} />
      <CreditsCard item={item} />
    </div>
  );
};

/** A queued item that drags into another place (M46.8). */
const SortableQueueRow: FC<{ item: QueueItem }> = ({ item }) => {
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({
    id: item.id,
  });
  return (
    <li
      ref={setNodeRef}
      style={{ transform: CSS.Translate.toString(transform), transition }}
      className={isDragging ? 'relative z-10 opacity-80' : ''}
      {...attributes}
      {...listeners}
      // A list item that drags, not a second button around the row's own.
      role="listitem"
    >
      <QueueRow item={item} />
    </li>
  );
};

const Queue: FC<{ item: QueueItem }> = ({ item }) => {
  const { t } = useTranslation();
  const { upNext } = useNowPlaying();
  const createPlaylist = useCreatePlaylist();
  const addToPlaylist = useAddToPlaylist();
  const [saved, setSaved] = useState<string | null>(null);
  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 6 } }));
  const clear = async () => {
    for (const next of upNext) {
      await sendControl({ type: 'remove', itemId: next.id });
    }
  };
  // Dropped before the item now after it, or at the end.
  const onDragEnd = ({ active, over }: DragEndEvent) => {
    if (!over || active.id === over.id) return;
    const ids = upNext.map((next) => next.id);
    const moved = arrayMove(ids, ids.indexOf(String(active.id)), ids.indexOf(String(over.id)));
    const at = moved.indexOf(String(active.id));
    void sendControl({
      type: 'move',
      itemId: String(active.id),
      beforeItemId: moved[at + 1] ?? null,
    });
  };
  // The whole queue, playing song first, as a new playlist (M46.8).
  const saveQueue = () => {
    const name = t('player.queueSavedName', { date: new Date().toLocaleString() });
    createPlaylist.mutate(name, {
      onSuccess: (playlist) =>
        addToPlaylist.mutate(
          { id: playlist.id, tracks: [item, ...upNext].map(fromQueueItem) },
          { onSuccess: () => setSaved(name) },
        ),
    });
  };

  return (
    <div className="flex flex-col px-2 pb-3">
      <h3 className="text-muted px-2 pt-3 pb-1 text-xs font-semibold tracking-wide uppercase">
        {t('player.nowPlayingHeading')}
      </h3>
      <QueueRow item={item} current />
      <div className="flex items-center justify-between px-2 pt-4 pb-1">
        <h3 className="text-muted text-xs font-semibold tracking-wide uppercase">
          {t('player.upNext')}
        </h3>
        <span className="flex items-center">
          <button
            type="button"
            disabled={createPlaylist.isPending || addToPlaylist.isPending}
            onClick={saveQueue}
            className="text-muted hover:text-foreground rounded-full px-2 py-0.5 text-sm"
          >
            {t('player.saveQueue')}
          </button>
          {upNext.length > 0 && (
            <button
              type="button"
              onClick={() => void clear()}
              className="text-muted hover:text-foreground rounded-full px-2 py-0.5 text-sm"
            >
              {t('player.clearQueue')}
            </button>
          )}
        </span>
      </div>
      {saved && (
        <p role="status" className="text-primary px-2 pb-1 text-xs">
          {t('player.queueSaved', { name: saved })}
        </p>
      )}
      {upNext.length === 0 ? (
        <p className="text-muted px-2 py-2 text-sm">{t('player.queueEmpty')}</p>
      ) : (
        <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={onDragEnd}>
          <SortableContext
            items={upNext.map((next) => next.id)}
            strategy={verticalListSortingStrategy}
          >
            <ul data-testid="queue-panel-list" className="flex flex-col">
              {upNext.map((next) => (
                <SortableQueueRow key={next.id} item={next} />
              ))}
            </ul>
          </SortableContext>
        </DndContext>
      )}
    </div>
  );
};

const QueueRow: FC<{ item: QueueItem; current?: boolean }> = ({ item, current = false }) => {
  const { t } = useTranslation();
  return (
    <div className="group hover:bg-background/40 flex items-center gap-1 rounded-lg pr-1">
      <button
        type="button"
        aria-label={item.title}
        disabled={current}
        onClick={() => void sendControl({ type: 'playItem', itemId: item.id })}
        className="flex min-w-0 flex-1 items-center gap-3 py-1.5 pl-2 text-left"
      >
        <Artwork url={item.artworkUrl} className="size-10 shrink-0 rounded-md" iconSize={18} />
        <span className="flex min-w-0 flex-col">
          <span className={`truncate text-sm font-medium ${current ? 'text-primary' : ''}`}>
            {item.title}
          </span>
          {item.artist && <span className="text-muted truncate text-xs">{item.artist}</span>}
        </span>
      </button>
      {!current && (
        <button
          type="button"
          aria-label={t('player.removeFromQueue', { title: item.title })}
          onClick={() => void sendControl({ type: 'remove', itemId: item.id })}
          className="text-muted hover:text-foreground grid size-8 shrink-0 place-items-center rounded-full opacity-0 group-hover:opacity-100 focus:opacity-100"
        >
          <X aria-hidden size={16} />
        </button>
      )}
    </div>
  );
};
