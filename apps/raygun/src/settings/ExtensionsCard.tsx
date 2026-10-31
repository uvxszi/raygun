import { RefreshCw } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useExtensions, useReloadExtensions } from '../search/useOnlineSearch';

/** What's loaded, from where, and which hosts each extension may reach. */
export const ExtensionsCard: FC = () => {
  const { t } = useTranslation();
  const extensions = useExtensions();
  const reload = useReloadExtensions();
  const error = extensions.error ?? reload.error;

  return (
    <section
      data-testid="extensions-card"
      className="bg-surface flex flex-col gap-3 rounded-xl px-5 py-4 text-sm"
    >
      <div className="flex items-center justify-between">
        <h2 className="text-muted">{t('settings.extensions')}</h2>
        <button
          type="button"
          onClick={() => reload.mutate()}
          disabled={reload.isPending}
          className="text-muted flex items-center gap-1.5 text-xs disabled:opacity-50"
        >
          <RefreshCw aria-hidden size={14} className={reload.isPending ? 'animate-spin' : ''} />
          {t('settings.reloadExtensions')}
        </button>
      </div>
      {error && (
        <p role="alert" className="text-primary">
          {t('common.coreUnavailable', { message: error.message })}
        </p>
      )}
      {extensions.data?.length === 0 && <p className="text-muted">{t('settings.noExtensions')}</p>}
      <ul className="flex flex-col gap-3">
        {extensions.data?.map((extension) => (
          <li key={`${extension.origin}-${extension.id}`} data-testid="extension-row">
            <p className="flex flex-wrap items-baseline gap-x-2">
              <span className="font-medium">{extension.name}</span>
              {extension.version && <span className="text-muted text-xs">{extension.version}</span>}
              <span className="text-muted text-xs">
                {t(
                  extension.origin === 'builtIn'
                    ? 'settings.extensionBuiltIn'
                    : 'settings.extensionInstalled',
                )}
              </span>
            </p>
            {extension.status === 'failed' ? (
              <p role="alert" className="text-primary text-xs break-words">
                {t('settings.extensionFailed', { message: extension.error ?? '' })}
              </p>
            ) : (
              <p className="text-muted text-xs break-all">
                {extension.net.length > 0
                  ? t('settings.extensionNet', { hosts: extension.net.join(', ') })
                  : t('settings.extensionNoNet')}
              </p>
            )}
          </li>
        ))}
      </ul>
    </section>
  );
};
