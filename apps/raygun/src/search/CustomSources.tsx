import { SlidersHorizontal } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import type { Transition } from 'motion/react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { AdultMark } from '../shell/AdultMark';
import { ListSearchField } from '../shell/ListSearchField';
import { SwitchTrack } from '../shell/OnOffSwitch';
import { matchesSearch } from '../shell/useListSearch';
import type { SourceOption } from './useCustomSources';

/** Sources in the panel before it folds; finding one by name shows every match. */
const FOLDED = 12;

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

/** One source's box: its name and its switch; the whole box switches it. */
const SourceBox: FC<{ source: SourceOption; on: boolean; onChange: (on: boolean) => void }> = ({
  source,
  on,
  onChange,
}) => {
  const { t } = useTranslation();
  return (
    <button
      type="button"
      role="switch"
      aria-checked={on}
      aria-label={source.adult ? `${source.name}, ${t('common.adultLabel')}` : source.name}
      onClick={() => onChange(!on)}
      className={`flex min-w-0 items-center gap-2 rounded-md border px-3 py-2 text-left text-sm ${
        on ? 'border-primary/40 bg-primary/10' : 'border-foreground/10 bg-background'
      }`}
    >
      <span className="min-w-0 flex-1 truncate">{source.name}</span>
      {source.adult && <AdultMark />}
      <SwitchTrack checked={on} />
    </button>
  );
};

/**
 * The panel Custom sources unfolds: a box with a switch per source, in the order the sources are
 * listed (as their chips were), All and None, and a field to find one in a long list (which folds
 * after a dozen).
 */
export const SourceSwitchPanel: FC<{
  open: boolean;
  id: string;
  sources: readonly SourceOption[];
  picked: readonly string[];
  onChange: (id: string, on: boolean) => void;
  onAll: (on: boolean) => void;
}> = ({ open, id, sources, picked, onChange, onAll }) => {
  const { t } = useTranslation();
  const [finding, setFinding] = useState('');
  const [unfolded, setUnfolded] = useState(false);
  const needle = finding.trim();
  // Folded, the first dozen show; finding by name shows every match.
  const shown = needle
    ? sources.filter((source) => matchesSearch(needle, [source.name]))
    : unfolded || sources.length <= FOLDED
      ? sources
      : sources.slice(0, FOLDED);
  const on = new Set(picked);
  const action = 'text-muted hover:text-foreground rounded-md px-2 py-1 text-sm';
  return (
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
          <div
            role="group"
            aria-label={t('search.sources')}
            className="border-foreground/10 bg-surface flex flex-col gap-3 rounded-lg border p-3"
          >
            <div className="flex flex-wrap items-center gap-2">
              <span className="text-muted text-sm">
                {t('search.customSources.on', { count: picked.length, total: sources.length })}
              </span>
              <span className="ml-auto flex items-center gap-1">
                {sources.length > FOLDED && (
                  <ListSearchField
                    value={finding}
                    onChange={setFinding}
                    label={t('search.findSource')}
                    className="w-40"
                  />
                )}
                <button type="button" onClick={() => onAll(true)} className={action}>
                  {t('search.customSources.all')}
                </button>
                <button type="button" onClick={() => onAll(false)} className={action}>
                  {t('search.customSources.none')}
                </button>
              </span>
            </div>
            {shown.length > 0 && (
              <div className="grid grid-cols-[repeat(auto-fill,minmax(10.5rem,1fr))] gap-2">
                {shown.map((source) => (
                  <SourceBox
                    key={source.id}
                    source={source}
                    on={on.has(source.id)}
                    onChange={(next) => onChange(source.id, next)}
                  />
                ))}
              </div>
            )}
            {needle && shown.length === 0 && (
              <p className="text-muted text-sm">{t('sources.page.noMatches')}</p>
            )}
            {sources.length > FOLDED && !needle && (
              <button
                type="button"
                aria-expanded={unfolded}
                onClick={() => setUnfolded(!unfolded)}
                className="text-primary self-start rounded-md px-2 py-1 text-sm"
              >
                {unfolded
                  ? t('search.fewerSources')
                  : t('search.moreSources', { count: sources.length - FOLDED })}
              </button>
            )}
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
};
