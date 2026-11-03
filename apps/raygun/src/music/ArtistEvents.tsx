import { commands } from '@raygun/bindings';
import { useQuery } from '@tanstack/react-query';
import { CalendarDays } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';

/**
 * An artist's coming concerts and events (M46.42), from open data (MusicBrainz). Hidden when
 * none are known.
 */
export const ArtistEvents: FC<{ name: string }> = ({ name }) => {
  const { t, i18n } = useTranslation();
  const events = useQuery({
    queryKey: ['music', 'artistEvents', name],
    queryFn: () => unwrapResult(commands.musicArtistEvents(name)),
    enabled: name.trim() !== '',
    staleTime: 6 * 60 * 60_000,
  });
  const list = events.data ?? [];
  if (list.length === 0) return null;
  const day = (date: string) => {
    const parsed = new Date(`${date}T12:00:00`);
    return Number.isNaN(parsed.getTime())
      ? date
      : new Intl.DateTimeFormat(i18n.language, { dateStyle: 'medium' }).format(parsed);
  };
  return (
    <section
      aria-label={t('follow.events')}
      data-testid="artist-events"
      className="flex flex-col gap-3"
    >
      <h2 className="text-lg font-bold">{t('follow.events')}</h2>
      <ul className="flex flex-col gap-1">
        {list.map((event) => (
          <li key={`${event.date}-${event.name}`}>
            <button
              type="button"
              disabled={!event.url}
              onClick={() => event.url && void commands.musicOpenInfoPage(event.url)}
              className="hover:bg-surface flex w-full items-center gap-3 rounded-lg p-2 text-left disabled:cursor-default"
            >
              <CalendarDays aria-hidden size={18} className="text-primary shrink-0" />
              <span className="flex min-w-0 flex-col">
                <span className="truncate text-sm font-medium">{event.name}</span>
                <span className="text-muted truncate text-xs">
                  {[day(event.date), event.place, event.source].filter(Boolean).join(' · ')}
                </span>
              </span>
            </button>
          </li>
        ))}
      </ul>
    </section>
  );
};
