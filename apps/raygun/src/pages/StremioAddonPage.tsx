import { Link, useSearch } from '@tanstack/react-router';
import { LoaderCircle } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { AdultMark } from '../shell/AdultMark';
import { TopBar } from '../shell/TopBar';
import { StremioCatalogRowView } from '../watch/StremioRow';
import { useStremioAddons, useStremioCatalogs } from '../watch/useStremio';
import { SourceIssue } from '../shell/SourceIssue';

/**
 * One Stremio addon's catalogues as rows (CloudStream's provider home), opened from Browse; the
 * ones that need a choice first (a year, a list) open in Discover.
 */
export const StremioAddonPage: FC = () => {
  const { t } = useTranslation();
  const { id } = useSearch({ from: '/watch/addon' });
  const addons = useStremioAddons();
  const catalogs = useStremioCatalogs();
  const addon = addons.data?.find((candidate) => candidate.id === id);
  const own = catalogs.data?.filter((catalog) => catalog.addonId === id) ?? [];
  const rows = own.filter((catalog) => catalog.board);
  const discover = own.filter((catalog) => !catalog.board && catalog.discoverable);
  const kind = (type: string) => t(`watch.kind.${type}`, { defaultValue: type });

  return (
    <section data-testid="stremio-addon-page" className="flex flex-col gap-6">
      <TopBar
        back
        fallback="/browse"
        title={addon?.name ?? t('watch.addons')}
        titleMark={addon?.adult && <AdultMark />}
        subtitle={addon?.description}
      />
      {catalogs.isPending && (
        <LoaderCircle aria-label={t('watch.loading')} className="text-muted mx-auto animate-spin" />
      )}
      {catalogs.error && <SourceIssue>{catalogs.error.message}</SourceIssue>}
      {catalogs.data && rows.length === 0 && discover.length === 0 && (
        <p className="text-muted text-sm">{t('watch.noCatalogs')}</p>
      )}
      {discover.length > 0 && (
        <ul aria-label={t('watch.discover')} className="flex flex-wrap gap-2">
          {discover.map((catalog) => (
            <li key={`${catalog.kind}-${catalog.catalogId}`}>
              <Link
                to="/watch/catalog"
                search={{ addon: catalog.addonId, type: catalog.kind, catalog: catalog.catalogId }}
                className="bg-surface block rounded-full px-4 py-1.5 text-sm"
              >
                {`${catalog.name} · ${kind(catalog.kind)}`}
              </Link>
            </li>
          ))}
        </ul>
      )}
      {rows.map((catalog) => (
        <StremioCatalogRowView key={`${catalog.kind}-${catalog.catalogId}`} catalog={catalog} />
      ))}
    </section>
  );
};
