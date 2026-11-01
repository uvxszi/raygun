import type { CatalogEntry } from '@raygun/bindings';
import { Globe, LoaderCircle } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { hostLabel } from './hostLabel';

import { useInstallExtension } from './useExtensions';

type InstallSheetProps = {
  entry: CatalogEntry;
  onClose: () => void;
};

/** Confirms an install or update after showing what the extension may access. */
export const InstallSheet: FC<InstallSheetProps> = ({ entry, onClose }) => {
  const { t } = useTranslation();
  const install = useInstallExtension();
  const updating = entry.state === 'updateAvailable';

  const confirm = () =>
    install.mutate({ repoId: entry.repoId, extensionId: entry.id }, { onSuccess: onClose });

  return (
    <div className="fixed inset-0 z-50 flex items-end justify-center bg-black/60 sm:items-center">
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby="install-title"
        className="bg-surface flex w-full max-w-md flex-col gap-4 rounded-t-2xl px-5 pt-5 pb-[calc(1.25rem+env(safe-area-inset-bottom))] sm:rounded-2xl"
      >
        <div className="flex flex-col gap-1">
          <h2 id="install-title" className="text-lg font-semibold">
            {t(updating ? 'extensions.updateTitle' : 'extensions.installTitle', {
              name: entry.name,
            })}
          </h2>
          <p className="text-muted text-sm">
            {entry.version}
            {entry.author ? ` · ${entry.author}` : ''}
          </p>
        </div>
        <div className="flex flex-col gap-2 text-sm">
          <h3 className="font-medium">{t('extensions.permissionsTitle')}</h3>
          {entry.nuclear ? (
            <p className="text-muted">{t('extensions.nuclearNetwork')}</p>
          ) : entry.net.length > 0 ? (
            <ul data-testid="install-permissions" className="flex flex-col gap-1">
              {entry.net.map((host) => (
                <li key={host} className="text-muted flex items-center gap-2 wrap-anywhere">
                  <Globe aria-hidden size={14} className="shrink-0" />
                  {hostLabel(host, t)}
                </li>
              ))}
            </ul>
          ) : (
            <p className="text-muted">{t('extensions.noNetwork')}</p>
          )}
        </div>
        {install.error && (
          <p role="alert" className="text-primary text-sm wrap-anywhere">
            {install.error.message}
          </p>
        )}
        <div className="flex justify-end gap-2">
          <button
            type="button"
            onClick={onClose}
            disabled={install.isPending}
            className="bg-background rounded-full px-5 py-2 text-sm disabled:opacity-50"
          >
            {t('extensions.cancel')}
          </button>
          <button
            type="button"
            onClick={confirm}
            disabled={install.isPending}
            className="bg-primary text-background flex items-center gap-2 rounded-full px-5 py-2 text-sm font-semibold disabled:opacity-70"
          >
            {install.isPending && <LoaderCircle aria-hidden size={16} className="animate-spin" />}
            {t(updating ? 'extensions.update' : 'extensions.install')}
          </button>
        </div>
      </div>
    </div>
  );
};
