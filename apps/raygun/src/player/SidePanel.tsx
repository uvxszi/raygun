import type { QueueItem } from '@raygun/bindings';
import { ListMusic, Maximize2, X } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { Artwork } from '../library/Artwork';
import { Marquee } from '../shell/Marquee';
import { useWideScreen } from '../shell/useWideScreen';
import { FavoriteButton } from './FavoriteButton';
import { LyricsView } from './LyricsView';
import type { PlayerPanel } from './playerUiStore';
import { usePlayerUiStore } from './playerUiStore';
import { sendControl, sourceNote, useNowPlaying } from './usePlayback';
import { useVideoSurface } from './useVideoSurface';

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
  const { item } = useNowPlaying();
  if (!panel || !wide || !item) return null;

  return (
    <aside
      aria-label={t(`player.panel.${panel}`)}
      data-testid="side-panel"
      className="bg-surface my-2 mr-2 flex w-72 shrink-0 flex-col overflow-hidden rounded-2xl lg:mr-4 lg:w-80 xl:w-96"
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
                panel === tab ? 'bg-background text-foreground' : 'text-muted hover:text-foreground'
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
      <div role="tabpanel" className="flex min-h-0 flex-1 flex-col overflow-y-auto">
        {panel === 'nowPlaying' ? <NowPlaying item={item} /> : <Queue item={item} />}
      </div>
    </aside>
  );
};

const NowPlaying: FC<{ item: QueueItem }> = ({ item }) => {
  const { t } = useTranslation();
  const { state, upNext } = useNowPlaying();
  const sheetOpen = usePlayerUiStore((store) => store.sheetOpen);
  const openSheet = usePlayerUiStore((store) => store.openSheet);
  const setPanel = usePlayerUiStore((store) => store.setPanel);
  const isVideo = item.source.kind === 'youTube';
  // The one video dock plays here unless the full player has it.
  const videoRef = useVideoSurface(isVideo && !sheetOpen);
  const note = sourceNote(state, item);
  const next = upNext[0];

  return (
    <div className="flex flex-col gap-4 px-4 pt-3 pb-4">
      {isVideo ? (
        <div
          ref={videoRef}
          data-testid="side-panel-video"
          className="aspect-video w-full overflow-hidden rounded-xl bg-black"
        />
      ) : (
        <Artwork
          key={item.artworkUrl ?? 'none'}
          url={item.artworkUrl}
          className="aspect-square w-full rounded-xl shadow-lg"
          iconSize={72}
        />
      )}
      <div className="flex items-center gap-1">
        <div className="flex min-w-0 flex-1 flex-col">
          <Marquee testId="side-panel-title" text={item.title} className="text-xl font-bold" />
          {item.artist && <Marquee text={item.artist} className="text-muted text-sm" />}
          {note && (
            <span className="text-muted text-xs">
              {note.kind === 'finding'
                ? t('player.findingSource')
                : t('player.via', { source: note.source })}
            </span>
          )}
        </div>
        <FavoriteButton item={item} compact />
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
    </div>
  );
};

const Queue: FC<{ item: QueueItem }> = ({ item }) => {
  const { t } = useTranslation();
  const { upNext } = useNowPlaying();
  const clear = async () => {
    for (const next of upNext) {
      await sendControl({ type: 'remove', itemId: next.id });
    }
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
        {upNext.length > 0 && (
          <button
            type="button"
            onClick={() => void clear()}
            className="text-muted hover:text-foreground rounded-full px-2 py-0.5 text-sm"
          >
            {t('player.clearQueue')}
          </button>
        )}
      </div>
      {upNext.length === 0 ? (
        <p className="text-muted px-2 py-2 text-sm">{t('player.queueEmpty')}</p>
      ) : (
        <ul data-testid="queue-panel-list" className="flex flex-col">
          {upNext.map((next) => (
            <li key={next.id}>
              <QueueRow item={next} />
            </li>
          ))}
        </ul>
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
