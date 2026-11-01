import { Link } from '@tanstack/react-router';
import { ChevronRight, RefreshCw } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useSettings } from '../settings/useSettings';
import { useExtensions, useReloadExtensions } from './useExtensions';

export const InstalledTab: FC = () => {
  const { t } = useTranslation();
  const extensions = useExtensions();
  const showNsfw = useSettings().data?.showNsfw ?? false;
  const reload = useReloadExtensions();
  const error = extensions.error ?? reload.error;

  return (
    <div data-testid="installed-tab" className="flex flex-col gap-3">
      <div className="flex justify-end">
        <button
          type="button"
          onClick={() => reload.mutate()}
          disabled={reload.isPending}
          className="text-muted flex items-center gap-1.5 text-sm disabled:opacity-50"
        >
          <RefreshCw aria-hidden size={14} className={reload.isPending ? 'animate-spin' : ''} />
          {t('extensions.reload')}
        </button>
      </div>
      {error && (
        <p role="alert" className="text-primary text-sm wrap-anywhere">
          {t('common.coreUnavailable', { message: error.message })}
        </p>
      )}
      {extensions.data?.length === 0 && (
        <p className="text-muted text-sm">{t('extensions.noneInstalled')}</p>
      )}
      <ul className="bg-surface flex flex-col rounded-xl">
        {extensions.data?.map((extension) => (
          <li key={`${extension.origin}-${extension.id}`}>
            <Link
              to="/extensions/detail"
              search={{ id: extension.id }}
              data-testid="extension-row"
              className="flex items-center gap-3 px-4 py-3"
            >
              <span className="flex min-w-0 flex-1 flex-col">
                <span className="flex flex-wrap items-baseline gap-x-2">
                  <span className="font-medium">{extension.name}</span>
                  <span className="text-muted text-xs">{extension.version}</span>
                  <span className="text-muted text-xs">
                    {t(
                      extension.origin === 'builtIn'
                        ? 'extensions.builtIn'
                        : 'extensions.installedOrigin',
                    )}
                  </span>
                  {extension.contentRating === 'nsfw' && (
                    <span className="bg-primary/20 text-primary rounded px-1.5 text-xs font-semibold">
                      {t('extensions.nsfwBadge')}
                    </span>
                  )}
                </span>
                {extension.contentRating === 'nsfw' && !showNsfw && (
                  <span className="text-muted text-xs">{t('extensions.nsfwHidden')}</span>
                )}
                {extension.status === 'failed' ? (
                  <span role="alert" className="text-primary truncate text-xs">
                    {t('extensions.failed', { message: extension.error ?? '' })}
                  </span>
                ) : (
                  extension.description && (
                    <span className="text-muted truncate text-xs">{extension.description}</span>
                  )
                )}
              </span>
              <ChevronRight aria-hidden size={18} className="text-muted shrink-0" />
            </Link>
          </li>
        ))}
      </ul>
    </div>
  );
};
