import { Lock } from 'lucide-react';
import { useId } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useSettings, useUpdateSetting } from '../settings/useSettings';
import { searchFilterOf } from './searchFilter';

const Toggle: FC<{
  label: string;
  on: boolean;
  forced: boolean;
  hint: string;
  onToggle: () => void;
}> = ({ label, on, forced, hint, onToggle }) => {
  const hintId = useId();
  return (
    <>
      <button
        type="button"
        role="switch"
        aria-checked={on}
        disabled={forced}
        title={forced ? hint : undefined}
        aria-describedby={forced ? hintId : undefined}
        onClick={onToggle}
        className={`flex shrink-0 items-center gap-1.5 rounded-full border px-3 py-1 text-xs ${
          on ? 'border-primary/40 bg-primary/15 text-primary' : 'border-border text-muted'
        } disabled:opacity-70`}
      >
        {forced ? (
          <Lock aria-hidden size={12} />
        ) : (
          <span
            aria-hidden
            className={`size-2 rounded-full ${on ? 'bg-primary' : 'border-border border'}`}
          />
        )}
        {label}
      </button>
      {forced && (
        <span id={hintId} className="sr-only">
          {hint}
        </span>
      )}
    </>
  );
};

/**
 * Search's "No 18+" and "No explicit" switches, kept in `Settings::searchFilter` so they sync.
 * Where Show NSFW or Explicit music already hide that content they show on and can't be changed.
 */
export const SearchFilterToggles: FC = () => {
  const { t } = useTranslation();
  const settings = useSettings().data;
  const update = useUpdateSetting();
  if (!settings) return null;
  const filter = searchFilterOf(settings);
  const own = settings.searchFilter;
  const set = (change: Partial<typeof own>) =>
    update.mutate({ key: 'searchFilter', value: { ...own, ...change } });
  return (
    <div role="group" aria-label={t('search.filter.label')} className="flex shrink-0 gap-2">
      <Toggle
        label={t('search.filter.noAdult')}
        on={filter.noAdult}
        forced={filter.adultForced}
        hint={t('search.filter.adultHidden')}
        onToggle={() => set({ noAdult: !own.noAdult })}
      />
      <Toggle
        label={t('search.filter.noExplicit')}
        on={filter.noExplicit}
        forced={filter.explicitForced}
        hint={t('search.filter.explicitHidden')}
        onToggle={() => set({ noExplicit: !own.noExplicit })}
      />
    </div>
  );
};
