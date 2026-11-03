import type { ListeningStats } from '@raygun/bindings';
import { Clock, Headphones, LoaderCircle, Mic2, Music2, RefreshCw, Sparkles } from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useMixCardMenu } from '../music/cardMenus';
import { MixCard } from '../music/MixCard';
import { SHELVES, useMadeForYou, useRefreshMadeForYou } from '../music/useMadeForYou';
import { Carousel } from '../shell/Carousel';
import { EmptyState } from '../shell/EmptyState';
import { SourceIssue } from '../shell/SourceIssue';
import { TopBar, TopBarAction } from '../shell/TopBar';

const StatCard: FC<{ icon: LucideIcon; label: string; value: string }> = ({
  icon: Icon,
  label,
  value,
}) => (
  <li className="bg-surface flex min-w-0 flex-col gap-1 rounded-2xl px-4 py-3">
    <span className="text-muted flex items-center gap-1.5 text-xs font-semibold uppercase">
      <Icon aria-hidden size={14} />
      {label}
    </span>
    <span className="truncate text-lg font-bold">{value}</span>
  </li>
);

/** QuietPlay's numbers: this week's listening, this month's top artist and song, the peak hour. */
const Stats: FC<{ stats: ListeningStats }> = ({ stats }) => {
  const { t, i18n } = useTranslation();
  const hour =
    stats.peakHour === null
      ? null
      : new Intl.DateTimeFormat(i18n.language, { hour: 'numeric' }).format(
          new Date(2000, 0, 1, stats.peakHour),
        );
  return (
    <section aria-label={t('forYou.stats.title')} className="flex flex-col gap-3">
      <h2 className="text-xl font-bold">{t('forYou.stats.title')}</h2>
      <ul className="grid grid-cols-2 gap-3 lg:grid-cols-4">
        <StatCard
          icon={Clock}
          label={t('forYou.stats.week')}
          value={t('forYou.stats.minutes', { count: stats.minutesThisWeek })}
        />
        <StatCard icon={Mic2} label={t('forYou.stats.topArtist')} value={stats.topArtist ?? '—'} />
        <StatCard
          icon={Music2}
          label={t('forYou.stats.topSong')}
          value={stats.topSong ? stats.topSong.title : '—'}
        />
        <StatCard icon={Headphones} label={t('forYou.stats.peak')} value={hour ?? '—'} />
      </ul>
    </section>
  );
};

/**
 * Made for You (M46.20), as QuietPlay and Spotify have it: Daily Mixes, Discover Weekly, Release
 * Radar, On Repeat, a mix for this time of day, Repeat Rewind, "Because you listened to", artist
 * radios, genre and mood mixes, and the user's listening numbers. Made each day from what the
 * user plays and saves.
 */
export const MadeForYouPage: FC = () => {
  const { t } = useTranslation();
  const overview = useMadeForYou();
  const refresh = useRefreshMadeForYou();
  const mixMenu = useMixCardMenu();
  const data = overview.data;
  const mixes = data?.mixes ?? [];
  const making = (data?.making ?? false) || refresh.isPending;

  return (
    <section data-testid="made-for-you-page" className="flex flex-col gap-8">
      <TopBar
        back
        fallback="/"
        title={t('forYou.title')}
        actions={
          <TopBarAction
            label={t('forYou.refresh')}
            icon={RefreshCw}
            onClick={() => refresh.mutate()}
          />
        }
      />
      {making && (
        <p role="status" className="text-muted flex items-center gap-2 text-sm">
          <LoaderCircle aria-hidden size={16} className="animate-spin" />
          {mixes.length === 0 ? t('forYou.making') : t('forYou.remaking')}
        </p>
      )}
      {overview.error && <SourceIssue>{overview.error.message}</SourceIssue>}
      {data && !making && mixes.length === 0 && (
        <EmptyState
          icon={Sparkles}
          title={t('forYou.emptyTitle')}
          body={t('forYou.emptyBody')}
          action={{ label: t('forYou.browse'), to: '/browse' }}
        />
      )}
      {SHELVES.map((shelf) => {
        const shown = mixes.filter((mix) => shelf.kinds.includes(mix.kind));
        if (shown.length === 0) return null;
        return (
          <section
            key={shelf.id}
            aria-label={t(`forYou.shelves.${shelf.id}`)}
            className="flex flex-col gap-2"
          >
            <h2 className="text-xl font-bold">{t(`forYou.shelves.${shelf.id}`)}</h2>
            <Carousel className="-mx-4 gap-1 px-2 lg:-mx-2">
              {shown.map((mix) => (
                <MixCard key={mix.id} mix={mix} onMenu={mixMenu(mix)} />
              ))}
            </Carousel>
          </section>
        );
      })}
      {data && data.stats.songsKnown > 0 && <Stats stats={data.stats} />}
    </section>
  );
};
