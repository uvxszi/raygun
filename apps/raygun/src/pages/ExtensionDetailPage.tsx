import { useNavigate, useSearch } from '@tanstack/react-router';
import { Blocks, Globe } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { hostLabel } from '../extensions/hostLabel';

import { SettingsForm } from '../extensions/SettingsForm';
import { useExtension, useUninstallExtension } from '../extensions/useExtensions';
import { LetterAvatar } from '../shell/LetterAvatar';
import { PageHeader } from './PageHeader';
import { SourceIssue } from '../shell/SourceIssue';

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
          void navigate({ to: '/browse', search: { tab: 'extensions' }, replace: true });
        }
        setConfirming(false);
      },
    });
  };

  return (
    <section data-testid="extension-detail-page" className="flex max-w-2xl flex-col gap-4">
      <PageHeader back fallback="/browse" plainTitle title={t('extensions.title')} />
      <header className="flex items-center gap-4">
        <LetterAvatar name={info.name} logo={info.icon} size="lg" />
        <div className="flex min-w-0 flex-col gap-0.5">
          <h1 className="text-xl font-bold tracking-tight wrap-anywhere">{info.name}</h1>
          <p className="text-muted text-sm">
            {[
              info.version,
              t(info.origin === 'builtIn' ? 'extensions.builtIn' : 'extensions.installedOrigin'),
            ].join(' · ')}
          </p>
        </div>
      </header>
      {info.description && <p className="text-sm leading-relaxed">{info.description}</p>}
      {info.status === 'failed' && (
        <SourceIssue>{t('extensions.failed', { message: info.error ?? '' })}</SourceIssue>
      )}
      {info.providers.length > 0 && (
        <section aria-labelledby="extension-provides" className="flex flex-col gap-1 text-sm">
          <h2 id="extension-provides" className="text-primary px-1 pt-2 font-semibold">
            {t('extensions.providers')}
          </h2>
          <ul className="flex flex-col">
            {info.providers.map((provider) => (
              <li key={provider.id} className="flex items-center gap-3 px-1 py-2">
                <Blocks aria-hidden size={18} className="text-muted shrink-0" />
                <span className="flex min-w-0 flex-col">
                  <span className="font-medium">{provider.name}</span>
                  <span className="text-muted text-xs">
                    {t(`extensions.capability.${provider.kind}`, { defaultValue: provider.kind })}
                  </span>
                </span>
              </li>
            ))}
          </ul>
        </section>
      )}
      <section aria-labelledby="extension-network" className="flex flex-col gap-1 text-sm">
        <h2 id="extension-network" className="text-primary px-1 pt-2 font-semibold">
          {t('extensions.permissionsTitle')}
        </h2>
        {info.net.length > 0 ? (
          <ul data-testid="extension-permissions" className="flex flex-col">
            {info.net.map((host) => (
              <li key={host} className="flex items-center gap-3 px-1 py-2 wrap-anywhere">
                <Globe aria-hidden size={18} className="text-muted shrink-0" />
                {hostLabel(host, t)}
              </li>
            ))}
          </ul>
        ) : (
          <p className="text-muted px-1">{t('extensions.noNetwork')}</p>
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
            className="border-primary text-primary self-start rounded-full border px-5 py-2 text-sm font-semibold disabled:opacity-50"
          >
            {t(confirming ? 'extensions.uninstallConfirm' : 'extensions.uninstall')}
          </button>
          {uninstall.error && <SourceIssue>{uninstall.error.message}</SourceIssue>}
        </div>
      )}
    </section>
  );
};
