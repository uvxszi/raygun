import { useQueryClient } from '@tanstack/react-query';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useSettings, useUpdateSetting } from './useSettings';

/** Settings → Content: the NSFW switch (off by default). */
export const ContentCard: FC = () => {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const settings = useSettings();
  const updateSetting = useUpdateSetting();
  const on = settings.data?.showNsfw ?? false;

  const toggle = () =>
    updateSetting.mutate(
      { key: 'showNsfw', value: !on },
      // Sources, listings and the library all depend on it.
      { onSuccess: () => void queryClient.invalidateQueries() },
    );

  return (
    <section
      data-testid="content-card"
      className="bg-surface flex w-full flex-col gap-3 rounded-xl px-5 py-4 text-sm"
    >
      <h2 className="text-muted">{t('settings.content')}</h2>
      <div className="flex items-center gap-3">
        <div className="flex min-w-0 flex-1 flex-col gap-1">
          <span id="nsfw-label" className="font-medium">
            {t('settings.showNsfw')}
          </span>
          <span className="text-muted text-xs">{t('settings.showNsfwHint')}</span>
        </div>
        <button
          type="button"
          role="switch"
          aria-checked={on}
          aria-labelledby="nsfw-label"
          disabled={!settings.data || updateSetting.isPending}
          onClick={toggle}
          className={`relative h-7 w-12 shrink-0 rounded-full transition-colors disabled:opacity-60 ${on ? 'bg-primary' : 'bg-background'}`}
        >
          <span
            aria-hidden
            className={`bg-foreground absolute top-1 size-5 rounded-full transition-all ${on ? 'left-6' : 'left-1'}`}
          />
        </button>
      </div>
    </section>
  );
};
