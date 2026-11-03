import type { StremioAddon } from '@raygun/bindings';
import {
  ArrowDown,
  ArrowUp,
  Check,
  Copy,
  LoaderCircle,
  Plus,
  Power,
  PowerOff,
  Puzzle,
  Search,
  Settings2,
  Trash2,
} from 'lucide-react';
import { useState } from 'react';
import type { FC, FormEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { useLocalNetworkGate } from '../network/useLocalNetworkGate';
import { OverflowMenu } from '../shell/OverflowMenu';
import type { OverflowItem } from '../shell/OverflowMenu';
import { Sheet } from '../shell/Sheet';
import { SourceIssue } from '../shell/SourceIssue';
import { TopBarAction } from '../shell/TopBar';
import {
  useAddAddon,
  useAddonPreview,
  useConfigureAddon,
  useRemoveAddon,
  useSetAddonEnabled,
  useSetAddonOrder,
  useStremioAddons,
  useStremioSuggestions,
} from '../watch/useStremio';
import { PageHeader } from './PageHeader';
import { useSearchMode } from '../shell/useSearchMode';
import { matchesSearch } from '../shell/useListSearch';

const AddonLogo: FC<{ url: string | null }> = ({ url }) =>
  url ? (
    <img src={url} alt="" loading="lazy" className="size-10 shrink-0 rounded-lg object-contain" />
  ) : (
    <span
      aria-hidden
      className="bg-background text-muted grid size-10 shrink-0 place-items-center rounded-lg"
    >
      <Puzzle size={20} />
    </span>
  );

const chipClass = 'bg-background rounded-full px-2.5 py-0.5 text-xs';

/** What an addon provides, as chips: "Movie catalogs · 3", "Series details", "Streams". */
export const ProvidesChips: FC<{ addon: StremioAddon }> = ({ addon }) => {
  const { t } = useTranslation();
  const kind = (value: string | null) =>
    value
      ? t(`watch.kindOne.${value}`, {
          defaultValue: value.charAt(0).toUpperCase() + value.slice(1),
        })
      : '';
  const chips = addon.provides.map((entry) => {
    switch (entry.resource) {
      case 'catalog':
        return t('watch.provides.catalogs', { kind: kind(entry.kind), count: entry.count });
      case 'meta':
        return t('watch.provides.meta', { kind: kind(entry.kind) });
      case 'stream':
        return t('watch.provides.stream');
      case 'subtitles':
        return t('watch.provides.subtitles');
      default:
        return entry.resource;
    }
  });
  if (addon.p2p) chips.push(t('watch.provides.torrents'));
  if (chips.length === 0) return null;
  return (
    <ul aria-label={t('watch.providesOf', { name: addon.name })} className="flex flex-wrap gap-1.5">
      {chips.map((chip) => (
        <li key={chip} className={chipClass}>
          {chip}
        </li>
      ))}
    </ul>
  );
};

const AddonItem: FC<{
  addon: StremioAddon;
  onMove: ((step: -1 | 1) => void) | null;
  first: boolean;
  last: boolean;
}> = ({ addon, onMove, first, last }) => {
  const { t } = useTranslation();
  const remove = useRemoveAddon();
  const setEnabled = useSetAddonEnabled();
  const configure = useConfigureAddon();
  const error = remove.error ?? setEnabled.error ?? configure.error;
  const items: OverflowItem[] = [
    ...(onMove && !first
      ? [{ label: t('watch.moveUp'), icon: ArrowUp, onSelect: () => onMove(-1) }]
      : []),
    ...(onMove && !last
      ? [{ label: t('watch.moveDown'), icon: ArrowDown, onSelect: () => onMove(1) }]
      : []),
    {
      label: t(addon.enabled ? 'watch.turnOff' : 'watch.turnOn'),
      icon: addon.enabled ? PowerOff : Power,
      onSelect: () => setEnabled.mutate({ id: addon.id, enabled: !addon.enabled }),
    },
    ...(addon.configureUrl
      ? [{ label: t('watch.setUp'), icon: Settings2, onSelect: () => configure.mutate(addon.url) }]
      : []),
    {
      label: t('watch.copyAddress'),
      icon: Copy,
      onSelect: () => void navigator.clipboard?.writeText(addon.url),
    },
    {
      label: t('watch.removeAddon', { name: addon.name }),
      icon: Trash2,
      onSelect: () => remove.mutate(addon.id),
    },
  ];
  return (
    <li
      aria-label={addon.name}
      className={`bg-surface flex items-start gap-3 rounded-xl px-4 py-3 ${addon.enabled ? '' : 'opacity-60'}`}
    >
      <AddonLogo url={addon.logo} />
      <div className="flex min-w-0 flex-1 flex-col gap-1">
        <span className="font-medium wrap-anywhere">
          {addon.name}
          {addon.version && <span className="text-muted text-xs"> {addon.version}</span>}
        </span>
        {(!addon.enabled || (addon.failing && !addon.error)) && (
          <span className="text-muted text-xs">
            {[
              !addon.enabled && t('watch.addonOff'),
              addon.failing && !addon.error && t('watch.addonFailingLately'),
            ]
              .filter(Boolean)
              .join(' · ')}
          </span>
        )}
        {addon.error ? (
          <SourceIssue compact source={addon.name}>
            {addon.error}
          </SourceIssue>
        ) : (
          <>
            {addon.description && (
              <span className="text-muted line-clamp-2 text-xs">{addon.description}</span>
            )}
            <ProvidesChips addon={addon} />
          </>
        )}
        {error && <SourceIssue compact>{error.message}</SourceIssue>}
      </div>
      <div className="shrink-0">
        <OverflowMenu label={t('watch.addonActions', { name: addon.name })} items={items} />
      </div>
    </li>
  );
};

/** The addon an address leads to, before it's added: what it provides, and Add or Set up. */
const AddonPreview: FC<{
  addon: StremioAddon;
  adding: boolean;
  onAdd: () => void;
}> = ({ addon, adding, onAdd }) => {
  const { t } = useTranslation();
  const configure = useConfigureAddon();
  const added = addon.id !== '';
  return (
    <div
      aria-label={addon.name}
      role="group"
      className="bg-background flex flex-col gap-2 rounded-xl px-4 py-3"
    >
      <div className="flex items-start gap-3">
        <AddonLogo url={addon.logo} />
        <div className="flex min-w-0 flex-1 flex-col gap-0.5">
          <span className="font-medium wrap-anywhere">
            {addon.name}
            {addon.version && <span className="text-muted text-xs"> {addon.version}</span>}
          </span>
          {addon.description && (
            <span className="text-muted line-clamp-3 text-xs">{addon.description}</span>
          )}
        </div>
      </div>
      <ProvidesChips addon={addon} />
      {addon.needsSetup && <p className="text-muted text-xs">{t('watch.needsSetup')}</p>}
      <div className="flex flex-wrap justify-end gap-2">
        {addon.configureUrl && (
          <button
            type="button"
            onClick={() => configure.mutate(addon.url)}
            className="bg-surface flex items-center gap-1.5 rounded-full px-4 py-2 text-sm"
          >
            <Settings2 aria-hidden size={16} />
            {t('watch.setUp')}
          </button>
        )}
        {!addon.needsSetup && (
          <button
            type="button"
            disabled={adding || added}
            onClick={onAdd}
            className="bg-primary text-background flex items-center gap-1.5 rounded-full px-5 py-2 text-sm font-semibold disabled:opacity-60"
          >
            {adding ? (
              <LoaderCircle aria-hidden size={16} className="animate-spin" />
            ) : added ? (
              <Check aria-hidden size={16} />
            ) : (
              <Plus aria-hidden size={16} />
            )}
            {t(added ? 'watch.added' : 'watch.add')}
          </button>
        )}
      </div>
      {configure.error && <SourceIssue compact>{configure.error.message}</SourceIssue>}
    </div>
  );
};

/**
 * Stremio addons: what's installed (in the order details are asked in), well-known ones, and
 * "+" to add one by its address.
 */
export const WatchAddonsPage: FC = () => {
  const { t } = useTranslation();
  const find = useSearchMode(t('watch.findAddon'));
  const addons = useStremioAddons();
  const suggestions = useStremioSuggestions();
  const add = useAddAddon();
  const preview = useAddonPreview();
  const setOrder = useSetAddonOrder();
  const localNetwork = useLocalNetworkGate('addon');
  const [url, setUrl] = useState('');
  const [addingByUrl, setAddingByUrl] = useState(false);
  const missing = (suggestions.data ?? []).filter((suggestion) => !suggestion.added);
  const list = addons.data ?? [];
  const shown = list.filter((addon) => matchesSearch(find.query, [addon.name, addon.description]));

  const close = () => {
    setAddingByUrl(false);
    setUrl('');
    preview.reset();
    add.reset();
  };
  const onLookUp = (event: FormEvent) => {
    event.preventDefault();
    const address = url.trim();
    if (address) {
      add.reset();
      // An addon on the user's network needs Android 17's local network access first.
      void localNetwork.guard(address, () => preview.mutate(address));
    }
  };
  const onAdd = () => {
    const address = preview.data?.url ?? url.trim();
    add.mutate(address, { onSuccess: close });
  };
  const move = (index: number, step: -1 | 1) => {
    const ids = list.map((addon) => addon.id);
    const [moved] = ids.splice(index, 1);
    if (moved === undefined) return;
    ids.splice(index + step, 0, moved);
    setOrder.mutate(ids);
  };
  const looking = preview.isPending || localNetwork.checking;
  // The preview is of what was typed: typing again asks again.
  const current = preview.data && preview.variables === url.trim() ? preview.data : null;

  return (
    <section data-testid="watch-addons-page" className="flex flex-col gap-5">
      <PageHeader
        back
        fallback="/browse"
        title={t('watch.addons')}
        search={find.search}
        actions={
          <>
            {find.action}
            <TopBarAction
              label={t('watch.addAddon')}
              icon={Plus}
              onClick={() => setAddingByUrl(true)}
            />
          </>
        }
      />
      {add.error && !addingByUrl && (
        <p role="alert" className="text-muted text-sm wrap-anywhere">
          {add.error.message}
        </p>
      )}
      {addons.error && <SourceIssue>{addons.error.message}</SourceIssue>}
      {setOrder.error && <SourceIssue compact>{setOrder.error.message}</SourceIssue>}
      {addons.isPending && (
        <LoaderCircle aria-label={t('watch.loading')} className="text-muted mx-auto animate-spin" />
      )}
      {addons.data && list.length === 0 && (
        <p className="text-muted text-sm">{t('watch.noAddons')}</p>
      )}
      {shown.length > 0 && (
        <ul aria-label={t('watch.installedAddons')} className="flex flex-col gap-2">
          {shown.map((addon) => {
            const index = list.indexOf(addon);
            return (
              <AddonItem
                key={addon.id}
                addon={addon}
                // The order is the whole list's: no moving while it's narrowed by a search.
                onMove={find.query ? null : (step) => move(index, step)}
                first={index === 0}
                last={index === list.length - 1}
              />
            );
          })}
        </ul>
      )}
      {missing.length > 0 && (
        <section aria-labelledby="addon-suggestions" className="flex flex-col gap-2">
          <h2 id="addon-suggestions" className="font-semibold">
            {t('watch.suggestedAddons')}
          </h2>
          <ul className="flex flex-col gap-2">
            {missing.map((suggestion) => (
              <li
                key={suggestion.url}
                aria-label={suggestion.name}
                className="bg-surface flex items-center gap-3 rounded-xl px-4 py-3"
              >
                <div className="flex min-w-0 flex-1 flex-col gap-0.5">
                  <span className="font-medium">{suggestion.name}</span>
                  <span className="text-muted text-xs">{suggestion.description}</span>
                </div>
                <button
                  type="button"
                  aria-label={t('watch.addNamed', { name: suggestion.name })}
                  disabled={add.isPending}
                  onClick={() => add.mutate(suggestion.url)}
                  className="bg-primary text-background grid size-9 shrink-0 place-items-center rounded-full disabled:opacity-60"
                >
                  {suggestion.added ? (
                    <Check aria-hidden size={16} />
                  ) : (
                    <Plus aria-hidden size={16} />
                  )}
                </button>
              </li>
            ))}
          </ul>
        </section>
      )}
      {addingByUrl && (
        <Sheet title={t('watch.addAddon')} onClose={close}>
          <form onSubmit={onLookUp} className="flex flex-col gap-3 text-sm">
            <input
              type="text"
              inputMode="url"
              autoCapitalize="off"
              autoCorrect="off"
              spellCheck={false}
              autoFocus
              value={url}
              onChange={(event) => setUrl(event.target.value)}
              placeholder="https://…/manifest.json"
              aria-label={t('watch.addAddon')}
              className="bg-background placeholder:text-muted rounded-xl px-4 py-3 outline-none"
            />
            {(preview.error ?? add.error) && (
              <p role="alert" className="text-muted text-xs wrap-anywhere">
                {(add.error ?? preview.error)?.message}
              </p>
            )}
            {current ? (
              <AddonPreview addon={current} adding={add.isPending} onAdd={onAdd} />
            ) : (
              <button
                type="submit"
                disabled={!url.trim() || looking}
                className="bg-primary text-background flex items-center gap-1.5 self-end rounded-full px-5 py-2.5 font-semibold disabled:opacity-60"
              >
                {looking ? (
                  <LoaderCircle aria-hidden size={16} className="animate-spin" />
                ) : (
                  <Search aria-hidden size={16} />
                )}
                {t('watch.lookUp')}
              </button>
            )}
          </form>
        </Sheet>
      )}
      {localNetwork.sheet}
    </section>
  );
};
