import type { ContentOverride, ExtensionInfo, RepoCatalog, ServerInfo } from '@raygun/bindings';
import { Link, useNavigate, useSearch } from '@tanstack/react-router';
import { ChevronRight, LoaderCircle, Plus, RefreshCw } from 'lucide-react';
import { useDeferredValue, useMemo, useState } from 'react';
import type { FC, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { useContentRating } from '../extensions/contentRating';
import { ExtensionCatalog } from '../extensions/ExtensionCatalog';
import { RatingHint } from '../extensions/RatingHint';
import { RepoRow, SuggestedRepos } from '../extensions/RepoRows';
import { useCatalogRefresh } from '../extensions/useCatalogRefresh';
import { useCatalog, useExtensions, useRepos } from '../extensions/useExtensions';
import { useSourceSwitches } from '../search/useSourceSwitch';
import { ServerRow } from '../servers/ServerRow';
import { ServerSheet } from '../servers/ServerSheet';
import { useServers } from '../servers/useServers';
import { AdultMark } from '../shell/AdultMark';
import { BulkSwitchBar, SelectButton, SelectCheck } from '../shell/BulkSwitch';
import { LetterAvatar } from '../shell/LetterAvatar';
import { OnOffSwitch } from '../shell/OnOffSwitch';
import { VirtualRows } from '../shell/VirtualRows';
import type { RowProps } from '../shell/windowedRows';
import { SourceIssue } from '../shell/SourceIssue';
import { TopBarAction } from '../shell/TopBar';
import { useBulkSelection } from '../shell/useBulkSelection';
import type { BulkSelection } from '../shell/useBulkSelection';
import { matchesSearch } from '../shell/useListSearch';
import { useSearchMode } from '../shell/useSearchMode';
import { AddSourceSheet } from '../sources/AddSourceSheet';
import {
  SOURCE_MEDIA,
  SOURCE_TYPES,
  addonMedia,
  mediaOf,
  mediaOfAll,
} from '../sources/sourceMedia';
import type { SourceMedia, SourceType, SourcesTab } from '../sources/sourceMedia';
import { AddonItem, AddonSuggestions } from '../watch/AddonRows';
import { useSetAddonEnabled, useSetAddonOrder, useStremioAddons } from '../watch/useStremio';
import { PageHeader } from './PageHeader';
import { originLabel } from '../extensions/originLabel';

/** Connections to servers are extensions too; they're listed as servers. */
const isServer = (extension: ExtensionInfo) => extension.id.startsWith('server.');

const chip = (on: boolean) =>
  `shrink-0 rounded-full px-3 py-1.5 text-sm ${on ? 'bg-primary/20 text-primary font-semibold' : 'bg-surface'}`;

/**
 * An installed extension: its page, where it came from, and its switch. Its 18+ mark comes from
 * the page, which reads the marks once for every row.
 */
const ExtensionRow: FC<{
  extension: ExtensionInfo;
  selection: BulkSelection;
  enabled: boolean;
  busy: boolean;
  mark: ContentOverride;
  onSwitch: (on: boolean) => void;
  row: RowProps;
}> = ({ extension, selection, enabled, busy, mark, onSwitch, row }) => {
  const { t } = useTranslation();
  return (
    <li {...row} aria-label={extension.name} className="flex items-center gap-1">
      <SelectCheck selection={selection} id={`extension:${extension.id}`} name={extension.name} />
      <Link
        to="/extensions/detail"
        search={{ id: extension.id }}
        data-testid="source-extension-row"
        className={`hover:bg-surface flex min-w-0 flex-1 items-center gap-4 rounded-xl px-1 py-2.5 ${enabled ? '' : 'opacity-50'}`}
      >
        <LetterAvatar name={extension.name} logo={extension.icon} />
        <span className="flex min-w-0 flex-1 flex-col gap-0.5">
          <span className="flex flex-wrap items-baseline gap-x-2">
            <span className="font-medium">{extension.name}</span>
            <span className="text-muted text-xs">{extension.version}</span>
            <span className="bg-surface text-muted rounded px-1.5 text-[10px] font-semibold tracking-wide uppercase">
              {originLabel(extension, t)}
            </span>
            {extension.adult && <AdultMark />}
            <RatingHint mark={mark} />
          </span>
          <span className="text-muted truncate text-xs">
            {!enabled
              ? t('sources.disabledNote', { detail: extension.description ?? extension.version })
              : extension.status === 'failed'
                ? t('extensions.notLoaded')
                : (extension.description ?? '')}
          </span>
        </span>
        <ChevronRight aria-hidden size={18} className="text-muted shrink-0" />
      </Link>
      <OnOffSwitch
        checked={enabled}
        label={t('sources.switch', { name: extension.name })}
        disabled={busy}
        onChange={onSwitch}
      />
    </li>
  );
};

const Section: FC<{ title: string; children: ReactNode }> = ({ title, children }) => (
  <section aria-label={title} className="flex flex-col gap-2">
    <h2 className="text-muted px-1 pt-2 text-sm font-medium">{title}</h2>
    {children}
  </section>
);

/** The pills after All: each kind of source, then the extension catalogue. */
const SOURCES_TABS: SourcesTab[] = [...SOURCE_TYPES, 'catalogue'];

/**
 * Sources (M46.52): everything content comes from, in one place — extension repositories (every
 * format Raygun reads), Stremio addons, servers and the extensions on this device. Pills pick a
 * kind and what they're for (Audio, Videos…); rows keep their details, switch, selection, order
 * and removal; Add takes any address and works out what it is. The last pill is the extension
 * catalogue of every repository, as Browse → Extensions shows it (install, update, remove).
 */
export const SourcesPage: FC = () => {
  const { t } = useTranslation();
  const { type, media, add, list = 'all' } = useSearch({ from: '/sources' });
  const navigate = useNavigate({ from: '/sources' });
  const find = useSearchMode(t('sources.page.find'));
  const catalogue = type === 'catalogue';
  const catalogRefresh = useCatalogRefresh(list);
  const [adding, setAdding] = useState(add === true);
  const [signingIn, setSigningIn] = useState<ServerInfo | null>(null);

  const repos = useRepos();
  const catalog = useCatalog();
  const addons = useStremioAddons();
  const servers = useServers();
  const extensions = useExtensions();
  const switches = useSourceSwitches();
  const setAddon = useSetAddonEnabled();
  const setOrder = useSetAddonOrder();

  const rating = useContentRating();
  // Lists follow typing once there's a moment to spare.
  const query = useDeferredValue(find.query);
  // What each repository is for, worked out once per catalogue (thousands of entries).
  const listings = useMemo(
    () => new Map<string, RepoCatalog>(catalog.data?.map((listing) => [listing.repo.id, listing])),
    [catalog.data],
  );
  const repoMedia = useMemo(
    () =>
      new Map(
        [...listings].map(([id, listing]) => [
          id,
          mediaOfAll(listing.entries.map((entry) => entry.provides)),
        ]),
      ),
    [listings],
  );

  // Every row with what it's for; the pills narrow them.
  const allRepos = useMemo(
    () =>
      (repos.data ?? []).map((repo) => ({
        repo,
        media: repoMedia.get(repo.id) ?? mediaOfAll([]),
      })),
    [repos.data, repoMedia],
  );
  const allAddons = addons.data ?? [];
  const allServers = (servers.data ?? []).map((server) => ({
    server,
    media: mediaOf(server.provides),
  }));
  const allExtensions = useMemo(
    () =>
      (extensions.data ?? [])
        .filter((extension) => !isServer(extension))
        .map((extension) => ({ extension, media: mediaOf(extension.provides) })),
    [extensions.data],
  );

  const of = (section: SourceType) => !type || type === section;
  const forMedia = (rowMedia: SourceMedia[]) => !media || rowMedia.includes(media);
  const shownRepos = allRepos.filter(
    (row) => forMedia(row.media) && matchesSearch(query, [row.repo.name, row.repo.url]),
  );
  const shownAddons = allAddons.filter(
    (addon) =>
      forMedia(addonMedia(addon.resources)) &&
      matchesSearch(query, [addon.name, addon.description]),
  );
  const shownServers = allServers.filter(
    (row) =>
      forMedia(row.media) &&
      matchesSearch(query, [row.server.name, row.server.address, row.server.kind]),
  );
  const shownExtensions = allExtensions.filter(
    (row) =>
      forMedia(row.media) && matchesSearch(query, [row.extension.name, row.extension.description]),
  );

  // What the pills offer: the media the rows of the chosen kind are for.
  const present = new Set<SourceMedia>([
    ...(of('repositories') ? allRepos.flatMap((row) => row.media) : []),
    ...(of('addons') ? allAddons.flatMap((addon) => addonMedia(addon.resources)) : []),
    ...(of('servers') ? allServers.flatMap((row) => row.media) : []),
    ...(of('extensions') ? allExtensions.flatMap((row) => row.media) : []),
  ]);
  // The chosen one stays, so it can be let go of under another kind.
  const mediaPills = SOURCE_MEDIA.filter((option) => present.has(option) || option === media);
  const counts: Record<SourceType, number> = {
    repositories: allRepos.length,
    addons: allAddons.length,
    servers: allServers.length,
    extensions: allExtensions.length,
  };

  // Rows with a switch can be picked and turned on or off together.
  const switchable = [
    ...(of('addons') ? shownAddons.map((addon) => `addon:${addon.id}`) : []),
    ...(of('servers') ? shownServers.map((row) => `server:${row.server.id}`) : []),
    ...(of('extensions') ? shownExtensions.map((row) => `extension:${row.extension.id}`) : []),
  ];
  const selection = useBulkSelection(switchable);
  const switchMany = (ids: string[], on: boolean) => {
    const picked = (prefix: string) =>
      ids.filter((id) => id.startsWith(prefix)).map((id) => id.slice(prefix.length));
    for (const id of picked('addon:')) {
      const addon = allAddons.find((candidate) => candidate.id === id);
      if (addon && addon.enabled !== on) setAddon.mutate({ id, enabled: on });
    }
    const extensionIds = [...picked('server:'), ...picked('extension:')];
    if (extensionIds.length > 0) switches.setMany(extensionIds, on);
  };
  const move = (index: number, step: -1 | 1) => {
    const ids = allAddons.map((addon) => addon.id);
    const [moved] = ids.splice(index, 1);
    if (moved === undefined) return;
    ids.splice(index + step, 0, moved);
    setOrder.mutate(ids);
  };
  const narrowed = query.trim() !== '' || media !== undefined;
  // With no repository yet (a new install starts with none), All suggests one too.
  const noRepos = !repos.isPending && allRepos.length === 0;
  const loading = repos.isPending || addons.isPending || servers.isPending || extensions.isPending;
  const nothing =
    !catalogue &&
    !loading &&
    (!of('repositories') || shownRepos.length === 0) &&
    (!of('addons') || shownAddons.length === 0) &&
    (!of('servers') || shownServers.length === 0) &&
    (!of('extensions') || shownExtensions.length === 0);
  // The catalogue has filters of its own: what sources are for doesn't follow it there.
  const pill = (next: { type?: SourcesTab; media?: SourceMedia }) => ({
    ...(next.type ? { type: next.type } : {}),
    ...(next.media && next.type !== 'catalogue' ? { media: next.media } : {}),
  });

  return (
    <section data-testid="sources-page" className="flex flex-col gap-3">
      <PageHeader
        back
        fallback="/browse"
        title={t('sources.title')}
        search={catalogue ? null : find.search}
        actions={
          <>
            {catalogue ? (
              <TopBarAction
                label={catalogRefresh.label}
                icon={RefreshCw}
                onClick={catalogRefresh.run}
              />
            ) : (
              find.action
            )}
            <TopBarAction
              label={t('sources.add.title')}
              icon={Plus}
              onClick={() => setAdding(true)}
            />
          </>
        }
      />
      <nav
        aria-label={t('sources.page.types')}
        className="-mx-4 flex gap-2 no-scrollbar overflow-x-auto px-4 lg:mx-0 lg:px-0"
      >
        {[undefined, ...SOURCES_TABS].map((option) => (
          <Link
            key={option ?? 'all'}
            to="/sources"
            search={pill({ type: option, media })}
            replace
            activeOptions={{ exact: true, includeSearch: true }}
            aria-current={option === type ? 'page' : undefined}
            className={chip(option === type)}
          >
            {option === 'catalogue'
              ? t('sources.page.type.catalogue')
              : option
                ? `${t(`sources.page.type.${option}`)} · ${counts[option]}`
                : t('sources.page.type.all')}
          </Link>
        ))}
      </nav>
      {catalogue && (
        <>
          {catalogRefresh.status}
          <ExtensionCatalog
            list={list}
            linkOf={(option) => ({
              to: '/sources',
              search: { type: 'catalogue', ...(option === 'all' ? {} : { list: option }) },
            })}
          />
        </>
      )}
      {!catalogue && (
        <div className="flex items-start gap-2">
          {mediaPills.length > 1 || media ? (
            <div
              role="group"
              aria-label={t('sources.page.media')}
              // To the screen's edge, unless Select sits at the row's end: pills would pass under it.
              className={`-ml-4 flex min-w-0 flex-1 gap-2 no-scrollbar overflow-x-auto pl-4 lg:ml-0 lg:pl-0 ${
                switchable.length > 0 ? '' : '-mr-4 pr-4 lg:mr-0 lg:pr-0'
              }`}
            >
              {mediaPills.map((option) => (
                <button
                  key={option}
                  type="button"
                  aria-pressed={media === option}
                  onClick={() =>
                    void navigate({
                      search: pill({ type, media: media === option ? undefined : option }),
                      replace: true,
                    })
                  }
                  className={chip(media === option)}
                >
                  {t(`sources.media.${option}`)}
                </button>
              ))}
            </div>
          ) : (
            <span className="flex-1" />
          )}
          {switchable.length > 0 && <SelectButton selection={selection} />}
        </div>
      )}
      {!catalogue && (
        <BulkSwitchBar
          selection={selection}
          busy={switches.busy || setAddon.isPending}
          onSwitch={switchMany}
        />
      )}
      {loading && !catalogue && (
        <LoaderCircle aria-label={t('watch.loading')} className="text-muted mx-auto animate-spin" />
      )}
      {!catalogue && (addons.error ?? setOrder.error) && (
        <SourceIssue compact>{(addons.error ?? setOrder.error)?.message}</SourceIssue>
      )}
      {nothing && (
        // Adding is the top bar's "+".
        <p className="text-muted px-1 py-4 text-sm">
          {t(narrowed ? 'sources.page.noMatches' : 'sources.page.empty')}
        </p>
      )}
      {of('repositories') &&
        (shownRepos.length > 0 || (!narrowed && (type === 'repositories' || noRepos))) && (
          <Section title={t('sources.page.type.repositories')}>
            {shownRepos.length > 0 && (
              <VirtualRows
                items={shownRepos}
                keyOf={({ repo }) => repo.id}
                rowRem={6}
                gapRem={0.5}
                className="flex flex-col gap-2"
                renderRow={({ repo }, _index, row) => (
                  <RepoRow repo={repo} listing={listings.get(repo.id)} row={row} />
                )}
              />
            )}
            {!narrowed && <SuggestedRepos />}
          </Section>
        )}
      {of('addons') && (shownAddons.length > 0 || (type === 'addons' && !narrowed)) && (
        <Section title={t('sources.page.type.addons')}>
          {shownAddons.length > 0 && (
            <ul aria-label={t('watch.installedAddons')} className="flex flex-col gap-2">
              {shownAddons.map((addon) => {
                const index = allAddons.indexOf(addon);
                return (
                  <AddonItem
                    key={addon.id}
                    addon={addon}
                    // The order is the whole list's: no moving while it's narrowed.
                    onMove={narrowed ? null : (step) => move(index, step)}
                    first={index === 0}
                    last={index === allAddons.length - 1}
                    selection={selection}
                    selectId={`addon:${addon.id}`}
                  />
                );
              })}
            </ul>
          )}
          {!narrowed && <AddonSuggestions />}
        </Section>
      )}
      {of('servers') && shownServers.length > 0 && (
        <Section title={t('sources.page.type.servers')}>
          <ul className="flex flex-col">
            {shownServers.map(({ server }) => (
              <ServerRow
                key={server.id}
                server={server}
                onSignIn={() => setSigningIn(server)}
                off={!switches.isEnabled(server.id)}
                leading={
                  <SelectCheck
                    selection={selection}
                    id={`server:${server.id}`}
                    name={server.name}
                  />
                }
                trailing={
                  <OnOffSwitch
                    checked={switches.isEnabled(server.id)}
                    label={t('sources.switch', { name: server.name })}
                    disabled={switches.busy}
                    onChange={(on) => switches.setMany([server.id], on)}
                  />
                }
              />
            ))}
          </ul>
        </Section>
      )}
      {servers.error && of('servers') && (
        <SourceIssue compact title={t('servers.loadFailed')}>
          {servers.error.message}
        </SourceIssue>
      )}
      {of('extensions') && shownExtensions.length > 0 && (
        <Section title={t('sources.page.type.extensions')}>
          <VirtualRows
            items={shownExtensions}
            keyOf={({ extension }) => `${extension.origin}-${extension.id}`}
            rowRem={4}
            className="flex flex-col"
            renderRow={({ extension }, _index, row) => (
              <ExtensionRow
                extension={extension}
                selection={selection}
                enabled={switches.isEnabled(extension.id)}
                busy={switches.busy}
                mark={rating.markOf('extensions', extension.id)}
                onSwitch={(on) => switches.setMany([extension.id], on)}
                row={row}
              />
            )}
          />
        </Section>
      )}
      {adding && <AddSourceSheet onClose={() => setAdding(false)} />}
      {signingIn && <ServerSheet server={signingIn} onClose={() => setSigningIn(null)} />}
    </section>
  );
};
