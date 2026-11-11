import type { AppUpdate } from '@raygun/bindings';
import { Download, ExternalLink, FolderGit2, RefreshCw, ScrollText, Sparkles } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { PrefAction, PrefGroup, PrefSwitch } from '../settings/prefs';
import { Problem } from '../settings/Problem';
import { useSettings, useUpdateSetting } from '../settings/useSettings';
import { useAppUpdate, useAppUpdateActions } from './useAppUpdate';
import { useBundledNotes } from './useReleaseNotes';
import { WhatsNewSheet } from './WhatsNewSheet';

/** What the main button does for this copy: install it, or open the release page. */
const selfUpdates = (update: AppUpdate) => update.assetName !== null;

/** "Checked 5 minutes ago", in the reader's language. */
const useCheckedWhen = () => {
  const { t, i18n } = useTranslation();
  const relative = new Intl.RelativeTimeFormat(i18n.language, { numeric: 'auto' });
  return (at: number | null | undefined) => {
    if (!at) return null;
    const minutes = Math.round((at - Date.now()) / 60_000);
    const when =
      Math.abs(minutes) < 60
        ? relative.format(minutes, 'minute')
        : Math.abs(minutes) < 48 * 60
          ? relative.format(Math.round(minutes / 60), 'hour')
          : relative.format(Math.round(minutes / 1440), 'day');
    return t('updater.checked', { when });
  };
};

/**
 * About → Updates (M46.47): this version, the newest release with its notes, and Update, which
 * downloads and installs it where this copy can (the setup exe, the MSI, Android) or opens the
 * release page (the portable exe). The repository is a tap away.
 */
export const AppUpdateCard: FC = () => {
  const { t } = useTranslation();
  const update = useAppUpdate();
  const { check, install, openPage } = useAppUpdateActions();
  const settings = useSettings().data;
  const updateSetting = useUpdateSetting();
  const checkedWhen = useCheckedWhen();
  const bundled = useBundledNotes().data ?? [];
  const [notes, setNotes] = useState<'next' | 'bundled' | null>(null);
  const info = update.data;
  if (!info) return null;
  const busy =
    info.state === 'checking' || info.state === 'downloading' || info.state === 'installing';
  const failure = check.error
    ? { title: t('updater.checkFailed'), detail: check.error.message }
    : install.error
      ? { title: t('updater.installFailed'), detail: install.error.message }
      : openPage.error
        ? { title: t('updater.openFailed'), detail: openPage.error.message }
        : info.state === 'failed' && info.message
          ? { title: t('updater.failed'), detail: info.message }
          : null;
  const status =
    info.available && info.latest
      ? t('updater.available', { latest: info.latest, current: info.current })
      : info.latest
        ? t('updater.newest', { current: info.current })
        : t('updater.notChecked', { current: info.current });

  return (
    <div data-testid="app-update">
      <PrefGroup title={t('updater.title')}>
        <li className="flex flex-col gap-0.5 px-1 py-3">
          <span data-testid="update-status" className="font-medium">
            {status}
          </span>
          {info.state === 'idle' && info.message ? (
            <span data-testid="update-note" className="text-muted text-sm">
              {info.message}
            </span>
          ) : (
            checkedWhen(info.checkedAt) && (
              <span className="text-muted text-sm">{checkedWhen(info.checkedAt)}</span>
            )
          )}
        </li>
        {info.state === 'downloading' && (
          <li className="px-1 pb-2">
            <div
              role="progressbar"
              aria-label={t('updater.downloading')}
              aria-valuemin={0}
              aria-valuemax={100}
              aria-valuenow={Math.round((info.progress ?? 0) * 100)}
              className="bg-surface h-1.5 overflow-hidden rounded-full"
            >
              <div
                className="bg-primary h-full origin-left transition-transform"
                style={{ transform: `scaleX(${info.progress ?? 0})` }}
              />
            </div>
          </li>
        )}
        {info.state === 'installing' && (
          <li className="text-muted px-1 py-2 text-sm">{t('updater.installing')}</li>
        )}
        {info.state === 'needsPermission' && (
          <li className="text-muted px-1 py-2 text-sm">{t('updater.needsPermission')}</li>
        )}
        {info.available && info.installKind === 'portable' && (
          <li className="text-muted px-1 py-2 text-sm">{t('updater.portableHint')}</li>
        )}
        {info.available && info.skipped && (
          <li data-testid="update-skipped" className="text-muted px-1 py-2 text-sm">
            {t('updater.skippedNote')}
          </li>
        )}
        {info.available ? (
          <PrefAction
            icon={selfUpdates(info) ? Download : ExternalLink}
            title={
              selfUpdates(info)
                ? info.state === 'needsPermission'
                  ? t('updater.tryAgain')
                  : t('updater.install', { latest: info.latest })
                : t('updater.openRelease', { latest: info.latest })
            }
            disabled={busy || install.isPending}
            onClick={() => install.mutate()}
          />
        ) : (
          <PrefAction
            icon={RefreshCw}
            title={t('updater.check')}
            disabled={busy || check.isPending}
            onClick={() => check.mutate()}
          />
        )}
        {info.available && info.notes && info.latest && (
          <PrefAction
            icon={Sparkles}
            title={t('updater.whatsNewIn', { version: info.latest })}
            onClick={() => setNotes('next')}
          />
        )}
        {failure && (
          <li className="px-1 py-2">
            <Problem title={failure.title} detail={failure.detail} />
          </li>
        )}
        {settings && (
          <PrefSwitch
            title={t('updater.autoCheck')}
            summary={t('updater.autoCheckSummary')}
            checked={settings.checkUpdates ?? true}
            onChange={(on) => updateSetting.mutate({ key: 'checkUpdates', value: on })}
          />
        )}
        <PrefAction
          icon={FolderGit2}
          title={t('updater.repo')}
          onClick={() => openPage.mutate(info.repoUrl)}
        />
        {bundled.length > 0 && (
          <PrefAction
            icon={ScrollText}
            title={t('updater.whatsNew')}
            summary={t('updater.whatsNewSummary', { current: info.current })}
            onClick={() => setNotes('bundled')}
          />
        )}
      </PrefGroup>
      {notes === 'next' && info.latest && info.notes && (
        <WhatsNewSheet
          releases={[{ version: info.latest, notes: info.notes }]}
          onClose={() => setNotes(null)}
        />
      )}
      {notes === 'bundled' && <WhatsNewSheet releases={bundled} onClose={() => setNotes(null)} />}
    </div>
  );
};
