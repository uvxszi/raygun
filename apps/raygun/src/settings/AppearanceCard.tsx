import type { Theme } from '@raygun/bindings';
import type { FC } from 'react';

import { useSettings, useUpdateSetting } from './useSettings';

const THEME_OPTIONS: { value: Theme; label: string }[] = [
  { value: 'system', label: 'System' },
  { value: 'light', label: 'Light' },
  { value: 'dark', label: 'Dark' },
  { value: 'amoled', label: 'AMOLED' },
];

export const AppearanceCard: FC = () => {
  const settings = useSettings();
  const updateSetting = useUpdateSetting();

  if (!settings.data) {
    return null;
  }

  return (
    <section
      data-testid="appearance-card"
      className="bg-surface flex w-full max-w-md flex-col gap-3 rounded-xl px-5 py-4 text-sm"
    >
      <h2 id="theme-label" className="text-muted">
        Theme
      </h2>
      <div role="radiogroup" aria-labelledby="theme-label" className="grid grid-cols-4 gap-2">
        {THEME_OPTIONS.map((option) => {
          const selected = settings.data.theme === option.value;
          return (
            <button
              key={option.value}
              type="button"
              role="radio"
              aria-checked={selected}
              disabled={updateSetting.isPending}
              onClick={() => updateSetting.mutate({ key: 'theme', value: option.value })}
              className={
                selected
                  ? 'bg-primary text-background rounded-lg px-2 py-2 font-semibold'
                  : 'bg-background text-foreground rounded-lg px-2 py-2'
              }
            >
              {option.label}
            </button>
          );
        })}
      </div>
      {updateSetting.isError && (
        <p role="alert" className="text-primary">
          Couldn't save: {updateSetting.error.message}
        </p>
      )}
    </section>
  );
};
