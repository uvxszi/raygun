import { commands } from '@raygun/bindings';
import type {
  AudiobookResult,
  MangaCard,
  MangaSource,
  MangaTitleView,
  PodcastSearchResult,
  VideoTitleView,
} from '@raygun/bindings';
import { useMutation, useQueries, useQuery, useQueryClient } from '@tanstack/react-query';
import type { QueryClient } from '@tanstack/react-query';
import { useEffect, useMemo, useState } from 'react';
import { useNavigate } from '@tanstack/react-router';
import {
  ArrowRightLeft,
  BookOpen,
  Check,
  FolderPlus,
  Heart,
  LoaderCircle,
  Plus,
  Search,
  SearchCheck,
} from 'lucide-react';
import type { TFunction } from 'i18next';
import type { FC, FormEvent, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { useOpenAudiobook } from '../audiobooks/useAudiobooks';
import { useExtensions, useKnownCatalog } from '../extensions/useExtensions';
import { LANGUAGES, nativeName } from '../i18n/languages';
import { unwrapResult } from '../lib/commandResult';
import { PodcastArt } from '../podcasts/PodcastArt';
import { formatLength, runtimeMs } from '../podcasts/formatLength';
import { useOpenFeed, useSetSubscribed } from '../podcasts/usePodcasts';
import { gridSources } from '../search/customGrid';
import { titleScore } from '../search/rankResults';
import { sourceSearchOptions } from '../search/useCatalogSearch';
import type { MenuItem } from '../shell/contextMenuStore';
import { GroupPickSheet } from '../select/GroupPickSheet';
import { MenuButton } from '../shell/MenuButton';
import { OnOffSwitch } from '../shell/OnOffSwitch';
import { collectFromCache } from '../shell/queryMemory';
import { SourceIssue } from '../shell/SourceIssue';
import { TextField } from '../shell/TextField';
import { PoweredByGoogle, TranslatorSelect } from '../translate/TranslatorSelect';
import { isGoogle, useTranslatingProviders } from '../translate/translators';
import { StremioMatchList } from '../watch/StremioMatchList';
import { useStremioMatches } from '../watch/stremioMatches';
import { videoTitleScore } from '../watch/videoMatch';
import { groupWords } from './groupWords';
import { authorsIn, bookWords, withAuthors } from './bookWords';
import { MangaCover } from './MangaCover';
import { MATCH_ACTION, MatchRow } from './MatchRow';
import { createLimiter, formatNumber, mangaSize, videoSize } from './matchSize';
import type { MatchSize } from './matchSize';
import { catalogCommands, ROUTES } from './medium';
import type { Medium } from './medium';
import { authorWeight, hostOf, languageFits, languagesOf, sourceFits } from './otherSourcesMatch';
import { useMangaSources } from './useManga';

/** A match is at least this close to the title (search's own scale, 100 = the same). */
const CLOSE_ENOUGH = 60;
/** The closest matches whose chapters or episodes are fetched to compare them. */
const DETAILED = 12;
/** Title pages fetched at once: other sources are often slow, and a phone is shared. */
const limit = createLimiter(3);
/** A match's details stay fresh this long (they are the title page's own query). */
const DETAILS_STALE_MS = 10 * 60_000;
/** A directory's answer stays fresh this long. */
const DIRECTORY_STALE_MS = 10 * 60_000;
/** The fields search again this long after typing stops (Enter searches at once). */
const TYPING_MS = 700;

const FIELD = 'bg-background rounded-xl pl-3 pr-1';
const FIELD_INPUT = 'py-2 text-sm';
const SELECT = 'bg-background min-w-0 rounded-xl px-3 py-2 text-sm outline-none';

/** What Find in other sources looks through: a catalogue's titles, or a directory's books or
 * shows. */
export type FindMedium = Medium | 'audiobook' | 'podcast';

/** The button that shows (and hides) Find in other sources on a title's page. */
export const FindInOtherSourcesButton: FC<{ open: boolean; onToggle: () => void }> = ({
  open,
  onToggle,
}) => {
  const { t } = useTranslation();
  return (
    <button
      type="button"
      aria-expanded={open}
      onClick={onToggle}
      className="bg-surface hover:bg-primary/15 flex items-center gap-2 self-start rounded-full px-4 py-2 text-sm font-medium"
    >
      <SearchCheck aria-hidden size={16} />
      {t('otherSources.find')}
    </button>
  );
};

/** `value` once it has stayed the same for `ms`; `now` sets it at once. */
const useSettled = (value: string, ms: number) => {
  const [settled, setSettled] = useState(value);
  useEffect(() => {
    const timer = setTimeout(() => setSettled(value), ms);
    return () => clearTimeout(timer);
  }, [value, ms]);
  return { settled, now: () => setSettled(value) };
};

/**
 * What's searched for: the title and author as typed (prefilled, searched again once typing
 * stops), the language the other sources should be in, and the title translated into it when
 * asked (with the translator picked, else the last one picked for translating).
 */
const useFindQuery = (title: string, author: string) => {
  const [typedTitle, setTypedTitle] = useState(title);
  const [typedAuthor, setTypedAuthor] = useState(author);
  const asked = useSettled(typedTitle.trim(), TYPING_MS);
  const byline = useSettled(typedAuthor.trim(), TYPING_MS);
  const [language, setLanguage] = useState('');
  const [translating, setTranslating] = useState(false);
  const [pickedTranslator, setTranslator] = useState<string | null>(null);
  const lastChoice = useQuery({
    queryKey: ['textTranslate', 'choice', ''],
    queryFn: () => unwrapResult(commands.textTranslateChoice(null)),
    staleTime: 60_000,
  });
  const translator = pickedTranslator ?? lastChoice.data?.translator ?? '';
  const wantsTranslation = translating && language !== '' && asked.settled !== '';
  const translated = useQuery({
    queryKey: ['textTranslate', asked.settled, '', language, translator, ''],
    queryFn: () =>
      unwrapResult(
        commands.textTranslate({
          text: asked.settled,
          from: '',
          to: language,
          translator,
          hint: '',
        }),
      ),
    enabled: wantsTranslation,
    retry: false,
    staleTime: 10 * 60_000,
  });
  // A translation still coming searches nothing yet.
  const query = wantsTranslation ? (translated.data?.text.trim() ?? '') : asked.settled;
  return {
    typedTitle,
    setTypedTitle,
    typedAuthor,
    setTypedAuthor,
    /** Searches what's typed now, without waiting. */
    searchNow: () => {
      asked.now();
      byline.now();
    },
    byline: byline.settled,
    language,
    setLanguage,
    translating,
    setTranslating,
    translator,
    setTranslator,
    wantsTranslation,
    translated,
    query,
  };
};

type FindQuery = ReturnType<typeof useFindQuery>;

/**
 * The search's fields: the title and (for written works and shows) the author, both editable;
 * the language the other sources should be in (`languages`: those the sources have), and
 * whether the title is translated into it, by which translator.
 */
const FindForm: FC<{
  find: FindQuery;
  languages: readonly string[];
  showAuthor: boolean;
  /** Under the language: how many sources it searches. */
  note?: ReactNode;
}> = ({ find, languages, showAuthor, note }) => {
  const { t } = useTranslation();
  const providers = useTranslatingProviders();
  const submit = (event: FormEvent) => {
    event.preventDefault();
    find.searchNow();
  };
  // The one picked stays listed, though the sources turn out to have none in it.
  const options = [...new Set([...languages, ...(find.language ? [find.language] : [])])].sort(
    (one, other) => nativeName(one).localeCompare(nativeName(other)),
  );
  const result = find.translated.data;
  const google = isGoogle(find.translator, providers);
  // Google's terms name the action that asks it: "Translate with Google".
  const translateLabel = t(
    google ? 'otherSources.translateTitleWithGoogle' : 'otherSources.translateTitle',
    { language: nativeName(find.language || 'en') },
  );
  return (
    <form
      onSubmit={submit}
      role="search"
      aria-label={t('otherSources.searchLabel')}
      className="flex flex-col gap-2"
    >
      <div className="flex flex-wrap gap-2">
        <label className="flex min-w-[12rem] flex-[2] flex-col gap-1">
          <span className="text-muted text-xs">{t('otherSources.titleField')}</span>
          <TextField
            value={find.typedTitle}
            onChange={find.setTypedTitle}
            aria-label={t('otherSources.titleField')}
            enterKeyHint="search"
            className={FIELD}
            inputClassName={FIELD_INPUT}
          />
        </label>
        {showAuthor && (
          <label className="flex min-w-[10rem] flex-1 flex-col gap-1">
            <span className="text-muted text-xs">{t('otherSources.authorField')}</span>
            <TextField
              value={find.typedAuthor}
              onChange={find.setTypedAuthor}
              aria-label={t('otherSources.authorField')}
              enterKeyHint="search"
              className={FIELD}
              inputClassName={FIELD_INPUT}
            />
          </label>
        )}
        <button
          type="submit"
          aria-label={t('otherSources.searchAgain')}
          title={t('otherSources.searchAgain')}
          className="bg-background hover:bg-primary/15 grid size-10 shrink-0 place-items-center self-end rounded-full"
        >
          <Search aria-hidden size={17} />
        </button>
      </div>
      <div className="flex flex-wrap items-center gap-2 text-sm">
        <select
          value={find.language}
          aria-label={t('otherSources.language')}
          onChange={(event) => find.setLanguage(event.target.value)}
          className={SELECT}
        >
          <option value="">{t('otherSources.anyLanguage')}</option>
          {options.map((tag) => (
            <option key={tag} value={tag}>
              {nativeName(tag)}
            </option>
          ))}
        </select>
        {find.language && (
          <span className="flex items-center gap-2">
            <OnOffSwitch
              checked={find.translating}
              label={translateLabel}
              onChange={find.setTranslating}
            />
            <span className="text-xs">{translateLabel}</span>
          </span>
        )}
        {find.language && find.translating && (
          <TranslatorSelect
            value={find.translator}
            label={t('otherSources.translator')}
            onChange={find.setTranslator}
            className={SELECT}
          />
        )}
      </div>
      {note && <p className="text-muted text-xs">{note}</p>}
      {find.wantsTranslation && find.translated.isFetching && (
        <p role="status" className="text-muted flex items-center gap-2 text-xs">
          <LoaderCircle aria-hidden size={14} className="animate-spin" />
          {t('otherSources.translating')}
        </p>
      )}
      {find.wantsTranslation && result && !find.translated.isFetching && (
        <div className="flex flex-col gap-0.5 text-xs">
          <p>
            {t('otherSources.searchingFor', { query: result.text })}{' '}
            <span className="text-muted">
              {result.kind
                ? t('otherSources.translatedBy', { name: result.translatorName })
                : t('otherSources.translatedOnDevice')}
            </span>
          </p>
          {google && <PoweredByGoogle />}
        </div>
      )}
      {find.wantsTranslation && find.translated.error && (
        <SourceIssue compact>{find.translated.error.message}</SourceIssue>
      )}
    </form>
  );
};

/** The section: its heading (with a spinner while sources answer), the fields, the matches. */
const FindSection: FC<{ busy: boolean; form: ReactNode; children: ReactNode }> = ({
  busy,
  form,
  children,
}) => {
  const { t } = useTranslation();
  return (
    <section
      data-testid="other-sources"
      aria-label={t('otherSources.title')}
      className="bg-surface flex flex-col gap-3 rounded-2xl p-3"
    >
      <h2 className="flex items-center gap-2 text-sm font-semibold">
        {t('otherSources.title')}
        {busy && <LoaderCircle aria-hidden size={14} className="text-muted animate-spin" />}
      </h2>
      {form}
      {children}
    </section>
  );
};

/**
 * The same comic, book, video, audiobook or show elsewhere (M46.38): every other source that can
 * search is asked for the title, and the close matches listed to open, add to the library, or
 * (catalogues) move the library entry onto. The title and author are editable; a language
 * narrows the sources to those in it, and the title can be translated into it to search there.
 */
export const OtherSources: FC<{
  medium: FindMedium;
  title: string;
  /** Who wrote it: editable, to tell matches apart. */
  author?: string | null;
  /** The catalogue source it's from (comics, books, videos; `stremio` for Stremio's titles). */
  sourceId?: string;
  /** The title shown, stored; with `inLibrary`, it can move to a match's source. */
  titleId?: string;
  inLibrary?: boolean;
  /** An audiobook's or a show's own feed, left out of the matches. */
  feedUrl?: string;
  /** A Stremio title's `<type>:<id>`, left out of the Stremio addons' matches. */
  stremioId?: string;
}> = ({
  medium,
  title,
  author,
  sourceId = '',
  titleId,
  inLibrary = false,
  feedUrl = '',
  stremioId,
}) =>
  medium === 'audiobook' || medium === 'podcast' ? (
    <DirectoryOtherSources medium={medium} title={title} author={author ?? ''} feedUrl={feedUrl} />
  ) : (
    <CatalogOtherSources
      medium={medium}
      title={title}
      author={author ?? ''}
      sourceId={sourceId}
      titleId={titleId}
      inLibrary={inLibrary}
      stremioId={stremioId ?? null}
    />
  );

// ---- Catalogues: comics, books, videos ----

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

type Found = {
  card: MangaCard;
  sourceName: string;
  /** How close, the byline counted. */
  score: number;
  size: MatchSize | null;
  /** Its title page is being fetched for its size. */
  loading: boolean;
  /** Who wrote it, for a book's drawn cover and the byline. */
  authors?: readonly string[];
};

type Order = 'closest' | 'most';

/**
 * Each source's catalogue language (`en`, `ja`, `all`), by source id, once a language is picked
 * (the repositories' catalogue, as Browse → Extensions reads it, is only read then); null until
 * known. `failed` when the catalogue couldn't be read: languages are then unknown.
 */
const useSourceLanguages = (sources: readonly MangaSource[] | undefined, wanted: boolean) => {
  const extensions = useExtensions().data;
  const catalog = useKnownCatalog(wanted);
  const langs = useMemo(() => {
    if (!sources || !catalog.data) return null;
    return new Map(
      gridSources(sources, extensions, catalog.data).map((source) => [source.id, source.lang]),
    );
  }, [sources, extensions, catalog.data]);
  return { langs, failed: catalog.isError };
};

const CatalogOtherSources: FC<{
  medium: Medium;
  title: string;
  author: string;
  sourceId: string;
  titleId?: string;
  inLibrary: boolean;
  stremioId: string | null;
}> = ({ medium, title, author, sourceId, titleId, inLibrary, stremioId }) => {
  const { t } = useTranslation();
  const find = useFindQuery(title, author);
  const sources = useMangaSources(medium);
  const [order, setOrder] = useState<Order>('closest');
  // Movies and series: the Stremio addons' catalogs too (their streams come for the title's id).
  const stremio = useStremioMatches(find.query, stremioId, medium === 'video');
  // A video site (its own Search pill: YouTube, PeerTube) holds videos, not a catalogue's titles.
  const others = useMemo(
    () =>
      (sources.data ?? []).filter(
        (source) => source.enabled && source.id !== sourceId && !source.searchPill,
      ),
    [sources.data, sourceId],
  );
  const { langs, failed } = useSourceLanguages(others, find.language !== '');
  // A language picked asks only the sources in it (or in many); until their languages are
  // known, none; when they can't be known, every source.
  const filtering = find.language !== '' && !failed;
  const searched = !filtering
    ? others
    : langs
      ? others.filter((source) => sourceFits(langs.get(source.id), find.language))
      : [];
  const searchedIds = new Set(searched.map((source) => source.id));
  const query = find.query;
  const results = useQueries({
    queries: query ? searched.map((source) => sourceSearchOptions(medium, source.id, query)) : [],
  });
  const waiting =
    !sources.data || (filtering && !langs) || (find.wantsTranslation && find.translated.isFetching);
  const pending = waiting || results.some((result) => result.isPending);
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
          const authors = authorsIn(record);
          return typeof id === 'string'
            ? {
                sourceId: record.sourceId,
                id,
                title: record.title,
                coverUrl: typeof record.coverUrl === 'string' ? record.coverUrl : null,
                inLibrary: record.inLibrary === true,
                ...(authors ? { authors } : {}),
              }
            : null;
        },
        (card) => `${card.sourceId}:${card.id}`,
        withAuthors,
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
  const close = query
    ? [
        ...live,
        ...seen
          .filter(
            (card) =>
              !liveKeys.has(`${card.sourceId}:${card.id}`) && searchedIds.has(card.sourceId),
          )
          .map((card) => ({ card, sourceName: names.get(card.sourceId) ?? '' })),
      ]
        .map((match) => ({ ...match, score: closeness(medium, match.card.title, query) }))
        .filter((match) => match.score >= CLOSE_ENOUGH && match.sourceName !== '')
        .sort((a, b) => b.score - a.score)
    : [];
  // How much each of the closest has: their title pages, a few at a time, quiet when they fail.
  const details = useQueries({
    queries: close.slice(0, DETAILED).map(({ card }) => detailsQuery(medium, card)),
  });
  const found: Found[] = close
    .map((match, index) => {
      const view = details[index]?.data;
      const size = view ? ('episodes' in view ? videoSize(view) : mangaSize(view)) : null;
      // The card's authors, else those of the title page asked for its size (a book's).
      const authors =
        match.card.authors?.length || !view || !('authors' in view)
          ? match.card.authors
          : view.authors;
      return {
        ...match,
        size,
        authors,
        loading: index < DETAILED && details[index]?.isPending === true,
        score: match.score + authorWeight(find.byline, authors),
      };
    })
    // Another writer's book of the same name drops out once its byline is known.
    .filter((match) => match.score >= CLOSE_ENOUGH)
    .sort((a, b) => b.score - a.score);
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
  const languages = langs ? languagesOf(langs.values()) : [];
  const note = !find.language
    ? null
    : failed
      ? t('otherSources.languagesUnknown')
      : langs && t('otherSources.sourcesIn', { count: searched.length });
  const searching = (pending || stremio.pending) && query !== '';
  return (
    <FindSection
      busy={searching}
      form={
        <FindForm
          find={find}
          languages={languages.length > 0 ? languages : LANGUAGES}
          showAuthor={medium !== 'video'}
          note={note}
        />
      }
    >
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
      {!searching && found.length === 0 && stremio.matches.length === 0 && query !== '' && (
        <p className="text-muted text-sm">
          {searched.length === 0
            ? find.language
              ? t('otherSources.noSourcesIn', { language: nativeName(find.language) })
              : t('otherSources.noSources')
            : t('otherSources.none')}
        </p>
      )}
      <ul className="flex flex-col">
        {shown.map((match) => (
          <li key={`${match.card.sourceId}:${match.card.id}`}>
            <Match
              medium={medium}
              match={match}
              most={match === mostKey ? mostLabel : null}
              migrateFrom={inLibrary && titleId ? titleId : null}
            />
          </li>
        ))}
      </ul>
      <StremioMatchList matches={stremio.matches} />
    </FindSection>
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
  most: string | null;
  /** The library title this match can replace. */
  migrateFrom: string | null;
}> = ({ medium, match, most, migrateFrom }) => {
  const { card, sourceName, size, authors, loading } = match;
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
    <>
      <MatchRow
        open={route}
        cover={
          <MangaCover
            url={card.coverUrl}
            video={medium === 'video'}
            book={bookWords(medium, card.title, authors)}
            className="w-10 rounded"
          />
        }
        title={card.title}
        lines={[medium !== 'video' && authors?.length ? authors.join(', ') : null, sourceName]}
        chips={
          loading ? <SizeShimmer /> : size && <SizeChips medium={medium} size={size} most={most} />
        }
        actions={
          <button
            type="button"
            disabled={added || add.isPending}
            onClick={() => add.mutate()}
            aria-label={t(added ? 'otherSources.added' : 'otherSources.add', {
              source: sourceName,
            })}
            className={MATCH_ACTION}
          >
            {added ? <Check aria-hidden size={14} /> : <Plus aria-hidden size={14} />}
            {t(added ? 'manga.inLibrary' : 'otherSources.addShort')}
          </button>
        }
        menu={<MenuButton label={t('otherSources.more', { source: sourceName })} items={items} />}
        issue={
          error && (
            <SourceIssue compact source={sourceName}>
              {error.message}
            </SourceIssue>
          )
        }
      />
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
    </>
  );
};

// ---- Directories: audiobooks, podcasts ----

/** A directory's match, as the shared row shows it. */
type DirectoryMatch = {
  key: string;
  feedUrl: string;
  title: string;
  lines: ReactNode[];
  artworkUrl: string | null;
};

const DirectoryOtherSources: FC<{
  medium: 'audiobook' | 'podcast';
  title: string;
  author: string;
  feedUrl: string;
}> = ({ medium, title, author, feedUrl }) => {
  const { t } = useTranslation();
  const find = useFindQuery(title, author);
  const query = find.query;
  const books = useQuery({
    queryKey: ['audiobooks', 'elsewhere', query, find.byline, feedUrl],
    queryFn: () => unwrapResult(commands.audiobooksElsewhere(query, find.byline || null, feedUrl)),
    enabled: medium === 'audiobook' && query !== '',
    staleTime: DIRECTORY_STALE_MS,
    retry: false,
  });
  const shows = useQuery({
    queryKey: ['podcasts', 'elsewhere', query],
    queryFn: () => unwrapResult(commands.podcastSearch(query)),
    enabled: medium === 'podcast' && query !== '',
    staleTime: DIRECTORY_STALE_MS,
    retry: false,
  });
  const answer = medium === 'audiobook' ? books : shows;
  const matches: DirectoryMatch[] =
    medium === 'audiobook'
      ? (books.data ?? [])
          .filter((book) => languageFits(book.language, find.language))
          .map((book) => bookMatch(book, t))
      : (shows.data ?? [])
          .filter((show) => show.feedUrl !== feedUrl)
          .map((show) => ({
            show,
            score:
              titleScore(show.title, query) +
              authorWeight(find.byline, show.author ? [show.author] : null),
          }))
          .filter((found) => found.score >= CLOSE_ENOUGH)
          .sort((a, b) => b.score - a.score)
          .map(({ show }) => showMatch(show));
  const busy =
    (find.wantsTranslation && find.translated.isFetching) || (query !== '' && answer.isFetching);
  return (
    <FindSection busy={busy} form={<FindForm find={find} languages={LANGUAGES} showAuthor />}>
      {answer.error && (
        <SourceIssue compact onRetry={() => void answer.refetch()}>
          {answer.error.message}
        </SourceIssue>
      )}
      {answer.isSuccess && !answer.isFetching && matches.length === 0 && (
        <p className="text-muted text-sm">{t('otherSources.none')}</p>
      )}
      <ul className="flex flex-col">
        {matches.map((match) => (
          <li key={match.key}>
            {medium === 'audiobook' ? <BookMatch match={match} /> : <ShowMatch match={match} />}
          </li>
        ))}
      </ul>
    </FindSection>
  );
};

/** A book at another source: who wrote it, which recording, how long, in what, and where. */
const bookMatch = (book: AudiobookResult, t: TFunction): DirectoryMatch => {
  const length = runtimeMs(book.runtime);
  return {
    key: book.feedUrl,
    feedUrl: book.feedUrl,
    title: book.title,
    artworkUrl: book.artworkUrl,
    lines: [
      book.author,
      [book.edition, length ? formatLength(length, t) : book.runtime, book.language]
        .filter(Boolean)
        .join(' · '),
      book.source,
    ],
  };
};

/** A show: who makes it, and the feed's site (the same show often has an old feed). */
const showMatch = (show: PodcastSearchResult): DirectoryMatch => ({
  key: show.feedUrl,
  feedUrl: show.feedUrl,
  title: show.title,
  artworkUrl: show.artworkUrl,
  lines: [show.author, hostOf(show.feedUrl)],
});

/** A book at another source: open it, or add it to the library. */
const BookMatch: FC<{ match: DirectoryMatch }> = ({ match }) => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const client = useQueryClient();
  const open = useOpenAudiobook();
  const [added, setAdded] = useState(false);
  const add = useMutation({
    mutationFn: async () => {
      const id = await open.mutateAsync(match.feedUrl);
      await unwrapResult(commands.podcastSetSubscribed(id, true));
    },
    onSuccess: () => {
      setAdded(true);
      void client.invalidateQueries({ queryKey: ['audiobooks'] });
    },
  });
  const go = () =>
    open.mutate(match.feedUrl, {
      onSuccess: (id) => void navigate({ to: '/audiobook', search: { id } as never }),
    });
  return (
    <DirectoryRow
      match={match}
      onOpen={go}
      added={added}
      adding={add.isPending}
      onAdd={() => add.mutate()}
      addLabel={t('otherSources.addToLibrary')}
      addedLabel={t('manga.inLibrary')}
      error={(open.error ?? add.error)?.message}
    />
  );
};

/** A show in another directory or under another feed: open it, or follow it. */
const ShowMatch: FC<{ match: DirectoryMatch }> = ({ match }) => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const open = useOpenFeed();
  const follow = useSetSubscribed();
  const [added, setAdded] = useState(false);
  const add = useMutation({
    mutationFn: async () => {
      const view = await open.mutateAsync(match.feedUrl);
      await follow.mutateAsync({ id: view.id, subscribed: true });
    },
    onSuccess: () => setAdded(true),
  });
  const go = () =>
    open.mutate(match.feedUrl, {
      onSuccess: (view) => void navigate({ to: '/podcast', search: { id: view.id } as never }),
    });
  return (
    <DirectoryRow
      match={match}
      onOpen={go}
      added={added}
      adding={add.isPending}
      onAdd={() => add.mutate()}
      addLabel={t('otherSources.follow')}
      addedLabel={t('otherSources.following')}
      error={(open.error ?? add.error)?.message}
    />
  );
};

const DirectoryRow: FC<{
  match: DirectoryMatch;
  onOpen: () => void;
  added: boolean;
  adding: boolean;
  onAdd: () => void;
  addLabel: string;
  addedLabel: string;
  error?: string;
}> = ({ match, onOpen, added, adding, onAdd, addLabel, addedLabel, error }) => {
  const { t } = useTranslation();
  const items = (): MenuItem[] => [
    { kind: 'heading', label: match.title, artworkUrl: match.artworkUrl },
    { label: t('menu.open'), icon: BookOpen, onSelect: onOpen },
    {
      label: added ? addedLabel : addLabel,
      icon: added ? Check : Plus,
      disabled: added || adding,
      onSelect: onAdd,
    },
  ];
  return (
    <MatchRow
      open={onOpen}
      cover={<PodcastArt url={match.artworkUrl} className="w-10 rounded" />}
      title={match.title}
      lines={match.lines}
      actions={
        <button
          type="button"
          disabled={added || adding}
          onClick={onAdd}
          aria-label={`${added ? addedLabel : addLabel}: ${match.title}`}
          className={MATCH_ACTION}
        >
          {added ? <Check aria-hidden size={14} /> : <Plus aria-hidden size={14} />}
          {added ? addedLabel : addLabel}
        </button>
      }
      menu={<MenuButton label={t('otherSources.moreFor', { title: match.title })} items={items} />}
      issue={error && <SourceIssue compact>{error}</SourceIssue>}
    />
  );
};
