import { commands } from '@raygun/bindings';
import type { MangaCard, MangaTitleView, VideoTitleView } from '@raygun/bindings';
import { useMutation, useQueries, useQueryClient } from '@tanstack/react-query';
import type { QueryClient } from '@tanstack/react-query';
import { useMemo, useState } from 'react';
import { Link, useNavigate } from '@tanstack/react-router';
import {
  ArrowRightLeft,
  BookOpen,
  Check,
  FolderPlus,
  Heart,
  LoaderCircle,
  Plus,
} from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { titleScore } from '../search/rankResults';
import { sourceSearchOptions } from '../search/useCatalogSearch';
import type { MenuItem } from '../shell/contextMenuStore';
import { openContextMenu } from '../shell/contextMenuStore';
import { GroupPickSheet } from '../select/GroupPickSheet';
import { MenuButton } from '../shell/MenuButton';
import { collectFromCache } from '../shell/queryMemory';
import { SourceIssue } from '../shell/SourceIssue';
import { videoTitleScore } from '../watch/videoMatch';
import { groupWords } from './groupWords';
import { MangaCover } from './MangaCover';
import { createLimiter, formatNumber, mangaSize, videoSize } from './matchSize';
import type { MatchSize } from './matchSize';
import { catalogCommands, ROUTES } from './medium';
import type { Medium } from './medium';
import { useMangaSources } from './useManga';

/** A match is at least this close to the title (search's own scale, 100 = the same). */
const CLOSE_ENOUGH = 60;
/** The closest matches whose chapters or episodes are fetched to compare them. */
const DETAILED = 12;
/** Title pages fetched at once: other sources are often slow, and a phone is shared. */
const limit = createLimiter(3);
/** A match's details stay fresh this long (they are the title page's own query). */
const DETAILS_STALE_MS = 10 * 60_000;

/** How close a found title is: a video's other seasons never are ("Overlord II" for "Overlord"). */
const closeness = (medium: Medium, found: string, title: string) =>
  medium === 'video' ? videoTitleScore(found, title, true) : titleScore(found, title);

/** The title page's query (`useMangaTitle`, `useVideoTitle`), so opening a match is instant. */
const detailsQuery = (medium: Medium, card: MangaCard) => ({
  queryKey: [medium, 'title', card.sourceId, card.id],
  queryFn: (): Promise<MangaTitleView | VideoTitleView> =>
    limit<MangaTitleView | VideoTitleView>(() =>
      medium === 'video'
        ? unwrapResult(commands.videoTitle(card.sourceId, card.id, false))
        : unwrapResult(catalogCommands(medium).title(card.sourceId, card.id, false)),
    ),
  staleTime: DETAILS_STALE_MS,
  retry: false,
});

/** Stores a found title (opening it does that) and returns its stored id. */
const storeTitle = async (client: QueryClient, medium: Medium, card: MangaCard) => {
  const view = await client.fetchQuery(detailsQuery(medium, card));
  return view.id;
};

/** Stores a found title and adds it to the library. */
const addToLibrary = async (client: QueryClient, medium: Medium, card: MangaCard) => {
  const id = await storeTitle(client, medium, card);
  await unwrapResult(
    medium === 'video'
      ? commands.videoSetInLibrary(id, true)
      : catalogCommands(medium).setInLibrary(id, true),
  );
  return id;
};

const refreshLibrary = (client: QueryClient, medium: Medium) =>
  Promise.all([
    client.invalidateQueries({ queryKey: [medium, 'library'] }),
    client.invalidateQueries({ queryKey: [medium, 'title'] }),
    client.invalidateQueries({ queryKey: [medium, 'categories'] }),
  ]);

type Found = { card: MangaCard; sourceName: string; score: number; size: MatchSize | null };

type Order = 'closest' | 'most';

/**
 * The same comic, book or video in the other sources (M46.38): every enabled source is searched
 * for the title and its close matches listed with how much each has (chapters or episodes, the
 * latest one and its date, seasons, length), to open, add to the library, or move the library
 * entry onto. A video matches only the same season.
 */
export const OtherSources: FC<{
  medium: Medium;
  title: string;
  sourceId: string;
  /** The title shown, stored; with `inLibrary`, it can move to a match's source. */
  titleId?: string;
  inLibrary?: boolean;
}> = ({ medium, title, sourceId, titleId, inLibrary = false }) => {
  const { t } = useTranslation();
  const sources = useMangaSources(medium);
  const [order, setOrder] = useState<Order>('closest');
  // A video site (its own Search pill: YouTube, PeerTube) holds videos, not a catalogue's titles.
  const others = (sources.data ?? []).filter(
    (source) => source.enabled && source.id !== sourceId && !source.searchPill,
  );
  const results = useQueries({
    queries: others.map((source) => sourceSearchOptions(medium, source.id, title)),
  });
  const pending = !sources.data || results.some((result) => result.isPending);
  const client = useQueryClient();
  // What earlier searches and source pages already showed (M46.40), at once; the searches
  // below then add and replace.
  const seen = useMemo(
    () =>
      collectFromCache(
        client,
        [medium],
        (record): MangaCard | null => {
          if (
            typeof record.sourceId !== 'string' ||
            record.sourceId === sourceId ||
            typeof record.title !== 'string' ||
            !('inLibrary' in record)
          ) {
            return null;
          }
          // A title's page carries the stored title's id; its card is by the source's.
          const id = typeof record.remoteId === 'string' ? record.remoteId : record.id;
          return typeof id === 'string'
            ? {
                sourceId: record.sourceId,
                id,
                title: record.title,
                coverUrl: typeof record.coverUrl === 'string' ? record.coverUrl : null,
                inLibrary: record.inLibrary === true,
              }
            : null;
        },
        (card) => `${card.sourceId}:${card.id}`,
      ),
    [client, medium, sourceId],
  );
  const names = new Map((sources.data ?? []).map((source) => [source.id, source.name]));
  const live = results.flatMap((result) =>
    (result.data?.items ?? []).map((card) => ({
      card,
      sourceName: result.data?.source.name ?? '',
    })),
  );
  const liveKeys = new Set(live.map(({ card }) => `${card.sourceId}:${card.id}`));
  const close = [
    ...live,
    ...seen
      .filter((card) => !liveKeys.has(`${card.sourceId}:${card.id}`))
      .map((card) => ({ card, sourceName: names.get(card.sourceId) ?? '' })),
  ]
    .map((match) => ({ ...match, score: closeness(medium, match.card.title, title) }))
    .filter((match) => match.score >= CLOSE_ENOUGH && match.sourceName !== '')
    .sort((a, b) => b.score - a.score);
  // How much each of the closest has: their title pages, a few at a time, quiet when they fail.
  const details = useQueries({
    queries: close.slice(0, DETAILED).map(({ card }) => detailsQuery(medium, card)),
  });
  const found: Found[] = close.map((match, index) => {
    const view = details[index]?.data;
    const size = view ? ('episodes' in view ? videoSize(view) : mangaSize(view)) : null;
    return { ...match, size };
  });
  const loadingAt = (index: number) => index < DETAILED && details[index]?.isPending === true;
  const most = Math.max(0, ...found.map((match) => match.size?.units ?? 0));
  // The one with the most is marked only when another has fewer.
  const mostKey =
    most > 0 && found.some((match) => (match.size?.units ?? most) < most)
      ? found.find((match) => match.size?.units === most)
      : undefined;
  const shown =
    order === 'most'
      ? [...found].sort((a, b) => (b.size?.units ?? -1) - (a.size?.units ?? -1))
      : found;
  const mostLabel = t(
    medium === 'video' ? 'otherSources.mostEpisodes' : 'otherSources.mostChapters',
  );
  return (
    <section
      data-testid="other-sources"
      aria-label={t('otherSources.title')}
      className="bg-surface flex flex-col gap-2 rounded-2xl p-3"
    >
      <div className="flex flex-wrap items-center gap-2">
        <h2 className="flex flex-1 items-center gap-2 text-sm font-semibold">
          {t('otherSources.title')}
          {pending && <LoaderCircle aria-hidden size={14} className="text-muted animate-spin" />}
        </h2>
        {found.length > 1 && (
          <div role="radiogroup" aria-label={t('otherSources.sortBy')} className="flex gap-1">
            {(['closest', 'most'] as const).map((value) => (
              <button
                key={value}
                type="button"
                role="radio"
                aria-checked={order === value}
                onClick={() => setOrder(value)}
                className={`rounded-full px-3 py-1 text-xs font-semibold ${order === value ? 'bg-primary text-background' : 'bg-background hover:bg-primary/15'}`}
              >
                {value === 'closest' ? t('otherSources.closest') : mostLabel}
              </button>
            ))}
          </div>
        )}
      </div>
      {!pending && found.length === 0 && (
        <p className="text-muted text-sm">
          {others.length === 0 ? t('otherSources.noSources') : t('otherSources.none')}
        </p>
      )}
      <ul className="flex flex-col">
        {shown.map((match) => (
          <li key={`${match.card.sourceId}:${match.card.id}`}>
            <Match
              medium={medium}
              match={match}
              loading={loadingAt(found.indexOf(match))}
              most={match === mostKey ? mostLabel : null}
              migrateFrom={inLibrary && titleId ? titleId : null}
            />
          </li>
        ))}
      </ul>
    </section>
  );
};

/** How much a match has, as chips. */
const SizeChips: FC<{ medium: Medium; size: MatchSize; most: string | null }> = ({
  medium,
  size,
  most,
}) => {
  const { t, i18n } = useTranslation();
  const video = medium === 'video';
  const chips: string[] = [];
  if (size.units !== null) {
    chips.push(t(video ? 'otherSources.episodes' : 'otherSources.chapters', { count: size.units }));
  }
  if (size.seasons !== null) {
    chips.push(t('otherSources.seasons', { count: size.seasons }));
  }
  if (size.latest !== null) {
    const number = formatNumber(size.latest);
    chips.push(
      size.latestSeason !== null
        ? t('otherSources.latestSeasonEpisode', { season: size.latestSeason, number })
        : t(video ? 'otherSources.latestEpisode' : 'otherSources.latestChapter', { number }),
    );
  }
  if (size.latestDate !== null) {
    chips.push(
      new Intl.DateTimeFormat(i18n.language, { dateStyle: 'medium' }).format(size.latestDate),
    );
  }
  if (size.durationMs !== null) {
    const minutes = Math.round(size.durationMs / 60_000);
    chips.push(
      minutes >= 60
        ? t('otherSources.hours', { hours: Math.floor(minutes / 60), minutes: minutes % 60 })
        : t('otherSources.minutes', { minutes }),
    );
  }
  if (chips.length === 0 && !most) {
    return null;
  }
  return (
    <span className="flex flex-wrap gap-1" data-testid="match-size">
      {most && (
        <span className="bg-primary/15 text-primary rounded-full px-2 py-0.5 text-2xs font-semibold">
          {most}
        </span>
      )}
      {chips.map((chip) => (
        <span key={chip} className="bg-background text-muted rounded-full px-2 py-0.5 text-2xs">
          {chip}
        </span>
      ))}
    </span>
  );
};

/** Chips still coming in: a soft pulse, still where motion is reduced. */
const SizeShimmer: FC = () => {
  const { t } = useTranslation();
  return (
    <span
      role="status"
      aria-label={t('otherSources.loadingSize')}
      className="flex gap-1"
      data-testid="match-size-loading"
    >
      {[16, 12, 20].map((width) => (
        <span
          key={width}
          className="bg-background h-4 animate-pulse rounded-full motion-reduce:animate-none"
          style={{ width: `${width / 4}rem` }}
        />
      ))}
    </span>
  );
};

const Match: FC<{
  medium: Medium;
  match: Found;
  loading: boolean;
  most: string | null;
  /** The library title this match can replace. */
  migrateFrom: string | null;
}> = ({ medium, match, loading, most, migrateFrom }) => {
  const { card, sourceName, size } = match;
  const { t } = useTranslation();
  const client = useQueryClient();
  const navigate = useNavigate();
  const [added, setAdded] = useState(card.inLibrary);
  const [picking, setPicking] = useState(false);
  const route = { to: ROUTES[medium].title, search: { source: card.sourceId, id: card.id } };
  const add = useMutation({
    mutationFn: () => addToLibrary(client, medium, card),
    onSuccess: () => {
      setAdded(true);
      void refreshLibrary(client, medium);
    },
  });
  const favorite = useMutation({
    mutationFn: async () => {
      const id = await storeTitle(client, medium, card);
      // A favorite joins the library too.
      await unwrapResult(commands.librarySetFavorite(medium, [id], true));
    },
    onSuccess: () => {
      setAdded(true);
      void refreshLibrary(client, medium);
    },
  });
  const migrate = useMutation({
    mutationFn: (from: string) =>
      unwrapResult(commands.libraryMigrate(medium, from, card.sourceId, card.id)),
    onSuccess: async () => {
      await refreshLibrary(client, medium);
      void navigate(route as never);
    },
  });
  const error = add.error ?? favorite.error ?? migrate.error;
  const items = (): MenuItem[] => [
    { kind: 'heading', label: card.title, subtitle: sourceName, artworkUrl: card.coverUrl },
    { label: t('menu.open'), icon: BookOpen, onSelect: () => void navigate(route as never) },
    {
      label: t(added ? 'manga.inLibrary' : 'otherSources.addToLibrary'),
      icon: added ? Check : Plus,
      disabled: added || add.isPending,
      onSelect: () => add.mutate(),
    },
    {
      label: t('library.categories.addTo', groupWords(medium)),
      icon: FolderPlus,
      onSelect: () => setPicking(true),
    },
    {
      label: t('otherSources.favorite'),
      icon: Heart,
      disabled: favorite.isPending,
      onSelect: () => favorite.mutate(),
    },
    ...(migrateFrom
      ? [
          { kind: 'separator' as const },
          {
            label: t('otherSources.useInstead'),
            icon: ArrowRightLeft,
            disabled: migrate.isPending,
            confirm: t('otherSources.useInsteadConfirm', { source: sourceName }),
            onSelect: () => migrate.mutate(migrateFrom),
          },
        ]
      : []),
  ];
  return (
    <div
      className="flex flex-col gap-1 py-1.5"
      onContextMenu={(event) => openContextMenu(event, items())}
    >
      <div className="flex items-center gap-3">
        <Link {...route} className="flex min-w-0 flex-1 items-center gap-3">
          <MangaCover
            url={card.coverUrl}
            video={medium === 'video'}
            className="w-10 shrink-0 rounded"
          />
          <span className="flex min-w-0 flex-col gap-0.5">
            <span className="truncate text-sm font-medium">{card.title}</span>
            <span className="text-muted truncate text-xs">{sourceName}</span>
            {loading ? (
              <SizeShimmer />
            ) : (
              size && <SizeChips medium={medium} size={size} most={most} />
            )}
          </span>
        </Link>
        <button
          type="button"
          disabled={added || add.isPending}
          onClick={() => add.mutate()}
          aria-label={t(added ? 'otherSources.added' : 'otherSources.add', { source: sourceName })}
          className="bg-background hover:bg-primary/15 flex shrink-0 items-center gap-1 rounded-full px-3 py-1.5 text-xs font-semibold disabled:opacity-60"
        >
          {added ? <Check aria-hidden size={14} /> : <Plus aria-hidden size={14} />}
          {t(added ? 'manga.inLibrary' : 'otherSources.addShort')}
        </button>
        <MenuButton label={t('otherSources.more', { source: sourceName })} items={items} />
      </div>
      {error && (
        <SourceIssue compact source={sourceName}>
          {error.message}
        </SourceIssue>
      )}
      {picking && (
        <GroupPickSheet
          medium={medium}
          titles={[
            {
              kind: 'title',
              medium,
              sourceId: card.sourceId,
              remoteId: card.id,
              title: card.title,
            },
          ]}
          onDone={() => {
            setAdded(true);
            setPicking(false);
          }}
          onClose={() => setPicking(false)}
        />
      )}
    </div>
  );
};
