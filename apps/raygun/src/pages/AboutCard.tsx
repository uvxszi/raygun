import { commands } from '@raygun/bindings';
import type { StorageMode } from '@raygun/bindings';
import { useQuery } from '@tanstack/react-query';
import { FileText, Scale } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { PrefAction, PrefGroup } from '../settings/prefs';
import { Problem } from '../settings/Problem';
import { useAppUpdate, useAppUpdateActions } from '../updates/useAppUpdate';

const STORAGE_MODE_KEYS: Record<StorageMode, string> = {
  portable: 'settings.storagePortable',
  installed: 'settings.storageInstalled',
  mobile: 'settings.storageMobile',
  override: 'settings.storageOverride',
};

/** Raygun's repository, when the updater hasn't said it yet. */
const REPO_URL = 'https://github.com/uvxszi/raygun';

const Fact: FC<{ title: string; testId: string; children: string }> = ({
  title,
  testId,
  children,
}) => (
  <li className="flex flex-col gap-0.5 px-1 py-3">
    <span className="font-medium">{title}</span>
    <span data-testid={testId} className="selectable text-muted text-sm break-all">
      {children}
    </span>
  </li>
);

/** About: this copy (version, build, platform, where its data lives) and its licences. */
export const AboutCard: FC = () => {
  const { t } = useTranslation();
  const appInfo = useQuery({ queryKey: ['appInfo'], queryFn: commands.appInfo });
  const storageInfo = useQuery({ queryKey: ['storageInfo'], queryFn: commands.storageInfo });
  const repo = useAppUpdate().data?.repoUrl ?? REPO_URL;
  const { openPage } = useAppUpdateActions();
  const error = appInfo.error ?? storageInfo.error;
  const app = appInfo.data;
  const storage = storageInfo.data;

  return (
    <div data-testid="about-card">
      <PrefGroup title={t('settings.about')}>
        {app && (
          <>
            <Fact title={t('settings.version')} testId="app-version">
              {app.version}
            </Fact>
            <li className="flex flex-col gap-0.5 px-1 py-3">
              <span className="font-medium">{t('settings.platform')}</span>
              <span className="text-muted text-sm">
                <span data-testid="platform">{app.platform}</span>
                {` · ${app.arch} · `}
                <span data-testid="build-kind">
                  {t(app.debug ? 'settings.buildDebug' : 'settings.buildRelease')}
                </span>
              </span>
            </li>
          </>
        )}
        {storage && (
          <>
            <Fact title={t('settings.storage')} testId="storage-mode">
              {t(STORAGE_MODE_KEYS[storage.mode])}
            </Fact>
            <Fact title={t('settings.dataFolder')} testId="storage-root">
              {storage.root}
            </Fact>
          </>
        )}
        {error && (
          <li className="px-1 py-2">
            <Problem title={t('settings.aboutFailed')} detail={String(error)} />
          </li>
        )}
      </PrefGroup>
      <PrefGroup title={t('settings.licenses')}>
        <PrefAction
          icon={Scale}
          title={t('settings.license')}
          summary={t('settings.licenseName')}
          onClick={() => openPage.mutate(`${repo}/blob/main/LICENSE`)}
        />
        <PrefAction
          icon={FileText}
          title={t('settings.notices')}
          onClick={() => openPage.mutate(`${repo}/blob/main/NOTICE`)}
        />
      </PrefGroup>
    </div>
  );
};
