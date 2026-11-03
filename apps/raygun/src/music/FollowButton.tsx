import { Bell, BellRing, Check, UserPlus } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { sameArtist, useFollowActions, useFollowedArtists } from './follows';

/**
 * Follow an artist (M46.42): into the library and quick access, and out again. Following shows
 * the bell, which turns on alerts for new releases.
 */
export const FollowButton: FC<{ name: string; imageUrl: string | null }> = ({ name, imageUrl }) => {
  const { t } = useTranslation();
  const followed = useFollowedArtists();
  const actions = useFollowActions();
  const mine = followed.data?.find((artist) => sameArtist(artist.name, name));
  const busy = actions.follow.isPending || actions.unfollow.isPending;
  return (
    <div className="flex items-center gap-2">
      <button
        type="button"
        aria-pressed={Boolean(mine)}
        disabled={busy || !followed.data}
        onClick={() =>
          mine ? actions.unfollow.mutate(mine.name) : actions.follow.mutate({ name, imageUrl })
        }
        className={`flex items-center gap-2 rounded-full border px-5 py-2 text-sm font-semibold disabled:opacity-60 ${mine ? 'border-primary text-primary' : 'border-foreground/30'}`}
      >
        {mine ? <Check aria-hidden size={16} /> : <UserPlus aria-hidden size={16} />}
        {t(mine ? 'follow.following' : 'follow.follow')}
      </button>
      {mine && (
        <button
          type="button"
          aria-pressed={mine.alerts}
          aria-label={t(mine.alerts ? 'follow.alertsOff' : 'follow.alertsOn', { name: mine.name })}
          title={t(mine.alerts ? 'follow.alertsOff' : 'follow.alertsOn', { name: mine.name })}
          disabled={actions.alerts.isPending}
          onClick={() => actions.alerts.mutate({ name: mine.name, on: !mine.alerts })}
          className={`grid size-10 place-items-center rounded-full ${mine.alerts ? 'bg-primary/15 text-primary' : 'bg-surface text-muted'}`}
        >
          {mine.alerts ? <BellRing aria-hidden size={18} /> : <Bell aria-hidden size={18} />}
        </button>
      )}
    </div>
  );
};
