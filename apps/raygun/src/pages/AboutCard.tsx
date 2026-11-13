import { commands } from '@raygun/bindings';
import type { StorageMode } from '@raygun/bindings';
import { useQuery } from '@tanstack/react-query';
import { Bug, FileText, FolderOpen, Scale } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { PrefAction, PrefGroup, PrefLink } from '../settings/prefs';
import { Problem } from '../settings/Problem';
import { NeedsApp } from '../shell/NeedsApp';
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
  const [logsNote, setLogsNote] = useState<string | null>(null);
  // Desktop shows the folder; Android has no file manager for it, so every log goes in a zip.
  const openLogs = async () => {
    const android = app?.platform === 'android';
    const result = android ? await commands.logsSaveAll() : await commands.logsOpenFolder();
    if (result.status === 'error') setLogsNote(result.error);
    else if (typeof result.data === 'string')
      setLogsNote(t('crash.savedIn', { place: result.data }));
  };

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
      {/* Logs are the app's files (docs/WEB.md). */}
      <NeedsApp hide>
        <PrefGroup title={t('crash.logs')}>
          <PrefLink
            icon={Bug}
            title={t('crash.logs')}
            summary={t('crash.logsSummary')}
            to="/about/crashes"
            search={{ id: undefined }}
          />
          <PrefAction
            icon={FolderOpen}
            title={t('crash.logsFolder')}
            summary={
              logsNote ??
              t(app?.platform === 'android' ? 'crash.logsFolderAndroid' : 'crash.logsFolderDesktop')
            }
            onClick={() => void openLogs()}
          />
        </PrefGroup>
      </NeedsApp>
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
