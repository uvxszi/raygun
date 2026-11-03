import { commands } from '@raygun/bindings';
import type { TitleRating } from '@raygun/bindings';
import { useQuery } from '@tanstack/react-query';
import { ExternalLink, LoaderCircle, Star } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { Sheet } from '../shell/Sheet';

/**
 * A title's stars, saying whose they are (M46.45): "★ 7.8 IMDb". Pressing them lists every
 * rating the addons have (IMDb's and any other site's), each opening its page.
 */
export const TitleRatings: FC<{ kind: string; id: string; ratings: TitleRating[] }> = ({
  kind,
  id,
  ratings: own,
}) => {
  const { t } = useTranslation();
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const all = useQuery({
    queryKey: ['stremio', 'ratings', kind, id],
    queryFn: () => unwrapResult(commands.stremioRatings(kind, id)),
    enabled: anchor !== null,
    staleTime: 60 * 60_000,
  });
  const first = own[0];
  if (!first) return null;
  const list = all.data && all.data.length > 0 ? all.data : own;
  return (
    <>
      <button
        type="button"
        aria-haspopup="dialog"
        aria-label={t('watch.ratingsOf', { value: first.value, source: first.source })}
        onClick={(event) => setAnchor(anchor ? null : event.currentTarget)}
        className="hover:text-foreground inline-flex items-center gap-1 underline-offset-2 hover:underline"
      >
        <Star aria-hidden size={13} className="fill-amber-400 text-amber-400" />
        {first.value} {first.source}
        {own.length > 1 && <span className="text-xs">+{own.length - 1}</span>}
      </button>
      {anchor && (
        <Sheet
          title={t('watch.ratings')}
          anchor={anchor}
          popoverWidth={280}
          onClose={() => setAnchor(null)}
        >
          <ul data-testid="title-ratings" className="-mx-2 flex flex-col">
            {list.map((rating) => (
              <li key={rating.source}>
                <button
                  type="button"
                  disabled={!rating.url}
                  onClick={() => rating.url && void commands.musicOpenInfoPage(rating.url)}
                  className="hover:bg-background flex w-full items-center gap-3 rounded-lg px-3 py-2 text-left text-sm disabled:cursor-default disabled:hover:bg-transparent"
                >
                  <Star aria-hidden size={15} className="shrink-0 fill-amber-400 text-amber-400" />
                  <span className="min-w-0 flex-1 truncate font-medium">{rating.source}</span>
                  <span className="tabular-nums">{rating.value}</span>
                  {rating.url && (
                    <ExternalLink
                      role="img"
                      aria-label={t('watch.openRating', { source: rating.source })}
                      size={14}
                      className="text-muted shrink-0"
                    />
                  )}
                </button>
              </li>
            ))}
          </ul>
          {all.isFetching && (
            <p className="text-muted flex items-center gap-2 pt-2 text-xs">
              <LoaderCircle aria-hidden size={12} className="animate-spin" />
              {t('watch.ratingsLoading')}
            </p>
          )}
        </Sheet>
      )}
    </>
  );
};
