import type { QueueItem } from '@raygun/bindings';
import { X } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { Artwork } from '../library/Artwork';
import { useWideScreen } from '../shell/useWideScreen';
import { usePlayerUiStore } from './playerUiStore';
import { sendControl, useNowPlaying } from './usePlayback';

/**
 * The queue beside the player bar on wide windows (M46.4), like Spotify's: what plays now, then
 * what comes next. A click plays an item; each can be removed, and Clear removes what's next.
 */
export const QueuePanel: FC = () => {
  const { t } = useTranslation();
  const open = usePlayerUiStore((state) => state.queueOpen);
  const close = usePlayerUiStore((state) => state.closeQueue);
  const wide = useWideScreen();
  const { item, upNext } = useNowPlaying();
  if (!open || !wide || !item) return null;

  const clear = async () => {
    for (const next of upNext) {
      await sendControl({ type: 'remove', itemId: next.id });
    }
  };

  return (
    <aside
      aria-label={t('player.queue')}
      className="bg-surface fixed top-3 right-3 bottom-24 z-40 flex w-80 flex-col overflow-hidden rounded-2xl shadow-2xl shadow-black/30 lg:right-4"
    >
      <header className="flex items-center justify-between gap-2 px-4 pt-3 pb-1">
        <h2 className="text-base font-semibold">{t('player.queue')}</h2>
        <div className="flex items-center gap-1">
          {upNext.length > 0 && (
            <button
              type="button"
              onClick={() => void clear()}
              className="text-muted hover:text-foreground rounded-full px-3 py-1 text-sm"
            >
              {t('player.clearQueue')}
            </button>
          )}
          <button
            type="button"
            aria-label={t('player.closeQueue')}
            onClick={close}
            className="text-muted hover:text-foreground grid size-9 place-items-center rounded-full"
          >
            <X aria-hidden size={18} />
          </button>
        </div>
      </header>
      <div className="flex min-h-0 flex-1 flex-col overflow-y-auto px-2 pb-3">
        <h3 className="text-muted px-2 pt-2 pb-1 text-xs font-semibold tracking-wide uppercase">
          {t('player.nowPlayingHeading')}
        </h3>
        <QueueRow item={item} current />
        <h3 className="text-muted px-2 pt-4 pb-1 text-xs font-semibold tracking-wide uppercase">
          {t('player.upNext')}
        </h3>
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
    </aside>
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
