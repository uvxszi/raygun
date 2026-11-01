import { Link } from '@tanstack/react-router';
import { ChevronRight } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useMangaSources } from '../manga/useManga';
import { PageHeader } from './PageHeader';

/** The installed manga sources. */
export const MangaSourcesPage: FC = () => {
  const { t } = useTranslation();
  const sources = useMangaSources();

  return (
    <section data-testid="manga-sources-page" className="flex flex-col gap-4">
      <PageHeader title={t('manga.sources')} />
      {sources.data?.length === 0 && (
        <p className="text-muted text-sm">
          {t('manga.noSources')}{' '}
          <Link to="/extensions" search={{ tab: 'browse' }} className="text-primary">
            {t('manga.getSources')}
          </Link>
        </p>
      )}
      <ul className="flex flex-col gap-2">
        {sources.data?.map((source) => (
          <li key={source.id}>
            <Link
              to="/manga/source"
              search={{ id: source.id }}
              className="bg-surface flex items-center justify-between rounded-xl px-5 py-4"
            >
              <span className="font-medium">{source.name}</span>
              <ChevronRight aria-hidden size={18} className="text-muted" />
            </Link>
          </li>
        ))}
      </ul>
    </section>
  );
};
