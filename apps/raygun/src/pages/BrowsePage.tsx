import type { MangaSource } from '@raygun/bindings';
import { useIsFetching } from '@tanstack/react-query';
import { Link, useNavigate, useSearch } from '@tanstack/react-router';
import {
  Compass,
  Copy,
  LayoutList,
  LoaderCircle,
  Package,
  Puzzle,
  RefreshCw,
  Search,
  Settings2,
  Square,
  SquareCheck,
} from 'lucide-react';
import { Fragment, useEffect, useState } from 'react';
import type { FC, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import type { TFunction } from 'i18next';

import { BrowseTab as AvailableExtensions } from '../extensions/BrowseTab';
import { useContentRating } from '../extensions/contentRating';
import type { RatingItem } from '../extensions/contentRating';
import { RatingHint } from '../extensions/RatingHint';
import { updateCount } from '../extensions/catalogFilters';
import { InstalledTab } from '../extensions/InstalledTab';
import { InstallProgress } from '../extensions/InstallProgress';
import { EXTENSION_LISTS } from '../extensions/tabs';
import type { ExtensionList } from '../extensions/tabs';
import {
  useCatalog,
  useExtensions,
  useRefreshCatalog,
  useReloadExtensions,
} from '../extensions/useExtensions';
import { useLayout } from '../layout/useLayout';
import { useMangaSources } from '../manga/useManga';
import { AdultMark } from '../shell/AdultMark';
import { LetterAvatar } from '../shell/LetterAvatar';
import { ListSearchField } from '../shell/ListSearchField';
import { TextField } from '../shell/TextField';
import { matchesSearch } from '../shell/useListSearch';
import { OverflowMenu } from '../shell/OverflowMenu';
import { Tabs } from '../shell/Tabs';
import { BrowseAll } from '../music/BrowseAll';
import { useIncognitoItem } from '../privacy/useIncognito';
import { useRememberSearch } from '../search/useRecentSearches';
import { SearchPanel } from '../search/SearchPanel';
import { failingLast, useSourceHealth } from '../search/sourceHealth';
import { useSettings, useUpdateSetting } from '../settings/useSettings';
import { QuickAccessButton } from '../shell/QuickAccessButton';
import { useIsDesktop } from '../shell/useIsDesktop';
import { SourceMenu } from '../search/SourceMenu';
import { buildMenu, copyText } from '../music/mediaMenu';
import type { MenuItem } from '../shell/contextMenuStore';
import { mediaOfKind } from '../sources/sourceMedia';
import { WebsitesSection } from '../websites/WebsitesSection';
import { OnOffSwitch } from '../shell/OnOffSwitch';
import { BulkSwitchBar, SelectButton, SelectCheck } from '../shell/BulkSwitch';
import { useBulkSelection } from '../shell/useBulkSelection';
import type { BulkSelection } from '../shell/useBulkSelection';
import { useSourceSwitches } from '../search/useSourceSwitch';
import type { SearchType, SearchView } from '../search/searchTypes';
import { useSetAddonEnabled, useStremioAddons } from '../watch/useStremio';
import { useVideoSources } from '../watch/useVideoSources';
import { SourceIssue } from '../shell/SourceIssue';
import { BROWSE_TAB_LABEL, browseStartTab } from './browseTabs';
import { MenuButton } from '../shell/MenuButton';

export type BrowseSection = 'search' | 'discover' | 'sources' | 'extensions';
export type { ExtensionList };

type SourceRow = {
  key: string;
  name: string;
  detail: string;
  logo?: string | null;
  to: string;
  search: Record<string, unknown>;
  /** Where "Latest" leads, for sources that list new titles. */
  latest?: { to: string; search: Record<string, unknown> };
  /** An extension's source: its ⋯ menu, and whether the user turned it off (M46.8). */
  source?: { id: string; extensionId: string; enabled: boolean };
  /** Rated for adults: an 18+ tag after the name. */
  adult?: boolean;
  /** A Stremio addon's stored id: its ⋯ menu marks it 18+ or not. */
  addonId?: string;
  /** Whether that addon is on. */
  addonEnabled?: boolean;
  /** An extension's audio sources (music, podcasts, audiobooks), switched together. */
  audio?: { extensionId: string; sourceIds: string[]; enabled: boolean };
};

/** Whether a row's source or addon is on. */
const rowEnabled = (row: SourceRow) =>
  row.source?.enabled ?? row.addonEnabled ?? row.audio?.enabled ?? true;

type Page = { to: string; search: Record<string, unknown> };

/** A Stremio addon's ⋮ menu: open it, manage addons, copy its name, mark it 18+ or not. */
const addonMenu = (
  row: SourceRow,
  t: TFunction,
  go: (page: Page) => void,
  marks: RatingItem[],
): MenuItem[] =>
  buildMenu({
    heading: { label: row.name, subtitle: row.detail, artworkUrl: row.logo ?? null },
    play: [
      {
        label: t('sources.open'),
        icon: Compass,
        onSelect: () => go({ to: row.to, search: row.search }),
      },
    ],
    goTo: [
      {
        label: t('menu.manageAddons'),
        icon: Settings2,
        onSelect: () => go({ to: '/sources', search: { type: 'addons' } }),
      },
    ],
    share: [{ label: t('menu.copyName'), icon: Copy, onSelect: () => copyText(row.name) }],
    edit: row.addonId ? marks : [],
  });

const SourceRowLink: FC<{ row: SourceRow }> = ({ row }) => {
  const { t } = useTranslation();
  const rating = useContentRating();
  const mark = row.source
    ? rating.markOf('extensions', row.source.extensionId)
    : row.addonId
      ? rating.markOf('addons', row.addonId)
      : 'default';
  return (
    <Link
      to={row.to}
      search={row.search}
      aria-label={row.name}
      className={`hover:bg-surface flex min-w-0 flex-1 items-center gap-4 rounded-xl px-1 py-2.5 ${rowEnabled(row) ? '' : 'opacity-50'}`}
    >
      <LetterAvatar name={row.name} logo={row.logo} />
      <span className="flex min-w-0 flex-col">
        <span className="flex min-w-0 items-center gap-1.5">
          <span className="truncate font-medium">{row.name}</span>
          {row.adult && <AdultMark />}
          <RatingHint mark={mark} />
        </span>
        <span className="text-muted truncate text-xs">
          {!rowEnabled(row) ? t('sources.disabledNote', { detail: row.detail }) : row.detail}
        </span>
      </span>
    </Link>
  );
};

const LatestLink: FC<{ row: SourceRow }> = ({ row }) => {
  const { t } = useTranslation();
  return row.latest ? (
    <Link
      to={row.latest.to}
      search={row.latest.search}
      aria-label={t('browse.latestOf', { name: row.name })}
      className="text-primary shrink-0 rounded-full px-3 py-2 text-sm font-medium"
    >
      {t('manga.list.latest')}
    </Link>
  ) : null;
};

/** An extension's source, with its ⋯ (SourceMenu). */
const ExtensionSourceRow: FC<{
  row: SourceRow;
  source: NonNullable<SourceRow['source']>;
}> = ({ row, source }) => {
  return (
    <>
      <SourceRowLink row={row} />
      <LatestLink row={row} />
      <SourceMenu sourceId={source.id} extensionId={source.extensionId} name={row.name} />
    </>
  );
};

/** Turns sources and addons on or off by row key, one or many at once. */
type RowSwitch = { busy: boolean; set: (keys: string[], on: boolean) => void };

const SourceSection: FC<{
  title: string;
  rows: SourceRow[];
  selection: BulkSelection;
  switcher: RowSwitch;
}> = ({ title, rows, selection, switcher }) => {
  const { t } = useTranslation();
  const rating = useContentRating();
  const navigate = useNavigate();
  if (rows.length === 0) return null;
  const go = (page: Page) => void navigate({ to: page.to, search: page.search as never });
  return (
    <section aria-label={title} className="flex flex-col">
      <h2 className="text-muted px-1 pt-2 pb-1 text-sm font-medium">{title}</h2>
      <ul className="flex flex-col">
        {rows.map((row) => (
          <li key={row.key} className="flex items-center gap-1">
            <SelectCheck selection={selection} id={row.key} name={row.name} />
            {row.source ? (
              <ExtensionSourceRow row={row} source={row.source} />
            ) : row.audio ? (
              // An extension's audio sources: its page holds their settings.
              <SourceRowLink row={row} />
            ) : (
              <>
                <SourceRowLink row={row} />
                <LatestLink row={row} />
                <MenuButton
                  label={t('menu.options', { name: row.name })}
                  items={() =>
                    addonMenu(row, t, go, rating.menuItems('addons', row.addonId ?? '', row.adult))
                  }
                />
              </>
            )}
            <OnOffSwitch
              checked={rowEnabled(row)}
              label={t('sources.switch', { name: row.name })}
              disabled={switcher.busy}
              onChange={(on) => switcher.set([row.key], on)}
            />
          </li>
        ))}
      </ul>
    </section>
  );
};

type SourceKind = 'all' | 'video' | 'audio' | 'manga' | 'novel' | 'websites';
const SOURCE_KIND_KEY = 'raygun.browse.sourceKind';

const readKind = (): SourceKind => {
  try {
    const saved = localStorage.getItem(SOURCE_KIND_KEY);
    return saved === 'video' ||
      saved === 'audio' ||
      saved === 'manga' ||
      saved === 'novel' ||
      saved === 'websites'
      ? saved
      : 'all';
  } catch {
    return 'all';
  }
};

/** What an extension's audio sources bring, as their row says it: "Music · Audiobooks". */
const AUDIO_KINDS: [prefix: string, label: string][] = [
  ['music.', 'browse.audioKind.music'],
  ['lyrics', 'browse.audioKind.lyrics'],
  ['podcast.', 'browse.audioKind.podcast'],
  ['audiobook.', 'browse.audioKind.audiobook'],
];

/**
 * Browse → Sources: every catalogue to browse, by kind (Aniyomi's Browse). Chips pick one kind
 * and a field finds sources by name (M46.35), so a long list stays short.
 */
const SourcesList: FC = () => {
  const { t } = useTranslation();
  const layout = useLayout();
  const [kind, setKindState] = useState<SourceKind>(readKind);
  const [query, setQuery] = useState('');
  const setKind = (next: SourceKind) => {
    setKindState(next);
    try {
      localStorage.setItem(SOURCE_KIND_KEY, next);
    } catch {
      // A per-device convenience.
    }
  };
  const found = (rows: SourceRow[]) =>
    rows.filter((row) => matchesSearch(query, [row.name, row.detail]));
  const extensions = useExtensions();
  const addons = useStremioAddons();
  const video = useVideoSources();
  const manga = useMangaSources('manga');
  const novel = useMangaSources('novel');
  // Sources that failed lately go last (M46.9), unless Settings says otherwise.
  const failures = useSourceHealth((state) => state.failures);
  const failingAtEnd = useSettings().data?.failingSourcesLast ?? true;
  const extensionOf = (source: MangaSource) =>
    extensions.data?.find((extension) => extension.id === source.extensionId);
  const catalogue = (sources: MangaSource[] | undefined, route: string): SourceRow[] =>
    failingLast(sources ?? [], failures, failingAtEnd).map((source) => ({
      key: source.id,
      name: source.name,
      detail:
        source.id in failures
          ? t('sources.failedLately', { detail: extensionOf(source)?.name ?? source.extensionId })
          : (extensionOf(source)?.name ?? source.extensionId),
      // A source shows its extension's icon (M46.2).
      logo: extensionOf(source)?.icon,
      to: route,
      search: { id: source.id },
      latest: { to: route, search: { id: source.id, mode: 'latest' } },
      source: { id: source.id, extensionId: source.extensionId, enabled: source.enabled },
      adult: source.adult === true || extensionOf(source)?.adult === true,
    }));

  const videoRows: SourceRow[] = [
    ...(addons.data ?? [])
      // Addons that are off stay listed, dimmed, so their switch can turn them back on.
      .filter((addon) => addon.catalogs > 0 && !addon.error)
      .map((addon) => ({
        key: addon.id,
        name: addon.name,
        detail: t('browse.stremioAddon'),
        logo: addon.logo,
        to: '/watch/addon',
        search: { id: addon.id },
        adult: addon.adult,
        addonId: addon.id,
        addonEnabled: addon.enabled,
      })),
    ...catalogue(video.data, '/watch/source'),
  ];
  const switches = useSourceSwitches();
  // Music, podcast and audiobook sources (and the servers bringing any), one row per extension:
  // its page holds their settings, its switch turns them all on or off.
  const audioRows: SourceRow[] = (extensions.data ?? []).flatMap((extension) => {
    const sources = extension.providers.filter(
      (provider) => mediaOfKind(provider.kind) === 'audio',
    );
    if (extension.status !== 'loaded' || sources.length === 0) return [];
    const brings = AUDIO_KINDS.filter(([prefix]) =>
      sources.some((provider) => provider.kind.startsWith(prefix)),
    ).map(([, label]) => t(label));
    const sourceIds = sources.map((provider) => provider.id);
    return [
      {
        key: `audio:${extension.id}`,
        name: extension.name,
        detail: brings.join(' · '),
        logo: extension.icon,
        to: '/extensions/detail',
        search: { id: extension.id },
        adult: extension.adult,
        audio: {
          extensionId: extension.id,
          sourceIds,
          enabled: switches.isEnabled(extension.id) && sourceIds.every(switches.isEnabled),
        },
      },
    ];
  });
  const loading = addons.isPending || video.isPending || manga.isPending || novel.isPending;
  const empty =
    !loading &&
    videoRows.length === 0 &&
    audioRows.length === 0 &&
    (manga.data?.length ?? 0) === 0 &&
    (novel.data?.length ?? 0) === 0;

  const mangaRows = catalogue(manga.data, '/manga/source');
  const novelRows = catalogue(novel.data, '/novel/source');
  const shownRows = [
    ...(kind === 'all' || kind === 'video' ? found(videoRows) : []),
    ...(kind === 'all' || kind === 'audio' ? found(audioRows) : []),
    ...(kind === 'all' || kind === 'manga' ? found(mangaRows) : []),
    ...(kind === 'all' || kind === 'novel' ? found(novelRows) : []),
  ];
  const selection = useBulkSelection(shownRows.map((row) => row.key));
  const setAddon = useSetAddonEnabled();
  const switcher: RowSwitch = {
    busy: switches.busy || setAddon.isPending,
    set: (keys, on) => {
      const rows = shownRows.filter((row) => keys.includes(row.key));
      const sourceIds = rows.flatMap((row) =>
        row.source ? [row.source.id] : (row.audio?.sourceIds ?? []),
      );
      // An audio row's extension may have been turned off whole: on turns it back on too.
      const extensionIds = on
        ? rows.flatMap((row) => (row.audio ? [row.audio.extensionId] : []))
        : [];
      if (sourceIds.length + extensionIds.length > 0) {
        switches.setMany([...sourceIds, ...extensionIds], on);
      }
      for (const row of rows) {
        if (row.addonId && row.addonEnabled !== on)
          setAddon.mutate({ id: row.addonId, enabled: on });
      }
    },
  };
  // The sections in the user's order (M46.5), narrowed by kind and name (M46.35).
  const sections: Record<string, ReactNode> = {
    video: (
      <SourceSection
        title={t('browse.kind.video')}
        rows={found(videoRows)}
        selection={selection}
        switcher={switcher}
      />
    ),
    audio: (
      <SourceSection
        title={t('browse.kind.audio')}
        rows={found(audioRows)}
        selection={selection}
        switcher={switcher}
      />
    ),
    manga: (
      <SourceSection
        title={t('library.media.manga')}
        rows={found(mangaRows)}
        selection={selection}
        switcher={switcher}
      />
    ),
    novel: (
      <SourceSection
        title={t('library.media.novel')}
        rows={found(novelRows)}
        selection={selection}
        switcher={switcher}
      />
    ),
    websites: <WebsitesSection query={query} />,
  };
  const visible = layout.browseSections.filter((item) => item.visible);
  const counts: Record<string, number | null> = {
    video: videoRows.length,
    audio: audioRows.length,
    manga: mangaRows.length,
    novel: novelRows.length,
    websites: null,
  };
  const chip = (on: boolean) =>
    `shrink-0 rounded-full px-3 py-1.5 text-sm ${on ? 'bg-primary/20 text-primary font-semibold' : 'bg-surface'}`;
  return (
    <div data-testid="browse-sources" className="flex flex-col gap-2">
      {empty && (
        // Where sources come from, as the two ways to add some.
        <ul aria-label={t('browse.noSources')} className="bg-surface flex flex-col rounded-xl">
          <li>
            <Link
              to="/browse"
              search={{ tab: 'extensions', list: 'all' }}
              className="flex items-center gap-3 px-4 py-3"
            >
              <Package aria-hidden size={18} className="text-muted shrink-0" />
              <span className="font-medium">{t('browse.addExtensions')}</span>
            </Link>
          </li>
          <li>
            <Link
              to="/sources"
              search={{ add: true }}
              className="flex items-center gap-3 px-4 py-3"
            >
              <Puzzle aria-hidden size={18} className="text-muted shrink-0" />
              <span className="font-medium">{t('browse.addSource')}</span>
            </Link>
          </li>
        </ul>
      )}
      <div className="flex flex-wrap items-center gap-2 pt-1">
        <div
          role="group"
          aria-label={t('browse.sourceKinds')}
          className="-mx-4 flex min-w-0 gap-2 no-scrollbar overflow-x-auto px-4 lg:mx-0 lg:px-0"
        >
          {(['all', ...visible.map((item) => item.id)] as SourceKind[]).map((id) => (
            <button
              key={id}
              type="button"
              aria-pressed={kind === id}
              onClick={() => setKind(id)}
              className={chip(kind === id)}
            >
              {id === 'all'
                ? t('browse.allKinds')
                : id === 'websites'
                  ? t('websites.title')
                  : `${t(id === 'video' || id === 'audio' ? `browse.kind.${id}` : `library.media.${id}`)} · ${counts[id] ?? 0}`}
            </button>
          ))}
        </div>
        <div className="flex w-full items-center gap-2 sm:ml-auto sm:w-auto">
          <ListSearchField
            value={query}
            onChange={setQuery}
            label={t('browse.findSource')}
            className="min-w-0 flex-1 sm:w-64"
          />
          {shownRows.length > 0 && <SelectButton selection={selection} />}
        </div>
      </div>
      <BulkSwitchBar selection={selection} busy={switcher.busy} onSwitch={switcher.set} />
      {visible
        .filter((item) => kind === 'all' || item.id === kind)
        .map((item) => (
          <Fragment key={item.id}>{sections[item.id]}</Fragment>
        ))}
    </div>
  );
};

/** Wait for a pause in typing before searching, so each keystroke doesn't hit the network. */
export const SEARCH_DEBOUNCE_MS = 400;

/**
 * Browse: where content is found (Mihon's Browse with CloudStream's search). The search bar is the
 * header; under it Search (every kind of media, or one with its sources' filters), Sources (the
 * catalogues by kind) and Extensions. Typing switches to Search. The library never browses.
 */
export const BrowsePage: FC = () => {
  const { t } = useTranslation();
  const current = useSearch({ from: '/browse' });
  const layout = useLayout();
  // Without a tab in the address, Sources, or the first tab the user shows (M46.5).
  const shownTabs = layout.browseTabs
    .filter((item) => item.visible)
    .map((item) => item.id)
    .filter(
      (id): id is BrowseSection =>
        id === 'search' || id === 'discover' || id === 'sources' || id === 'extensions',
    );
  const settings = useSettings().data;
  // Browse opens on the tab chosen in Settings → Browse (Browse all by default, as Spotify's
  // does), else the first shown tab.
  const firstTab = browseStartTab(settings?.browseStartTab, shownTabs);
  // On desktop the app bar holds the search field (M46.9).
  const desktop = useIsDesktop();
  const updateSetting = useUpdateSetting();
  const failingAtEnd = settings?.failingSourcesLast ?? true;
  const incognitoItem = useIncognitoItem();
  const { tab = firstTab, list = 'installed', q = '', type = 'all', view, pill } = current;
  const navigate = useNavigate({ from: '/browse' });
  const reload = useReloadExtensions();
  const refresh = useRefreshCatalog();
  const catalog = useCatalog(tab === 'extensions');
  const updates = updateCount(catalog.data);
  const fetching = useIsFetching() > 0;
  const remember = useRememberSearch();
  const [input, setInput] = useState(q);
  // The address's query last seen, and the one this field last searched for.
  const [shown, setShown] = useState(q);
  const [sent, setSent] = useState(q);
  if (q !== shown) {
    setShown(q);
    // A search changed elsewhere (Back, a link) shows in the field; this field's own search
    // arriving leaves what's been typed since alone.
    if (q !== sent) {
      setSent(q);
      setInput(q);
    }
  }

  // The query lives in the URL, so Back to this page shows the same results. On desktop the app
  // bar's field owns it (M46.9); this hidden one must not write its old text back.
  useEffect(() => {
    if (desktop || input.trim() === q) return;
    const timer = setTimeout(() => {
      const next = input.trim();
      setSent(next);
      void navigate({ search: { ...current, q: next || undefined }, replace: true });
    }, SEARCH_DEBOUNCE_MS);
    return () => clearTimeout(timer);
  }, [input, q, current, navigate, desktop]);

  const onInput = (value: string) => {
    setInput(value);
    // Typing searches: the Search tab shows the results.
    if (value.trim() && tab !== 'search') {
      void navigate({ search: { ...current, tab: 'search' }, replace: true });
    }
  };
  /** Searches at once (Enter, or a kept search), and remembers the search. */
  const searchNow = (text: string, kind: SearchType = type) => {
    const next = text.trim();
    setInput(next);
    setSent(next);
    remember(next, kind);
    void navigate({
      search: {
        ...current,
        tab: 'search',
        q: next || undefined,
        type: kind === 'all' ? undefined : kind,
        ...(kind === type ? {} : { view: undefined, pill: undefined }),
      },
      replace: true,
    });
  };
  // A new type starts on its Relevant pill unless one is named (M46.25).
  const setType = (next: SearchType, nextView?: SearchView, pill?: string) =>
    void navigate({
      search: {
        ...current,
        tab: 'search',
        type: next === 'all' ? undefined : next,
        view: nextView && nextView !== 'relevant' ? nextView : undefined,
        pill: next === 'video' ? pill : undefined,
      },
      replace: true,
    });
  const searchParams = {
    tab: 'search',
    ...(q ? { q } : {}),
    ...(type === 'all' ? {} : { type }),
  };

  const menu = (
    <OverflowMenu
      items={[
        ...(tab === 'extensions' && list === 'installed'
          ? [
              {
                label: t('extensions.reload'),
                icon: RefreshCw,
                onSelect: () => reload.mutate(),
              },
            ]
          : []),
        ...(tab === 'extensions' && list !== 'installed'
          ? [
              {
                label: t('extensions.refresh'),
                icon: RefreshCw,
                onSelect: () => refresh.mutate(),
              },
            ]
          : []),
        {
          label: t('sources.failingLast'),
          icon: failingAtEnd ? SquareCheck : Square,
          onSelect: () => updateSetting.mutate({ key: 'failingSourcesLast', value: !failingAtEnd }),
        },
        { label: t('sources.title'), icon: Puzzle, to: '/sources' },
        incognitoItem,
        {
          label: t('layout.editBrowse'),
          icon: LayoutList,
          to: '/settings/layout/browseSections',
        },
      ]}
    />
  );
  // Updates shows only while there are some (or while it's open).
  const extensionLists = EXTENSION_LISTS.filter(
    (option) => option !== 'updates' || updates > 0 || list === 'updates',
  );
  const tabs = (
    <Tabs
      label={t('browse.title')}
      active={tab}
      items={shownTabs.concat(shownTabs.includes(tab) ? [] : [tab]).map((id) => ({
        id,
        label: t(BROWSE_TAB_LABEL[id]),
        to: '/browse',
        search: id === 'search' ? searchParams : id === firstTab ? {} : { tab: id },
        testId: `browse-tab-${id}`,
      }))}
    />
  );

  return (
    <section data-testid="browse-page" className="flex flex-col gap-3">
      <h1 className="sr-only">{t('browse.title')}</h1>
      {desktop ? (
        // The app bar holds the search field on desktop; the tabs take its row.
        <div className="flex items-center gap-2">
          <div className="min-w-0 flex-1">{tabs}</div>
          {menu}
        </div>
      ) : (
        <>
          <header className="bg-background sticky top-0 z-30 -mx-4 flex min-h-16 items-center gap-1 px-3 py-2 lg:-mx-10 lg:px-8">
            <TextField
              type="search"
              value={input}
              onChange={onInput}
              // Tapping the bar is starting a search: Search shows the ones made lately.
              onFocus={() => {
                if (tab !== 'search') {
                  void navigate({ search: { ...current, tab: 'search' }, replace: true });
                }
              }}
              onKeyDown={(event) => {
                if (event.key === 'Enter') searchNow(input);
              }}
              icon={Search}
              iconSize={20}
              autoComplete="off"
              placeholder={t(`search.placeholderFor.${type}`)}
              aria-label={t('search.title')}
              className="bg-surface flex-1 gap-3 rounded-full py-1.5 pr-2 pl-5"
              inputClassName="py-1.5"
              trailing={
                fetching && q && tab === 'search' ? (
                  <LoaderCircle
                    aria-label={t('search.searching')}
                    role="status"
                    size={20}
                    className="text-muted shrink-0 animate-spin"
                  />
                ) : null
              }
            />
            {menu}
            <QuickAccessButton />
          </header>
          {tabs}
        </>
      )}
      {(reload.error ?? refresh.error) && (
        <SourceIssue>{(reload.error ?? refresh.error)?.message}</SourceIssue>
      )}
      {(reload.isPending || refresh.isPending) && (
        <p role="status" className="text-muted flex items-center gap-2 text-sm">
          <RefreshCw aria-hidden size={14} className="text-primary animate-spin" />
          {t(reload.isPending ? 'extensions.reload' : 'extensions.refresh')}
        </p>
      )}
      {tab === 'search' && (
        <SearchPanel
          query={q}
          type={type}
          view={view}
          pill={pill}
          onType={setType}
          onSearch={searchNow}
        />
      )}
      {tab === 'discover' && <BrowseAll />}
      {tab === 'sources' && <SourcesList />}
      {tab === 'extensions' && (
        <div className="flex flex-col gap-4">
          <nav
            aria-label={t('extensions.tabs')}
            className="-mx-4 flex gap-2 no-scrollbar overflow-x-auto px-4 lg:mx-0 lg:px-0"
          >
            {extensionLists.map((option) => (
              <Link
                key={option}
                to="/browse"
                search={{ tab: 'extensions', ...(option === 'installed' ? {} : { list: option }) }}
                replace
                data-testid={`extensions-tab-${option}`}
                aria-current={option === list ? 'page' : undefined}
                className={`shrink-0 rounded-full px-4 py-1.5 text-sm ${option === list ? 'bg-primary/20 text-primary font-semibold' : 'bg-surface'}`}
              >
                {option === 'updates'
                  ? t('extensions.tab.updatesCount', { count: updates })
                  : t(`extensions.tab.${option}`)}
              </Link>
            ))}
          </nav>
          <InstallProgress />
          {list === 'installed' ? <InstalledTab /> : <AvailableExtensions show={list} />}
        </div>
      )}
    </section>
  );
};
