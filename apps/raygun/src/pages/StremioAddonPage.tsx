import { useSearch } from '@tanstack/react-router';
import { LoaderCircle } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { TopBar } from '../shell/TopBar';
import { StremioRow } from '../watch/StremioRow';
import { useStremioAddons, useStremioHome } from '../watch/useStremio';

/** One Stremio addon's catalogues as rows (CloudStream's provider home), opened from Browse. */
export const StremioAddonPage: FC = () => {
  const { t } = useTranslation();
  const { id } = useSearch({ from: '/watch/addon' });
  const addons = useStremioAddons();
  const home = useStremioHome();
  const addon = addons.data?.find((candidate) => candidate.id === id);
  const rows = home.data?.filter((row) => row.addonId === id) ?? [];

  return (
    <section data-testid="stremio-addon-page" className="flex flex-col gap-6">
      <TopBar
        back
        fallback="/browse"
        title={addon?.name ?? t('watch.addons')}
        subtitle={addon?.description}
      />
      {home.isPending && (
        <LoaderCircle aria-label={t('watch.loading')} className="text-muted mx-auto animate-spin" />
      )}
      {home.error && (
        <p role="alert" className="text-primary text-sm wrap-anywhere">
          {home.error.message}
        </p>
      )}
      {home.data && rows.length === 0 && (
        <p className="text-muted text-sm">{t('watch.noCatalogs')}</p>
      )}
      {rows.map((row) => (
        <StremioRow key={`${row.kind}-${row.catalogId}`} row={row} />
      ))}
    </section>
  );
};
