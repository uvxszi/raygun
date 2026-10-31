import { Search } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { PageHeader } from './PageHeader';

export const SearchPage: FC = () => {
  const { t } = useTranslation();
  return (
    <section data-testid="search-page" className="flex flex-col gap-6">
      <PageHeader title={t('search.title')} />
      <label className="bg-surface flex items-center gap-3 rounded-xl px-4 py-3">
        <Search aria-hidden size={20} className="text-muted" />
        <input
          type="search"
          disabled
          placeholder={t('search.placeholder')}
          aria-label={t('search.title')}
          className="placeholder:text-muted w-full bg-transparent outline-none"
        />
      </label>
      <p className="text-muted text-sm">{t('search.emptyBody')}</p>
    </section>
  );
};
