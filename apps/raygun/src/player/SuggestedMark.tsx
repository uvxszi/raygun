import { ListPlus, Sparkles, ThumbsDown } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { openMenuAt } from '../shell/contextMenuStore';
import type { MenuItem } from '../shell/contextMenuStore';
import { addToThisPlaylist, notForMe, usePlaysFromPlaylist, useSuggestedIds } from './smartShuffle';

/** A song Smart Shuffle added to the list: a sparkle beside its title (queue and player). */
export const SuggestedMark: FC<{ size?: number }> = ({ size = 13 }) => {
  const { t } = useTranslation();
  return (
    <span
      role="img"
      aria-label={t('player.suggested')}
      title={t('player.suggested')}
      data-testid="suggested-mark"
      className="text-primary inline-grid shrink-0 place-items-center"
    >
      <Sparkles aria-hidden size={size} />
    </span>
  );
};
/**
 * The player's sparkle for a song Smart Shuffle added: a button whose menu adds it to the playlist
 * playing (when it's one) or turns it down for this list. Nothing for the list's own songs.
 */
export const SuggestedButton: FC<{ itemId: string | undefined; size?: number }> = ({
  itemId,
  size = 14,
}) => {
  const { t } = useTranslation();
  const suggested = useSuggestedIds();
  const fromPlaylist = usePlaysFromPlaylist();
  if (!itemId || !suggested.has(itemId)) return null;
  const items: MenuItem[] = [
    { kind: 'heading', label: t('player.suggested') },
    ...(fromPlaylist
      ? [
          {
            label: t('player.addToThisPlaylist'),
            icon: ListPlus,
            onSelect: () => void addToThisPlaylist(itemId).catch(() => undefined),
          },
        ]
      : []),
    {
      label: t('player.notForMe'),
      icon: ThumbsDown,
      onSelect: () => void notForMe(itemId).catch(() => undefined),
    },
  ];
  return (
    <button
      type="button"
      aria-label={t('player.suggested')}
      title={t('player.suggested')}
      data-testid="suggested-button"
      onClick={(event) => {
        event.stopPropagation();
        openMenuAt(event.currentTarget, items);
      }}
      className="text-primary grid shrink-0 place-items-center rounded-full p-0.5"
    >
      <Sparkles aria-hidden size={size} />
    </button>
  );
};
