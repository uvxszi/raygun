import { Link } from '@tanstack/react-router';
import { ChevronRight } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useExtensions } from '../extensions/useExtensions';

/** A summary of loaded extensions, linking to the extensions page. */
export const ExtensionsCard: FC = () => {
  const { t } = useTranslation();
  const extensions = useExtensions();
  const failed = extensions.data?.filter((extension) => extension.status === 'failed').length ?? 0;

  return (
    <Link
      to="/extensions"
      data-testid="extensions-card"
      className="bg-surface flex items-center gap-3 rounded-xl px-5 py-4 text-sm"
    >
      <span className="flex min-w-0 flex-1 flex-col gap-1">
        <span className="text-muted">{t('settings.extensions')}</span>
        <span>
          {extensions.data
            ? extensions.data.map((extension) => extension.name).join(', ') ||
              t('extensions.noneInstalled')
            : ''}
        </span>
        {failed > 0 && (
          <span role="alert" className="text-primary text-xs">
            {t('settings.extensionsFailed', { count: failed })}
          </span>
        )}
      </span>
      <span className="text-muted flex items-center gap-1">
        {t('settings.manageExtensions')}
        <ChevronRight aria-hidden size={18} />
      </span>
    </Link>
  );
};
