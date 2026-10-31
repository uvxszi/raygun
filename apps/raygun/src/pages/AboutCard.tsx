import { commands } from '@raygun/bindings';
import type { StorageMode } from '@raygun/bindings';
import { useQuery } from '@tanstack/react-query';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

const STORAGE_MODE_KEYS: Record<StorageMode, string> = {
  portable: 'settings.storagePortable',
  installed: 'settings.storageInstalled',
  mobile: 'settings.storageMobile',
  override: 'settings.storageOverride',
};

export const AboutCard: FC = () => {
  const { t } = useTranslation();
  const appInfo = useQuery({ queryKey: ['appInfo'], queryFn: commands.appInfo });
  const storageInfo = useQuery({ queryKey: ['storageInfo'], queryFn: commands.storageInfo });
  const error = appInfo.error ?? storageInfo.error;

  return (
    <section
      data-testid="about-card"
      className="bg-surface flex flex-col gap-3 rounded-xl px-5 py-4 text-sm"
    >
      <h2 className="text-muted">{t('settings.about')}</h2>
      {error && (
        <p role="alert" className="text-primary">
          {t('common.coreUnavailable', { message: String(error) })}
        </p>
      )}
      <dl className="grid grid-cols-[auto_1fr] gap-x-6 gap-y-1">
        {appInfo.data && (
          <>
            <dt className="text-muted">{t('settings.version')}</dt>
            <dd data-testid="app-version">{appInfo.data.version}</dd>
            <dt className="text-muted">{t('settings.platform')}</dt>
            <dd data-testid="platform">{appInfo.data.platform}</dd>
            <dt className="text-muted">{t('settings.arch')}</dt>
            <dd>{appInfo.data.arch}</dd>
            <dt className="text-muted">{t('settings.build')}</dt>
            <dd data-testid="build-kind">
              {t(appInfo.data.debug ? 'settings.buildDebug' : 'settings.buildRelease')}
            </dd>
          </>
        )}
        {storageInfo.data && (
          <>
            <dt className="text-muted">{t('settings.storage')}</dt>
            <dd data-testid="storage-mode">{t(STORAGE_MODE_KEYS[storageInfo.data.mode])}</dd>
            <dt className="text-muted">{t('settings.dataFolder')}</dt>
            <dd data-testid="storage-root" className="break-all">
              {storageInfo.data.root}
            </dd>
          </>
        )}
      </dl>
    </section>
  );
};
