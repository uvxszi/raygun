import { Link } from '@tanstack/react-router';
import { ChevronRight } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { ROUTES } from '../manga/medium';
import type { Medium } from '../manga/medium';
import { useMangaSources } from '../manga/useManga';
import { PageHeader } from './PageHeader';

/** The installed sources of a medium. */
export const MangaSourcesPage: FC<{ medium?: Medium }> = ({ medium = 'manga' }) => {
  const { t } = useTranslation();
  const sources = useMangaSources(medium);

  return (
    <section data-testid="manga-sources-page" className="flex flex-col gap-4">
      <PageHeader title={t(medium === 'novel' ? 'novel.sources' : 'manga.sources')} />
      {sources.data?.length === 0 && (
        <p className="text-muted text-sm">
          {t(medium === 'novel' ? 'novel.noSources' : 'manga.noSources')}{' '}
          <Link to="/extensions" search={{ tab: 'browse' }} className="text-primary">
            {t('manga.getSources')}
          </Link>
        </p>
      )}
      <ul className="flex flex-col gap-2">
        {sources.data?.map((source) => (
          <li key={source.id}>
            <Link
              to={ROUTES[medium].source}
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

export const NovelSourcesPage: FC = () => <MangaSourcesPage medium="novel" />;
