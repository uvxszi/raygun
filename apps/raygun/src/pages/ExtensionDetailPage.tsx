import { useNavigate, useSearch } from '@tanstack/react-router';
import { Globe } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { hostLabel } from '../extensions/hostLabel';

import { SettingsForm } from '../extensions/SettingsForm';
import { useExtension, useUninstallExtension } from '../extensions/useExtensions';
import { PageHeader } from './PageHeader';

export const ExtensionDetailPage: FC = () => {
  const { t } = useTranslation();
  const { id } = useSearch({ from: '/extensions/detail' });
  const navigate = useNavigate();
  const extension = useExtension(id);
  const uninstall = useUninstallExtension();
  // Uninstalling takes a second tap, so a stray one can't remove an extension.
  const [confirming, setConfirming] = useState(false);
  const info = extension.data;

  if (!info) {
    return (
      <section data-testid="extension-detail-page" className="flex flex-col gap-4">
        {extension.isSuccess && <p className="text-muted text-sm">{t('extensions.notFound')}</p>}
      </section>
    );
  }

  const onUninstall = () => {
    if (!confirming) {
      setConfirming(true);
      return;
    }
    uninstall.mutate(info.id, {
      onSuccess: (extensions) => {
        // A built-in copy may take its place; otherwise go back to the list.
        if (!extensions.some((other) => other.id === info.id)) {
          void navigate({ to: '/extensions', search: { tab: 'installed' }, replace: true });
        }
        setConfirming(false);
      },
    });
  };

  return (
    <section data-testid="extension-detail-page" className="flex max-w-2xl flex-col gap-4">
      <div className="flex flex-col gap-1">
        <PageHeader title={info.name} />
        <p className="text-muted text-sm">
          {[
            info.version,
            t(info.origin === 'builtIn' ? 'extensions.builtIn' : 'extensions.installedOrigin'),
          ].join(' · ')}
        </p>
        {info.description && <p className="text-sm">{info.description}</p>}
      </div>
      {info.status === 'failed' && (
        <p role="alert" className="text-primary text-sm wrap-anywhere">
          {t('extensions.failed', { message: info.error ?? '' })}
        </p>
      )}
      <section className="bg-surface flex flex-col gap-2 rounded-xl px-5 py-4 text-sm">
        <h2 className="text-muted">{t('extensions.permissionsTitle')}</h2>
        {info.net.length > 0 ? (
          <ul data-testid="extension-permissions" className="flex flex-col gap-1">
            {info.net.map((host) => (
              <li key={host} className="flex items-center gap-2 wrap-anywhere">
                <Globe aria-hidden size={14} className="text-muted shrink-0" />
                {hostLabel(host, t)}
              </li>
            ))}
          </ul>
        ) : (
          <p>{t('extensions.noNetwork')}</p>
        )}
        {info.providers.length > 0 && (
          <>
            <h2 className="text-muted mt-2">{t('extensions.providers')}</h2>
            <ul className="flex flex-col gap-1">
              {info.providers.map((provider) => (
                <li key={provider.id}>
                  {provider.name} <span className="text-muted text-xs">{provider.kind}</span>
                </li>
              ))}
            </ul>
          </>
        )}
      </section>
      {info.compatibility && (
        <section
          data-testid="extension-compatibility"
          className="bg-surface flex flex-col gap-2 rounded-xl px-5 py-4 text-sm"
        >
          <h2 className="text-muted">{t('extensions.compatibilityTitle')}</h2>
          <p>{t('extensions.compatibilityFrom', { package: info.compatibility.package })}</p>
          {info.compatibility.unsupportedCategories.length > 0 && (
            <p className="text-muted">
              {t('extensions.compatibilityCategories', {
                list: info.compatibility.unsupportedCategories.join(', '),
              })}
            </p>
          )}
          {info.compatibility.stubbedApis.length > 0 && (
            <p className="text-muted">
              {t('extensions.compatibilityApis', {
                list: info.compatibility.stubbedApis.join(', '),
              })}
            </p>
          )}
          {info.compatibility.uiImports.length > 0 && (
            <p className="text-muted">{t('extensions.compatibilityUi')}</p>
          )}
          {info.compatibility.unsupportedCategories.length === 0 &&
            info.compatibility.stubbedApis.length === 0 &&
            info.compatibility.uiImports.length === 0 && (
              <p className="text-muted">{t('extensions.compatibilityFull')}</p>
            )}
        </section>
      )}
      {info.settings.length > 0 && <SettingsForm extensionId={info.id} settings={info.settings} />}
      {info.origin === 'installed' && (
        <div className="flex flex-col gap-2">
          <button
            type="button"
            onClick={onUninstall}
            disabled={uninstall.isPending}
            className="bg-surface text-primary self-start rounded-full px-5 py-2 text-sm font-semibold disabled:opacity-50"
          >
            {t(confirming ? 'extensions.uninstallConfirm' : 'extensions.uninstall')}
          </button>
          {uninstall.error && (
            <p role="alert" className="text-primary text-sm wrap-anywhere">
              {uninstall.error.message}
            </p>
          )}
        </div>
      )}
    </section>
  );
};
