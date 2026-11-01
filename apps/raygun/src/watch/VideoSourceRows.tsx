import type { MangaSource } from '@raygun/bindings';
import { Link } from '@tanstack/react-router';
import { ChevronRight } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { StremioPoster } from './StremioRow';
import { useVideoSourceRows, useVideoSources } from './useVideoSources';

/** A row per video source on the Watch home: its popular titles, and its whole catalogue. */
export const VideoSourceRows: FC = () => {
  const { t } = useTranslation();
  const sources = useVideoSources();
  const list: MangaSource[] = sources.data ?? [];
  const rows = useVideoSourceRows(list.map((source) => source.id));

  return list.map((source, index) => {
    const row = rows[index];
    const title = `${t('manga.list.popular')} · ${source.name}`;
    return (
      <section key={source.id} aria-label={title} className="flex flex-col gap-2">
        <div className="flex items-baseline justify-between gap-2">
          <h2 className="min-w-0 font-semibold">
            <span className="line-clamp-1">{title}</span>
          </h2>
          <Link
            to="/watch/source"
            search={{ id: source.id }}
            aria-label={t('watch.seeAllOf', { name: source.name })}
            className="text-primary flex shrink-0 items-center gap-0.5 text-sm"
          >
            {t('search.seeAll')}
            <ChevronRight aria-hidden size={16} />
          </Link>
        </div>
        {row?.error ? (
          <p className="text-muted text-sm wrap-anywhere">
            {t('watch.catalogFailed', { message: row.error.message })}
          </p>
        ) : row?.data?.items.length === 0 ? (
          <p className="text-muted text-sm">{t('watch.catalogEmpty')}</p>
        ) : (
          <ul className="-mx-4 flex gap-3 overflow-x-auto px-4 lg:mx-0 lg:px-0">
            {row?.data?.items.slice(0, 20).map((item) => (
              <li key={item.id} className="w-28 shrink-0">
                <Link
                  to="/watch/video"
                  search={{ source: item.sourceId, id: item.id }}
                  aria-label={item.title}
                  className="flex flex-col gap-1"
                >
                  <StremioPoster url={item.coverUrl} />
                  <span className="line-clamp-2 text-sm font-medium">{item.title}</span>
                </Link>
              </li>
            ))}
          </ul>
        )}
      </section>
    );
  });
};
