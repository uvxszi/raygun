import { commands } from '@raygun/bindings';
import type { MangaCard } from '@raygun/bindings';
import { useMutation, useQueries, useQueryClient } from '@tanstack/react-query';
import { useMemo } from 'react';
import { Link } from '@tanstack/react-router';
import { Check, LoaderCircle, Plus } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { titleScore } from '../search/rankResults';
import { sourceSearchOptions } from '../search/useCatalogSearch';
import { collectFromCache } from '../shell/queryMemory';
import { SourceIssue } from '../shell/SourceIssue';
import { videoTitleScore } from '../watch/videoMatch';
import { MangaCover } from './MangaCover';
import { catalogCommands, ROUTES } from './medium';
import type { Medium } from './medium';
import { useMangaSources } from './useManga';

/** A match is at least this close to the title (search's own scale, 100 = the same). */
const CLOSE_ENOUGH = 60;

/** How close a found title is: a video's other seasons never are ("Overlord II" for "Overlord"). */
const closeness = (medium: Medium, found: string, title: string) =>
  medium === 'video' ? videoTitleScore(found, title, true) : titleScore(found, title);

/** Opens a found title (storing it) and adds it to the library. */
const addToLibrary = async (medium: Medium, card: MangaCard) => {
  if (medium === 'video') {
    const view = await unwrapResult(commands.videoTitle(card.sourceId, card.id, false));
    await unwrapResult(commands.videoSetInLibrary(view.id, true));
    return;
  }
  const catalog = catalogCommands(medium);
  const view = await unwrapResult(catalog.title(card.sourceId, card.id, false));
  await unwrapResult(catalog.setInLibrary(view.id, true));
};

/**
 * The same comic, book or video in the other sources (M46.38): every enabled source is searched
 * for the title and its close matches listed, to open or add to the library — when one source
 * is slow, gone or has more chapters. A video matches only the same season.
 */
export const OtherSources: FC<{ medium: Medium; title: string; sourceId: string }> = ({
  medium,
  title,
  sourceId,
}) => {
  const { t } = useTranslation();
  const sources = useMangaSources(medium);
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
  const found = [
    ...live,
    ...seen
      .filter((card) => !liveKeys.has(`${card.sourceId}:${card.id}`))
      .map((card) => ({ card, sourceName: names.get(card.sourceId) ?? '' })),
  ]
    .map((match) => ({ ...match, score: closeness(medium, match.card.title, title) }))
    .filter((match) => match.score >= CLOSE_ENOUGH && match.sourceName !== '')
    .sort((a, b) => b.score - a.score);
  return (
    <section
      data-testid="other-sources"
      aria-label={t('otherSources.title')}
      className="bg-surface flex flex-col gap-2 rounded-2xl p-3"
    >
      <h2 className="flex items-center gap-2 text-sm font-semibold">
        {t('otherSources.title')}
        {pending && <LoaderCircle aria-hidden size={14} className="text-muted animate-spin" />}
      </h2>
      {!pending && found.length === 0 && (
        <p className="text-muted text-sm">
          {others.length === 0 ? t('otherSources.noSources') : t('otherSources.none')}
        </p>
      )}
      <ul className="flex flex-col">
        {found.map(({ card, sourceName }) => (
          <li key={`${card.sourceId}:${card.id}`}>
            <Match medium={medium} card={card} sourceName={sourceName} />
          </li>
        ))}
      </ul>
    </section>
  );
};

const Match: FC<{ medium: Medium; card: MangaCard; sourceName: string }> = ({
  medium,
  card,
  sourceName,
}) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const [added, setAdded] = useState(card.inLibrary);
  // Adding needs the title stored first: opening it does that.
  const add = useMutation({
    mutationFn: () => addToLibrary(medium, card),
    onSuccess: () => {
      setAdded(true);
      void client.invalidateQueries({ queryKey: [medium, 'library'] });
    },
  });
  return (
    <div className="flex items-center gap-3 py-1.5">
      <Link
        to={ROUTES[medium].title}
        search={{ source: card.sourceId, id: card.id }}
        className="flex min-w-0 flex-1 items-center gap-3"
      >
        <MangaCover
          url={card.coverUrl}
          video={medium === 'video'}
          className="w-10 shrink-0 rounded"
        />
        <span className="flex min-w-0 flex-col">
          <span className="truncate text-sm font-medium">{card.title}</span>
          <span className="text-muted truncate text-xs">{sourceName}</span>
        </span>
      </Link>
      <Link
        to={ROUTES[medium].title}
        search={{ source: card.sourceId, id: card.id }}
        className="bg-background hover:bg-primary/15 shrink-0 rounded-full px-3 py-1.5 text-xs font-semibold"
      >
        {t('otherSources.open')}
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
      {add.error && <SourceIssue>{add.error.message}</SourceIssue>}
    </div>
  );
};
