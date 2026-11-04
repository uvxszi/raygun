import type { StremioCatalogRow, StremioItem } from '@raygun/bindings';
import { Link } from '@tanstack/react-router';
import { ChevronRight, Clapperboard } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';
import { Carousel } from '../shell/Carousel';
import { SourceIssue } from '../shell/SourceIssue';

const ASPECT: Record<string, string> = {
  landscape: 'aspect-video',
  square: 'aspect-square',
};

/** A poster in the shape its catalog asks for, or a placeholder (also when it fails to load);
 * full width unless sized. */
export const StremioPoster: FC<{ url: string | null; shape?: string; className?: string }> = ({
  url,
  shape = 'poster',
  className = 'w-full',
}) => {
  const [failed, setFailed] = useState<string | null>(null);
  const aspect = ASPECT[shape] ?? 'aspect-[2/3]';
  return url && failed !== url ? (
    <img
      src={url}
      alt=""
      loading="lazy"
      onError={() => setFailed(url)}
      className={`bg-surface rounded-lg object-cover ${aspect} ${className}`}
    />
  ) : (
    <span
      aria-hidden
      className={`bg-surface text-muted grid place-items-center rounded-lg ${aspect} ${className}`}
    >
      <Clapperboard size={28} />
    </span>
  );
};

/** A movie, series or channel, opening its page. */
export const StremioCard: FC<{ item: StremioItem; className?: string }> = ({
  item,
  className = '',
}) => (
  <Link
    to="/watch/title"
    search={{ type: item.kind, id: item.id }}
    aria-label={item.name}
    className={`flex flex-col gap-1 ${className}`}
  >
    <StremioPoster url={item.poster} shape={item.posterShape} />
    <span className="line-clamp-2 text-sm font-medium">{item.name}</span>
    {(item.releaseInfo || item.rating) && (
      <span className="text-muted line-clamp-1 text-xs">
        {[item.releaseInfo, item.rating && `★ ${item.rating}`].filter(Boolean).join(' · ')}
      </span>
    )}
  </Link>
);

/** One catalog's first page as a sideways row; `browse` leads to the whole catalog. */
export const StremioRow: FC<{ row: StremioCatalogRow; browse?: boolean }> = ({
  row,
  browse = true,
}) => {
  const { t } = useTranslation();
  const kind = t(`watch.kind.${row.kind}`, { defaultValue: row.kind });
  // Search results come from each addon's search catalog, whose own name says nothing.
  const title = browse ? `${row.name} · ${kind}` : kind;
  return (
    <section aria-label={title} className="flex flex-col gap-2">
      <div className="flex items-baseline justify-between gap-2">
        <h2 className="min-w-0 font-semibold">
          <span className="line-clamp-1">{title}</span>
          <span className="text-muted block text-xs font-normal">{row.addonName}</span>
        </h2>
        {browse && (
          <Link
            to="/watch/catalog"
            search={{ addon: row.addonId, type: row.kind, catalog: row.catalogId }}
            aria-label={t('watch.seeAllOf', { name: title })}
            className="text-primary flex shrink-0 items-center gap-0.5 text-sm"
          >
            {t('search.seeAll')}
            <ChevronRight aria-hidden size={16} />
          </Link>
        )}
      </div>
      {row.error ? (
        <SourceIssue compact source={row.addonName}>
          {row.error}
        </SourceIssue>
      ) : row.items.length === 0 ? (
        <p className="text-muted text-sm">{t('watch.catalogEmpty')}</p>
      ) : (
        <Carousel className="-mx-4 gap-3 px-4 lg:mx-0 lg:px-0">
          {row.items.slice(0, 20).map((item) => (
            <li
              key={item.id}
              className={item.posterShape === 'landscape' ? 'w-48 shrink-0' : 'w-28 shrink-0'}
            >
              <StremioCard item={item} />
            </li>
          ))}
        </Carousel>
      )}
    </section>
  );
};
