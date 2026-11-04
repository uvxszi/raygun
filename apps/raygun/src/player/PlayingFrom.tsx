import type { PlayContext, QueueItem, QueueSnapshot } from '@raygun/bindings';
import { useNavigate } from '@tanstack/react-router';
import {
  BookOpen,
  CircleArrowDown,
  Disc3,
  Heart,
  ListMusic,
  Podcast,
  Search,
  Sparkles,
  Tag,
  TrendingUp,
  UserRound,
} from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { episodeReveal, songReveal, useRevealStore } from '../music/reveal';
import { usePlayerUiStore } from './playerUiStore';
import { useCurrentItem, useQueueSelect } from './usePlayback';
import { useSpokenItem } from './spoken';

const ICONS: Record<string, LucideIcon> = {
  album: Disc3,
  playlist: ListMusic,
  mix: Sparkles,
  artist: UserRound,
  chart: TrendingUp,
  tag: Tag,
  podcast: Podcast,
  audiobook: BookOpen,
  favorites: Heart,
  downloaded: CircleArrowDown,
  search: Search,
};

const contextOf = (queue: QueueSnapshot | undefined) => queue?.context ?? null;

/** Where the queue was played from (kept with the queue in Rust), or null. */
const useQueueContext = (): PlayContext | null => useQueueSelect(contextOf);

/**
 * Opens the page the queue was played from and, where it lists songs, scrolls to the one playing
 * (`shell/RevealHost`). Search just opens.
 */
const useOpenContext = (item: QueueItem | undefined) => {
  const navigate = useNavigate();
  const spoken = useSpokenItem(item);
  return (context: PlayContext) => {
    usePlayerUiStore.getState().closeSheet();
    if (item && context.kind !== 'search') {
      useRevealStore.getState().request({
        keys: (spoken
          ? episodeReveal(spoken.episodeId)
          : songReveal(item.title, item.artist)
        ).split('\t'),
        to: context.to,
        search: context.search,
      });
    }
    void navigate({ to: context.to, search: context.search as never });
  };
};

/**
 * "Playing from" (the full player beside Up next, the side panel, the bar): the album, playlist,
 * search... the queue came from; opens it at the playing song. Hidden without one.
 * `icon` is the bar's: the kind's icon alone, named by its label.
 */
export const PlayingFrom: FC<{ variant?: 'pill' | 'row' | 'icon'; className?: string }> = ({
  variant = 'pill',
  className = '',
}) => {
  const { t } = useTranslation();
  const context = useQueueContext();
  const item = useCurrentItem();
  const open = useOpenContext(item);
  if (!context) return null;
  const Icon = ICONS[context.kind] ?? ListMusic;
  const label = t('player.playingFrom', { name: context.name });
  if (variant === 'icon') {
    return (
      <button
        type="button"
        aria-label={label}
        title={label}
        onClick={() => open(context)}
        className={`text-muted hover:text-foreground grid size-8 shrink-0 place-items-center rounded-full ${className}`}
      >
        <Icon aria-hidden size={16} />
      </button>
    );
  }
  if (variant === 'row') {
    return (
      <button
        type="button"
        aria-label={label}
        onClick={() => open(context)}
        className={`text-muted hover:text-foreground flex min-w-0 items-center gap-2 text-left text-xs ${className}`}
      >
        <Icon aria-hidden size={14} className="shrink-0" />
        <span className="shrink-0">{t('player.playingFromShort')}</span>
        <span className="text-foreground truncate font-semibold">{context.name}</span>
      </button>
    );
  }
  return (
    <button
      type="button"
      aria-label={label}
      onClick={() => open(context)}
      className={`bg-surface/80 hover:bg-surface flex min-w-0 items-center gap-2 rounded-full px-4 py-1.5 text-sm shadow-sm ${className}`}
    >
      <Icon aria-hidden size={16} className="shrink-0" />
      <span className="truncate font-semibold">{context.name}</span>
    </button>
  );
};
