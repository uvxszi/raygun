import { SlidersHorizontal } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import type { Transition } from 'motion/react';
import { useCallback, useDeferredValue, useMemo, useState } from 'react';
import type { FC, KeyboardEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { useExtensions, useKnownCatalog } from '../extensions/useExtensions';
import { nativeName } from '../i18n/languages';
import type { Medium } from '../manga/medium';
import { ListSearchField } from '../shell/ListSearchField';
import { SourceGrid } from './SourceGrid';
import {
  GROUPINGS,
  arrangeSources,
  bulkChoice,
  defaultView,
  gridSources,
  readView,
  saveView,
} from './customGrid';
import type { BulkAction, GridView, Grouping } from './customGrid';
import type { SourceOption } from './useCustomSources';

/** Past this many sources the panel offers its field, groups and order. */
const TOOLS_FROM = 12;
/** Past this many, sources come in groups until another way is picked. */
const GROUPS_FROM = 24;

const NOTHING: ReadonlySet<string> = new Set();

/** Custom sources: off, a search uses every source; on, the panel under it chooses them. */
export const CustomSourcesPill: FC<{
  on: boolean;
  /** Sources switched on, and how many there are. */
  count: number;
  total: number;
  panelId: string;
  onChange: (on: boolean) => void;
}> = ({ on, count, total, panelId, onChange }) => {
  const { t } = useTranslation();
  return (
    <button
      type="button"
      aria-pressed={on}
      aria-controls={on ? panelId : undefined}
      onClick={() => onChange(!on)}
      className={`flex shrink-0 items-center gap-2 rounded-full px-4 py-1.5 text-sm ${on ? 'bg-primary/20 text-primary font-semibold' : 'bg-surface'}`}
    >
      <SlidersHorizontal aria-hidden size={15} />
      {t('search.customSources.title')}
      {on && (
        <span className="text-xs font-normal tabular-nums opacity-80">
          {t('search.customSources.count', { count, total })}
        </span>
      )}
    </button>
  );
};

/**
 * How the panel unfolds, by the motion level in effect (`data-motion`, Settings → Appearance):
 * Lively and Calm open and close it; Reduced only fades it; Off shows it at once (motion's own
 * switch skips every animation then).
 */
const unfolding = (): Transition => {
  const level = document.documentElement.dataset.motion;
  if (level === 'reduced') return { height: { duration: 0 }, opacity: { duration: 0.12 } };
  if (level === 'off') return { duration: 0 };
  const duration = level === 'calm' ? 0.16 : 0.22;
  return { duration, ease: [0.2, 0.8, 0.2, 1] };
};

type PanelProps = {
  medium: Medium;
  sources: readonly SourceOption[];
  picked: readonly string[];
  onChange: (id: string, on: boolean) => void;
  /** Exactly these sources on (All, None, Invert, a group's switch). */
  onChoose: (ids: string[]) => void;
};

const toggled = (keys: readonly string[], key: string) =>
  keys.includes(key) ? keys.filter((other) => other !== key) : [...keys, key];

/**
 * The panel Custom sources unfolds: a tile per source in a grid that scrolls in its own box, with
 * how many are on, All, None and Invert for the tiles shown, and with many sources a field to
 * find them by name, groups (by repository or language, folding) and the ones on first.
 */
export const SourceSwitchPanel: FC<PanelProps & { open: boolean; id: string }> = ({
  open,
  id,
  ...panel
}) => (
  <AnimatePresence initial={false}>
    {open && (
      <motion.div
        key="custom-sources"
        id={id}
        data-testid="custom-sources-panel"
        initial={{ height: 0, opacity: 0 }}
        animate={{ height: 'auto', opacity: 1 }}
        exit={{ height: 0, opacity: 0 }}
        transition={unfolding()}
        className="overflow-hidden"
      >
        <PanelBody {...panel} />
      </motion.div>
    )}
  </AnimatePresence>
);

const PanelBody: FC<PanelProps> = ({ medium, sources, picked, onChange, onChoose }) => {
  const { t } = useTranslation();
  const many = sources.length > TOOLS_FROM;
  const [view, setViewState] = useState<GridView>(
    () => readView(medium) ?? defaultView(sources.length, GROUPS_FROM),
  );
  const setView = (next: GridView) => {
    setViewState(next);
    saveView(medium, next);
  };
  const [finding, setFinding] = useState('');
  // Typing stays quick however many tiles the grid redraws.
  const needle = useDeferredValue(finding.trim());
  // Groups folded while finding (every group with a match opens), forgotten after.
  const [foldedFinding, setFoldedFinding] = useState<readonly string[]>([]);
  const on = useMemo(() => new Set(picked), [picked]);
  // The order On first follows: the sources on when the list was last arranged (opened, found
  // in, grouped or reordered), so a tile switched doesn't jump away.
  const [first, setFirst] = useState<ReadonlySet<string>>(on);
  const grouping: Grouping = many ? view.grouping : 'none';
  const extensions = useExtensions().data;
  // Languages are the catalogue's (already read for Browse → Extensions; read once otherwise).
  const catalog = useKnownCatalog(grouping === 'language');
  const languages = grouping === 'language' ? catalog.data : undefined;
  const tiles = useMemo(
    () => gridSources(sources, extensions, languages),
    [sources, extensions, languages],
  );
  const labelOf = useCallback(
    (by: Grouping, key: string) =>
      by === 'repository'
        ? key || t('search.customSources.addedByYou')
        : key === 'multi'
          ? t('extensions.multiLanguage')
          : key
            ? nativeName(key)
            : t('search.customSources.unknownLanguage'),
    [t],
  );
  const groups = useMemo(
    () =>
      arrangeSources(tiles, {
        needle,
        // Until the catalogue is read, its languages aren't known.
        grouping: grouping === 'language' && !languages ? 'none' : grouping,
        first: view.onFirst ? first : null,
        labelOf,
      }),
    [tiles, needle, grouping, languages, view.onFirst, first, labelOf],
  );
  const headers = groups.length > 1;
  const foldedKeys = needle ? foldedFinding : view.folded;
  const folded = useMemo(
    () => (headers && foldedKeys.length > 0 ? new Set(foldedKeys) : NOTHING),
    [headers, foldedKeys],
  );
  // What's shown: All, None and Invert act on these.
  const shown = useMemo(
    () =>
      groups.flatMap((group) =>
        folded.has(group.key) ? [] : group.sources.map((source) => source.id),
      ),
    [groups, folded],
  );
  const matches = groups.reduce((count, group) => count + group.sources.length, 0);

  const rearrange = () => setFirst(on);
  const find = (value: string) => {
    setFinding(value);
    if (foldedFinding.length > 0) setFoldedFinding([]);
    rearrange();
  };
  // Down from the field goes on to the tiles.
  const fromField = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key !== 'ArrowDown' || !(event.target as HTMLElement).matches('input')) return;
    const stop = event.currentTarget.querySelector<HTMLElement>('[data-cell][tabindex="0"]');
    if (!stop) return;
    event.preventDefault();
    stop.focus();
  };
  const bulk = (action: BulkAction) => onChoose(bulkChoice(picked, shown, action));
  const fold = (key: string) => {
    if (needle) setFoldedFinding(toggled(foldedFinding, key));
    else setView({ ...view, folded: toggled(view.folded, key) });
  };
  const switchGroup = (key: string) => {
    const ids = groups.find((group) => group.key === key)?.sources.map((source) => source.id);
    if (!ids) return;
    onChoose(bulkChoice(picked, ids, ids.every((source) => on.has(source)) ? 'none' : 'all'));
  };
  const action = 'text-muted hover:text-foreground rounded-md px-2 py-1 text-sm';
  const chip = 'bg-background h-8 shrink-0 rounded-full px-3 text-xs';

  return (
    <div
      role="group"
      aria-label={t('search.sources')}
      onKeyDown={fromField}
      className="border-foreground/10 bg-surface @container/sources flex flex-col gap-2 rounded-lg border p-3"
    >
      {many && (
        // The field has a line of its own on a narrow panel; the groups and the order share one.
        <div className="flex flex-wrap items-center gap-2">
          <ListSearchField
            value={finding}
            onChange={find}
            label={t('search.findSource')}
            className="min-w-0 flex-1 basis-full @md/sources:basis-auto"
          />
          <select
            aria-label={t('search.customSources.groupBy')}
            value={view.grouping}
            onChange={(event) => {
              setView({ ...view, grouping: event.target.value as Grouping, folded: [] });
              setFoldedFinding([]);
              rearrange();
            }}
            className={`${chip} text-foreground outline-none`}
          >
            {GROUPINGS.map((by) => (
              <option key={by} value={by}>
                {t(`search.customSources.grouping.${by}`)}
              </option>
            ))}
          </select>
          <button
            type="button"
            aria-pressed={view.onFirst}
            onClick={() => {
              setView({ ...view, onFirst: !view.onFirst });
              rearrange();
            }}
            className={`${chip} ${view.onFirst ? 'bg-primary/20 text-primary font-semibold' : 'text-muted'}`}
          >
            {t('search.customSources.onFirst')}
          </button>
        </div>
      )}
      <div className="flex flex-wrap items-center gap-x-2 gap-y-1">
        <span className="text-muted text-sm tabular-nums">
          {t('search.customSources.on', { count: picked.length, total: sources.length })}
          {needle && ` · ${t('search.customSources.matching', { count: matches })}`}
        </span>
        <span className="ml-auto flex items-center gap-1">
          <button type="button" onClick={() => bulk('all')} className={action}>
            {t('search.customSources.all')}
          </button>
          <button type="button" onClick={() => bulk('none')} className={action}>
            {t('search.customSources.none')}
          </button>
          <button type="button" onClick={() => bulk('invert')} className={action}>
            {t('search.customSources.invert')}
          </button>
        </span>
      </div>
      {groups.length > 0 ? (
        <SourceGrid
          groups={groups}
          headers={headers}
          folded={folded}
          on={on}
          adultMark={t('common.adult')}
          adultLabel={t('common.adultLabel')}
          groupSwitchLabel={(name) => t('search.customSources.groupSwitch', { name })}
          onToggle={(source) => onChange(source, !on.has(source))}
          onFold={fold}
          onGroup={switchGroup}
        />
      ) : (
        needle && <p className="text-muted text-sm">{t('sources.page.noMatches')}</p>
      )}
    </div>
  );
};
