import type { StremioItem } from '@raygun/bindings';
import { useNavigate } from '@tanstack/react-router';
import type { TFunction } from 'i18next';
import { ChevronRight, Layers } from 'lucide-react';
import { memo, useCallback, useMemo, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { AdultMark } from '../shell/AdultMark';
import { openMenuAt } from '../shell/contextMenuStore';
import type { MenuItem } from '../shell/contextMenuStore';
import { SourceTag } from '../shell/SourceTag';
import { StremioCardList } from '../watch/StremioRow';
import type { StremioListCard } from '../watch/StremioRow';
import type { KindSection, MergedTitle } from './videoKinds';

/** Titles a kind's row shows (as many as an addon's row showed); See all shows the rest. */
export const ROW_TITLES = 20;
/** Titles a kind's grid shows at first, and how many more each Show more adds. */
const GRID_PAGE = 48;

/** A kind's heading: Movies, Series, Channels…, or the kind as an addon names it. */
const kindName = (t: TFunction, section: Pick<KindSection, 'key' | 'kind'>) =>
  t(`watch.kind.${section.key}`, {
    defaultValue: section.kind.charAt(0).toUpperCase() + section.kind.slice(1),
  });

const nameOf = (item: StremioItem) => item.name || item.id;

/**
 * Where a title was found: its addon's tag, or with several addons "3 sources", a button whose
 * menu lists them, each opening its own copy (the first is the one the card opens). Hooks stay
 * with the section: dozens of cards draw no listeners of their own.
 */
const TitleSources: FC<{
  title: MergedTitle;
  adult: boolean;
  t: TFunction;
  onOpen: (item: StremioItem) => void;
}> = ({ title, adult, t, onOpen }) => {
  const name = nameOf(title.item);
  const count = title.copies.length;
  const sources = (): MenuItem[] => [
    {
      kind: 'heading',
      label: name,
      subtitle: t('search.sourcesCount', { count }),
      artworkUrl: title.item.poster,
    },
    ...title.copies.map((copy): MenuItem => ({
      label:
        nameOf(copy.item) === name
          ? copy.addonName
          : t('search.video.copyNamed', { source: copy.addonName, title: nameOf(copy.item) }),
      onSelect: () => onOpen(copy.item),
    })),
  ];
  return (
    <span className="flex min-w-0 items-center gap-1">
      {adult && <AdultMark />}
      {count === 1 ? (
        <SourceTag name={title.copies[0]!.addonName} />
      ) : (
        <button
          type="button"
          aria-haspopup="menu"
          aria-label={t('search.video.sourcesOf', { count, title: name })}
          data-testid="title-sources"
          onClick={(event) => {
            // Inside the card's link: only the menu opens.
            event.preventDefault();
            event.stopPropagation();
            openMenuAt(event.currentTarget, sources());
          }}
          // A tag's look, with room around it for a finger.
          className="border-foreground/20 text-muted hover:text-foreground relative inline-flex max-w-full min-w-0 items-center gap-1 rounded-md border px-1.5 py-0.5 align-middle text-[calc(0.6875rem*var(--text-scale,1))] leading-4 after:absolute after:-inset-1 after:content-['']"
        >
          <Layers aria-hidden size={11} className="shrink-0" />
          <span className="truncate">{t('search.sourcesCount', { count })}</span>
        </button>
      )}
    </span>
  );
};

/**
 * One kind's titles from every addon (Videos' search): a row of the best ones, or every one in a
 * grid on the kind's own pill. A title several addons have shows once, saying how many.
 */
const VideoKindSectionView: FC<{
  section: KindSection;
  layout: 'row' | 'grid';
  /** Addons rated for adults: their titles are marked 18+. */
  adult: ReadonlySet<string>;
  /** See all shows: the kind's own pill, or its every title in place. */
  more: boolean;
  onMore: (kind: string) => void;
}> = ({ section, layout, adult, more, onMore }) => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [shownCount, setShownCount] = useState(GRID_PAGE);
  const name = kindName(t, section);
  const onOpen = useCallback(
    (item: StremioItem) =>
      void navigate({ to: '/watch/title', search: { type: item.kind, id: item.id } }),
    [navigate],
  );
  const cap = layout === 'row' ? ROW_TITLES : shownCount;
  // The same cards while the section is the same: its row isn't drawn again for nothing.
  const cards: StremioListCard[] = useMemo(
    () =>
      section.titles.slice(0, cap).map((title) => ({
        key: title.key,
        item: title.item,
        footer: (
          <TitleSources
            title={title}
            adult={title.copies.some((copy) => adult.has(copy.addonId))}
            t={t}
            onOpen={onOpen}
          />
        ),
      })),
    [section, cap, adult, t, onOpen],
  );
  return (
    <section aria-label={name} data-testid="video-kind" className="flex flex-col gap-2">
      <div className="flex items-baseline justify-between gap-2">
        <h2 className="line-clamp-1 min-w-0 font-semibold">{name}</h2>
        {layout === 'row' && more && (
          <button
            type="button"
            onClick={() => onMore(section.key)}
            aria-label={t('watch.seeAllOf', { name })}
            className="text-primary flex shrink-0 items-center gap-0.5 text-sm"
          >
            {t('search.seeAll')}
            <ChevronRight aria-hidden size={16} />
          </button>
        )}
      </div>
      <StremioCardList cards={cards} layout={layout} />
      {layout === 'grid' && section.titles.length > cap && (
        <button
          type="button"
          onClick={() => setShownCount(cap + GRID_PAGE)}
          className="bg-surface self-center rounded-full px-5 py-2 text-sm"
        >
          {t('search.showMore', { count: Math.min(GRID_PAGE, section.titles.length - cap) })}
        </button>
      )}
    </section>
  );
};

/** Drawn again only when its section, layout or marks change. */
export const VideoKindSection = memo(VideoKindSectionView);
