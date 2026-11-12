import type { StremioAddon } from '@raygun/bindings';
import {
  ArrowDown,
  ArrowUp,
  Check,
  Copy,
  Plus,
  Power,
  PowerOff,
  Puzzle,
  Settings2,
  Trash2,
} from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useContentRating } from '../extensions/contentRating';
import { NotForRaygunNote, RemoveUnused } from '../extensions/NotForRaygun';
import { RatingHint } from '../extensions/RatingHint';
import { AdultMark } from '../shell/AdultMark';
import { SelectCheck } from '../shell/BulkSwitch';
import { OnOffSwitch } from '../shell/OnOffSwitch';
import { OverflowMenu } from '../shell/OverflowMenu';
import type { OverflowItem } from '../shell/OverflowMenu';
import { Picture } from '../shell/Picture';
import { SourceIssue } from '../shell/SourceIssue';
import type { BulkSelection } from '../shell/useBulkSelection';
import { AddonConfigSheet } from './AddonConfig';
import {
  hasConfigForm,
  useAddAddon,
  useConfigureAddon,
  useRemoveAddon,
  useSetAddonEnabled,
  useStremioSuggestions,
} from './useStremio';

/** An addon's logo, fading in, or a puzzle piece without one (or when it fails to load). */
export const AddonLogo: FC<{ url: string | null }> = ({ url }) => (
  <Picture
    src={url}
    fit="object-contain"
    placeholder={
      <span className="bg-background text-muted grid size-full place-items-center">
        <Puzzle size={20} />
      </span>
    }
    className="size-10 shrink-0 rounded-lg"
  />
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

/**
 * An installed addon (Sources → Addons): what it provides, whether it's off or failing, its
 * switch, and its menu (order, on and off, setup, 18+, copy, remove). `onMove` is left out while
 * the list is narrowed, since the order is the whole list's. Setting it up opens Raygun's own
 * form when its manifest declares its settings, else its website. One that does nothing in
 * Raygun says why and offers Remove in place of its switch.
 */
export const AddonItem: FC<{
  addon: StremioAddon;
  onMove: ((step: -1 | 1) => void) | null;
  first: boolean;
  last: boolean;
  selection: BulkSelection;
  /** Its id in the selection. */
  selectId: string;
}> = ({ addon, onMove, first, last, selection, selectId }) => {
  const { t } = useTranslation();
  const remove = useRemoveAddon();
  const setEnabled = useSetAddonEnabled();
  const configure = useConfigureAddon();
  const [settingUp, setSettingUp] = useState(false);
  const error = remove.error ?? setEnabled.error ?? configure.error;
  const rating = useContentRating();
  const setUp = hasConfigForm(addon)
    ? () => setSettingUp(true)
    : addon.configureUrl
      ? () => configure.mutate(addon.url)
      : null;
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
    ...(setUp ? [{ label: t('watch.setUp'), icon: Settings2, onSelect: setUp }] : []),
    ...rating.menuItems('addons', addon.id, addon.adult),
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
      <SelectCheck selection={selection} id={selectId} name={addon.name} />
      <AddonLogo url={addon.logo} />
      <div className="flex min-w-0 flex-1 flex-col gap-1">
        <span className="font-medium wrap-anywhere">
          {addon.name}
          {addon.adult && <AdultMark className="ml-1.5" />}
          {addon.version && <span className="text-muted text-xs"> {addon.version}</span>}
          <RatingHint mark={rating.markOf('addons', addon.id)} className="ml-1.5" />
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
            {addon.notForRaygun ? (
              <NotForRaygunNote reason={addon.notForRaygun} />
            ) : (
              addon.description && (
                <span className="text-muted line-clamp-2 text-xs">{addon.description}</span>
              )
            )}
            <ProvidesChips addon={addon} />
          </>
        )}
        {error && <SourceIssue compact>{error.message}</SourceIssue>}
      </div>
      <div className="flex shrink-0 items-center">
        {addon.notForRaygun ? (
          <RemoveUnused
            name={addon.name}
            tone="bg-background"
            busy={remove.isPending}
            onRemove={() => remove.mutate(addon.id)}
          />
        ) : (
          <OnOffSwitch
            checked={addon.enabled}
            label={t('sources.switch', { name: addon.name })}
            disabled={setEnabled.isPending}
            onChange={(on) => setEnabled.mutate({ id: addon.id, enabled: on })}
          />
        )}
        <OverflowMenu label={t('watch.addonActions', { name: addon.name })} items={items} />
      </div>
      {settingUp && <AddonConfigSheet addon={addon} onClose={() => setSettingUp(false)} />}
    </li>
  );
};

/** Well-known addons not added yet, each added with one tap. */
export const AddonSuggestions: FC = () => {
  const { t } = useTranslation();
  const suggestions = useStremioSuggestions();
  const add = useAddAddon();
  const missing = (suggestions.data ?? []).filter((suggestion) => !suggestion.added);
  if (missing.length === 0) return null;
  return (
    <section aria-labelledby="addon-suggestions" className="flex flex-col gap-2">
      <h3 id="addon-suggestions" className="text-muted px-1 text-sm font-medium">
        {t('watch.suggestedAddons')}
      </h3>
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
              {suggestion.added ? <Check aria-hidden size={16} /> : <Plus aria-hidden size={16} />}
            </button>
          </li>
        ))}
      </ul>
      {add.error && <SourceIssue compact>{add.error.message}</SourceIssue>}
    </section>
  );
};
