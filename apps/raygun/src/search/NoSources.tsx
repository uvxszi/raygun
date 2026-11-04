import { Link } from '@tanstack/react-router';
import { Package } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

/** Search with nothing to ask: what's missing, and the way to the extensions that bring it. */
export const NoSources: FC<{ label: string }> = ({ label }) => {
  const { t } = useTranslation();
  return (
    <div data-testid="no-sources" className="flex flex-col items-start gap-2">
      <p className="text-muted text-sm">{label}</p>
      <Link
        to="/browse"
        search={{ tab: 'extensions', list: 'all' }}
        className="bg-surface flex items-center gap-2 rounded-full px-4 py-2 text-sm font-semibold"
      >
        <Package aria-hidden size={16} />
        {t('search.findExtensions')}
      </Link>
    </div>
  );
};
