import { Library } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { PageHeader } from './PageHeader';

export const LibraryPage: FC = () => {
  const { t } = useTranslation();
  return (
    <section data-testid="library-page" className="flex flex-col gap-6">
      <PageHeader title={t('library.title')} />
      <div className="bg-surface flex flex-col items-center gap-3 rounded-xl px-6 py-10 text-center">
        <Library aria-hidden size={40} className="text-muted" />
        <p className="font-semibold">{t('library.emptyTitle')}</p>
        <p className="text-muted text-sm">{t('library.emptyBody')}</p>
      </div>
    </section>
  );
};
